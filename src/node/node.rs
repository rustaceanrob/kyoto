use std::{
    collections::HashSet,
    net::IpAddr,
    sync::{Arc, Mutex},
    time::Duration,
};

use bitcoin::{
    block::Header,
    p2p::{
        message_filter::{CFHeaders, CFilter},
        Address, ServiceFlags,
    },
    Block, Network,
};
use rand::{prelude::SliceRandom, thread_rng};
use thiserror::Error;
use tokio::sync::mpsc;

use crate::{
    chain::{chain::HeaderChain, error::HeaderSyncError},
    node::peer_map::PeerMap,
    peers::dns::Dns,
    tx::memory::MemoryTransactionCache,
};

use super::channel_messages::{
    GetBlockConfig, GetHeaderConfig, MainThreadMessage, PeerMessage, PeerThreadMessage,
    RemoteVersion,
};
use crate::db::sqlite::peer_db::SqlitePeerDb;

#[derive(Debug, Clone, Copy)]
pub enum NodeState {
    // We need to sync headers to the known tip
    Behind,
    // We need to start getting filter headers
    HeadersSynced,
    // We need to get the CP filters
    FilterHeadersSynced,
    // We can start asking for blocks with matches
    FiltersSynced,
}

pub struct Node {
    state: Arc<Mutex<NodeState>>,
    header_chain: Arc<Mutex<HeaderChain>>,
    peer_db: Arc<Mutex<SqlitePeerDb>>,
    best_known_height: u32,
    required_peers: usize,
    white_list: Option<Vec<(IpAddr, u16)>>,
    network: Network,
}

