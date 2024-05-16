extern crate alloc;
use core::panic;
use std::{
    collections::HashSet,
    time::{SystemTime, UNIX_EPOCH},
};

use bitcoin::{
    block::Header,
    consensus::Params,
    constants::genesis_block,
    p2p::message_filter::{CFHeaders, CFilter, GetCFHeaders, GetCFilters},
    Block, BlockHash, Network, ScriptBuf, TxIn, TxOut, Work,
};

use super::{
    checkpoints::HeaderCheckpoints,
    error::{BlockScanError, HeaderPersistenceError, HeaderSyncError},
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
    prelude::MEDIAN_TIME_PAST,
};

const NUM_LOCATORS: usize = 25;

type Headers = Vec<Header>;
#[derive(Debug)]
pub(crate) struct HeaderChain {
    headers: Headers,
    checkpoints: HeaderCheckpoints,
    params: Params,
    db: SqliteHeaderDb,
    cf_header_chain: CFHeaderChain,
    filter_chain: FilterChain,
    best_known_height: Option<u32>,
    scripts: HashSet<ScriptBuf>,
    block_queue: Vec<BlockHash>,
}

impl HeaderChain {
    pub(crate) fn new(
        network: &Network,
        scripts: HashSet<ScriptBuf>,
    ) -> Result<Self, HeaderPersistenceError> {
        let mut checkpoints = HeaderCheckpoints::new(network);
        let params = match network {
            Network::Bitcoin => panic!("unimplemented network"),
            Network::Testnet => Params::new(*network),
            Network::Signet => Params::new(*network),
            Network::Regtest => panic!("unimplemented network"),
            _ => unreachable!(),
        };
        let cf_header_chain = CFHeaderChain::new(None, 1);
        let filter_chain = FilterChain::new(None);
        let mut db = SqliteHeaderDb::new(*network, checkpoints.last()).map_err(|e| {
            println!("{}", e.to_string());
            HeaderPersistenceError::SQLite
        })?;
        let loaded_headers = db.load().map_err(|e| {
            println!("{}", e.to_string());
            HeaderPersistenceError::SQLite
        })?;
        let genesis = genesis_block(params.clone()).header;
        let headers = if loaded_headers.len().eq(&0) {
            vec![genesis]
        } else {
            if loaded_headers
                .first()
                .unwrap()
                .block_hash()
                .ne(&genesis.block_hash())
            {
                println!("Genesis mismatch");
                return Err(HeaderPersistenceError::GenesisMismatch);
            } else if loaded_headers
                .iter()
                .zip(loaded_headers.iter().skip(1))
                .any(|(first, second)| first.block_hash().ne(&second.prev_blockhash))
            {
                println!("Blockhash pointer mismatch");
                return Err(HeaderPersistenceError::HeadersDoNotLink);
            }
            for (height, header) in loaded_headers.iter().enumerate() {
                if let Some(checkpoint) = checkpoints.next() {
                    if height.eq(&checkpoint.height) {
                        if checkpoint.hash.eq(&header.block_hash()) {
                            checkpoints.advance()
                        } else {
                            println!("Checkpoint mismatch");
                            return Err(HeaderPersistenceError::MismatchedCheckpoints);
                        }
                    }
                }
            }
            loaded_headers
        };
        Ok(HeaderChain {
            headers,
            checkpoints,
            params,
            db,
            cf_header_chain,
            filter_chain,
            best_known_height: None,
            scripts,
            block_queue: Vec::new(),
        })
    }

    // the genesis block or base of alternative chain
    pub(crate) fn root(&self) -> &Header {
        &self
            .headers
            .first()
            .expect("all header chains have at least one element")
    }

    // top of the chain
    pub(crate) fn tip(&self) -> &Header {
        &self
            .headers
            .last()
            .expect("all header chains have at least one element")
    }

    // the canoncial height of the chain, one less than the length
    pub(crate) fn height(&self) -> usize {
        self.headers.len() - 1
    }

    // this header chain contains a block hash
    pub(crate) fn contains_hash(&self, blockhash: BlockHash) -> bool {
        self.headers
            .iter()
            .any(|header| header.block_hash().eq(&blockhash))
    }

    // this header chain contains a block hash
    pub(crate) async fn height_of_hash(&self, blockhash: BlockHash) -> Option<usize> {
        self.headers
            .iter()
            .position(|header| header.block_hash().eq(&blockhash))
    }

