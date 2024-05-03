extern crate alloc;
use core::panic;

use bitcoin::{block::Header, consensus::Params, BlockHash, Network, Target, Work};
use num_bigint::BigUint;
use thiserror::Error;

use super::checkpoints::HeaderCheckpoints;
use crate::{headers::header_batch::HeadersBatch, prelude::MEDIAN_TIME_PAST};

#[derive(Debug)]
pub(crate) struct HeaderChain {
    pub headers: Vec<Header>,
    checkpoints: HeaderCheckpoints,
    params: Params,
}

impl HeaderChain {
    pub(crate) fn new(start: Header, network: &Network) -> Self {
        let headers: Vec<Header> = vec![start];
        let checkpoints = HeaderCheckpoints::new(network);
        let params = match network {
            Network::Bitcoin => panic!("unimplemented network"),
            Network::Testnet => Params::new(*network),
            Network::Signet => Params::new(*network),
            Network::Regtest => panic!("unimplemented network"),
            _ => unreachable!(),
        };
        HeaderChain {
            headers,
            checkpoints,
            params,
        }
    }

    // pub(crate) fn new_from_vec(headers: Vec<Header>, network: &Network) -> Self {
    //     let checkpoints = HeaderCheckpoints::new(network);
    //     HeaderChain {
    //         headers,
    //         checkpoints,
    //     }
    // }

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
            // we may have to handle potential valid forks of the main chain
        }

        Ok(())
    }

    // these are invariants in all batches of headers we receive
    async fn sanity_check(&self, header_batch: &HeadersBatch) -> Result<(), HeaderSyncError> {
        let initially_syncing = !self.checkpoints.is_exhausted();
        // some basic sanity checks that should result in peer bans on errors

        // if we aren't synced up to the checkpoints we don't accept any forks
        if initially_syncing && self.tip().ne(header_batch.first()) {
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
            return Err(HeaderSyncError::InvalidHeaderTimes);
        }
        Ok(())
    }

    async fn catch_up_sync(&mut self, header_batch: HeadersBatch) -> Result<(), HeaderSyncError> {
        assert_eq!(self.tip(), header_batch.last());
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
                self.checkpoints.advance();
            } else {
                self.rollback_to_index(last_best_index);
                return Err(HeaderSyncError::InvalidCheckpoint);
            }
        }
        // check the difficulty adjustment when possible
        Ok(())
    }

    // rollback the chain to an index, non-inclusive
    fn rollback_to_index(&mut self, index: usize) {
        self.headers = self.headers.split_at(index).0.to_vec();
    }

    // append a new batch and return the length of the chain before the merge
    fn append_naive(&mut self, batch: HeadersBatch) -> usize {
        let ind = self.height() + 1;
        self.headers.extend_from_slice(batch.inner());
        ind
    }

    // audit the difficulty adjustment of the blocks we received
    async fn audit_difficulty_adjustment(&self) -> Result<(), HeaderSyncError> {
        assert!(!self.params.allow_min_difficulty_blocks);
        assert!(!self.params.no_pow_retargeting);
        let audit_height = self
            .height()
            .saturating_sub(self.params.difficulty_adjustment_interval() as usize + 1);
        let difficulty_retarget_indexes: Vec<usize> = self
            .headers
            .iter()
            .enumerate()
            .filter(|(index, _)| *index > audit_height)
            .filter(|(index, _)| {
                *index % self.params.difficulty_adjustment_interval() as usize == 0
            })
            .map(|(index, _)| index)
            .collect();

        Ok(())
    }

    async fn calc_retargets(&self, indexes: Vec<usize>) {}

    async fn retarget_difficulty(&self, first: &Header, second: &Header) -> Target {
        let cur_target = first.target();
        let abs_max = self.params.max_attainable_target;
        // TODO: max check depending on network
        let max_threshold = first.target().max_transition_threshold(self.params.clone());
        let min_threshold = first.target().min_transition_threshold();

        let timespan = second.time - first.time;
        // how tf do we get U256 without forking the repo? floresta did it
        let target_bignum = BigUint::from_bytes_be(&cur_target.to_be_bytes());
        let mult_adjust = target_bignum * timespan;
        let retarget = mult_adjust / self.params.pow_target_timespan;
        // I get Be and Le mixed up, not sure if correct
        let mut retarget_buffer = [0; 32];
        retarget_buffer.copy_from_slice(&retarget.to_bytes_le());
        let new_target = Target::from_le_bytes(retarget_buffer);

        if new_target > max_threshold {
            if new_target > abs_max {
                abs_max
            } else {
                max_threshold
            }
        } else if new_target < min_threshold {
            min_threshold
        } else {
            new_target
        }
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
