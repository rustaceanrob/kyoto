use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    net::IpAddr,
    sync::Arc,
    time::Duration,
};

use bitcoin::{
    key::rand,
    p2p::{address::AddrV2, ServiceFlags},
    FeeRate, Network,
};
use rand::{rngs::StdRng, seq::IteratorRandom, SeedableRng};
use tokio::{
    sync::{
        mpsc::{self, Sender},
        Mutex,
    },
    task::JoinHandle,
};

use crate::{
    broadcaster::BroadcastQueue,
    chain::HeightMonitor,
    channel_messages::{MainThreadMessage, PeerThreadMessage},
    db::{traits::PeerStore, PeerStatus, PersistedPeer},
    dialog::Dialog,
    error::PeerManagerError,
    network::{
        dns::{bootstrap_dns, DnsResolver},
        error::PeerError,
        peer::Peer,
        PeerId, PeerTimeoutConfig,
    },
    prelude::{default_port_from_network, Netgroup},
    PeerStoreSizeConfig, TrustedPeer, Warning,
};

use super::ConnectionType;

const MAX_TRIES: usize = 50;

// Preferred peers to connect to based on the user configuration
type Whitelist = Vec<TrustedPeer>;

// A peer that is or was connected to the node
#[derive(Debug)]
pub(crate) struct ManagedPeer {
    address: AddrV2,
    port: u16,
    service_flags: ServiceFlags,
    broadcast_min: FeeRate,
    ptx: Sender<MainThreadMessage>,
    handle: JoinHandle<Result<(), PeerError>>,
}

// The `PeerMap` manages connections with peers, adds and bans peers, and manages the peer database
#[derive(Debug)]
pub(crate) struct PeerMap<P: PeerStore + 'static> {
    pub(crate) tx_queue: Arc<Mutex<BroadcastQueue>>,
    current_id: PeerId,
    heights: Arc<Mutex<HeightMonitor>>,
    network: Network,
    mtx: Sender<PeerThreadMessage>,
    map: HashMap<PeerId, ManagedPeer>,
    db: Arc<Mutex<P>>,
    connector: ConnectionType,
    whitelist: Whitelist,
    dialog: Arc<Dialog>,
    target_db_size: PeerStoreSizeConfig,
    net_groups: HashSet<String>,
    timeout_config: PeerTimeoutConfig,
    dns_resolver: DnsResolver,
}