    // this header chain contains a block hash
    pub(crate) async fn header_at_height(&self, index: usize) -> Option<&Header> {
        self.headers.get(index)
    }

    // this header chain contains a block hash
    pub(crate) fn contains_header(&self, header: Header) -> bool {
        self.headers.contains(&header)
    }

    // canoncial chainwork
    pub(crate) fn chainwork(&self) -> Work {
        self.headers
            .iter()
            .map(|header| header.work())
            .reduce(|acc, next| acc + next)
            .expect("all chains have at least one header")
    }

    // calculate the chainwork after a fork height to evalutate the fork
    pub(crate) fn chainwork_after_height(&self, height: usize) -> Work {
        assert!(height + 1 <= self.height());
        self.headers
            .iter()
            .enumerate()
            .filter(|(h, _)| h.gt(&height))
            .map(|(_, header)| header.work())
            .reduce(|acc, next| acc + next)
            .expect("all chains have at least one header")
    }

    // human readable chainwork
    pub(crate) fn log2_work(&self) -> f64 {
        self.headers
            .iter()
            .map(|header| header.work().log2())
            .reduce(|acc, next| acc + next)
            .expect("all chains have at least one header")
    }

    // have we hit the known checkpoints
    pub(crate) fn checkpoints_complete(&self) -> bool {
        self.checkpoints.is_exhausted()
    }

    // set the best known height to our peer
    pub(crate) fn set_best_known_height(&mut self, height: u32) {
        println!("Best known peer height: {}", height);
        self.best_known_height = Some(height);
    }

    // do we have best known height and is our height equal to it
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

    // the "locators" are the headers we inform our peers we know about
    pub(crate) fn locators(&self) -> Vec<BlockHash> {
        if !self.checkpoints_complete() {
            vec![self.tip().block_hash()]
        } else {
            let locators = self
                .headers
                .iter()
                .rev()
                .take(NUM_LOCATORS)
                .map(|header_ref| header_ref.block_hash())
                .collect::<Vec<BlockHash>>();
            locators
        }
    }

    // write the chain to disk
    pub(crate) async fn flush_to_disk(&mut self) {
        if let Err(e) = self.db.write(&self.headers).await {
            println!("Error persisting to storage: {}", e);
        }
    }

