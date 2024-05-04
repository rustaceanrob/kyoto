use bitcoin::block::Header;
use thiserror::Error;

use crate::prelude::{Median, MEDIAN_TIME_PAST};

pub(crate) struct HeadersBatch {
    batch: Vec<Header>,
}

// the expected response for the majority of getheaders messages are 2000 headers that should be in order.
// this struct provides basic sanity checks and helper methods.
impl HeadersBatch {
    pub(crate) fn new(batch: Vec<Header>) -> Result<Self, HeadersBatchError> {
        println!("Got batch of headers with len {}", batch.len());
        // for (i, header) in batch.iter().enumerate() {
        //     println!("block: {} time: {}", i, header.time);
        // }
        if batch.len() < 1 {
            return Err(HeadersBatchError::EmptyVec);
        }
        Ok(HeadersBatch { batch })
    }

    // are they all logically connected?
    pub(crate) async fn all_connected(&self) -> bool {
        self.batch
            .iter()
            .zip(self.batch.iter().skip(1))
            .all(|(first, second)| first.block_hash().eq(&second.prev_blockhash))
    }

    // are all the blocks of sufficient work and meet their own target?
    pub(crate) async fn individually_valid_pow(&self) -> bool {
        !self.batch.iter().any(|header| {
            let target = header.target();
            let valid_pow = header.validate_pow(target);
            valid_pow.is_err()
        })
    }

    // do the blocks pass the time requirements
    pub(crate) async fn valid_median_time_past(&self, previous_buffer: &mut Vec<Header>) -> bool {
        previous_buffer.extend_from_slice(&self.batch);
        let median_times: Vec<u32> = previous_buffer
            .windows(MEDIAN_TIME_PAST)
            .map(|window| window.iter().map(|block| block.time).collect::<Vec<_>>())
            .map(|mut times| times.median().expect("at least one time"))
            .collect();
        median_times
            .iter()
            .zip(&self.batch)
            .all(|(median, header)| header.time > *median)
    }

    // the tip of the list
    pub(crate) fn last(&self) -> &Header {
        self.batch
            .last()
            .expect("headers have at least one element by construction")
    }

    // this should connect to the last header we have
    pub(crate) fn first(&self) -> &Header {
        self.batch
            .first()
            .expect("headers have at least one element by construction")
    }

    pub(crate) fn len(&self) -> usize {
        self.batch.len()
    }

    pub(crate) fn inner(&self) -> &Vec<Header> {
        &self.batch
    }
}

#[derive(Error, Debug)]
pub(crate) enum HeadersBatchError {
    #[error("no headers were found in the initialization vector")]
    EmptyVec,
}
