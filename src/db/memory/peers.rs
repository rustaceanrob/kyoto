use std::{collections::HashMap, net::IpAddr};

use async_trait::async_trait;
use rand::{rngs::StdRng, seq::IteratorRandom, SeedableRng};

use crate::db::{error::DatabaseError, traits::PeerStore, PersistedPeer};

/// A simple peer store that does not save state in between sessions.
/// If DNS is not enabled, a node will require at least one peer to connect to.
/// Thereafter, the node will find peers to connect to throughout the session.
pub struct StatelessPeerStore {
    list: HashMap<IpAddr, PersistedPeer>,
}

impl StatelessPeerStore {
    pub fn new() -> Self {
        Self {
            list: HashMap::new(),
        }
    }
}

#[async_trait]
impl PeerStore for StatelessPeerStore {
    async fn update(&mut self, peer: PersistedPeer, replace: bool) -> Result<(), DatabaseError> {
        // Don't add back peers we already connected to this session.
        if peer.tried {
            return Ok(());
        }
        if replace {
            // We are banning a peer and want to keep track of who we don't get along with
            self.list.insert(peer.addr, peer);
        } else {
            self.list.entry(peer.addr).or_insert(peer);
        }
        Ok(())
    }

    async fn random(&mut self) -> Result<PersistedPeer, DatabaseError> {
        let mut rng = StdRng::from_entropy();
        let random_peer = {
            let iter = self.list.iter_mut().filter(|(_, peer)| !peer.banned);
            iter.choose(&mut rng).map(|(key, _)| *key)
        };
        match random_peer {
            Some(ip) => self.list.remove(&ip).ok_or(DatabaseError::Load),
            None => Err(DatabaseError::Load),
        }
    }

    async fn num_unbanned(&mut self) -> Result<u32, DatabaseError> {
        Ok(self.list.iter().filter(|(_, peer)| !peer.banned).count() as u32)
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
        let ip_1 = IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1));
        let ip_2 = IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2));
        let ip_3 = IpAddr::V4(Ipv4Addr::new(3, 3, 3, 3));
        let peer_1 = PersistedPeer::new(ip_1, 0, ServiceFlags::NONE, false, false);
        let peer_2 = PersistedPeer::new(ip_2, 0, ServiceFlags::NONE, false, false);
        let peer_3 = PersistedPeer::new(ip_3, 0, ServiceFlags::NONE, false, false);
        let try_peer_2 = PersistedPeer::new(ip_2, 0, ServiceFlags::NONE, true, false);
        let ban_peer_1 = PersistedPeer::new(ip_1, 0, ServiceFlags::NONE, false, true);
        peer_store.update(peer_1, false).await.unwrap();
        assert_eq!(peer_store.num_unbanned().await.unwrap(), 1);
        peer_store.update(peer_2, false).await.unwrap();
        assert_eq!(peer_store.num_unbanned().await.unwrap(), 2);
        peer_store.update(peer_3, false).await.unwrap();
        assert_eq!(peer_store.num_unbanned().await.unwrap(), 3);
        peer_store.update(try_peer_2, true).await.unwrap();
        assert_eq!(peer_store.num_unbanned().await.unwrap(), 3);
        peer_store.update(ban_peer_1, true).await.unwrap();
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
