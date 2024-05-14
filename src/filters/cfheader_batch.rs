use bitcoin::{p2p::message_filter::CFHeaders, BlockHash, FilterHash, FilterHeader};

pub(crate) struct CFHeaderBatch {
    inner: Vec<(FilterHeader, FilterHash)>,
    prev_filter_header: FilterHeader,
    stop_hash: BlockHash,
}

impl CFHeaderBatch {
    // although BIP 157 specifies two new message types,
    // the CFHeader message may make it easier to detect
    // faulty peers sooner
    pub(crate) fn new(batch: CFHeaders) -> Self {
        let mut headers: Vec<(FilterHeader, FilterHash)> = vec![];
        let mut prev_header = batch.previous_filter_header;
        for hash in batch.filter_hashes {
            let next_header = hash.filter_header(&prev_header);
            headers.push((next_header, hash));
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

    pub(crate) fn stop_hash(&self) -> &BlockHash {
        &self.stop_hash
    }

    pub(crate) fn len(&self) -> usize {
        self.inner.len()
    }

    pub(crate) fn inner(&self) -> Vec<(FilterHeader, FilterHash)> {
        self.inner.clone()
    }
}

impl Into<CFHeaderBatch> for CFHeaders {
    fn into(self) -> CFHeaderBatch {
        CFHeaderBatch::new(self)
    }
}
