use std::collections::HashMap;

use bitcoin::{BlockHash, FilterHash, FilterHeader};

use super::{cfheader_batch::CFHeaderBatch, error::CFHeaderSyncError};

type InternalChain = Vec<(FilterHeader, FilterHash)>;
type FilterChain = Vec<Vec<u8>>;

pub(crate) enum AppendAttempt {
    // nothing to do yet
    AddedToQueue,
    // we sucessfully extended the current chain and should broadcast the next round of CF header messages
    Extended,
    // we found a conflict in the peers CF header messages at this index
    Conflict(usize),
}

#[derive(Debug)]
pub(crate) struct CFHeaderChain {
    anchor_height: usize,
    header_chain: InternalChain,
    merged_queue: HashMap<u32, InternalChain>,
    prev_stophash_request: Option<BlockHash>,
    quorum_required: usize,
}

impl CFHeaderChain {
    pub(crate) fn new(anchor_height: Option<usize>, quorum_required: usize) -> Self {
        Self {
            anchor_height: anchor_height.unwrap_or(190_000),
            header_chain: vec![],
            merged_queue: HashMap::new(),
            prev_stophash_request: None,
            quorum_required,
        }
    }

    pub(crate) async fn append(
        &mut self,
        peer_id: u32,
        cf_headers: CFHeaderBatch,
    ) -> Result<AppendAttempt, CFHeaderSyncError> {
        if self.merged_queue.get(&peer_id).is_some() {
            return Err(CFHeaderSyncError::UnexpectedCFHeaderMessage);
        }
        self.merged_queue.insert(peer_id, cf_headers.inner());
        self.try_merge().await
    }

    async fn try_merge(&mut self) -> Result<AppendAttempt, CFHeaderSyncError> {
        let staged_headers = self.merged_queue.values().count();
        if staged_headers.ge(&self.quorum_required) {
            // println!("Trying to extend the filter header chain");
            self.append_or_conflict().await
        } else {
            // println!("Added compact filter headers to the queue");
            Ok(AppendAttempt::AddedToQueue)
        }
    }

    async fn append_or_conflict(&mut self) -> Result<AppendAttempt, CFHeaderSyncError> {
        let ready = self
            .merged_queue
            .values_mut()
            .collect::<Vec<&mut Vec<(FilterHeader, FilterHash)>>>();
        // take any reference from the queue, we will start comparing the other peers to this one
        let reference_peer = ready.first().expect("all quorums have at least one peer");
        // move over the peers, skipping the reference
        for peer in ready.iter().skip(1) {
            // iterate over each index in the reference
            for index in 0..reference_peer.len() {
                // take the reference header
                let (header, _) = reference_peer[index];
                // compare it to the other peer
                if let Some(comparitor) = peer.get(index) {
                    if header.ne(&comparitor.0) {
                        println!(
                            "Found a conflict with CF headers at height: {}",
                            self.anchor_height + index
                        );
                        return Ok(AppendAttempt::Conflict(self.anchor_height + index));
                    }
                }
            }
        }
        // made it through without finding any conflicts, we can extend the current chain by the reference
        self.header_chain.extend_from_slice(&reference_peer);
        // reset the merge queue
        self.merged_queue.clear();
        println!(
            "Extended the chain of compact filter headers, synced up to height: {}",
            self.header_height()
        );
        Ok(AppendAttempt::Extended)
    }

    pub(crate) fn header_height(&self) -> usize {
        self.anchor_height + self.header_chain.len()
    }

    pub(crate) fn prev_header(&self) -> Option<FilterHeader> {
        if self.header_chain.is_empty() {
            None
        } else {
            Some(self.header_chain.last().unwrap().0)
        }
    }

    pub(crate) fn set_last_stop_hash(&mut self, stop_hash: BlockHash) {
        self.prev_stophash_request = Some(stop_hash)
    }

    pub(crate) fn last_stop_hash_request(&mut self) -> &Option<BlockHash> {
        &self.prev_stophash_request
    }

    pub(crate) fn filter_hash_at_height(&self, height: usize) -> Option<FilterHash> {
        let adjusted_height = height - self.anchor_height;
        if let Some((_, hash)) = self.header_chain.get(adjusted_height) {
            Some(*hash)
        } else {
            None
        }
    }
}
