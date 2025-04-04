//! Strucutres and checkpoints related to the blockchain.
//!
//! Notably, [`checkpoints`] contains known Bitcoin block hashes and heights with significant work, so Kyoto nodes do not have to sync from genesis.
pub(crate) mod block_queue;
#[allow(clippy::module_inception)]
pub(crate) mod chain;
/// Expected block header checkpoints and corresponding structure.
pub mod checkpoints;
/// Errors associated with the blockchain representation.
#[allow(dead_code)]
pub(crate) mod error;
pub(crate) mod graph;
pub(crate) mod header_batch;

use std::collections::HashMap;

use bitcoin::block::Header;

use crate::network::PeerId;

type Height = u32;

/// A block header with associated height.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct IndexedHeader {
    /// The height in the blockchain for this header.
    pub height: u32,
    /// The block header.
    pub header: Header,
}

impl IndexedHeader {
    pub(crate) fn new(height: u32, header: Header) -> Self {
        Self { height, header }
    }
}

#[derive(Debug)]
pub(crate) struct HeightMonitor {
    map: HashMap<PeerId, Height>,
}

impl HeightMonitor {
    pub(crate) fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub(crate) fn max(&self) -> Option<Height> {
        self.map.values().copied().max()
    }

    pub(crate) fn retain(&mut self, peers: &[PeerId]) {
        self.map.retain(|peer_id, _| peers.contains(peer_id));
    }

    pub(crate) fn insert(&mut self, peer_id: PeerId, height: Height) {
        self.map.insert(peer_id, height);
    }

    pub(crate) fn increment(&mut self, peer_id: PeerId) {
        if let Some(height) = self.map.get_mut(&peer_id) {
            *height = height.saturating_add(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_height_monitor() {
        let peer_one = 1.into();
        let peer_two = 2.into();
        let peer_three = 3.into();

        let mut height_monitor = HeightMonitor::new();
        height_monitor.insert(peer_one, 10);
        assert!(height_monitor.max().unwrap().eq(&10));
        height_monitor.insert(peer_two, 11);
        height_monitor.insert(peer_three, 12);
        assert!(height_monitor.max().unwrap().eq(&12));
        // this should remove peer three
        height_monitor.retain(&[peer_one, peer_two]);
        assert!(height_monitor.max().unwrap().eq(&11));
        height_monitor.retain(&[]);
        assert!(height_monitor.max().is_none());
        height_monitor.insert(peer_one, 10);
        assert!(height_monitor.max().unwrap().eq(&10));
        height_monitor.insert(peer_two, 11);
        // peer one is now at 11
        height_monitor.increment(peer_one);
        // peer one is now at 12
        height_monitor.increment(peer_one);
        assert!(height_monitor.max().unwrap().eq(&12));
    }
}
