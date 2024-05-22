extern crate alloc;
use core::panic;
use std::{collections::HashSet, path::PathBuf};

use bitcoin::{
    block::Header,
    consensus::Params,
    p2p::message_filter::{CFHeaders, CFilter, GetCFHeaders, GetCFilters},
    Block, BlockHash, Network, ScriptBuf, TxIn, TxOut, Work,
};
use tokio::sync::{mpsc::Sender, RwLock};

use super::{
    checkpoints::{HeaderCheckpoint, HeaderCheckpoints},
    error::{BlockScanError, HeaderPersistenceError, HeaderSyncError},
    header_chain::HeaderChain,
};
use crate::{
    chain::header_batch::HeadersBatch,
    db::sqlite::header_db::SqliteHeaderDb,
    filters::{
        cfheader_batch::CFHeaderBatch,
        cfheader_chain::{AppendAttempt, CFHeaderChain},
        error::{CFHeaderSyncError, CFilterSyncError},
        filter::Filter,
        filter_chain::FilterChain,
        CF_HEADER_BATCH_SIZE, FILTER_BATCH_SIZE,
    },
    node::node_messages::NodeMessage,
    prelude::MEDIAN_TIME_PAST,
    tx::{memory::MemoryTransactionCache, store::TransactionStore},
};

#[derive(Debug)]
pub(crate) struct Chain {
    header_chain: HeaderChain,
    checkpoints: HeaderCheckpoints,
    params: Params,
    db: RwLock<SqliteHeaderDb>,
    cf_header_chain: CFHeaderChain,
    filter_chain: FilterChain,
    best_known_height: Option<u32>,
    scripts: HashSet<ScriptBuf>,
    block_queue: Vec<BlockHash>,
    tx_store: MemoryTransactionCache,
    dialog: Sender<NodeMessage>,
}

