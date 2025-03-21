use std::collections::HashSet;

use bitcoin::BlockHash;

use crate::chain::checkpoints::HeaderCheckpoint;

const INITIAL_BUFFER_SIZE: usize = 20_000;

// Block filters can be 300 bytes or more. Assuming that some users may
// run the node for extended periods of time, there is little advantage to actually
// storing them. Instead we keep track of the filters we have seen by saving their block hash.
#[derive(Debug)]
pub(crate) struct FilterChain {
    anchor_checkpoint: HeaderCheckpoint,
    // Because we are checking the filters on the fly, we don't actually store them
    chain: HashSet<BlockHash>,
    prev_stophash_request: Option<BlockHash>,
}

impl FilterChain {
    pub(crate) fn new(anchor_checkpoint: HeaderCheckpoint) -> Self {
        Self {
            anchor_checkpoint,
            chain: HashSet::with_capacity(INITIAL_BUFFER_SIZE),
            prev_stophash_request: None,
        }
    }

    pub(crate) fn put_hash(&mut self, hash: BlockHash) {
        self.chain.insert(hash);
    }

    // Some blocks got invalidated, so we remove them from our "chain"
    pub(crate) fn remove(&mut self, hashes: &[BlockHash]) {
        for hash in hashes {
            self.chain.remove(hash);
        }
    }

    pub(crate) fn clear_cache(&mut self) {
        self.chain.clear()
    }

    pub(crate) fn height(&self) -> u32 {
        self.anchor_checkpoint.height + self.chain.len() as u32
    }

    pub(crate) fn set_last_stop_hash(&mut self, stop_hash: BlockHash) {
        self.prev_stophash_request = Some(stop_hash)
    }

    pub(crate) fn last_stop_hash_request(&mut self) -> &Option<BlockHash> {
        &self.prev_stophash_request
    }
}
