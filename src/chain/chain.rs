extern crate alloc;
use std::{collections::HashSet, sync::Arc};

use bitcoin::{
    block::Header,
    consensus::Params,
    p2p::message_filter::{CFHeaders, CFilter, GetCFHeaders, GetCFilters},
    Block, BlockHash, Network, ScriptBuf, TxIn, TxOut, Work,
};
use tokio::sync::Mutex;

use super::{
    block_queue::BlockQueue,
    checkpoints::{HeaderCheckpoint, HeaderCheckpoints},
    error::{BlockScanError, HeaderPersistenceError, HeaderSyncError},
    header_chain::HeaderChain,
};
use crate::{
    chain::header_batch::HeadersBatch,
    db::traits::HeaderStore,
    filters::{
        cfheader_batch::CFHeaderBatch,
        cfheader_chain::{AppendAttempt, CFHeaderChain, CFHeaderSyncResult},
        error::{CFHeaderSyncError, CFilterSyncError},
        filter::Filter,
        filter_chain::FilterChain,
        CF_HEADER_BATCH_SIZE, FILTER_BATCH_SIZE,
    },
    node::{dialog::Dialog, node_messages::NodeMessage},
    prelude::{params_from_network, MEDIAN_TIME_PAST},
    tx::types::IndexedTransaction,
};

pub(crate) struct Chain {
    header_chain: HeaderChain,
    cf_header_chain: CFHeaderChain,
    filter_chain: FilterChain,
    checkpoints: HeaderCheckpoints,
    params: Params,
    db: Arc<Mutex<dyn HeaderStore + Send + Sync>>,
    best_known_height: Option<u32>,
    scripts: HashSet<ScriptBuf>,
    block_queue: BlockQueue,
    dialog: Dialog,
}

impl Chain {
    pub(crate) async fn new(
        network: &Network,
        scripts: HashSet<ScriptBuf>,
        anchor: HeaderCheckpoint,
        mut checkpoints: HeaderCheckpoints,
        mut dialog: Dialog,
        mut db: impl HeaderStore + Send + Sync + 'static,
        quorum_required: usize,
    ) -> Result<Self, HeaderPersistenceError> {
        let params = params_from_network(network);
        let loaded_headers = db
            .load()
            .await
            .map_err(|_| HeaderPersistenceError::SQLite)?;
        if loaded_headers.len().gt(&0) {
            if loaded_headers
                .first()
                .unwrap()
                .prev_blockhash
                .ne(&anchor.hash)
            {
                dialog
                    .send_warning("Checkpoint anchor mismatch".into())
                    .await;
                return Err(HeaderPersistenceError::GenesisMismatch);
            } else if loaded_headers
                .iter()
                .zip(loaded_headers.iter().skip(1))
                .any(|(first, second)| first.block_hash().ne(&second.prev_blockhash))
            {
                dialog
                    .send_warning("Blockhash pointer mismatch".into())
                    .await;
                return Err(HeaderPersistenceError::HeadersDoNotLink);
            }
            loaded_headers.iter().for_each(|header| {
                if let Some(checkpoint) = checkpoints.next() {
                    if header.block_hash().eq(&checkpoint.hash) {
                        checkpoints.advance()
                    }
                }
            })
        };
        let header_chain = HeaderChain::new(anchor, loaded_headers);
        let cf_header_chain = CFHeaderChain::new(anchor, quorum_required);
        let filter_chain = FilterChain::new(anchor);
        Ok(Chain {
            header_chain,
            checkpoints,
            params,
            db: Arc::new(Mutex::new(db)),
            cf_header_chain,
            filter_chain,
            best_known_height: None,
            scripts,
            block_queue: BlockQueue::new(),
            dialog,
        })
    }

    // The genesis block
    pub(crate) fn root(&self) -> BlockHash {
        self.header_chain.root()
    }

    // Top of the chain
    pub(crate) fn tip(&self) -> BlockHash {
        self.header_chain.tip()
    }

    // The canoncial height of the chain, one less than the length
    pub(crate) fn height(&self) -> usize {
        self.header_chain.height()
    }

