use std::{
    net::IpAddr,
    sync::{Arc, Mutex},
};

use bitcoin::{block::Header, p2p::Address, BlockHash, Network};
use rand::{prelude::SliceRandom, thread_rng, RngCore};
use thiserror::Error;
use tokio::sync::mpsc;

use crate::{
    headers::header_chain::{HeaderChain, HeaderSyncError},
    peers::{dns::Dns, peer::Peer},
};

use super::channel_messages::{GetHeaderConfig, MainThreadMessage, PeerMessage, PeerThreadMessage};
use crate::db::sqlite::peer_db::SqlitePeerDb;

pub enum NodeState {
    Behind,
    HeadersSynced,
    FilterHeadersSynced,
    FiltersSynced,
}

pub struct Node {
    state: Arc<NodeState>,
    header_chain: Arc<Mutex<HeaderChain>>,
    // fill filter headers, etc
    peer_db: Arc<Mutex<SqlitePeerDb>>,
    best_known_height: u32,
    best_known_hash: Option<BlockHash>,
    network: Network,
}

impl Node {
    pub fn new(network: Network) -> Result<Self, MainThreadError> {
        let state = Arc::new(NodeState::Behind);
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
            network,
        })
    }
    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error + '_>> {
        println!("Starting node");
        let ip = self.startup().await?;
        let (mtx, mut mrx) = mpsc::channel::<PeerThreadMessage>(32);
        let (ptx, prx) = mpsc::channel::<MainThreadMessage>(32);
        let mut rng = thread_rng();
        let mut peer = Peer::new(rng.next_u32(), ip, None, self.network, mtx, prx);
        tokio::spawn(async move { peer.connect().await });
        loop {
            // rehydrate on peers when lower than a threshold
            // try to update the state of our node periodically
            if let Some(peer_thread) = mrx.recv().await {
                match peer_thread.message {
                    // add the peer version to our tried db
                    // add the node to a BTreeMap
                    // start asking for headers from where our tip is (add check if we are already caught up)
                    PeerMessage::Version(version) => {
                        let mut guard = self
                            .header_chain
                            .lock()
                            .map_err(|_| MainThreadError::PoisonedGuard)?;
                        let peer_height = version.height as u32;
                        if peer_height > self.best_known_height {
                            self.best_known_height = peer_height;
                            guard.set_best_known_height(peer_height);
                            let next_headers = GetHeaderConfig {
                                // should be done a little smarter
                                locators: guard.locators(),
                                stop_hash: None,
                            };
                            let response = MainThreadMessage::GetHeaders(next_headers);
                            let _ = ptx.send(response).await;
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
                                let _ = ptx.send(response).await;
                            }
                        }
                        // remove node from the BTreeMap
                        Err(_) => continue,
                    },
                    PeerMessage::Disconnect => {
                        // remove the node from the BTreeMap
                        return Ok(());
                    }
                    _ => continue,
                }
            }
        }
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
                HeaderSyncError::EmptyMessage => {
                    if !guard.is_synced() {
                        let next_headers = GetHeaderConfig {
                            locators: guard.locators(),
                            stop_hash: None,
                        };
                        return Ok(Some(MainThreadMessage::GetHeaders(next_headers)));
                    }
                }
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

    async fn startup(&mut self) -> Result<IpAddr, MainThreadError> {
        let mut guard = self
            .peer_db
            .lock()
            .map_err(|_| MainThreadError::PoisonedGuard)?;
        let next_peer = guard.get_random_new().await.map_err(|e| {
            println!("Persistence failure: {}", e.to_string());
            MainThreadError::LoadError(PersistenceError::PeerLoadFailure)
        })?;
        match next_peer {
            Some((ip, _)) => {
                println!("Able to load a peer from persistence: {}", ip.to_string());
                Ok(ip)
            }
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
                Ok(ret_ip)
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
