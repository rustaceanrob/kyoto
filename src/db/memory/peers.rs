use std::collections::HashMap;

use bitcoin::{
    key::rand::{self, seq::IteratorRandom},
    p2p::address::AddrV2,
};
use rand::{rngs::StdRng, SeedableRng};

use crate::db::{error::DatabaseError, traits::PeerStore, FutureResult, PeerStatus, PersistedPeer};

/// A simple peer store that does not save state in between sessions.
/// If DNS is not enabled, a node will require at least one peer to connect to.
/// Thereafter, the node will find peers to connect to throughout the session.
pub struct StatelessPeerStore {
    list: HashMap<AddrV2, PersistedPeer>,
}

impl StatelessPeerStore {
    pub fn new() -> Self {
        Self {
            list: HashMap::new(),
        }
    }

    async fn update(&mut self, peer: PersistedPeer) -> Result<(), DatabaseError> {
        // Don't add back peers we already connected to this session.
        match peer.status {
            PeerStatus::New => {
                self.list.entry(peer.clone().addr).or_insert(peer);
                Ok(())
            }
            PeerStatus::Tried => Ok(()),
            PeerStatus::Ban => {
                self.list.insert(peer.clone().addr, peer);
                Ok(())
            }
        }
    }

    async fn random(&mut self) -> Result<PersistedPeer, DatabaseError> {
        let mut rng = StdRng::from_entropy();
        let random_peer = {
            let iter = self
                .list
                .iter_mut()
                .filter(|(_, peer)| peer.status != PeerStatus::Ban);
            iter.choose(&mut rng).map(|(key, _)| key.clone())
        };
        match random_peer {
            Some(ip) => self.list.remove(&ip).ok_or(DatabaseError::Load),
            None => Err(DatabaseError::Load),
        }
    }

    async fn num_unbanned(&mut self) -> Result<u32, DatabaseError> {
        Ok(self
            .list
            .iter()
            .filter(|(_, peer)| peer.status != PeerStatus::Ban)
            .count() as u32)
    }
}

impl PeerStore for StatelessPeerStore {
    fn update(&mut self, peer: PersistedPeer) -> FutureResult<(), DatabaseError> {
        Box::pin(self.update(peer))
    }

    fn random(&mut self) -> FutureResult<PersistedPeer, DatabaseError> {
        Box::pin(self.random())
    }

    fn num_unbanned(&mut self) -> FutureResult<u32, DatabaseError> {
        Box::pin(self.num_unbanned())
    }
}

impl Default for StatelessPeerStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use bitcoin::p2p::ServiceFlags;

    use super::*;

    #[tokio::test]
    async fn test_stateless_store() {
        let mut peer_store = StatelessPeerStore::new();
        let ip_1 = Ipv4Addr::new(1, 1, 1, 1);
        let ip_2 = Ipv4Addr::new(2, 2, 2, 2);
        let peer_1 = PersistedPeer::new(AddrV2::Ipv4(ip_1), 0, ServiceFlags::NONE, PeerStatus::New);
        let peer_2 = PersistedPeer::new(AddrV2::Ipv4(ip_2), 0, ServiceFlags::NONE, PeerStatus::New);
        let tor = AddrV2::TorV2([0; 10]);
        let peer_3 = PersistedPeer::new(tor, 0, ServiceFlags::NONE, PeerStatus::New);
        let try_peer_2 =
            PersistedPeer::new(AddrV2::Ipv4(ip_2), 0, ServiceFlags::NONE, PeerStatus::Tried);
        let ban_peer_1 =
            PersistedPeer::new(AddrV2::Ipv4(ip_1), 0, ServiceFlags::NONE, PeerStatus::Ban);
        peer_store.update(peer_1).await.unwrap();
        assert_eq!(peer_store.num_unbanned().await.unwrap(), 1);
        peer_store.update(peer_2).await.unwrap();
        assert_eq!(peer_store.num_unbanned().await.unwrap(), 2);
        peer_store.update(peer_3).await.unwrap();
        assert_eq!(peer_store.num_unbanned().await.unwrap(), 3);
        peer_store.update(try_peer_2).await.unwrap();
        assert_eq!(peer_store.num_unbanned().await.unwrap(), 3);
        peer_store.update(ban_peer_1).await.unwrap();
        assert_eq!(peer_store.num_unbanned().await.unwrap(), 2);
        let _ = peer_store.random().await.unwrap();
        assert_eq!(peer_store.num_unbanned().await.unwrap(), 1);
        let _ = peer_store.random().await.unwrap();
        assert_eq!(peer_store.num_unbanned().await.unwrap(), 0);
        assert_eq!(peer_store.list.len(), 1);
        let last_peer = peer_store.random().await;
        assert!(last_peer.is_err());
    }
}