    // This header chain contains a block hash
    pub(crate) fn contains_hash(&self, blockhash: BlockHash) -> bool {
        self.header_chain.contains_hash(blockhash)
    }

    // This header chain contains a block hash
    pub(crate) async fn height_of_hash(&self, blockhash: BlockHash) -> Option<usize> {
        self.header_chain.height_of_hash(blockhash).await
    }

    // This header chain contains a block hash
    pub(crate) async fn header_at_height(&self, height: usize) -> Option<&Header> {
        self.header_chain.header_at_height(height)
    }

    // This header chain contains a block hash
    pub(crate) fn contains_header(&self, header: Header) -> bool {
        self.header_chain.contains_header(header)
    }

    // Canoncial chainwork
    pub(crate) fn chainwork(&self) -> Work {
        self.header_chain.chainwork()
    }

    // Calculate the chainwork after a fork height to evalutate the fork
    pub(crate) fn chainwork_after_height(&self, height: usize) -> Work {
        self.header_chain.chainwork_after_height(height)
    }

    // Human readable chainwork
    pub(crate) fn log2_work(&self) -> f64 {
        self.header_chain.log2_work()
    }

    // Have we hit the known checkpoints
    pub(crate) fn checkpoints_complete(&self) -> bool {
        self.checkpoints.is_exhausted()
    }

    // Set the best known height to our peer
    pub(crate) async fn set_best_known_height(&mut self, height: u32) {
        self.dialog
            .send_dialog(format!("Best known peer height: {}", height))
            .await;
        self.best_known_height = Some(height);
    }

    // Do we have best known height and is our height equal to it
    pub(crate) fn is_synced(&self) -> bool {
        if let Some(height) = self.best_known_height {
            (self.height() as u32).ge(&height)
        } else {
            false
        }
    }

    // The "locators" are the headers we inform our peers we know about
    pub(crate) fn locators(&self) -> Vec<BlockHash> {
        if !self.checkpoints_complete() {
            vec![self.tip()]
        } else {
            self.header_chain.locators()
        }
    }

    // Write the chain to disk
    pub(crate) async fn flush_to_disk(&mut self) {
        if let Err(e) = self
            .db
            .lock()
            .await
            .write(self.header_chain.headers())
            .await
        {
            self.dialog
                .send_warning(format!("Error persisting to storage: {}", e))
                .await;
        }
    }

    // Sync the chain with headers from a peer, adjusting to reorgs if needed
    pub(crate) async fn sync_chain(&mut self, message: Vec<Header>) -> Result<(), HeaderSyncError> {
        let header_batch = HeadersBatch::new(message).map_err(|_| HeaderSyncError::EmptyMessage)?;
        // If our chain already has the last header in the message there is no new information
        if self.contains_header(*header_batch.last()) {
            return Ok(());
        }
        let initially_syncing = !self.checkpoints.is_exhausted();
        // We check first if the peer is sending us nonsense
        self.sanity_check(&header_batch).await?;
        // How we handle forks depends on if we are caught up through all checkpoints or not
        if initially_syncing {
            self.catch_up_sync(header_batch).await?;
        } else {
            // Nothing left to do but add the headers to the chain
            if self.tip().eq(&header_batch.first().prev_blockhash) {
                self.header_chain.extend(header_batch.inner());
                return Ok(());
            }
            // We are not accepting floating chains from any peer
            // the prev_hash of the last block in their chain does not
            // point to any header we know of
            if !self.contains_hash(header_batch.first().prev_blockhash) {
                return Err(HeaderSyncError::FloatingHeaders);
            }
            //
            self.evaluate_fork(&header_batch).await?;
        }
        Ok(())
    }

