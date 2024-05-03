use crate::prelude::*;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub struct TimeManager {
    offsets: LRUCache<i64>,
}

impl TimeManager {
    // we define the maximum number of peers and evict the oldest peer in the case we overflow
    pub(crate) fn new(max_peers: usize) -> Self {
        let cache = LRUCache::new(max_peers);
        TimeManager { offsets: cache }
    }

    // add a peer's time to the manager
    pub(crate) fn add_peer_time(&mut self, other: u64) {
        let now = SystemTime::now();
        let duration_since_epoch = now.duration_since(UNIX_EPOCH).expect("time went backwards");
        let unix_time = duration_since_epoch.as_secs();
        let offset = (unix_time as i64) - (other as i64);
        self.offsets.add(offset)
    }

    // pub(crate) fn offset_by_network_adjustment(&mut self, block_time: u64) -> u64 {
    //     let median_adjustment = self
    //         .offsets
    //         .median()
    //         .expect("at least one peer is serving blocks");
    //     let adjusted_time = block_time as i64 - median_adjustment;
    //     adjusted_time as u64
    // }

    pub(crate) fn network_offset(&mut self) -> i64 {
        self.offsets
            .median()
            .expect("at least one peer is serving blocks")
    }
}
