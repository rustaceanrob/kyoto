use std::{
    net::IpAddr,
    sync::{Arc, Mutex},
    time::Duration,
};

use bitcoin::{
    block::Header,
    p2p::{Address, ServiceFlags},
    BlockHash, Network,
};
use rand::{prelude::SliceRandom, thread_rng};
use thiserror::Error;
use tokio::sync::mpsc;

use crate::{
    headers::{error::HeaderSyncError, header_chain::HeaderChain},
    node::peer_map::PeerMap,
    peers::dns::Dns,
};

use super::channel_messages::{
    GetHeaderConfig, MainThreadMessage, PeerMessage, PeerThreadMessage, RemoteVersion,
};
use crate::db::sqlite::peer_db::SqlitePeerDb;

#[derive(Debug, Clone, Copy)]
pub enum NodeState {
    // we need to sync headers to the known tip
    Behind,
    // we need to start getting filter headers
    HeadersSynced,
    // we need to get the CP filters
    FilterHeadersSynced,
    // we can start asking for blocks with matches
    FiltersSynced,
}

pub struct Node {
    state: Arc<Mutex<NodeState>>,
    header_chain: Arc<Mutex<HeaderChain>>,
    // fill filter headers, etc
    peer_db: Arc<Mutex<SqlitePeerDb>>,
    best_known_height: u32,
    best_known_hash: Option<BlockHash>,
    required_peers: usize,
    network: Network,
}