impl Chain {
    pub(crate) async fn new(
        network: &Network,
        scripts: HashSet<ScriptBuf>,
        db_path: Option<PathBuf>,
        tx_store: MemoryTransactionCache,
        from_checkpoint: Option<HeaderCheckpoint>,
        dialog: Sender<NodeMessage>,
    ) -> Result<Self, HeaderPersistenceError> {
        let mut checkpoints = HeaderCheckpoints::new(network);
        let params = match network {
            Network::Bitcoin => panic!("unimplemented network"),
            Network::Testnet => Params::new(*network),
            Network::Signet => Params::new(*network),
            Network::Regtest => panic!("unimplemented network"),
            _ => unreachable!(),
        };
        let checkpoint = from_checkpoint.unwrap_or_else(|| checkpoints.last());
        checkpoints.prune_up_to(checkpoint);
        let mut db = SqliteHeaderDb::new(*network, checkpoints.last(), checkpoint, db_path)
            .map_err(|_| HeaderPersistenceError::SQLite)?;
        let loaded_headers = db
            .load()
            .await
            .map_err(|_| HeaderPersistenceError::SQLite)?;
        if loaded_headers.len().gt(&0) {
            if loaded_headers
                .first()
                .unwrap()
                .prev_blockhash
                .ne(&checkpoint.hash)
            {
                let _ = dialog
                    .send(NodeMessage::Warning("Checkpoint anchor mismatch".into()))
                    .await;
                return Err(HeaderPersistenceError::GenesisMismatch);
            } else if loaded_headers
                .iter()
                .zip(loaded_headers.iter().skip(1))
                .any(|(first, second)| first.block_hash().ne(&second.prev_blockhash))
            {
                let _ = dialog
                    .send(NodeMessage::Warning("Blockhash pointer mismatch".into()))
                    .await;
                return Err(HeaderPersistenceError::HeadersDoNotLink);
            }
            // for (height, header) in loaded_headers.iter().enumerate() {
            //     if let Some(checkpoint) = checkpoints.next() {
            //         if height.eq(&checkpoint.height) {
            //             if checkpoint.hash.eq(&header.block_hash()) {
            //                 checkpoints.advance()
            //             } else {
            //                 println!("Checkpoint mismatch");
            //                 return Err(HeaderPersistenceError::MismatchedCheckpoints);
            //             }
            //         }
            //     }
            // }
        };
        let header_chain = HeaderChain::new(checkpoint, loaded_headers);
        let cf_header_chain = CFHeaderChain::new(checkpoint, 1);
        let filter_chain = FilterChain::new(checkpoint);
        Ok(Chain {
            header_chain,
            checkpoints,
            params,
            db: RwLock::new(db),
            cf_header_chain,
            filter_chain,
            best_known_height: None,
            scripts,
            block_queue: Vec::new(),
            tx_store,
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
        self.header_chain.header_at_height(height).await
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
        self.send_dialog(format!("Best known peer height: {}", height))
            .await;
        self.best_known_height = Some(height);
    }

    // Do we have best known height and is our height equal to it
    pub(crate) fn is_synced(&self) -> bool {
        if let Some(height) = self.best_known_height {
            if (self.height() as u32).ge(&height) {
                true
            } else {
                false
            }
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
            .write()
            .await
            .write(&self.header_chain.headers())
            .await
        {
            self.send_warning(format!("Error persisting to storage: {}", e))
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
        //we check first if the peer is sending us nonsense
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
                .await
                .expect("height is greater than the base checkpoint")
                .block_hash()
                .eq(&checkpoint.hash)
            {
                self.send_dialog(format!("Hit checkpoint, height: {}", checkpoint.height))
                    .await;
                self.send_dialog(format!(
                    "Accumulated log base 2 chainwork: {}",
                    self.log2_work()
                ))
                .await;
                self.send_dialog("Writing progress to disk...".into()).await;
                self.checkpoints.advance();
                self.flush_to_disk().await;
            } else {
                self.send_warning(
                    "Peer is sending us malicious headers, restarting header sync.".into(),
                )
                .await;
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
        self.send_warning("Evaluting a potential fork...".into())
            .await;
        // We only care about the headers these two chains do not have in common
        let uncommon: Vec<Header> = header_batch
            .inner()
            .iter()
            .filter(|header| !self.contains_header(**header))
            .map(|a| *a)
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
            let current_chainwork = self.chainwork_after_height(stem);
            if current_chainwork.lt(&challenge_chainwork) {
                self.send_dialog("Valid reorganization found".into()).await;
                self.header_chain.extend(&uncommon);
                return Ok(());
            } else {
                self.send_warning(
                    "Peer sent us a fork with less work than the current chain".into(),
                )
                .await;
                return Err(HeaderSyncError::LessWorkFork);
            }
        } else {
            return Err(HeaderSyncError::FloatingHeaders);
        }
    }

    pub(crate) async fn sync_cf_headers(
        &mut self,
        peer_id: u32,
        cf_headers: CFHeaders,
    ) -> Result<Option<GetCFHeaders>, CFHeaderSyncError> {
        let batch: CFHeaderBatch = cf_headers.into();
        self.audit_cf_headers(&batch).await?;
        match self.cf_header_chain.append(peer_id, batch).await? {
            AppendAttempt::AddedToQueue => Ok(None),
            AppendAttempt::Extended => Ok(self.next_cf_header_message().await),
            AppendAttempt::Conflict(_) => {
                self.send_warning("Found a conflict while peers are sending filter headers".into())
                    .await;
                Ok(None)
            }
        }
    }

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

    // We need to make this public for new peers that connect to us throughout syncing the filter headers
    pub(crate) async fn next_cf_header_message(&mut self) -> Option<GetCFHeaders> {
        let stop_hash_index = self.cf_header_chain.height() + CF_HEADER_BATCH_SIZE + 1;
        let stop_hash = if let Some(hash) = self.header_at_height(stop_hash_index).await {
            hash.block_hash()
        } else {
            self.tip()
        };
        self.send_dialog(format!(
            "Request for CF headers staring at height {}, ending at height {},\nwith stop hash: {}",
            self.cf_header_chain.height(),
            stop_hash_index,
            stop_hash.to_string()
        ))
        .await;
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
        if filter
            .contains_any(&self.scripts)
            .await
            .map_err(|e| CFilterSyncError::Filter(e))?
        {
            // Add to the block queue
            self.block_queue.push(filter_message.block_hash);
            self.send_dialog(format!(
                "Found script at block: {}",
                filter_message.block_hash.to_string()
            ))
            .await;
        }
        let expected_filter_hash = self.cf_header_chain.hash_at(&filter_message.block_hash);
        if let Some(ref_hash) = expected_filter_hash {
            if filter.filter_hash().await.ne(&ref_hash) {
                return Err(CFilterSyncError::MisalignedFilterHash);
            }
        }
        self.filter_chain.append(filter).await;
        if let Some(stop_hash) = self.filter_chain.last_stop_hash_request() {
            if filter_message.block_hash.eq(stop_hash) {
                Ok(self.next_filter_message().await)
            } else {
                Ok(None)
            }
        } else {
            return Err(CFilterSyncError::UnrequestedStophash);
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
        self.send_dialog(format!(
            "Filters synced to height: {}",
            self.filter_chain.height()
        ))
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
        self.block_queue.is_empty()
    }

    // Scan an incoming block for transactions with our scripts
    pub(crate) async fn scan_block(&mut self, block: &Block) -> Result<(), BlockScanError> {
        let height_of_block = self.height_of_hash(block.block_hash()).await;
        for tx in &block.txdata {
            if self.scan_inputs(&tx.input) || self.scan_outputs(&tx.output) {
                self.tx_store
                    .add_transaction(&tx, height_of_block, &block.block_hash())
                    .await
                    .unwrap();
                self.send_dialog(format!(
                    "Found transaction: {}",
                    tx.compute_txid().to_string()
                ))
                .await;
            }
        }
        Ok(())
    }

    fn scan_inputs(&mut self, inputs: &Vec<TxIn>) -> bool {
        inputs
            .iter()
            .any(|input| self.scripts.contains(&input.script_sig))
    }

    fn scan_outputs(&mut self, inputs: &Vec<TxOut>) -> bool {
        inputs
            .iter()
            .any(|out| self.scripts.contains(&out.script_pubkey))
    }

    async fn send_dialog(&self, message: String) {
        let _ = self.dialog.send(NodeMessage::Dialog(message)).await;
    }

    async fn send_warning(&self, message: String) {
        let _ = self.dialog.send(NodeMessage::Warning(message)).await;
    }
}