impl Node {
    pub fn new(
        network: Network,
        white_list: Option<Vec<(IpAddr, u16)>>,
        addresses: Vec<bitcoin::Address>,
    ) -> Result<Self, MainThreadError> {
        let state = Arc::new(Mutex::new(NodeState::Behind));
        let peer_db = SqlitePeerDb::new(network).map_err(|e| {
            println!("Persistence failure: {}", e.to_string());
            MainThreadError::LoadError(PersistenceError::PeerLoadFailure)
        })?;
        let peer_db = Arc::new(Mutex::new(peer_db));
        let mut scripts = HashSet::new();
        scripts.extend(addresses.iter().map(|address| address.script_pubkey()));
        let in_memory_cache = MemoryTransactionCache::new();
        let loaded_chain = HeaderChain::new(&network, scripts, in_memory_cache)
            .map_err(|_| MainThreadError::LoadError(PersistenceError::HeaderLoadError))?;
        let best_known_height = loaded_chain.height() as u32;
        println!("Headers loaded from storage: {}", best_known_height);
        let header_chain = Arc::new(Mutex::new(loaded_chain));
        Ok(Self {
            state,
            header_chain,
            peer_db,
            best_known_height,
            required_peers: 1,
            white_list,
            network,
        })
    }
    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error + '_>> {
        println!("Starting node");
        let (mtx, mut mrx) = mpsc::channel::<PeerThreadMessage>(32);
        let mut node_map = PeerMap::new(mtx, self.network.clone());
        loop {
            node_map.clean().await;
            // rehydrate on peers when lower than a threshold
            if node_map.live() < 1 {
                println!(
                    "Required peers: {}, connected peers: {}",
                    1,
                    node_map.live()
                );
                println!("Not connected to enough peers, finding one...");
                let ip = self.next_peer().await?;
                node_map.dispatch(ip.0, ip.1).await
            }
            if let Some(block_request) = self
                .pop_block_queue()
                .await
                .map_err(|_| MainThreadError::PoisonedGuard)?
            {
                println!("Sending block request to a random peer");
                node_map.send_random(block_request).await;
            }
            while let Ok(Some(peer_thread)) =
                tokio::time::timeout(Duration::from_secs(1), mrx.recv()).await
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
                    PeerMessage::FilterHeaders(cf_headers) => {
                        match self.handle_cf_headers(peer_thread.nonce, cf_headers).await {
                            Ok(response) => {
                                if let Some(response) = response {
                                    node_map.broadcast(response).await;
                                }
                            }
                            Err(_) => continue,
                        }
                    }
                    PeerMessage::Filter(filter) => {
                        match self.handle_filter(peer_thread.nonce, filter).await {
                            Ok(response) => {
                                if let Some(response) = response {
                                    node_map.broadcast(response).await;
                                }
                            }
                            Err(_) => continue,
                        }
                    }
                    PeerMessage::Block(block) => match self.handle_block(block).await {
                        Ok(response) => {
                            if let Some(response) = response {
                                node_map.broadcast(response).await;
                            }
                        }
                        Err(_) => continue,
                    },
                    PeerMessage::NewBlocks(_blocks) => match self.handle_inventory_blocks().await {
                        Ok(response) => {
                            if let Some(response) = response {
                                node_map.broadcast(response).await;
                            }
                        }
                        Err(_) => continue,
                    },
                    PeerMessage::Disconnect => {
                        node_map.clean().await;
                    }
                    _ => continue,
                }
                self.advance_state().await?;
            }
        }
    }

    async fn advance_state(&mut self) -> Result<(), MainThreadError> {
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
                    return Ok(());
                }
                Ok(())
            }
            NodeState::HeadersSynced => {
                let header_guard = self
                    .header_chain
                    .lock()
                    .map_err(|_| MainThreadError::PoisonedGuard)?;
                if header_guard.is_cf_headers_synced() {
                    println!("CF Headers synced. Downloading block filters.");
                    *state = NodeState::FilterHeadersSynced;
                }
                Ok(())
            }
            NodeState::FilterHeadersSynced => {
                let header_guard = self
                    .header_chain
                    .lock()
                    .map_err(|_| MainThreadError::PoisonedGuard)?;
                if header_guard.is_filters_synced() {
                    println!("Filters synced. Checking blocks for new inclusions.");
                    *state = NodeState::FiltersSynced;
                }
                Ok(())
            }
            NodeState::FiltersSynced => Ok(()),
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
                    || !version_message.service_flags.has(ServiceFlags::NETWORK)
                {
                    println!("Connected peer does not serve compact filters or blocks");
                    return Ok(MainThreadMessage::Disconnect);
                }
            }
        }
        // even if we start the node as caught up in terms of height, we need to check for reorgs
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
                HeaderSyncError::EmptyMessage => {
                    if !guard.is_synced() {
                        return Ok(Some(MainThreadMessage::Disconnect));
                    } else if !guard.is_cf_headers_synced() {
                        return Ok(Some(MainThreadMessage::GetFilterHeaders(
                            guard.next_cf_header_message().await.unwrap(),
                        )));
                    }
                    return Ok(None);
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
        } else if !guard.is_cf_headers_synced() {
            return Ok(Some(MainThreadMessage::GetFilterHeaders(
                guard.next_cf_header_message().await.unwrap(),
            )));
        } else if !guard.is_filters_synced() {
            return Ok(Some(MainThreadMessage::GetFilters(
                guard.next_filter_message().await.unwrap(),
            )));
        }
        Ok(None)
    }

    async fn handle_cf_headers(
        &mut self,
        peer_id: u32,
        cf_headers: CFHeaders,
    ) -> Result<Option<MainThreadMessage>, MainThreadError> {
        let mut guard = self
            .header_chain
            .lock()
            .map_err(|_| MainThreadError::PoisonedGuard)?;
        match guard.sync_cf_headers(peer_id, cf_headers).await {
            Ok(potential_message) => match potential_message {
                Some(message) => Ok(Some(MainThreadMessage::GetFilterHeaders(message))),
                None => {
                    if !guard.is_filters_synced() {
                        Ok(Some(MainThreadMessage::GetFilters(
                            guard.next_filter_message().await.unwrap(),
                        )))
                    } else {
                        Ok(None)
                    }
                }
            },
            Err(e) => {
                println!("CF header sync error: {}", e.to_string());
                Ok(Some(MainThreadMessage::Disconnect))
            }
        }
    }

    async fn handle_filter(
        &mut self,
        _peer_id: u32,
        filter: CFilter,
    ) -> Result<Option<MainThreadMessage>, MainThreadError> {
        let mut guard = self
            .header_chain
            .lock()
            .map_err(|_| MainThreadError::PoisonedGuard)?;
        match guard.sync_filter(filter).await {
            Ok(potential_message) => match potential_message {
                Some(message) => Ok(Some(MainThreadMessage::GetFilters(message))),
                None => Ok(None),
            },
            Err(e) => {
                println!("block filter sync error: {}", e.to_string());
                Ok(Some(MainThreadMessage::Disconnect))
            }
        }
    }

    async fn handle_block(
        &mut self,
        block: Block,
    ) -> Result<Option<MainThreadMessage>, MainThreadError> {
        let state = *self
            .state
            .lock()
            .map_err(|_| MainThreadError::PoisonedGuard)?;
        let mut guard = self
            .header_chain
            .lock()
            .map_err(|_| MainThreadError::PoisonedGuard)?;
        match state {
            NodeState::Behind => Ok(Some(MainThreadMessage::Disconnect)),
            NodeState::HeadersSynced => {
                // do something with the block to resolve a conflict
                Ok(None)
            }
            NodeState::FilterHeadersSynced => Ok(None),
            NodeState::FiltersSynced => {
                let _ = guard.scan_block(&block).await;
                Ok(None)
            }
        }
    }

    async fn pop_block_queue(&mut self) -> Result<Option<MainThreadMessage>, MainThreadError> {
        let mut guard = self
            .header_chain
            .lock()
            .map_err(|_| MainThreadError::PoisonedGuard)?;
        let next_block_hash = guard.next_block();
        match next_block_hash {
            Some(block_hash) => {
                println!("Next block in queue: {}", block_hash.to_string());
                Ok(Some(MainThreadMessage::GetBlock(GetBlockConfig {
                    locator: block_hash,
                })))
            }
            None => Ok(None),
        }
    }

    async fn handle_inventory_blocks(
        &mut self,
    ) -> Result<Option<MainThreadMessage>, MainThreadError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| MainThreadError::PoisonedGuard)?;
        match *state {
            NodeState::Behind => return Ok(None),
            _ => {
                let guard = self
                    .header_chain
                    .lock()
                    .map_err(|_| MainThreadError::PoisonedGuard)?;
                *state = NodeState::Behind;
                let next_headers = GetHeaderConfig {
                    locators: guard.locators(),
                    stop_hash: None,
                };
                return Ok(Some(MainThreadMessage::GetHeaders(next_headers)));
            }
        }
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
        // self.any_peer().await
    }

    async fn cpf_peer(&mut self) -> Result<Option<(IpAddr, Option<u16>)>, MainThreadError> {
        let mut guard = self
            .peer_db
            .lock()
            .map_err(|_| MainThreadError::PoisonedGuard)?;
        if let Some(peer) = guard.get_random_cpf_peer().await.map_err(|e| {
            println!("Persistence failure: {}", e.to_string());
            MainThreadError::LoadError(PersistenceError::PeerLoadFailure)
        })? {
            return Ok(Some((peer.0, Some(peer.1))));
        }
        Ok(None)
    }

    async fn any_peer(&mut self) -> Result<(IpAddr, Option<u16>), MainThreadError> {
        // empty the whitelist if there is one
        if let Some(whitelist) = &mut self.white_list {
            match whitelist.pop() {
                Some((ip, port)) => {
                    return {
                        println!("Using a peer from the white list");
                        Ok((ip, Some(port)))
                    }
                }
                None => (),
            }
        }
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
    #[error("persistence failed")]
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
