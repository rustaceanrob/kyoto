use std::{
    collections::HashSet,
    net::IpAddr,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use bitcoin::{
    block::Header,
    p2p::{
        message_filter::{CFHeaders, CFilter},
        Address, ServiceFlags,
    },
    Block, Network, ScriptBuf,
};
use tokio::sync::{broadcast, mpsc::Receiver, Mutex, RwLock};
use tokio::{
    select,
    sync::mpsc::{self},
};

use crate::{
    chain::{
        chain::Chain,
        checkpoints::{HeaderCheckpoint, HeaderCheckpoints},
        error::HeaderSyncError,
    },
    db::{
        error::{DatabaseError, PeerManagerError},
        peer_man::PeerManager,
        traits::{HeaderStore, PeerStore},
    },
    filters::cfheader_chain::AppendAttempt,
    node::peer_map::PeerMap,
    TxBroadcastPolicy,
};

use super::{
    broadcaster::Broadcaster,
    channel_messages::{
        GetBlockConfig, GetHeaderConfig, MainThreadMessage, PeerMessage, PeerThreadMessage,
        RemoteVersion,
    },
    client::Client,
    config::NodeConfig,
    dialog::Dialog,
    error::NodeError,
    messages::{ClientMessage, NodeMessage, SyncUpdate},
};

type Whitelist = Option<Vec<(IpAddr, u16)>>;

/// The state of the node with respect to connected peers.
#[derive(Debug, Clone, Copy)]
pub enum NodeState {
    /// We are behind on block headers according to our peers.
    Behind,
    /// We may start downloading compact block filter headers.
    HeadersSynced,
    // We may start scanning compact block filters
    FilterHeadersSynced,
    // We may start asking for blocks with matches
    FiltersSynced,
    // We found all known transactions to the wallet
    TransactionsSynced,
}

/// A compact block filter client
#[derive(Debug)]
pub struct Node {
    state: Arc<RwLock<NodeState>>,
    chain: Arc<Mutex<Chain>>,
    peer_man: Arc<Mutex<PeerManager>>,
    required_peers: usize,
    white_list: Whitelist,
    network: Network,
    dialog: Dialog,
    client_recv: Receiver<ClientMessage>,
    is_running: AtomicBool,
}

impl Node {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        network: Network,
        white_list: Whitelist,
        scripts: HashSet<ScriptBuf>,
        header_checkpoint: Option<HeaderCheckpoint>,
        required_peers: usize,
        target_peer_size: u32,
        peer_store: impl PeerStore + Send + Sync + 'static,
        header_store: impl HeaderStore + Send + Sync + 'static,
    ) -> (Self, Client) {
        // Set up a communication channel between the node and client
        let (ntx, _) = broadcast::channel::<NodeMessage>(32);
        let (ctx, crx) = mpsc::channel::<ClientMessage>(5);
        let client = Client::new(ntx.clone(), ctx);
        // We always assume we are behind
        let state = Arc::new(RwLock::new(NodeState::Behind));
        // Configure the address manager
        let peer_man = Arc::new(Mutex::new(PeerManager::new(
            peer_store,
            &network,
            target_peer_size,
        )));
        // Prepare the header checkpoints for the chain source
        let mut checkpoints = HeaderCheckpoints::new(&network);
        let checkpoint = header_checkpoint.unwrap_or_else(|| checkpoints.last());
        checkpoints.prune_up_to(checkpoint);
        // A structured way to talk to the client
        let dialog = Dialog::new(ntx);
        // Build the chain
        let loaded_chain = Chain::new(
            &network,
            scripts,
            checkpoint,
            checkpoints,
            dialog.clone(),
            header_store,
            required_peers,
        );
        // Initialize the chain with the headers we loaded
        let chain = Arc::new(Mutex::new(loaded_chain));
        (
            Self {
                state,
                chain,
                peer_man,
                required_peers,
                white_list,
                network,
                dialog,
                client_recv: crx,
                is_running: AtomicBool::new(false),
            },
            client,
        )
    }

    pub(crate) fn new_from_config(
        config: &NodeConfig,
        network: Network,
        peer_store: impl PeerStore + Send + Sync + 'static,
        header_store: impl HeaderStore + Send + Sync + 'static,
    ) -> (Self, Client) {
        Node::new(
            network,
            config.white_list.clone(),
            config.addresses.clone(),
            config.header_checkpoint,
            config.required_peers as usize,
            config.target_peer_size,
            peer_store,
            header_store,
        )
    }

    /// Has [`Node::run`] been called.
    pub fn is_running(&self) -> bool {
        self.is_running.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Run the node continuously. Typically run on a separate thread than the underlying application.
    pub async fn run(&mut self) -> Result<(), NodeError> {
        self.dialog.send_dialog("Starting node".into()).await;
        self.is_running
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.fetch_headers().await?;
        let (mtx, mut mrx) = mpsc::channel::<PeerThreadMessage>(32);
        let mut node_map = PeerMap::new(mtx, self.network);
        let mut tx_broadcaster = Broadcaster::new();
        loop {
            // Try to advance the state of the node and remove old connections
            self.advance_state().await;
            node_map.clean().await;
            // Find more peers when lower than the desired threshold.
            if node_map.live() < self.next_required_peers().await {
                self.dialog
                    .send_dialog(format!(
                        "Required peers: {}, connected peers: {}",
                        self.required_peers,
                        node_map.live()
                    ))
                    .await;
                self.dialog
                    .send_dialog("Not connected to enough peers, finding one...".into())
                    .await;
                let ip = self.next_peer().await?;
                node_map.dispatch(ip.0, ip.1).await
            }
            // If there are blocks in the queue, we should request them of a random peer
            if let Some(block_request) = self.pop_block_queue().await {
                self.dialog
                    .send_dialog("Sending block request to a random peer".into())
                    .await;
                node_map.send_random(block_request).await;
            }
            // If we have a transaction to broadcast and we are connected to peers, we should broadcast it
            if node_map.live().ge(&self.required_peers) && !tx_broadcaster.is_empty() {
                let transaction = tx_broadcaster.next().unwrap();
                let txid = transaction.tx.compute_txid();
                match transaction.broadcast_policy {
                    TxBroadcastPolicy::AllPeers => {
                        self.dialog
                            .send_dialog(format!(
                                "Sending transaction to {} connected peers.",
                                node_map.live()
                            ))
                            .await;
                        node_map
                            .broadcast(MainThreadMessage::BroadcastTx(transaction.tx))
                            .await;
                    }
                    TxBroadcastPolicy::RandomPeer => {
                        self.dialog
                            .send_dialog("Sending transaction to a random peer.".into())
                            .await;
                        node_map
                            .send_random(MainThreadMessage::BroadcastTx(transaction.tx))
                            .await;
                    }
                }
                self.dialog.send_data(NodeMessage::TxSent(txid)).await;
            }
            // Either handle a message from a remote peer or from our client
            select! {
                peer = tokio::time::timeout(Duration::from_secs(1), mrx.recv()) => {
                    match peer {
                        Ok(Some(peer_thread)) => {
                            match peer_thread.message {
                                PeerMessage::Version(version) => {
                                    node_map.set_offset(peer_thread.nonce, version.timestamp);
                                    node_map.set_services(peer_thread.nonce, version.service_flags);
                                    node_map.set_height(peer_thread.nonce, version.height as u32);
                                    let best = *node_map.best_height().unwrap_or(&0);
                                    let response = self.handle_version(version, best).await;
                                    if self.need_peers().await? {
                                        node_map.send_message(peer_thread.nonce, MainThreadMessage::GetAddr).await;
                                    }
                                    node_map.send_message(peer_thread.nonce, response).await;
                                    self.dialog.send_dialog(format!("[Peer {}]: version", peer_thread.nonce))
                                        .await;
                                }
                                PeerMessage::Addr(addresses) => self.handle_new_addrs(addresses).await,
                                PeerMessage::Headers(headers) => {
                                    self.dialog.send_dialog(format!("[Peer {}]: headers", peer_thread.nonce))
                                        .await;
                                    match self.handle_headers(headers).await {
                                        Some(response) => {
                                            node_map.send_message(peer_thread.nonce, response).await;
                                        }
                                        None => continue,
                                    }
                                }
                                PeerMessage::FilterHeaders(cf_headers) => {
                                    self.dialog.send_dialog(format!("[Peer {}]: filter headers", peer_thread.nonce)).await;
                                    match self.handle_cf_headers(peer_thread.nonce, cf_headers).await {
                                        Some(response) => {
                                            // match depending on disconnect
                                            node_map.broadcast(response).await;
                                        }
                                        None => continue,
                                    }
                                }
                                PeerMessage::Filter(filter) => {
                                    match self.handle_filter(peer_thread.nonce, filter).await {
                                        Some(response) => {
                                            node_map.send_message(peer_thread.nonce, response).await;
                                        }
                                        None => continue,
                                    }
                                }
                                PeerMessage::Block(block) => match self.handle_block(block).await {
                                    Some(response) => {
                                        node_map.broadcast(response).await;
                                    }
                                    None => continue,
                                },
                                PeerMessage::NewBlocks(blocks) => {
                                    self.dialog.send_dialog(format!("[Peer {}]: inv", peer_thread.nonce))
                                        .await;
                                    for block in blocks {
                                        node_map.add_one_height(peer_thread.nonce);
                                        self.dialog.send_dialog(format!("New block: {}", block))
                                            .await;
                                    }
                                    let best = *node_map.best_height().unwrap_or(&0);
                                    match self.handle_inventory_blocks(best).await {
                                        Some(response) => {
                                            node_map.broadcast(response).await;
                                        }
                                        None => continue,
                                    }
                                }
                                PeerMessage::Reject(payload) => {
                                    self.dialog
                                        .send_warning(format!("Peer {} rejected transaction with ID {}", peer_thread.nonce, payload.txid)).await;
                                    self.dialog.send_data(NodeMessage::TxBroadcastFailure(payload)).await;
                                }
                                PeerMessage::Disconnect => {
                                    node_map.clean().await;
                                }
                                _ => continue,
                            }
                        },
                        _ => continue,
                    }
                },
                message = self.client_recv.recv() => {
                    if let Some(message) = message {
                        match message {
                            ClientMessage::Shutdown => return Ok(()),
                            ClientMessage::Broadcast(transaction) => tx_broadcaster.add(transaction),
                            ClientMessage::AddScripts(scripts) =>  self.add_scripts(scripts).await,
                            ClientMessage::Rescan => {
                                if let Some(response) = self.rescan().await {
                                    node_map.broadcast(response).await;
                                }
                            },
                        }
                    }
                }
            }
        }
    }

    async fn advance_state(&mut self) {
        let mut state = self.state.write().await;
        match *state {
            NodeState::Behind => {
                let mut header_chain = self.chain.lock().await;
                if header_chain.is_synced() {
                    header_chain.flush_to_disk().await;
                    self.dialog
                        .send_data(NodeMessage::StateChange(NodeState::HeadersSynced))
                        .await;
                    *state = NodeState::HeadersSynced;
                }
            }
            NodeState::HeadersSynced => {
                let header_chain = self.chain.lock().await;
                if header_chain.is_cf_headers_synced() {
                    self.dialog
                        .send_data(NodeMessage::StateChange(NodeState::FilterHeadersSynced))
                        .await;
                    *state = NodeState::FilterHeadersSynced;
                }
            }
            NodeState::FilterHeadersSynced => {
                let header_chain = self.chain.lock().await;
                if header_chain.is_filters_synced() {
                    self.dialog
                        .send_data(NodeMessage::StateChange(NodeState::FiltersSynced))
                        .await;
                    *state = NodeState::FiltersSynced;
                }
            }
            NodeState::FiltersSynced => {
                let header_chain = self.chain.lock().await;
                if header_chain.block_queue_empty() {
                    *state = NodeState::TransactionsSynced;
                    let update = SyncUpdate::new(
                        HeaderCheckpoint::new(header_chain.height(), header_chain.tip()),
                        header_chain.last_ten(),
                    );
                    self.dialog
                        .send_data(NodeMessage::StateChange(NodeState::TransactionsSynced))
                        .await;
                    let _ = self.dialog.send_data(NodeMessage::Synced(update)).await;
                }
            }
            NodeState::TransactionsSynced => (),
        }
    }

    // When syncing headers we are only interested in one peer to start
    async fn next_required_peers(&self) -> usize {
        let state = self.state.read().await;
        match *state {
            NodeState::Behind => 1,
            _ => self.required_peers,
        }
    }

    // We accepted a handshake with a peer but we may disconnect if they do not support CBF
    async fn handle_version(
        &mut self,
        version_message: RemoteVersion,
        best_height: u32,
    ) -> MainThreadMessage {
        let state = self.state.read().await;
        match *state {
            NodeState::Behind => (),
            _ => {
                if !version_message
                    .service_flags
                    .has(ServiceFlags::COMPACT_FILTERS)
                    || !version_message.service_flags.has(ServiceFlags::NETWORK)
                {
                    self.dialog
                        .send_warning(
                            "Connected peer does not serve compact filters or blocks".into(),
                        )
                        .await;
                    return MainThreadMessage::Disconnect;
                }
            }
        }
        let mut chain = self.chain.lock().await;
        if chain.height().le(&best_height) {
            chain.set_best_known_height(best_height).await;
        }
        // Even if we start the node as caught up in terms of height, we need to check for reorgs. So we can send this unconditionally.
        let next_headers = GetHeaderConfig {
            locators: chain.locators().await,
            stop_hash: None,
        };
        MainThreadMessage::GetHeaders(next_headers)
    }

    async fn handle_new_addrs(&mut self, new_peers: Vec<Address>) {
        self.dialog
            .send_dialog(format!(
                "Adding {} new peers to the peer database",
                new_peers.len()
            ))
            .await;
        let mut lock = self.peer_man.lock().await;
        for addr in new_peers {
            if let Err(e) = lock
                .add_new_peer(
                    addr.socket_addr()
                        .expect("IP should have been screened")
                        .ip(),
                    Some(addr.port),
                    Some(addr.services),
                )
                .await
            {
                self.dialog
                    .send_warning(format!(
                        "Encountered error adding peer to the database: {}",
                        e
                    ))
                    .await;
            }
        }
    }

    // We always send headers to our peers, so our next message depends on our state
    async fn handle_headers(&mut self, headers: Vec<Header>) -> Option<MainThreadMessage> {
        let mut chain = self.chain.lock().await;
        if let Err(e) = chain.sync_chain(headers).await {
            match e {
                HeaderSyncError::EmptyMessage => {
                    if !chain.is_synced() {
                        return Some(MainThreadMessage::Disconnect);
                    } else if !chain.is_cf_headers_synced() {
                        return Some(MainThreadMessage::GetFilterHeaders(
                            chain.next_cf_header_message().await,
                        ));
                    } else if !chain.is_filters_synced() {
                        return Some(MainThreadMessage::GetFilters(
                            chain.next_filter_message().await,
                        ));
                    }
                    return None;
                }
                _ => {
                    self.dialog
                        .send_warning(format!("Unexpected header syncing error: {}", e))
                        .await;
                    return Some(MainThreadMessage::Disconnect);
                }
            }
        }
        if !chain.is_synced() {
            let next_headers = GetHeaderConfig {
                locators: chain.locators().await,
                stop_hash: None,
            };
            return Some(MainThreadMessage::GetHeaders(next_headers));
        } else if !chain.is_cf_headers_synced() {
            return Some(MainThreadMessage::GetFilterHeaders(
                chain.next_cf_header_message().await,
            ));
        } else if !chain.is_filters_synced() {
            return Some(MainThreadMessage::GetFilters(
                chain.next_filter_message().await,
            ));
        }
        None
    }

    // Compact filter headers may result in a number of outcomes, including the need to audit filters.
    async fn handle_cf_headers(
        &mut self,
        peer_id: u32,
        cf_headers: CFHeaders,
    ) -> Option<MainThreadMessage> {
        let mut chain = self.chain.lock().await;
        match chain.sync_cf_headers(peer_id, cf_headers).await {
            Ok(potential_message) => match potential_message {
                AppendAttempt::AddedToQueue => None,
                AppendAttempt::Extended => {
                    // We added a batch to the queue and still are not at the required height
                    if !chain.is_cf_headers_synced() {
                        Some(MainThreadMessage::GetFilterHeaders(
                            chain.next_cf_header_message().await,
                        ))
                    } else if !chain.is_filters_synced() {
                        // The header chain and filter header chain are in sync, but we need filters still
                        Some(MainThreadMessage::GetFilters(
                            chain.next_filter_message().await,
                        ))
                    } else {
                        // Should be unreachable if we just added filter headers
                        None
                    }
                }
                AppendAttempt::Conflict(_) => {
                    // TODO: Request the filter and block from the peer
                    self.dialog
                        .send_warning(
                            "Found a conflict while peers are sending filter headers".into(),
                        )
                        .await;
                    Some(MainThreadMessage::Disconnect)
                }
            },
            Err(e) => {
                self.dialog
                    .send_warning(format!(
                        "Compact filter header syncing encountered an error: {}",
                        e
                    ))
                    .await;
                Some(MainThreadMessage::Disconnect)
            }
        }
    }

    async fn handle_filter(&mut self, _peer_id: u32, filter: CFilter) -> Option<MainThreadMessage> {
        let mut chain = self.chain.lock().await;
        match chain.sync_filter(filter).await {
            Ok(potential_message) => potential_message.map(MainThreadMessage::GetFilters),
            Err(e) => {
                self.dialog
                    .send_warning(format!(
                        "Compact filter syncing encountered an error: {}",
                        e
                    ))
                    .await;
                Some(MainThreadMessage::Disconnect)
            }
        }
    }

    async fn handle_block(&mut self, block: Block) -> Option<MainThreadMessage> {
        let mut chain = self.chain.lock().await;
        if let Err(e) = chain.scan_block(&block).await {
            self.dialog
                .send_warning(format!("Unexpected block scanning error: {}", e))
                .await;
            return Some(MainThreadMessage::Disconnect);
        }
        None
    }

    // The block queue holds all the block hashes we may be interested in
    async fn pop_block_queue(&mut self) -> Option<MainThreadMessage> {
        let state = self.state.read().await;
        let mut chain = self.chain.lock().await;
        // Do we actually need to wait for the headers to sync?
        match *state {
            NodeState::FiltersSynced => {
                let next_block_hash = chain.next_block();
                match next_block_hash {
                    Some(block_hash) => {
                        self.dialog
                            .send_dialog(format!("Next block in queue: {}", block_hash))
                            .await;
                        Some(MainThreadMessage::GetBlock(GetBlockConfig {
                            locator: block_hash,
                        }))
                    }
                    None => None,
                }
            }
            _ => None,
        }
    }

    // If new inventory came in, we need to download the headers and update the node state
    async fn handle_inventory_blocks(&mut self, new_height: u32) -> Option<MainThreadMessage> {
        let mut state = self.state.write().await;
        match *state {
            NodeState::Behind => None,
            _ => {
                self.dialog
                    .send_data(NodeMessage::StateChange(NodeState::Behind))
                    .await;
                *state = NodeState::Behind;
                let mut chain = self.chain.lock().await;
                let next_headers = GetHeaderConfig {
                    locators: chain.locators().await,
                    stop_hash: None,
                };
                chain.clear_compact_filter_queue();
                if chain.height().le(&new_height) {
                    chain.set_best_known_height(new_height).await;
                }
                Some(MainThreadMessage::GetHeaders(next_headers))
            }
        }
    }

    // Add more scripts to the chain to look for. Does not imply a rescan.
    async fn add_scripts(&mut self, scripts: HashSet<ScriptBuf>) {
        let mut chain = self.chain.lock().await;
        chain.put_scripts(scripts);
    }

    // Clear the filter hash cache and redownload the filters.
    async fn rescan(&mut self) -> Option<MainThreadMessage> {
        let mut state = self.state.write().await;
        let mut chain = self.chain.lock().await;
        match *state {
            NodeState::Behind => None,
            NodeState::HeadersSynced => None,
            _ => {
                chain.clear_filters().await;
                *state = NodeState::FilterHeadersSynced;
                Some(MainThreadMessage::GetFilters(
                    chain.next_filter_message().await,
                ))
            }
        }
    }

    // When the application starts, fetch any headers we know about from the database.
    async fn fetch_headers(&mut self) -> Result<(), NodeError> {
        self.dialog
            .send_dialog("Attempting to load headers from the database.".into())
            .await;
        let mut chain = self.chain.lock().await;
        chain
            .load_headers()
            .await
            .map_err(NodeError::HeaderDatabase)
    }

    // First we search the whitelist for peers that we trust. If we don't have any more whitelisted peers,
    // we try to get a new peer from the peer manager. If that fails and our database is empty, we try DNS.
    // Otherwise, the node throws an error.
    async fn next_peer(&mut self) -> Result<(IpAddr, u16), NodeError> {
        if let Some(whitelist) = &mut self.white_list {
            if let Some((ip, port)) = whitelist.pop() {
                return {
                    self.dialog
                        .send_dialog(format!("Using a peer from the white list: {}", ip))
                        .await;
                    Ok((ip, port))
                };
            }
        }
        let mut peer_manager = self.peer_man.lock().await;
        match peer_manager.next_peer().await {
            Ok((ip, port)) => {
                self.dialog
                    .send_dialog(format!("Found an existing peer in the database: {}", ip))
                    .await;
                Ok((ip, port))
            }
            Err(_) => {
                let current_count = peer_manager
                    .peer_count()
                    .await
                    .map_err(NodeError::PeerDatabase)?;
                if current_count < 1 {
                    self.dialog
                        .send_warning("There are no peers in the database".into())
                        .await;
                    #[cfg(feature = "dns")]
                    self.dialog
                        .send_dialog("Using DNS to find new peers".into())
                        .await;
                    #[cfg(feature = "dns")]
                    peer_manager
                        .bootstrap()
                        .await
                        .map_err(NodeError::PeerDatabase)?;

                    let next_peer = peer_manager
                        .next_peer()
                        .await
                        .map_err(NodeError::PeerDatabase)?;
                    return Ok((next_peer.0, next_peer.1));
                }
                self.dialog
                    .send_warning("An error occured while finding a new peer".into())
                    .await;
                Err(NodeError::PeerDatabase(PeerManagerError::Database(
                    DatabaseError::Load,
                )))
            }
        }
    }

    async fn need_peers(&mut self) -> Result<bool, NodeError> {
        self.peer_man
            .lock()
            .await
            .need_peers()
            .await
            .map_err(NodeError::PeerDatabase)
    }
}
