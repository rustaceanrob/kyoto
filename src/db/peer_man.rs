use std::{collections::HashSet, net::IpAddr, sync::Arc};

use bitcoin::{p2p::ServiceFlags, Network};
use rand::{prelude::SliceRandom, rngs::StdRng, SeedableRng};
use tokio::sync::Mutex;

use crate::{
    peers::dns::Dns,
    prelude::{default_port_from_network, SlashSixteen},
};

use super::{error::PeerManagerError, traits::PeerStore, PersistedPeer};

#[derive(Debug, Clone)]
pub(crate) struct PeerManager {
    db: Arc<Mutex<dyn PeerStore + Send + Sync>>,
    netgroups: HashSet<String>,
    network: Network,
    default_port: u16,
}

impl PeerManager {
    pub(crate) fn new(db: impl PeerStore + Send + Sync + 'static, network: &Network) -> Self {
        let default_port = default_port_from_network(network);
        Self {
            db: Arc::new(Mutex::new(db)),
            netgroups: HashSet::new(),
            network: *network,
            default_port,
        }
    }

    pub(crate) async fn next_peer(&mut self) -> Result<(IpAddr, u16), PeerManagerError> {
        let mut db_lock = self.db.lock().await;
        let mut tries = 0;
        while tries < 10 {
            let mut next = db_lock.random().await.map_err(PeerManagerError::Database)?;
            if !self.netgroups.contains(&next.addr.slash_sixteen()) {
                self.netgroups.insert(next.addr.slash_sixteen());
                return Ok((next.addr, next.port));
            }
            tries += 1;
        }
        let mut next = db_lock.random().await.map_err(PeerManagerError::Database)?;
        self.netgroups.insert(next.addr.slash_sixteen());
        Ok((next.addr, next.port))
    }

    pub(crate) async fn bootstrap(&mut self) -> Result<(), PeerManagerError> {
        let mut db_lock = self.db.lock().await;
        let mut new_peers = Dns::bootstrap(self.network)
            .await
            .map_err(|_| PeerManagerError::Dns)?;
        let mut rng = StdRng::from_entropy();
        new_peers.shuffle(&mut rng);
        // DNS fails if there is an insufficient number of peers
        for peer in new_peers {
            db_lock
                .update(PersistedPeer::new(
                    peer,
                    self.default_port,
                    ServiceFlags::NONE,
                    false,
                    false,
                ))
                .await
                .map_err(PeerManagerError::Database)?;
        }
        Ok(())
    }

    pub(crate) async fn peer_count(&mut self) -> Result<u32, PeerManagerError> {
        let mut db_lock = self.db.lock().await;
        db_lock
            .num_unbanned()
            .await
            .map_err(PeerManagerError::Database)
    }

    pub(crate) async fn add_new_peer(
        &mut self,
        addr: IpAddr,
        port: Option<u16>,
        services: Option<ServiceFlags>,
    ) -> Result<(), PeerManagerError> {
        self.internal_db_update(addr, port, services, false, false)
            .await
    }

    pub(crate) async fn tried_peer(
        &mut self,
        addr: IpAddr,
        port: Option<u16>,
        services: Option<ServiceFlags>,
    ) -> Result<(), PeerManagerError> {
        self.internal_db_update(addr, port, services, true, false)
            .await
    }

    pub(crate) async fn ban_peer(
        &mut self,
        addr: IpAddr,
        port: Option<u16>,
        services: Option<ServiceFlags>,
    ) -> Result<(), PeerManagerError> {
        self.internal_db_update(addr, port, services, true, true)
            .await
    }

    async fn internal_db_update(
        &mut self,
        addr: IpAddr,
        port: Option<u16>,
        services: Option<ServiceFlags>,
        tried: bool,
        ban: bool,
    ) -> Result<(), PeerManagerError> {
        let mut db_lock = self.db.lock().await;
        db_lock
            .update(PersistedPeer::new(
                addr,
                port.unwrap_or(self.default_port),
                services.unwrap_or(ServiceFlags::NONE),
                tried,
                ban,
            ))
            .await
            .map_err(PeerManagerError::Database)?;
        Ok(())
    }
}
