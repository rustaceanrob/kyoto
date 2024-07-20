use std::{
    collections::{HashMap, HashSet},
    net::IpAddr,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use bitcoin::{p2p::ServiceFlags, Network};
use rand::{rngs::StdRng, seq::IteratorRandom, SeedableRng};
use tokio::{
    sync::{
        mpsc::{self, Sender},
        Mutex,
    },
    task::JoinHandle,
};

use crate::{
    db::{error::PeerManagerError, traits::PeerStore, PeerStatus, PersistedPeer},
    peers::peer::{Peer, PeerError},
    prelude::{Median, Netgroup},
};

use super::{
    channel_messages::{MainThreadMessage, PeerThreadMessage},
    dialog::Dialog,
    error::NodeError,
    messages::Warning,
};

// Preferred peers to connect to based on the user configuration
type Whitelist = Option<Vec<(IpAddr, u16)>>;

// A peer that is or was connected to the node
#[derive(Debug)]
pub(crate) struct ManagedPeer {
    net_time: i64,
    ip_addr: IpAddr,
    port: u16,
    service_flags: Option<ServiceFlags>,
    ptx: Sender<MainThreadMessage>,
    handle: JoinHandle<Result<(), PeerError>>,
}

// The `PeerMap` manages connections with peers, adds and bans peers, and manages the peer database
#[derive(Debug)]
pub(crate) struct PeerMap {
    num_peers: u32,
    heights: HashMap<u32, u32>,
    network: Network,
    mtx: Sender<PeerThreadMessage>,
    map: HashMap<u32, ManagedPeer>,
    db: Arc<Mutex<dyn PeerStore + Send + Sync>>,
    whitelist: Whitelist,
    dialog: Dialog,
    target_db_size: u32,
    net_groups: HashSet<String>,
}

impl PeerMap {
    pub fn new(
        mtx: Sender<PeerThreadMessage>,
        network: Network,
        db: impl PeerStore + Send + Sync + 'static,
        whitelist: Whitelist,
        dialog: Dialog,
        target_db_size: u32,
    ) -> Self {
        Self {
            num_peers: 0,
            heights: HashMap::new(),
            network,
            mtx,
            map: HashMap::new(),
            db: Arc::new(Mutex::new(db)),
            whitelist,
            dialog,
            target_db_size,
            net_groups: HashSet::new(),
        }
    }

    // Remove any finished connections
    pub async fn clean(&mut self) {
        self.map.retain(|_, peer| !peer.handle.is_finished());
        self.heights.retain(|peer, _| self.map.contains_key(peer));
    }

    // The number of peers with live connections
    pub fn live(&mut self) -> usize {
        self.map
            .values()
            .filter(|peer| !peer.handle.is_finished())
            .count()
    }

    // The number of peers that serve compact block filters
    pub fn num_cpf_peers(&mut self) -> usize {
        self.map
            .values()
            .filter(|peer| !peer.handle.is_finished())
            .filter(|peer| {
                if let Some(flags) = peer.service_flags {
                    flags.has(ServiceFlags::COMPACT_FILTERS)
                } else {
                    false
                }
            })
            .count()
    }

    // Get the median time adjustment for the currently connected peers
    pub fn median_time_adjustment(&self) -> i64 {
        if self.map.values().len() > 0 {
            let mut time_offsets: Vec<i64> = self.map.values().map(|peer| peer.net_time).collect();
            return time_offsets.median().unwrap();
        }
        0
    }

    // Set the time offset of a connected peer
    pub fn set_offset(&mut self, peer: u32, time: i64) {
        if let Some(peer) = self.map.get_mut(&peer) {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time went backwards")
                .as_secs();
            peer.net_time = time - now as i64;
        }
    }

    // Send out a TCP connection to a new peer and begin tracking the task
    pub async fn dispatch(&mut self, ip: IpAddr, port: u16) {
        let (ptx, prx) = mpsc::channel::<MainThreadMessage>(32);
        let peer_num = self.num_peers + 1;
        self.num_peers = peer_num;
        let mut peer = Peer::new(
            peer_num,
            ip,
            port,
            self.network,
            self.mtx.clone(),
            prx,
            self.dialog.clone(),
        );
        let handle = tokio::spawn(async move { peer.connect().await });
        self.dialog
            .send_dialog(format!("Connecting to {}:{}", ip, port))
            .await;
        self.map.insert(
            peer_num,
            ManagedPeer {
                service_flags: None,
                ip_addr: ip,
                port,
                net_time: 0,
                ptx,
                handle,
            },
        );
    }

    // Set the services of a peer
    pub fn set_services(&mut self, nonce: u32, flags: ServiceFlags) {
        if let Some(peer) = self.map.get_mut(&nonce) {
            peer.service_flags = Some(flags)
        }
    }

    // Set the height of a peer upon receiving the version message
    pub fn set_height(&mut self, nonce: u32, height: u32) {
        self.heights.insert(nonce, height);
    }

    // Add one to the height of a peer when receiving inventory
    pub fn add_one_height(&mut self, nonce: u32) {
        if let Some(height) = self.heights.get(&nonce) {
            self.heights.insert(nonce, height + 1);
        }
    }

    // The best height of all known or connected peers
    pub fn best_height(&self) -> Option<&u32> {
        self.heights.values().max()
    }

    // Send a message to the specified peer
    pub async fn send_message(&mut self, nonce: u32, message: MainThreadMessage) {
        if let Some(peer) = self.map.get(&nonce) {
            let _ = peer.ptx.send(message).await;
        }
    }

    // Broadcast to all connected peers
    pub async fn broadcast(&mut self, message: MainThreadMessage) {
        let active = self.map.values().filter(|peer| !peer.handle.is_finished());
        for peer in active {
            let _ = peer.ptx.send(message.clone()).await;
        }
    }

    // Send to a random peer
    pub async fn send_random(&mut self, message: MainThreadMessage) {
        let mut rng = StdRng::from_entropy();
        if let Some((_, peer)) = self.map.iter().choose(&mut rng) {
            let _ = peer.ptx.send(message.clone()).await;
        }
    }

    // Pull a peer from the configuration if we have one. If not, select a random peer from the database,
    // as long as it is not from the same netgroup. If there are no peers in the database, try DNS.
    pub async fn next_peer(&mut self) -> Result<(IpAddr, u16), NodeError> {
        if let Some(whitelist) = &mut self.whitelist {
            if let Some((ip, port)) = whitelist.pop() {
                self.dialog
                    .send_dialog("Using a configured peer.".into())
                    .await;
                return Ok((ip, port));
            }
        }
        let current_count = {
            let mut peer_manager = self.db.lock().await;
            peer_manager
                .num_unbanned()
                .await
                .map_err(|e| NodeError::PeerDatabase(PeerManagerError::Database(e)))?
        };
        if current_count < 1 {
            self.dialog.send_warning(Warning::EmptyPeerDatabase).await;
            #[cfg(feature = "dns")]
            self.bootstrap().await.map_err(NodeError::PeerDatabase)?;
        }
        let mut peer_manager = self.db.lock().await;
        let mut tries = 0;
        while tries < 10 {
            let mut peer: (IpAddr, u16) = peer_manager
                .random()
                .await
                .map(|r| r.into())
                .map_err(|e| NodeError::PeerDatabase(PeerManagerError::Database(e)))?;
            if self.net_groups.contains(&peer.0.netgroup()) {
                tries += 1;
                continue;
            } else {
                return Ok(peer);
            }
        }
        peer_manager
            .random()
            .await
            .map(|r| r.into())
            .map_err(|e| NodeError::PeerDatabase(PeerManagerError::Database(e)))
    }

    // Do we need peers
    pub async fn need_peers(&mut self) -> Result<bool, NodeError> {
        let mut db = self.db.lock().await;
        let num_unbanned = db
            .num_unbanned()
            .await
            .map_err(|e| NodeError::PeerDatabase(PeerManagerError::Database(e)))?;
        Ok(num_unbanned < self.target_db_size)
    }

    // Add peers to the database that were gossiped over the p2p network
    pub async fn add_gossiped_peers(&mut self, peers: Vec<(IpAddr, u16, ServiceFlags)>) {
        let mut db = self.db.lock().await;
        for peer in peers {
            if let Err(e) = db
                .update(PersistedPeer::new(peer.0, peer.1, peer.2, PeerStatus::New))
                .await
            {
                self.dialog
                    .send_warning(Warning::FailedPersistance {
                        warning: format!(
                            "Encountered an error adding {}:{} flags: {} ... {e}",
                            peer.0, peer.1, peer.2
                        ),
                    })
                    .await;
            }
        }
    }

    pub async fn tried(&mut self, nonce: &u32) {
        if let Some(peer) = self.map.get(nonce) {
            let mut db = self.db.lock().await;
            if let Err(e) = db
                .update(PersistedPeer::new(
                    peer.ip_addr,
                    peer.port,
                    peer.service_flags.unwrap_or(ServiceFlags::NONE),
                    PeerStatus::Tried,
                ))
                .await
            {
                self.dialog
                    .send_warning(Warning::FailedPersistance {
                        warning: format!(
                            "Encountered an error adding {}:{} flags: {} ... {e}",
                            peer.ip_addr,
                            peer.port,
                            peer.service_flags.unwrap_or(ServiceFlags::NONE)
                        ),
                    })
                    .await;
            }
        }
    }

    pub async fn ban(&mut self, nonce: &u32) {
        if let Some(peer) = self.map.get(nonce) {
            let mut db = self.db.lock().await;
            if let Err(e) = db
                .update(PersistedPeer::new(
                    peer.ip_addr,
                    peer.port,
                    peer.service_flags.unwrap_or(ServiceFlags::NONE),
                    PeerStatus::Ban,
                ))
                .await
            {
                self.dialog
                    .send_warning(Warning::FailedPersistance {
                        warning: format!(
                            "Encountered an error adding {}:{} flags: {} ... {e}",
                            peer.ip_addr,
                            peer.port,
                            peer.service_flags.unwrap_or(ServiceFlags::NONE)
                        ),
                    })
                    .await;
            }
        }
    }

    #[cfg(feature = "dns")]
    async fn bootstrap(&mut self) -> Result<(), PeerManagerError> {
        use crate::{peers::dns::Dns, prelude::default_port_from_network};
        use rand::prelude::SliceRandom;
        self.dialog
            .send_dialog("Bootstraping peers with DNS".into())
            .await;
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
                    default_port_from_network(&self.network),
                    ServiceFlags::NONE,
                    PeerStatus::New,
                ))
                .await
                .map_err(PeerManagerError::Database)?;
        }
        Ok(())
    }
}