    // sync the chain with headers from a peer, adjusting to reorgs if needed
    pub(crate) async fn sync_chain(&mut self, message: Vec<Header>) -> Result<(), HeaderSyncError> {
        let header_batch = HeadersBatch::new(message).map_err(|_| HeaderSyncError::EmptyMessage)?;
        // if our chain already has the last header in the message there is no new information
        if self.contains_header(*header_batch.last()) {
            return Ok(());
        }
        let initially_syncing = !self.checkpoints.is_exhausted();
        // we check first if the peer is sending us nonsense
        self.sanity_check(&header_batch).await?;
        // how we handle forks depends on if we are caught up through all checkpoints or not
        if initially_syncing {
            self.catch_up_sync(header_batch).await?;
        } else {
            // nothing left to do but add the headers to the chain
            if self
                .tip()
                .block_hash()
                .eq(&header_batch.first().prev_blockhash)
            {
                self.append_naive(header_batch);
                return Ok(());
            }
            // we are not accepting floating chains from any peer
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

    // these are invariants in all batches of headers we receive
    async fn sanity_check(&self, header_batch: &HeadersBatch) -> Result<(), HeaderSyncError> {
        let initially_syncing = !self.checkpoints.is_exhausted();
        // some basic sanity checks that should result in peer bans on errors

        // if we aren't synced up to the checkpoints we don't accept any forks
        if initially_syncing
            && self
                .tip()
                .block_hash()
                .ne(&header_batch.first().prev_blockhash)
        {
            return Err(HeaderSyncError::PreCheckpointFork);
        }

        // all the headers connect with each other
        if !header_batch.all_connected().await {
            return Err(HeaderSyncError::HeadersNotConnected);
        }

        // all headers pass their own proof of work and the network minimum
        if !header_batch.individually_valid_pow().await {
            return Err(HeaderSyncError::InvalidHeaderWork);
        }

        // the headers have times that are greater than the median of the previous 11 blocks
        let mut last_relevant_mtp: Vec<Header> = self
            .headers
            .iter()
            .rev()
            .take(MEDIAN_TIME_PAST)
            .rev()
            .map(|header_ref| (*header_ref).clone())
            .collect();

        if !header_batch
            .valid_median_time_past(&mut last_relevant_mtp)
            .await
        {
            // the first validation may be incorrect because of median miscalculation,
            // but it is cheap to detect the peer is lying later from checkpoints
            // and difficulty of the SHA256 algorithm
            if self.height() > 1 {
                return Err(HeaderSyncError::InvalidHeaderTimes);
            }
        }
        Ok(())
    }

    async fn catch_up_sync(&mut self, header_batch: HeadersBatch) -> Result<(), HeaderSyncError> {
        assert!(!self.checkpoints.is_exhausted());
        // eagerly append the batch to the chain
        let last_best_index = self.append_naive(header_batch);
        let checkpoint = self
            .checkpoints
            .next()
            .expect("checkpoints are not exhausted");
        // we need to check a hard-coded checkpoint
        if self.height().ge(&checkpoint.height) {
            if self.headers[checkpoint.height]
                .block_hash()
                .eq(&checkpoint.hash)
            {
                println!("Hit checkpoint, height: {}", checkpoint.height);
                println!("Accumulated log base 2 chainwork: {}", self.log2_work());
                println!("Writing progress to disk...");
                self.checkpoints.advance();
                if let Err(e) = self.db.write(&self.headers).await {
                    println!("Error persisting to storage: {}", e);
                }
            } else {
                // rollback further?
                if self.height() > self.checkpoints.last().height {
                    self.rollback_to_index(self.checkpoints.last().height)
                } else {
                    self.rollback_to_index(last_best_index);
                }
                return Err(HeaderSyncError::InvalidCheckpoint);
            }
        }
        // check the difficulty adjustment when possible
        Ok(())
    }

    // audit the difficulty adjustment of the blocks we received

    // rollback the chain to an index, inclusive
    fn rollback_to_index(&mut self, index: usize) {
        self.headers.truncate(index + 1);
    }

    // append a new batch and return the length of the chain before the merge
    fn append_naive(&mut self, batch: HeadersBatch) -> usize {
        let ind = self.height();
        self.headers.extend_from_slice(batch.inner());
        ind
    }

    // this function draws from the neutrino implemention, where even if a fork is valid
    // we only accept it if there is more work provided. otherwise, we disconnect the peer sending
    // us this fork
    async fn evaluate_fork(&mut self, header_batch: &HeadersBatch) -> Result<(), HeaderSyncError> {
        // we only care about the headers these two chains do not have in common
        println!("Evaluting a potential fork...");
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
        let stem_position = self.headers.iter().position(|stem| {
            uncommon
                .first()
                .expect("all headers of a fork cannot be in our chain")
                .prev_blockhash
                .eq(&stem.block_hash())
        });
        if let Some(stem) = stem_position {
            let current_chainwork = self.chainwork_after_height(stem);
            if current_chainwork.lt(&challenge_chainwork) {
                println!("Valid reorganization found");
                self.rollback_to_index(stem);
                assert_eq!(
                    self.tip().block_hash(),
                    uncommon.first().unwrap().prev_blockhash,
                    "tried to reorg into an invalid chain"
                );
                self.headers.extend_from_slice(&uncommon);
                return Ok(());
            } else {
                println!("Peer sent us a fork with less work than the current chain");
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
                println!("Found conflict");
                Ok(None)
            }
        }
    }

    async fn audit_cf_headers(&mut self, batch: &CFHeaderBatch) -> Result<(), CFHeaderSyncError> {
        // does this stop hash even exist in our chain
        if !self.contains_hash(*batch.stop_hash()) {
            return Err(CFHeaderSyncError::UnknownStophash);
        }
        // does the filter header line up with our current chain of filter headers
        if let Some(prev_header) = self.cf_header_chain.prev_header() {
            if batch.prev_header().ne(&prev_header) {
                return Err(CFHeaderSyncError::PrevHeaderMismatch);
            }
        }
        // did they send us the right amount of headers
        let expected_stop_header = self
            .header_at_height(self.cf_header_chain.header_height() + batch.len() - 1)
            .await;
        if let Some(stop_header) = expected_stop_header {
            if stop_header.block_hash().ne(batch.stop_hash()) {
                return Err(CFHeaderSyncError::StopHashMismatch);
            }
        } else {
            return Err(CFHeaderSyncError::HeaderChainIndexOverflow);
        }
        // did we request up to this stop hash
        if let Some(prev_stophash) = self.cf_header_chain.last_stop_hash_request() {
            if prev_stophash.ne(batch.stop_hash()) {
                return Err(CFHeaderSyncError::StopHashMismatch);
            }
        } else {
            // if we never asked for a stophash before this was unsolitited
            return Err(CFHeaderSyncError::UnexpectedCFHeaderMessage);
        }
        Ok(())
    }

    // we need to make this public for new peers that connect to us throughout syncing the filter headers
    pub(crate) async fn next_cf_header_message(&mut self) -> Option<GetCFHeaders> {
        let stop_hash_index = self.cf_header_chain.header_height() + CF_HEADER_BATCH_SIZE;
        let stop_hash = if let Some(hash) = self.header_at_height(stop_hash_index).await {
            hash.block_hash()
        } else {
            self.tip().block_hash()
        };
        // println!(
        //     "Request for CF headers staring at height {}, ending at height {},\nwith stop hash: {}",
        //     self.cf_header_chain.height(),
        //     stop_hash_index,
        //     stop_hash.to_string()
        // );
        self.cf_header_chain.set_last_stop_hash(stop_hash);
        if !self.is_cf_headers_synced() {
            Some(GetCFHeaders {
                filter_type: 0x00,
                start_height: self.cf_header_chain.header_height() as u32,
                stop_hash,
            })
        } else {
            None
        }
    }

    // are the compact filter headers caught up to the header chain
    pub(crate) fn is_cf_headers_synced(&self) -> bool {
        self.height()
            .le(&(self.cf_header_chain.header_height() - 1))
    }

    // handle a filter
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
            // add to the block queue
            self.block_queue.push(filter_message.block_hash);
            println!(
                "Found script at block: {}",
                filter_message.block_hash.to_string()
            );
        }
        self.filter_chain.append(filter).await;

        // if !self.contains_hash(filter_message.block_hash) {
        //     return Err(CFilterSyncError::UnknownStophash);
        // }
        // cannot panic due do the previous check
        // let height = self
        //     .height_of_hash(filter_message.block_hash)
        //     .await
        //     .unwrap();
        // let filter_hash = self.cf_header_chain.filter_hash_at_height(height);
        // if let Some(ref_hash) = filter_hash {
        //     let filter = Filter::new(filter_message.filter);
        //     println!("Filter for height {}", height);
        //     println!("Computed filter hash: {}", filter.filter_hash().await);
        //     println!("Received filter hash in chain: {}", ref_hash);
        //     if filter.filter_hash().await.ne(&ref_hash) {
        //         return Err(CFilterSyncError::MisalignedFilterHash);
        //     }
        //     self.filter_chain.append(filter).await;
        // } else {
        //     return Err(CFilterSyncError::UnknownFilterHash);
        // }
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

    // next filter message, if there is one
    pub(crate) async fn next_filter_message(&mut self) -> Option<GetCFilters> {
        let stop_hash_index = self.filter_chain.height() + FILTER_BATCH_SIZE;
        let stop_hash = if let Some(hash) = self.header_at_height(stop_hash_index).await {
            hash.block_hash()
        } else {
            self.tip().block_hash()
        };
        println!("Filters synced to height: {}", self.filter_chain.height());
        self.filter_chain.set_last_stop_hash(stop_hash);
        if !self.is_filters_synced() {
            Some(GetCFilters {
                filter_type: 0x00,
                start_height: self.filter_chain.height() as u32,
                stop_hash,
            })
        } else {
            None
        }
    }

    // are we synced with filters
    pub(crate) fn is_filters_synced(&self) -> bool {
        self.height().le(&(self.filter_chain.height() - 1))
    }

    // we found a block of interest when scanning the filters
    pub(crate) fn next_block(&mut self) -> Option<BlockHash> {
        self.block_queue.pop()
    }

    // scan an incoming block for transactions with our scripts
    pub(crate) async fn scan_block(&mut self, block: &Block) -> Result<(), BlockScanError> {
        block.txdata.iter().for_each(|tx| {
            if self.scan_inputs(&tx.input) || self.scan_outputs(&tx.output) {
                // push tx to db
                println!("Found transaction: {}", tx.compute_txid().to_string());
            }
        });
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
}