impl Node {
    pub fn new(network: Network) -> Result<Self, MainThreadError> {
        let state = Arc::new(Mutex::new(NodeState::Behind));
        let peer_db = SqlitePeerDb::new(network).map_err(|e| {
            println!("Persistence failure: {}", e.to_string());
            MainThreadError::LoadError(PersistenceError::PeerLoadFailure)
        })?;
        let peer_db = Arc::new(Mutex::new(peer_db));
        let loaded_chain = HeaderChain::new(&network)
            .map_err(|_| MainThreadError::LoadError(PersistenceError::HeaderLoadError))?;
        let best_known_height = loaded_chain.height() as u32;
        println!("Headers loaded from storage: {}", best_known_height);
        let header_chain = Arc::new(Mutex::new(loaded_chain));
        let best_known_hash = None;
        Ok(Self {
            state,
            header_chain,
            peer_db,
            best_known_height,
            best_known_hash,
            required_peers: 1,
            network,
        })
    }
    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error + '_>> {
        println!("Starting node");
        let (mtx, mut mrx) = mpsc::channel::<PeerThreadMessage>(32);
        let mut node_map = PeerMap::new(mtx, self.network.clone());
        loop {
            let required_peers = self.try_advance_state().await?;
            node_map.clean().await;
            // rehydrate on peers when lower than a threshold
            if node_map.live() < required_peers {
                println!("Required peers: {}", required_peers);
                println!("Live peers: {}", node_map.live());
                println!("Not connected to enough peers, finding one...");
                let ip = self.next_peer().await?;
                node_map.dispatch(ip.0, ip.1).await
            }
            while let Ok(Some(peer_thread)) =
                tokio::time::timeout(Duration::from_secs(5), mrx.recv()).await
            {
                match peer_thread.message {
                    PeerMessage::Version(version) => {
                        node_map.set_offset(peer_thread.nonce, version.timestamp);
                        node_map.set_services(peer_thread.nonce, version.service_flags);
                        if let Ok(response) = self.handle_version(version).await {
                            node_map.send_message(peer_thread.nonce, response).await;
                        }
                    }
                    PeerMessage::Addr(addresses) => {
                        if let Err(e) = self.handle_new_addrs(addresses).await {
                            println!("Error storing new addresses: {}", e);
                        }
                    }
                    PeerMessage::Headers(headers) => match self.handle_headers(headers).await {
                        Ok(response) => {
                            if let Some(response) = response {
                                node_map.send_message(peer_thread.nonce, response).await;
                            }
                        }
                        Err(_) => continue,
                    },
                    PeerMessage::FilterHeaders(_cf_headers) => {
                        println!("Received compact filter headers");
                    }
                    PeerMessage::Disconnect => {
                        // remove the node from the BTreeMap
                        node_map.clean().await;
                    }
                    _ => continue,
                }
            }
        }
    }

    async fn try_advance_state(&mut self) -> Result<usize, MainThreadError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| MainThreadError::PoisonedGuard)?;
        match *state {
            NodeState::Behind => {
                let mut header_guard = self
                    .header_chain
                    .lock()
                    .map_err(|_| MainThreadError::PoisonedGuard)?;
                if header_guard.is_synced() {
                    println!("Headers synced. Auditing our chain with peers");
                    header_guard.flush_to_disk().await;
                    *state = NodeState::HeadersSynced;
                    return Ok(3);
                }
                return Ok(1);
            }
            NodeState::HeadersSynced => return Ok(3),
            NodeState::FilterHeadersSynced => return Ok(3),
            NodeState::FiltersSynced => return Ok(3),
        }
    }

    async fn handle_version(
        &mut self,
        version_message: RemoteVersion,
    ) -> Result<MainThreadMessage, MainThreadError> {
        let state = self
            .state
            .lock()
            .map_err(|_| MainThreadError::PoisonedGuard)?;
        match *state {
            NodeState::Behind => (),
            _ => {
                if !version_message
                    .service_flags
                    .has(ServiceFlags::COMPACT_FILTERS)
                {
                    println!("Connected peer does not serve compact filters");
                    return Ok(MainThreadMessage::Disconnect);
                }
            }
        }
        let mut guard = self
            .header_chain
            .lock()
            .map_err(|_| MainThreadError::PoisonedGuard)?;
        let peer_height = version_message.height as u32;
        if peer_height.ge(&self.best_known_height) {
            self.best_known_height = peer_height;
            guard.set_best_known_height(peer_height);
        }
        let next_headers = GetHeaderConfig {
            locators: guard.locators(),
            stop_hash: None,
        };
        // even if we start the node as caught up in terms of height, we need to check for reorgs
        let response = MainThreadMessage::GetHeaders(next_headers);
        Ok(response)
    }

    async fn handle_new_addrs(&mut self, new_peers: Vec<Address>) -> Result<(), MainThreadError> {
        let mut guard = self
            .peer_db
            .lock()
            .map_err(|_| MainThreadError::PoisonedGuard)?;
        if let Err(e) = guard.add_cpf_peers(new_peers).await {
            println!(
                "Encountered error adding peer to persistence: {}",
                e.to_string()
            );
        }
        Ok(())
    }

    async fn handle_headers(
        &mut self,
        headers: Vec<Header>,
    ) -> Result<Option<MainThreadMessage>, MainThreadError> {
        let mut guard = self
            .header_chain
            .lock()
            .map_err(|_| MainThreadError::PoisonedGuard)?;
        if let Err(e) = guard.sync_chain(headers).await {
            match e {
                HeaderSyncError::EmptyMessage => return Ok(None),
                _ => {
                    println!("{}", e.to_string());
                    return Ok(Some(MainThreadMessage::Disconnect));
                }
            }
        }
        if !guard.is_synced() {
            let next_headers = GetHeaderConfig {
                locators: guard.locators(),
                stop_hash: None,
            };
            return Ok(Some(MainThreadMessage::GetHeaders(next_headers)));
        }
        Ok(None)
    }

    async fn next_peer(&mut self) -> Result<(IpAddr, Option<u16>), MainThreadError> {
        let state = *self
            .state
            .lock()
            .map_err(|_| MainThreadError::PoisonedGuard)?;
        match state {
            NodeState::Behind => self.any_peer().await,
            _ => match self.cpf_peer().await {
                Ok(got_peer) => match got_peer {
                    Some(peer) => Ok(peer),
                    None => self.any_peer().await,
                },
                Err(e) => return Err(e),
            },
        }
    }

    async fn cpf_peer(&mut self) -> Result<Option<(IpAddr, Option<u16>)>, MainThreadError> {
        let mut guard = self
            .peer_db
            .lock()
            .map_err(|_| MainThreadError::PoisonedGuard)?;
        // we prefer peers with CPF always
        if let Some(peer) = guard.get_random_cpf_peer().await.map_err(|e| {
            println!("Persistence failure: {}", e.to_string());
            MainThreadError::LoadError(PersistenceError::PeerLoadFailure)
        })? {
            return Ok(Some((peer.0, Some(peer.1))));
        }
        Ok(None)
    }

    async fn any_peer(&mut self) -> Result<(IpAddr, Option<u16>), MainThreadError> {
        let mut guard = self
            .peer_db
            .lock()
            .map_err(|_| MainThreadError::PoisonedGuard)?;

        // try to get any new peer
        let next_peer = guard.get_random_new().await.map_err(|e| {
            println!("Persistence failure: {}", e.to_string());
            MainThreadError::LoadError(PersistenceError::PeerLoadFailure)
        })?;
        match next_peer {
            // we found some peer to use but may not be reachable
            Some(peer) => {
                println!(
                    "Able to load a peer from persistence: {}",
                    peer.0.to_string()
                );
                Ok((peer.0, Some(peer.1)))
            }
            // we have no peers in our DB, try DNS
            None => {
                let mut new_peers = Dns::bootstrap(self.network)
                    .await
                    .map_err(|_| MainThreadError::DnsFailure)?;
                let mut rng = thread_rng();
                new_peers.shuffle(&mut rng);
                // DNS fails if there is an insufficient number of peers
                let ret_ip = new_peers[0];
                for peer in new_peers {
                    if let Err(e) = guard.add_new(peer, None, None).await {
                        println!(
                            "Encountered error adding peer to persistence: {}",
                            e.to_string()
                        );
                    }
                }
                Ok((ret_ip, None))
            }
        }
    }
}

#[derive(Error, Debug)]
pub enum MainThreadError {
    #[error("the lock acquired on the mutex may have left data in an indeterminant state")]
    PoisonedGuard,
    #[error("tpersistence failed")]
    LoadError(PersistenceError),
    #[error("dns bootstrap failed")]
    DnsFailure,
}

#[derive(Error, Debug)]
pub enum PersistenceError {
    #[error("there was an error loading the headers from persistence")]
    HeaderLoadError,
    #[error("there was an error loading peers from the database")]
    PeerLoadFailure,
}