    // These are invariants in all batches of headers we receive
    async fn sanity_check(&self, header_batch: &HeadersBatch) -> Result<(), HeaderSyncError> {
        let initially_syncing = !self.checkpoints.is_exhausted();
        // Some basic sanity checks that should result in peer bans on errors

        // If we aren't synced up to the checkpoints we don't accept any forks
        if initially_syncing && self.tip().ne(&header_batch.first().prev_blockhash) {
            return Err(HeaderSyncError::PreCheckpointFork);
        }

        // All the headers connect with each other
        if !header_batch.all_connected().await {
            return Err(HeaderSyncError::HeadersNotConnected);
        }

        // All headers pass their own proof of work and the network minimum
        if !header_batch.individually_valid_pow().await {
            return Err(HeaderSyncError::InvalidHeaderWork);
        }

        // The headers have times that are greater than the median of the previous 11 blocks
        let mut last_relevant_mtp = self.header_chain.last_median_time_past_window();
        if !header_batch
            .valid_median_time_past(&mut last_relevant_mtp)
            .await
        {
            // The first validation may be incorrect because of median miscalculation,
            // but it is cheap to detect the peer is lying later from checkpoints
            // and difficulty of the SHA256 algorithm
            if self.header_chain.inner_len() > MEDIAN_TIME_PAST {
                return Err(HeaderSyncError::InvalidHeaderTimes);
            }
        }
        Ok(())
    }

    /// Sync with extra requirements on checkpoints and forks
    async fn catch_up_sync(&mut self, header_batch: HeadersBatch) -> Result<(), HeaderSyncError> {
        assert!(!self.checkpoints.is_exhausted());
        // Eagerly append the batch to the chain
        self.header_chain.extend(header_batch.inner());
        let checkpoint = self
            .checkpoints
            .next()
            .expect("checkpoints are not exhausted");
        // We need to check a hard-coded checkpoint
        if self.height().ge(&checkpoint.height) {
            if self
                .header_chain
                .header_at_height(checkpoint.height)
                .expect("height is greater than the base checkpoint")
                .block_hash()
                .eq(&checkpoint.hash)
            {
                self.dialog
                    .send_dialog(format!("Found checkpoint, height: {}", checkpoint.height))
                    .await;
                self.dialog
                    .send_dialog(format!(
                        "Accumulated log base 2 chainwork: {:.2}",
                        self.log2_work()
                    ))
                    .await;
                self.dialog
                    .send_dialog("Writing progress to disk...".into())
                    .await;
                self.checkpoints.advance();
                self.flush_to_disk().await;
            } else {
                self.dialog
                    .send_warning(
                        "Peer is sending us malicious headers, restarting header sync.".into(),
                    )
                    .await;
                // We assume that this would be so rare that we just clear the whole header chain
                self.header_chain.clear_all();
                return Err(HeaderSyncError::InvalidCheckpoint);
            }
        }
        // check the difficulty adjustment when possible
        Ok(())
    }

    // audit the difficulty adjustment of the blocks we received

    // This function draws from the neutrino implemention, where even if a fork is valid
    // we only accept it if there is more work provided. otherwise, we disconnect the peer sending
    // us this fork
    async fn evaluate_fork(&mut self, header_batch: &HeadersBatch) -> Result<(), HeaderSyncError> {
        self.dialog
            .send_warning("Evaluting a potential fork...".into())
            .await;
        // We only care about the headers these two chains do not have in common
        let uncommon: Vec<Header> = header_batch
            .inner()
            .iter()
            .filter(|header| !self.contains_header(**header))
            .copied()
            .collect();
        let challenge_chainwork = uncommon
            .iter()
            .map(|header| header.work())
            .reduce(|acc, next| acc + next)
            .expect("all headers of a fork cannot be in our chain");
        let stem_position = self.header_chain.headers().iter().position(|stem| {
            uncommon
                .first()
                .expect("all headers of a fork cannot be in our chain")
                .prev_blockhash
                .eq(&stem.block_hash())
        });
        if let Some(stem) = stem_position {
            let current_chainwork = self.header_chain.chainwork_after_index(stem);
            if current_chainwork.lt(&challenge_chainwork) {
                self.dialog
                    .send_dialog("Valid reorganization found".into())
                    .await;
                self.header_chain.extend(&uncommon);
                Ok(())
            } else {
                self.dialog
                    .send_warning(
                        "Peer sent us a fork with less work than the current chain".into(),
                    )
                    .await;
                Err(HeaderSyncError::LessWorkFork)
            }
        } else {
            Err(HeaderSyncError::FloatingHeaders)
        }
    }

