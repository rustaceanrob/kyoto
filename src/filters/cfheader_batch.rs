use bitcoin::{p2p::message_filter::CFHeaders, BlockHash, FilterHeader};

use crate::chain::FilterCommitment;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CFHeaderBatch {
    inner: Vec<FilterCommitment>,
    prev_filter_header: FilterHeader,
    stop_hash: BlockHash,
}

impl CFHeaderBatch {
    // Although BIP 157 specifies two new message types,
    // the CFHeader message may make it easier to detect
    // faulty peers sooner
    pub(crate) fn new(batch: CFHeaders) -> Self {
        let mut headers: Vec<FilterCommitment> = vec![];
        let mut prev_header = batch.previous_filter_header;
        for hash in batch.filter_hashes {
            let next_header = hash.filter_header(&prev_header);
            headers.push(FilterCommitment {
                header: next_header,
                filter_hash: hash,
            });
            prev_header = next_header;
        }
        Self {
            inner: headers,
            prev_filter_header: batch.previous_filter_header,
            stop_hash: batch.stop_hash,
        }
    }

    pub(crate) fn prev_header(&self) -> &FilterHeader {
        &self.prev_filter_header
    }

    pub(crate) fn stop_hash(&self) -> BlockHash {
        self.stop_hash
    }

    pub(crate) fn len(&self) -> u32 {
        self.inner.len() as u32
    }

    pub(crate) fn take_inner(&mut self) -> Vec<FilterCommitment> {
        core::mem::take(&mut self.inner)
    }
}

impl From<CFHeaders> for CFHeaderBatch {
    fn from(val: CFHeaders) -> Self {
        CFHeaderBatch::new(val)
    }
}
