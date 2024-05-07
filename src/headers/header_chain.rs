extern crate alloc;
use core::panic;

use bitcoin::{
    block::Header, consensus::Params, constants::genesis_block, BlockHash, Network, Work,
};
use thiserror::Error;

use super::checkpoints::HeaderCheckpoints;
use crate::{
    db::sqlite::header_db::SqliteHeaderDb, headers::header_batch::HeadersBatch,
    prelude::MEDIAN_TIME_PAST,
};

const NUM_LOCATORS: usize = 20;

#[derive(Debug)]
pub(crate) struct HeaderChain {
    headers: Vec<Header>,
    checkpoints: HeaderCheckpoints,
    params: Params,
    db: SqliteHeaderDb,
    best_known_height: Option<u32>,
}

impl HeaderChain {
    pub(crate) fn new(network: &Network) -> Result<Self, HeaderChainError> {
        let mut checkpoints = HeaderCheckpoints::new(network);
        let params = match network {
            Network::Bitcoin => panic!("unimplemented network"),
            Network::Testnet => Params::new(*network),
            Network::Signet => Params::new(*network),
            Network::Regtest => panic!("unimplemented network"),
            _ => unreachable!(),
        };
        let mut db = SqliteHeaderDb::new(*network, checkpoints.last()).map_err(|e| {
            println!("{}", e.to_string());
            HeaderChainError::PersistenceFailed
        })?;
        let loaded_headers = db.load().map_err(|e| {
            println!("{}", e.to_string());
            HeaderChainError::PersistenceFailed
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
                return Err(HeaderChainError::PersistenceFailed);
            } else if loaded_headers
                .iter()
                .zip(loaded_headers.iter().skip(1))
                .any(|(first, second)| first.block_hash().ne(&second.prev_blockhash))
            {
                println!("Blockhash pointer mismatch");
                return Err(HeaderChainError::PersistenceFailed);
            }
            for (height, header) in loaded_headers.iter().enumerate() {
                if let Some(checkpoint) = checkpoints.next() {
                    if height.eq(&checkpoint.height) {
                        if checkpoint.hash.eq(&header.block_hash()) {
                            checkpoints.advance()
                        } else {
                            println!("Checkpoint mismatch");
                            return Err(HeaderChainError::PersistenceFailed);
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
            best_known_height: None,
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

    // this header chain contains a header
    pub(crate) fn contains(&self, header: Header) -> bool {
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
        !self.checkpoints.is_exhausted()
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
                println!(
                    "Chain synced! Height: {}, Chainwork: {}",
                    self.height(),
                    self.log2_work()
                );
                true
            } else {
                false
            }
        } else {
            false
        }
    }

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
    pub(crate) async fn sync_chain(&mut self, message: Vec<Header>) -> Result<(), HeaderSyncError> {
        let header_batch = HeadersBatch::new(message).map_err(|_| HeaderSyncError::EmptyMessage)?;
        // if our chain already has the last header in the message there is no new information
        if self.contains(*header_batch.last()) {
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
            }
        }
        Ok(())
    }

    // these are invariants in all batches of headers we receive
    async fn sanity_check(&self, header_batch: &HeadersBatch) -> Result<(), HeaderSyncError> {
        let initially_syncing = !self.checkpoints.is_exhausted();
        // some basic sanity checks that should result in peer bans on errors

        // if we aren't synced up to the checkpoints we don't accept any forks
        // println!("Checking for premature fork");
        if initially_syncing
            && self
                .tip()
                .block_hash()
                .ne(&header_batch.first().prev_blockhash)
        {
            return Err(HeaderSyncError::PreCheckpointFork);
        }

        // println!("Checking all headers are connected");
        // all the headers connect with each other
        if !header_batch.all_connected().await {
            return Err(HeaderSyncError::HeadersNotConnected);
        }

        // println!("Checking each header passes its own proof of work");
        // all headers pass their own proof of work and the network minimum
        if !header_batch.individually_valid_pow().await {
            return Err(HeaderSyncError::InvalidHeaderWork);
        }

        // println!("Checking each header has a valid timestamp");
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
        // println!("Adding headers from batch");

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
                // rollback further
                self.rollback_to_index(last_best_index);
                return Err(HeaderSyncError::InvalidCheckpoint);
            }
        }
        // check the difficulty adjustment when possible
        Ok(())
    }

    // audit the difficulty adjustment of the blocks we received

    // rollback the chain to an index, inclusive
    fn rollback_to_index(&mut self, index: usize) {
        self.headers = self.headers.split_at(index + 1).0.to_vec();
    }

    // append a new batch and return the length of the chain before the merge
    fn append_naive(&mut self, batch: HeadersBatch) -> usize {
        let ind = self.height();
        self.headers.extend_from_slice(batch.inner());
        ind
    }
    // should return a fork
    // pub(crate) async fn reorganize(
    //     &mut self,
    //     fork_info: &ForkInfo,
    //     other: &HeaderChain,
    // ) -> Result<HeaderChain, ReorgError> {
    //     if fork_info.height + 1 > self.headers.len() {
    //         return Err(ReorgError::WrongForkHeight);
    //     }
    //     let stem_position = self
    //         .headers
    //         .iter()
    //         .position(|stem| other.root().prev_blockhash.eq(&stem.block_hash()))
    //         .ok_or(ReorgError::NoHashMatch)?;
    //     if fork_info.height != stem_position {
    //         return Err(ReorgError::WrongForkHeight);
    //     }
    //     let (mut base, reorged) = self.split_chain_above_height(stem_position);
    //     // calc new height of fork and hash
    //     base.extend_from_slice(&other.headers);
    //     self.headers = base;
    //     // return fork
    //     Ok(HeaderChain::new_from_vec(reorged, ))
    // }

    // fn split_chain_above_height(&self, height: usize) -> (Vec<Header>, Vec<Header>) {
    //     let (base, head) = self.headers.split_at(height + 1);
    //     return (base.to_vec(), head.to_vec());
    // }
}

#[derive(Error, Debug)]
pub enum HeaderSyncError {
    #[error("empty headers message")]
    EmptyMessage,
    #[error("the headers received do not connect")]
    HeadersNotConnected,
    #[error("one or more headers does not match its own PoW target")]
    InvalidHeaderWork,
    #[error("one or more headers does not have a valid block time")]
    InvalidHeaderTimes,
    #[error("the sync peer sent us a discontinuous chain")]
    PreCheckpointFork,
    #[error("a checkpoint in the chain did not match")]
    InvalidCheckpoint,
    #[error("a computed difficulty adjustment did not match")]
    MiscalculatedDifficulty,
}

#[derive(Error, Debug)]
pub enum HeaderChainError {
    #[error("failed to initialize the header database")]
    PersistenceFailed,
    #[error("this header does not belong in this chain")]
    OrphanError,
    #[error("this header is a duplicate")]
    DuplicateError,
}

#[derive(Error, Debug)]
pub enum ReorgError {
    #[error("the provided height to the fork was incorrect")]
    WrongForkHeight,
    #[error("the root of the reorg chain was not in the current chain")]
    NoHashMatch,
}

#[derive(Error, Debug)]
pub enum HeaderValidationError {
    #[error("this header does not properly calculate the difficult adjustment")]
    WrongDifficulty,
    #[error("this header has an invalid hash")]
    BadHash,
}