    // Sync the compact filter headers, possibly encountering conflicts
    pub(crate) async fn sync_cf_headers(
        &mut self,
        peer_id: u32,
        cf_headers: CFHeaders,
    ) -> Result<CFHeaderSyncResult, CFHeaderSyncError> {
        let batch: CFHeaderBatch = cf_headers.into();
        self.dialog
            .chain_update(
                self.height(),
                self.cf_header_chain.height(),
                self.filter_chain.height(),
                self.best_known_height
                    .unwrap_or(self.height() as u32)
                    .try_into()
                    .unwrap(),
            )
            .await;
        self.audit_cf_headers(&batch).await?;
        match self.cf_header_chain.append(peer_id, batch).await? {
            AppendAttempt::AddedToQueue => Ok(CFHeaderSyncResult::AddedToQueue),
            AppendAttempt::Extended => Ok(CFHeaderSyncResult::ReadyForNext),
            AppendAttempt::Conflict(height) => match self.header_at_height(height).await {
                Some(header) => Ok(CFHeaderSyncResult::Dispute(header.block_hash())),
                None => Err(CFHeaderSyncError::HeaderChainIndexOverflow),
            },
        }
    }

    /// Audit the validity of a batch of compact filter headers
    async fn audit_cf_headers(&mut self, batch: &CFHeaderBatch) -> Result<(), CFHeaderSyncError> {
        // Does this stop hash even exist in our chain
        if !self.contains_hash(*batch.stop_hash()) {
            return Err(CFHeaderSyncError::UnknownStophash);
        }
        // Does the filter header line up with our current chain of filter headers
        if let Some(prev_header) = self.cf_header_chain.prev_header() {
            if batch.prev_header().ne(&prev_header) {
                return Err(CFHeaderSyncError::PrevHeaderMismatch);
            }
        }
        // Did they send us the right amount of headers
        let expected_stop_header = self
            .header_at_height(self.cf_header_chain.height() + batch.len())
            .await;
        if let Some(stop_header) = expected_stop_header {
            if stop_header.block_hash().ne(batch.stop_hash()) {
                return Err(CFHeaderSyncError::StopHashMismatch);
            }
        } else {
            return Err(CFHeaderSyncError::HeaderChainIndexOverflow);
        }
        // Did we request up to this stop hash
        if let Some(prev_stophash) = self.cf_header_chain.last_stop_hash_request() {
            if prev_stophash.ne(batch.stop_hash()) {
                return Err(CFHeaderSyncError::StopHashMismatch);
            }
        } else {
            // If we never asked for a stophash before this was unsolitited
            return Err(CFHeaderSyncError::UnexpectedCFHeaderMessage);
        }
        Ok(())
    }

    // If we receive an inventory, our merge queue was interrupted
    pub(crate) fn clear_filter_header_queue(&mut self) {
        self.cf_header_chain.clear_queue()
    }

    // We need to make this public for new peers that connect to us throughout syncing the filter headers
    pub(crate) async fn next_cf_header_message(&mut self) -> Option<GetCFHeaders> {
        let stop_hash_index = self.cf_header_chain.height() + CF_HEADER_BATCH_SIZE + 1;
        let stop_hash = if let Some(hash) = self.header_at_height(stop_hash_index).await {
            hash.block_hash()
        } else {
            self.tip()
        };
        self.cf_header_chain.set_last_stop_hash(stop_hash);
        if !self.is_cf_headers_synced() {
            Some(GetCFHeaders {
                filter_type: 0x00,
                start_height: (self.cf_header_chain.height() + 1) as u32,
                stop_hash,
            })
        } else {
            self.cf_header_chain.join(self.header_chain.headers()).await;
            None
        }
    }

    // Are the compact filter headers caught up to the header chain
    pub(crate) fn is_cf_headers_synced(&self) -> bool {
        self.height().le(&self.cf_header_chain.height())
    }

