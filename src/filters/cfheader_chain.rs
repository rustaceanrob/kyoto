use std::collections::BTreeMap;

use bitcoin::{FilterHash, FilterHeader};

use super::cfheader_batch::CFHeaderBatch;

pub(crate) struct CFHeaderChain {
    anchor_height: usize,
    inner: Vec<(FilterHeader, FilterHash)>,
    peer_delta: BTreeMap<u32, usize>,
}

impl CFHeaderChain {
    pub(crate) fn new(anchor_height: Option<usize>) -> Self {
        Self {
            anchor_height: anchor_height.unwrap_or(180_000),
            inner: vec![],
            peer_delta: BTreeMap::new(),
        }
    }

    pub(crate) fn append(&mut self, peer_id: u32, cf_headers: CFHeaderBatch) {
        if self.inner.is_empty() {
            self.inner.extend_from_slice(&cf_headers.inner());
            self.peer_delta.insert(peer_id, cf_headers.len());
        } else {
        }
    }

    pub(crate) fn get_peer_height(&self, peer_id: u32) -> usize {
        self.anchor_height + self.peer_delta.get(&peer_id).unwrap_or(&0)
    }
}
