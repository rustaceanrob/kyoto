use bitcoin::{block::Header, params::Params, Target};

use crate::{
    impl_sourceless_error,
    prelude::{Median, MEDIAN_TIME_PAST},
};

pub(crate) struct HeadersBatch {
    batch: Vec<Header>,
}

// The expected response for the majority of getheaders messages are 2000 headers that should be in order.
// This struct provides basic sanity checks and helper methods.
impl HeadersBatch {
    pub(crate) fn new(batch: Vec<Header>) -> Result<Self, HeadersBatchError> {
        if batch.is_empty() {
            return Err(HeadersBatchError::EmptyVec);
        }
        Ok(HeadersBatch { batch })
    }

    // Are they all logically connected?
    pub(crate) async fn connected(&self) -> bool {
        self.batch
            .iter()
            .zip(self.batch.iter().skip(1))
            .all(|(first, second)| first.block_hash().eq(&second.prev_blockhash))
    }

    // Are all the blocks of sufficient work and meet their own target?
    pub(crate) async fn individually_valid_pow(&self) -> bool {
        !self.batch.iter().any(|header| {
            let target = header.target();
            let valid_pow = header.validate_pow(target);
            valid_pow.is_err()
        })
    }

    // Do the targets not change drastically within the batch?
    pub(crate) async fn bits_adhere_transition(&self, params: impl AsRef<Params>) -> bool {
        let params = params.as_ref();
        if params.allow_min_difficulty_blocks {
            return true;
        }
        self.batch
            .iter()
            .zip(self.batch.iter().skip(1))
            .all(|(first, second)| {
                let transition = Target::from_compact(first.bits).max_transition_threshold(params);
                Target::from_compact(second.bits).le(&transition)
            })
    }

    // Do the blocks pass the time requirements
    pub(crate) async fn valid_median_time_past(&self, previous_buffer: &mut Vec<Header>) -> bool {
        previous_buffer.extend_from_slice(&self.batch);
        let median_times: Vec<u32> = previous_buffer
            .windows(MEDIAN_TIME_PAST)
            .map(|window| window.iter().map(|block| block.time).collect::<Vec<_>>())
            .map(|mut times| times.median())
            .collect();
        median_times
            .iter()
            .zip(&self.batch)
            .all(|(median, header)| header.time >= *median)
    }

    // The tip of the list
    pub(crate) fn last(&self) -> &Header {
        self.batch
            .last()
            .expect("headers have at least one element by construction")
    }

    // This should connect to the last header we have
    pub(crate) fn first(&self) -> &Header {
        self.batch
            .first()
            .expect("headers have at least one element by construction")
    }

    pub(crate) fn len(&self) -> usize {
        self.batch.len()
    }

    pub(crate) fn get(&self, index: usize) -> Option<&Header> {
        self.batch.get(index)
    }

    pub(crate) fn inner(&self) -> &[Header] {
        &self.batch
    }
}

#[derive(Debug)]
pub(crate) enum HeadersBatchError {
    EmptyVec,
}

impl core::fmt::Display for HeadersBatchError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            HeadersBatchError::EmptyVec => {
                write!(f, "no headers were found in the initialization vector.")
            }
        }
    }
}

impl_sourceless_error!(HeadersBatchError);
