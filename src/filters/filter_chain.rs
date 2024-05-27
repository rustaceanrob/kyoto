use bitcoin::BlockHash;

use crate::chain::checkpoints::HeaderCheckpoint;

use super::filter::Filter;

type Filters = Vec<Filter>;

const INITIAL_BUFFER_SIZE: usize = 20_000;

#[derive(Debug)]
pub(crate) struct FilterChain {
    anchor_checkpoint: HeaderCheckpoint,
    chain: Filters,
    prev_stophash_request: Option<BlockHash>,
}

impl FilterChain {
    pub(crate) fn new(anchor_checkpoint: HeaderCheckpoint) -> Self {
        Self {
            anchor_checkpoint,
            chain: Vec::with_capacity(INITIAL_BUFFER_SIZE),
            prev_stophash_request: None,
        }
    }

    pub(crate) async fn append(&mut self, filter: Filter) {
        if !self.chain.contains(&filter) {
            self.chain.push(filter)
        }
    }
    pub(crate) fn height(&self) -> usize {
        self.anchor_checkpoint.height + self.chain.len()
    }

    pub(crate) fn set_last_stop_hash(&mut self, stop_hash: BlockHash) {
        self.prev_stophash_request = Some(stop_hash)
    }

    pub(crate) fn last_stop_hash_request(&mut self) -> &Option<BlockHash> {
        &self.prev_stophash_request
    }

    fn adjusted_height(&self, height: usize) -> Option<usize> {
        height.checked_sub(self.anchor_checkpoint.height + 1)
    }

    pub(crate) fn filter_at_height(&self, height: usize) -> Option<Filter> {
        let adjusted_height = self.adjusted_height(height);
        match adjusted_height {
            Some(height) => {
                if let Some(filter) = self.chain.get(height) {
                    Some(filter.clone())
                } else {
                    None
                }
            }
            None => None,
        }
    }
}