impl<P: PeerStore> PeerMap<P> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        mtx: Sender<PeerThreadMessage>,
        network: Network,
        db: P,
        whitelist: Whitelist,
        dialog: Arc<Dialog>,
        connection_type: ConnectionType,
        target_db_size: PeerStoreSizeConfig,
        timeout_config: PeerTimeoutConfig,
        height_monitor: Arc<Mutex<HeightMonitor>>,
        dns_resolver: DnsResolver,
    ) -> Self {
        Self {
            tx_queue: Arc::new(Mutex::new(BroadcastQueue::new())),
            current_id: PeerId(0),
            heights: height_monitor,
            network,
            mtx,
            map: HashMap::new(),
            db: Arc::new(Mutex::new(db)),
            connector: connection_type,
            whitelist,
            dialog,
            target_db_size,
            net_groups: HashSet::new(),
            timeout_config,
            dns_resolver,
        }
    }

    // Remove any finished connections
    pub async fn clean(&mut self) {
        self.map.retain(|_, peer| !peer.handle.is_finished());
        let active = self.map.keys().copied().collect::<Vec<PeerId>>();
        let mut height_lock = self.heights.lock().await;
        height_lock.retain(&active);
    }

    // The number of peers with live connections
    pub fn live(&mut self) -> usize {
        self.map
            .values()
            .filter(|peer| !peer.handle.is_finished())
            .count()
    }

    // Set a new timeout duration
    pub fn set_duration(&mut self, duration: Duration) {
        self.timeout_config.response_timeout = duration;
    }

    // Add a new trusted peer to the whitelist
    pub fn add_trusted_peer(&mut self, peer: TrustedPeer) {
        self.whitelist.push(peer);
    }

    // Send out a TCP connection to a new peer and begin tracking the task
    pub async fn dispatch(&mut self, loaded_peer: PersistedPeer) -> Result<(), PeerError> {
        let (ptx, prx) = mpsc::channel::<MainThreadMessage>(32);
        self.current_id.increment();
        let mut peer = Peer::new(
            self.current_id,
            self.network,
            self.mtx.clone(),
            prx,
            loaded_peer.services,
            Arc::clone(&self.dialog),
            Arc::clone(&self.db),
            self.timeout_config,
            Arc::clone(&self.tx_queue),
        );
        if !self.connector.can_connect(&loaded_peer.addr) {
            return Err(PeerError::UnreachableSocketAddr);
        }
        crate::log!(
            self.dialog,
            format!("Connecting to {:?}:{}", loaded_peer.addr, loaded_peer.port)
        );
        let connection = self
            .connector
            .connect(
                loaded_peer.addr.clone(),
                loaded_peer.port,
                self.timeout_config.handshake_timeout,
            )
            .await?;
        let handle = tokio::spawn(async move { peer.run(connection).await });
        self.map.insert(
            self.current_id,
            ManagedPeer {
                service_flags: loaded_peer.services,
                address: loaded_peer.addr,
                port: loaded_peer.port,
                broadcast_min: FeeRate::BROADCAST_MIN,
                ptx,
                handle,
            },
        );
        Ok(())
    }

    // Set the minimum fee rate this peer will accept
    pub fn set_broadcast_min(&mut self, nonce: PeerId, fee_rate: FeeRate) {
        if let Some(peer) = self.map.get_mut(&nonce) {
            peer.broadcast_min = fee_rate;
        }
    }

    // Set the services of a peer
    pub fn set_services(&mut self, nonce: PeerId, flags: ServiceFlags) {
        if let Some(peer) = self.map.get_mut(&nonce) {
            peer.service_flags = flags
        }
    }

    // Set the height of a peer upon receiving the version message
    pub async fn set_height(&mut self, nonce: PeerId, height: u32) {
        let mut height_lock = self.heights.lock().await;
        height_lock.insert(nonce, height);
    }

    // Add one to the height of a peer when receiving inventory
    pub async fn increment_height(&mut self, nonce: PeerId) {
        let mut height_lock = self.heights.lock().await;
        height_lock.increment(nonce);
    }

    // The minimum fee rate to successfully broadcast a transaction to all peers
    pub fn broadcast_min(&self) -> FeeRate {
        self.map
            .values()
            .map(|peer| peer.broadcast_min)
            .max()
            .unwrap_or(FeeRate::BROADCAST_MIN)
    }

    // Send a message to the specified peer
    pub async fn send_message(&mut self, nonce: PeerId, message: MainThreadMessage) {
        if let Some(peer) = self.map.get(&nonce) {
            let _ = peer.ptx.send(message).await;
        }
    }

    // Broadcast to all connected peers, returning if at least one peer received the message.
    pub async fn broadcast(&mut self, message: MainThreadMessage) -> bool {
        let active = self.map.values().filter(|peer| !peer.handle.is_finished());
        let mut sends = Vec::new();
        for peer in active {
            let res = peer.ptx.send(message.clone()).await;
            sends.push(res.is_ok());
        }
        sends.into_iter().any(|res| res)
    }

    // Send to a random peer, returning true if the message was sent.
    pub async fn send_random(&mut self, message: MainThreadMessage) -> bool {
        let mut rng = StdRng::from_entropy();
        if let Some((_, peer)) = self.map.iter().choose(&mut rng) {
            let res = peer.ptx.send(message).await;
            return res.is_ok();
        }
        false
    }

    // Pull a peer from the configuration if we have one. If not, select a random peer from the database,
    // as long as it is not from the same netgroup. If there are no peers in the database, try DNS.
    pub async fn next_peer(&mut self) -> Result<PersistedPeer, PeerManagerError<P::Error>> {
        if let Some(peer) = self.whitelist.pop() {
            crate::log!(self.dialog, "Using a configured peer");
            let port = peer
                .port
                .unwrap_or(default_port_from_network(&self.network));
            let peer =
                PersistedPeer::new(peer.address, port, peer.known_services, PeerStatus::Tried);
            return Ok(peer);
        }
        let current_count = {
            let mut peer_manager = self.db.lock().await;
            peer_manager.num_unbanned().await?
        };
        if current_count < 1 {
            self.dialog.send_warning(Warning::EmptyPeerDatabase);
            self.bootstrap().await?;
        }
        let mut peer_manager = self.db.lock().await;
        let mut tries = 0;
        let desired_status = PeerStatus::random();
        while tries < MAX_TRIES {
            let peer = peer_manager.random().await?;
            if self.net_groups.contains(&peer.addr.netgroup())
                || desired_status.ne(&peer.status)
                || !peer.services.has(ServiceFlags::COMPACT_FILTERS)
            {
                tries += 1;
                continue;
            } else {
                return Ok(peer);
            }
        }
        peer_manager.random().await.map_err(From::from)
    }

    // Do we need peers
    pub async fn need_peers(&mut self) -> Result<bool, PeerManagerError<P::Error>> {
        match self.target_db_size {
            PeerStoreSizeConfig::Unbounded => Ok(true),
            PeerStoreSizeConfig::Limit(limit) => {
                let mut db = self.db.lock().await;
                let num_unbanned = db.num_unbanned().await?;
                Ok(num_unbanned < limit)
            }
        }
    }

    // We tried this peer and successfully connected.
    pub async fn tried(&mut self, nonce: PeerId) {
        if let Some(peer) = self.map.get(&nonce) {
            let mut db = self.db.lock().await;
            if let Err(e) = db
                .update(PersistedPeer::new(
                    peer.address.clone(),
                    peer.port,
                    peer.service_flags,
                    PeerStatus::Tried,
                ))
                .await
            {
                self.dialog.send_warning(Warning::FailedPersistence {
                    warning: format!(
                        "Encountered an error adding {:?}:{} flags: {} ... {e}",
                        peer.address, peer.port, peer.service_flags
                    ),
                });
            }
        }
    }

    // This peer misbehaved in some way.
    pub async fn ban(&mut self, nonce: PeerId) {
        if let Some(peer) = self.map.get(&nonce) {
            let mut db = self.db.lock().await;
            if let Err(e) = db
                .update(PersistedPeer::new(
                    peer.address.clone(),
                    peer.port,
                    peer.service_flags,
                    PeerStatus::Ban,
                ))
                .await
            {
                self.dialog.send_warning(Warning::FailedPersistence {
                    warning: format!(
                        "Encountered an error adding {:?}:{} flags: {} ... {e}",
                        peer.address, peer.port, peer.service_flags
                    ),
                });
            }
        }
    }

    async fn bootstrap(&mut self) -> Result<(), PeerManagerError<P::Error>> {
        crate::log!(self.dialog, "Bootstrapping peers with DNS");
        let mut db_lock = self.db.lock().await;
        let new_peers = bootstrap_dns(self.network, self.dns_resolver)
            .await
            .into_iter()
            .map(|ip| match ip {
                IpAddr::V4(ip) => AddrV2::Ipv4(ip),
                IpAddr::V6(ip) => AddrV2::Ipv6(ip),
            })
            .collect::<Vec<AddrV2>>();
        crate::log!(
            self.dialog,
            format!("Adding {} sourced from DNS", new_peers.len())
        );
        for peer in new_peers {
            db_lock
                .update(PersistedPeer::new(
                    peer,
                    default_port_from_network(&self.network),
                    ServiceFlags::NONE,
                    PeerStatus::Gossiped,
                ))
                .await
                .map_err(PeerManagerError::Database)?;
        }
        Ok(())
    }
}
