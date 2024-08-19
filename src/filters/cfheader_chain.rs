use std::collections::HashMap;

use bitcoin::{BlockHash, FilterHash, FilterHeader};

use crate::chain::checkpoints::HeaderCheckpoint;

use super::cfheader_batch::CFHeaderBatch;

const INITIAL_BUFFER_SIZE: usize = 20_000;

#[derive(Debug, PartialEq)]
pub(crate) enum AppendAttempt {
    // Nothing to do yet
    AddedToQueue,
    // We sucessfully extended the current chain and should broadcast the next round of CF header messages
    Extended,
    // We found a conflict in the peers CF header messages at this index
    Conflict(BlockHash),
}

#[derive(Debug, Clone)]
pub(crate) struct QueuedCFHeader {
    pub block_hash: BlockHash,
    pub filter_header: FilterHeader,
    pub filter_hash: FilterHash,
}

impl QueuedCFHeader {
    pub(crate) fn new(
        block_hash: BlockHash,
        filter_header: FilterHeader,
        filter_hash: FilterHash,
    ) -> Self {
        Self {
            block_hash,
            filter_header,
            filter_hash,
        }
    }

    fn hash_tuple(&self) -> (BlockHash, FilterHash) {
        (self.block_hash, self.filter_hash)
    }

    fn header_and_hash(&self) -> (FilterHeader, FilterHash) {
        (self.filter_header, self.filter_hash)
    }

    fn tuple(&self) -> (BlockHash, FilterHeader, FilterHash) {
        (self.block_hash, self.filter_header, self.filter_hash)
    }
}

type Queue = Option<Vec<QueuedCFHeader>>;

#[derive(Debug)]
pub(crate) struct CFHeaderChain {
    anchor_checkpoint: HeaderCheckpoint,
    // We only really care about this relationship
    hash_chain: HashMap<BlockHash, FilterHash>,
    merged_queue: Queue,
    prev_stophash_request: Option<BlockHash>,
    prev_header: Option<FilterHeader>,
    quorum_required: usize,
    current_quorum: usize,
}

impl CFHeaderChain {
    pub(crate) fn new(anchor_checkpoint: HeaderCheckpoint, quorum_required: usize) -> Self {
        Self {
            anchor_checkpoint,
            hash_chain: HashMap::new(),
            merged_queue: None,
            prev_stophash_request: None,
            prev_header: None,
            quorum_required,
            current_quorum: 0,
        }
    }

    // Set a reference point for the block hashes and associated filter hash.
    pub(crate) async fn set_queue(&mut self, cf_headers: Vec<QueuedCFHeader>) -> AppendAttempt {
        self.merged_queue = Some(cf_headers);
        self.current_quorum += 1;
        self.attempt_merge().await
    }

    // Verify a batch of filter headers and hashes is what we expect.
    pub(crate) async fn verify(&mut self, cf_headers: CFHeaderBatch) -> AppendAttempt {
        // The caller is responsible for knowing if there is a queue or not
        for ((block_hash, header_one, hash_one), (header_two, hash_two)) in self
            .merged_queue
            .as_ref()
            .unwrap()
            .iter()
            .map(|queue| queue.tuple())
            .zip(cf_headers.inner())
        {
            if header_one.ne(&header_two) || hash_one.ne(&hash_two) {
                self.merged_queue = None;
                self.current_quorum = 0;
                return AppendAttempt::Conflict(block_hash);
            }
        }
        self.current_quorum += 1;
        self.attempt_merge().await
    }

    // If enough peers have responded, insert those block hashes and filter hashes into a map.
    async fn attempt_merge(&mut self) -> AppendAttempt {
        let queue = self.merged_queue.as_ref().unwrap();
        if self.current_quorum.ge(&self.quorum_required) {
            for (block_hash, filter_hash) in queue.iter().map(|queue| queue.hash_tuple()) {
                self.hash_chain.insert(block_hash, filter_hash);
            }
            self.current_quorum = 0;
            // Empty messages are rejected higher up the stack.
            self.prev_header = queue.last().map(|queue| queue.filter_header);
            self.merged_queue = None;
            return AppendAttempt::Extended;
        }
        AppendAttempt::AddedToQueue
    }

    pub(crate) fn height(&self) -> u32 {
        self.anchor_checkpoint.height + self.hash_chain.len() as u32
    }

    pub(crate) fn prev_header(&self) -> Option<FilterHeader> {
        self.prev_header
    }

    pub(crate) fn set_last_stop_hash(&mut self, stop_hash: BlockHash) {
        self.prev_stophash_request = Some(stop_hash)
    }

    pub(crate) fn last_stop_hash_request(&mut self) -> &Option<BlockHash> {
        &self.prev_stophash_request
    }

    pub(crate) fn has_queue(&self) -> bool {
        self.merged_queue.is_some()
    }

    pub(crate) fn clear_queue(&mut self) {
        self.current_quorum = 0;
        self.merged_queue = None;
    }

    pub(crate) fn clear_headers(&mut self) {
        self.prev_header = None;
        self.hash_chain.clear();
    }

    // Some blocks got reorganized, so we remove them as well as the previous header
    pub(crate) fn remove(&mut self, hashes: &[BlockHash]) {
        for hash in hashes {
            self.hash_chain.remove(hash);
        }
        self.prev_header = None;
    }

    pub(crate) fn hash_at(&self, block: &BlockHash) -> Option<&FilterHash> {
        self.hash_chain.get(block)
    }

    pub(crate) fn quorum_required(&self) -> usize {
        self.quorum_required
    }

    pub(crate) fn map_len(&self) -> usize {
        self.hash_chain.len()
    }
}