    // Handle a new filter
    pub(crate) async fn sync_filter(
        &mut self,
        filter_message: CFilter,
    ) -> Result<Option<GetCFilters>, CFilterSyncError> {
        if self.is_filters_synced() {
            return Ok(None);
        }
        let mut filter = Filter::new(filter_message.filter, filter_message.block_hash);
        let expected_filter_hash = self.cf_header_chain.hash_at(&filter_message.block_hash);
        if let Some(ref_hash) = expected_filter_hash {
            if filter.filter_hash().await.ne(ref_hash) {
                return Err(CFilterSyncError::MisalignedFilterHash);
            }
        }
        if !self.block_queue.contains(&filter_message.block_hash)
            && filter
                .contains_any(&self.scripts)
                .await
                .map_err(CFilterSyncError::Filter)?
        {
            // Add to the block queue
            self.block_queue.add(filter_message.block_hash);
            self.dialog
                .send_dialog(format!(
                    "Found script at block: {}",
                    filter_message.block_hash
                ))
                .await;
        }
        self.filter_chain.put(&filter).await;
        if let Some(stop_hash) = self.filter_chain.last_stop_hash_request() {
            if filter_message.block_hash.eq(stop_hash) {
                Ok(self.next_filter_message().await)
            } else {
                Ok(None)
            }
        } else {
            Err(CFilterSyncError::UnrequestedStophash)
        }
    }

    // Next filter message, if there is one
    pub(crate) async fn next_filter_message(&mut self) -> Option<GetCFilters> {
        let stop_hash_index = self.filter_chain.height() + FILTER_BATCH_SIZE + 1;
        let stop_hash = if let Some(hash) = self.header_at_height(stop_hash_index).await {
            hash.block_hash()
        } else {
            self.tip()
        };
        self.dialog
            .chain_update(
                self.height(),
                self.cf_header_chain.height(),
                self.filter_chain.height(),
                self.best_known_height
                    .unwrap_or(self.height() as u32)
                    .try_into()
                    .unwrap(),
            )
            .await;
        self.filter_chain.set_last_stop_hash(stop_hash);
        if !self.is_filters_synced() {
            Some(GetCFilters {
                filter_type: 0x00,
                start_height: (self.filter_chain.height() + 1) as u32,
                stop_hash,
            })
        } else {
            None
        }
    }

    // Are we synced with filters
    pub(crate) fn is_filters_synced(&self) -> bool {
        self.height().le(&self.filter_chain.height())
    }

    // Pop a block from the queue of interesting blocks
    pub(crate) fn next_block(&mut self) -> Option<BlockHash> {
        self.block_queue.pop()
    }

    // Are there any blocks left in the queue
    pub(crate) fn block_queue_empty(&self) -> bool {
        self.block_queue.complete()
    }

    // Scan an incoming block for transactions with our scripts
    pub(crate) async fn scan_block(&mut self, block: &Block) -> Result<(), BlockScanError> {
        self.block_queue.receive_one();
        let height_of_block = self.height_of_hash(block.block_hash()).await;
        for tx in &block.txdata {
            if self.scan_inputs(&tx.input) || self.scan_outputs(&tx.output) {
                // self.tx_store
                //     .add_transaction(&tx, height_of_block, &block.block_hash())
                //     .await
                //     .unwrap();
                self.dialog
                    .send_data(NodeMessage::Block(block.clone()))
                    .await;
                self.dialog
                    .send_data(NodeMessage::Transaction(IndexedTransaction::new(
                        tx.clone(),
                        height_of_block,
                        block.block_hash(),
                    )))
                    .await;
                self.dialog
                    .send_dialog(format!("Found transaction: {}", tx.compute_txid()))
                    .await;
            }
        }
        Ok(())
    }

    fn scan_inputs(&mut self, inputs: &[TxIn]) -> bool {
        inputs
            .iter()
            .any(|input| self.scripts.contains(&input.script_sig))
    }

    fn scan_outputs(&mut self, inputs: &[TxOut]) -> bool {
        inputs
            .iter()
            .any(|out| self.scripts.contains(&out.script_pubkey))
    }
}
