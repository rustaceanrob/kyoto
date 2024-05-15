use bitcoin::BlockHash;

use super::filter::Filter;

type Filters = Vec<Filter>;

#[derive(Debug)]
pub(crate) struct FilterChain {
    anchor_height: usize,
    chain: Filters,
    prev_stophash_request: Option<BlockHash>,
}

impl FilterChain {
    pub(crate) fn new(anchor_height: Option<usize>) -> Self {
        Self {
            anchor_height: anchor_height.unwrap_or(180_000),
            chain: vec![],
            prev_stophash_request: None,
        }
    }

    pub(crate) async fn append(&mut self, filter: Filter) {
        self.chain.push(filter)
    }
    pub(crate) fn header_height(&self) -> usize {
        self.anchor_height + self.chain.len()
    }

    pub(crate) fn set_last_stop_hash(&mut self, stop_hash: BlockHash) {
        self.prev_stophash_request = Some(stop_hash)
    }

    pub(crate) fn last_stop_hash_request(&mut self) -> &Option<BlockHash> {
        &self.prev_stophash_request
    }

    pub(crate) fn filter_at_height(&self, height: usize) -> Option<Filter> {
        let adjusted_height = height - self.anchor_height;
        if let Some(filter) = self.chain.get(adjusted_height) {
            Some(filter.clone())
        } else {
            None
        }
    }
}
