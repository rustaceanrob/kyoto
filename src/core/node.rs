use std::{
    collections::HashSet,
    ops::DerefMut,
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

use bitcoin::{
    block::Header,
    p2p::{
        message_filter::{CFHeaders, CFilter},
        message_network::VersionMessage,
        ServiceFlags,
    },
    Block, BlockHash, Network, ScriptBuf,
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
    core::{error::FetchHeaderError, peer_map::PeerMap},
    db::traits::{HeaderStore, PeerStore},
    filters::{cfheader_chain::AppendAttempt, error::CFilterSyncError},
    ConnectionType, FailurePayload, PeerStoreSizeConfig, TrustedPeer, TxBroadcastPolicy,
};

use super::{
    broadcaster::Broadcaster,
    channel_messages::{
        CombinedAddr, GetBlockConfig, GetHeaderConfig, MainThreadMessage, PeerMessage,
        PeerThreadMessage,
    },
    client::Client,
    config::NodeConfig,
    dialog::Dialog,
    error::NodeError,
    messages::{ClientMessage, NodeMessage, SyncUpdate, Warning},
    FilterSyncPolicy, LastBlockMonitor, PeerTimeoutConfig,
};

pub(crate) const ADDR_V2_VERSION: u32 = 70015;
const LOOP_TIMEOUT: u64 = 1;

type Whitelist = Vec<TrustedPeer>;
type PeerRequirement = usize;

/// The state of the node with respect to connected peers.
#[derive(Debug, Clone, Copy)]
pub enum NodeState {
    /// We are behind on block headers according to our peers.
    Behind,
    /// We may start downloading compact block filter headers.
    HeadersSynced,
    /// We may start scanning compact block filters.
    FilterHeadersSynced,
    /// We may start asking for blocks with matches.
    FiltersSynced,
    /// We found all known transactions to the wallet.
    TransactionsSynced,
}

/// A compact block filter node. Nodes download Bitcoin block headers, block filters, and blocks to send relevant events to a client.
#[derive(Debug)]
pub struct Node<H: HeaderStore, P: PeerStore> {
    state: Arc<RwLock<NodeState>>,
    chain: Arc<Mutex<Chain<H>>>,
    peer_map: Arc<Mutex<PeerMap<P>>>,
    tx_broadcaster: Arc<Mutex<Broadcaster>>,
    required_peers: PeerRequirement,
    dialog: Dialog,
    client_recv: Arc<Mutex<Receiver<ClientMessage>>>,
    peer_recv: Arc<Mutex<Receiver<PeerThreadMessage>>>,
    is_running: AtomicBool,
    filter_sync_policy: Arc<RwLock<FilterSyncPolicy>>,
}

impl<H: HeaderStore, P: PeerStore> Node<H, P> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        network: Network,
        white_list: Whitelist,
        scripts: HashSet<ScriptBuf>,
        header_checkpoint: Option<HeaderCheckpoint>,
        filter_startpoint: Option<u32>,
        required_peers: PeerRequirement,
        target_peer_size: PeerStoreSizeConfig,
        connection_type: ConnectionType,
        timeout_config: PeerTimeoutConfig,
        filter_sync_policy: FilterSyncPolicy,
        peer_store: P,
        header_store: H,
    ) -> (Self, Client) {
        // Set up a communication channel between the node and client
        let (ntx, _) = broadcast::channel::<NodeMessage>(32);
        let (ctx, crx) = mpsc::channel::<ClientMessage>(5);
        let client = Client::new(ntx.clone(), ctx);
        // A structured way to talk to the client
        let dialog = Dialog::new(ntx);
        // We always assume we are behind
        let state = Arc::new(RwLock::new(NodeState::Behind));
        // Configure the peer manager
        let (mtx, mrx) = mpsc::channel::<PeerThreadMessage>(32);
        let peer_map = Arc::new(Mutex::new(PeerMap::new(
            mtx,
            network,
            peer_store,
            white_list,
            dialog.clone(),
            connection_type,
            target_peer_size,
            timeout_config,
        )));
        // Set up the transaction broadcaster
        let tx_broadcaster = Arc::new(Mutex::new(Broadcaster::new()));
        // Prepare the header checkpoints for the chain source
        let mut checkpoints = HeaderCheckpoints::new(&network);
        let checkpoint = header_checkpoint.unwrap_or_else(|| checkpoints.last());
        checkpoints.prune_up_to(checkpoint);
        // Build the chain
        let chain = Chain::new(
            network,
            scripts,
            checkpoint,
            filter_startpoint,
            checkpoints,
            dialog.clone(),
            header_store,
            required_peers,
        );
        let chain = Arc::new(Mutex::new(chain));
        (
            Self {
                state,
                chain,
                peer_map,
                tx_broadcaster,
                required_peers,
                dialog,
                client_recv: Arc::new(Mutex::new(crx)),
                peer_recv: Arc::new(Mutex::new(mrx)),
                is_running: AtomicBool::new(false),
                filter_sync_policy: Arc::new(RwLock::new(filter_sync_policy)),
            },
            client,
        )
    }

    pub(crate) fn new_from_config(
        config: NodeConfig,
        network: Network,
        peer_store: P,
        header_store: H,
    ) -> (Self, Client) {
        let timeout_config =
            PeerTimeoutConfig::new(config.response_timeout, config.max_connection_time);
        Node::new(
            network,
            config.white_list,
            config.addresses,
            config.header_checkpoint,
            config.filter_startpoint,
            config.required_peers as PeerRequirement,
            config.target_peer_size,
            config.connection_type,
            timeout_config,
            config.filter_sync_policy,
            peer_store,
            header_store,
        )
    }

    /// Has [`Node::run`] been called.
    pub fn is_running(&self) -> bool {
        self.is_running.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Run the node continuously. Typically run on a separate thread than the underlying application.
    ///
    /// # Errors
    ///
    /// A node will cease running if a fatal error is encountered with either the [`PeerStore`] or [`HeaderStore`].
    pub async fn run(&self) -> Result<(), NodeError<H::Error, P::Error>> {
        self.dialog.send_dialog("Starting node").await;
        self.dialog
            .send_dialog(format!(
                "Configured connection requirement: {} peers",
                self.required_peers
            ))
            .await;
        self.is_running
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.fetch_headers().await?;
        let mut last_block = LastBlockMonitor::new();
        let mut peer_recv = self.peer_recv.lock().await;
        let mut client_recv = self.client_recv.lock().await;
        loop {
            // Try to advance the state of the node
            self.advance_state(&last_block).await;
            // Connect to more peers if we need them and remove old connections
            self.dispatch().await?;
            // If there are blocks we need in the queue, we should request them of a random peer
            self.get_blocks().await;
            // If we have a transaction to broadcast and we are connected to peers, we should broadcast them
            self.broadcast_transactions().await;
            // Either handle a message from a remote peer or from our client
            select! {
                peer = tokio::time::timeout(Duration::from_secs(LOOP_TIMEOUT), peer_recv.recv()) => {
                    match peer {
                        Ok(Some(peer_thread)) => {
                            match peer_thread.message {
                                PeerMessage::Version(version) => {
                                    let best = {
                                        let mut peer_map = self.peer_map.lock().await;
                                        peer_map.set_offset(peer_thread.nonce, version.timestamp);
                                        peer_map.set_services(peer_thread.nonce, version.services);
                                        peer_map.set_height(peer_thread.nonce, version.start_height as u32);
                                        *peer_map.best_height().unwrap_or(&0)
                                    };
                                    let response = self.handle_version(peer_thread.nonce, version, best).await?;
                                    self.send_message(peer_thread.nonce, response).await;
                                    self.dialog.send_dialog(format!("[Peer {}]: version", peer_thread.nonce))
                                        .await;
                                }
                                PeerMessage::Addr(addresses) => self.handle_new_addrs(addresses).await,
                                PeerMessage::Headers(headers) => {
                                    last_block.update();
                                    self.dialog.send_dialog(format!("[Peer {}]: headers", peer_thread.nonce))
                                        .await;
                                    match self.handle_headers(peer_thread.nonce, headers).await {
                                        Some(response) => {
                                            self.send_message(peer_thread.nonce, response).await;
                                        }
                                        None => continue,
                                    }
                                }
                                PeerMessage::FilterHeaders(cf_headers) => {
                                    self.dialog.send_dialog(format!("[Peer {}]: filter headers", peer_thread.nonce)).await;
                                    match self.handle_cf_headers(peer_thread.nonce, cf_headers).await {
                                        Some(response) => {
                                            self.broadcast(response).await;
                                        }
                                        None => continue,
                                    }
                                }
                                PeerMessage::Filter(filter) => {
                                    match self.handle_filter(peer_thread.nonce, filter).await {
                                        Some(response) => {
                                            self.send_message(peer_thread.nonce, response).await;
                                        }
                                        None => continue,
                                    }
                                }
                                PeerMessage::Block(block) => match self.handle_block(peer_thread.nonce, block).await {
                                    Some(response) => {
                                        self.send_message(peer_thread.nonce, response).await;
                                    }
                                    None => continue,
                                },
                                PeerMessage::NewBlocks(blocks) => {
                                    self.dialog.send_dialog(format!("[Peer {}]: inv", peer_thread.nonce))
                                        .await;
                                    match self.handle_inventory_blocks(peer_thread.nonce, blocks).await {
                                        Some(response) => {
                                            self.broadcast(response).await;
                                        }
                                        None => continue,
                                    }
                                }
                                PeerMessage::Reject(payload) => {
                                    self.dialog
                                        .send_warning(Warning::TransactionRejected).await;
                                    self.dialog.send_data(NodeMessage::TxBroadcastFailure(payload)).await;
                                }
                                _ => continue,
                            }
                        },
                        _ => continue,
                    }
                },
                message = client_recv.recv() => {
                    if let Some(message) = message {
                        match message {
                            ClientMessage::Shutdown => return Ok(()),
                            ClientMessage::Broadcast(transaction) => self.tx_broadcaster.lock().await.add(transaction),
                            ClientMessage::AddScript(script) =>  self.add_script(script).await,
                            ClientMessage::Rescan => {
                                if let Some(response) = self.rescan().await {
                                    self.broadcast(response).await;
                                }
                            },
                            ClientMessage::ContinueDownload => {
                                if let Some(response) = self.start_filter_download().await {
                                    self.broadcast(response).await
                                }
                            },
                            #[cfg(feature = "filter-control")]
                            ClientMessage::GetBlock(hash) => {
                                let mut state = self.state.write().await;
                                if matches!(*state, NodeState::TransactionsSynced) {
                                    *state = NodeState::FiltersSynced
                                }
                                drop(state);
                                let mut chain = self.chain.lock().await;
                                chain.get_block(hash).await;
                            },
                            ClientMessage::SetDuration(duration) => {
                                let mut peer_map = self.peer_map.lock().await;
                                peer_map.set_duration(duration);
                            },
                            ClientMessage::AddPeer(peer) => {
                                let mut peer_map = self.peer_map.lock().await;
                                peer_map.add_trusted_peer(peer);
                            },
                            ClientMessage::GetHeader(request) => {
                                let mut chain = self.chain.lock().await;
                                let header_opt = chain.fetch_header(request.height).await.map_err(|e| FetchHeaderError::DatabaseOptFailed { error: e.to_string() }).and_then(|opt| opt.ok_or(FetchHeaderError::UnknownHeight));
                                let send_result = request.oneshot.send(header_opt);
                                if send_result.is_err() {
                                    self.dialog.send_warning(Warning::ChannelDropped).await
                                };
                            }
                        }
                    }
                }
            }
        }
    }

    // Send a message to a specified peer
    async fn send_message(&self, nonce: u32, message: MainThreadMessage) {
        let mut peer_map = self.peer_map.lock().await;
        peer_map.send_message(nonce, message).await;
    }

    // Broadcast a messsage to all connected peers
    async fn broadcast(&self, message: MainThreadMessage) {
        let mut peer_map = self.peer_map.lock().await;
        peer_map.broadcast(message).await;
    }

    // Send a message to a random peer
    async fn send_random(&self, message: MainThreadMessage) {
        let mut peer_map = self.peer_map.lock().await;
        peer_map.send_random(message).await;
    }

    // Connect to a new peer if we are not connected to enough
    async fn dispatch(&self) -> Result<(), NodeError<H::Error, P::Error>> {
        let mut peer_map = self.peer_map.lock().await;
        peer_map.clean().await;
        // Find more peers when lower than the desired threshold.
        if peer_map.live() < self.next_required_peers().await {
            self.dialog
                .send_warning(Warning::NotEnoughConnections)
                .await;
            let address = peer_map.next_peer().await?;
            if peer_map.dispatch(address).await.is_err() {
                self.dialog.send_warning(Warning::CouldNotConnect).await;
            }
        }
        Ok(())
    }

    // If there are blocks in the queue, we should request them of a random peer
    async fn get_blocks(&self) {
        if let Some(block_request) = self.pop_block_queue().await {
            self.dialog
                .send_dialog("Sending block request to a random peer")
                .await;
            self.send_random(block_request).await;
        }
    }

    // Broadcast transactions according to the configured policy
    async fn broadcast_transactions(&self) {
        let mut broadcaster = self.tx_broadcaster.lock().await;
        if broadcaster.is_empty() {
            return;
        }
        let mut peer_map = self.peer_map.lock().await;
        if peer_map.live().ge(&self.required_peers) {
            for transaction in broadcaster.queue() {
                let txid = transaction.tx.compute_txid();
                let did_broadcast = match transaction.broadcast_policy {
                    TxBroadcastPolicy::AllPeers => {
                        self.dialog
                            .send_dialog(format!(
                                "Sending transaction to {} connected peers",
                                peer_map.live()
                            ))
                            .await;
                        peer_map
                            .broadcast(MainThreadMessage::BroadcastTx(transaction.tx))
                            .await
                    }
                    TxBroadcastPolicy::RandomPeer => {
                        self.dialog
                            .send_dialog("Sending transaction to a random peer")
                            .await;
                        peer_map
                            .send_random(MainThreadMessage::BroadcastTx(transaction.tx))
                            .await
                    }
                };
                if did_broadcast {
                    self.dialog.send_data(NodeMessage::TxSent(txid)).await;
                } else {
                    self.dialog
                        .send_data(NodeMessage::TxBroadcastFailure(FailurePayload::from_txid(
                            txid,
                        )))
                        .await;
                }
            }
        }
    }

    // Try to continue with the syncing process
    async fn advance_state(&self, last_block: &LastBlockMonitor) {
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
            NodeState::TransactionsSynced => {
                if last_block.stale() {
                    self.dialog.send_warning(Warning::PotentialStaleTip).await;
                    self.dialog
                        .send_dialog("Disconnecting from remote nodes to find new connections")
                        .await;
                    self.broadcast(MainThreadMessage::Disconnect).await;
                }
            }
        }
    }

    // When syncing headers we are only interested in one peer to start
    async fn next_required_peers(&self) -> PeerRequirement {
        let state = self.state.read().await;
        match *state {
            NodeState::Behind => 1,
            _ => self.required_peers,
        }
    }

    // After we receiving some chain-syncing message, we decide what chain of data needs to be
    // requested next.
    async fn next_stateful_message(&self, chain: &mut Chain<H>) -> Option<MainThreadMessage> {
        if !chain.is_synced() {
            let headers = GetHeaderConfig {
                locators: chain.locators().await,
                stop_hash: None,
            };
            return Some(MainThreadMessage::GetHeaders(headers));
        } else if !chain.is_cf_headers_synced() {
            return Some(MainThreadMessage::GetFilterHeaders(
                chain.next_cf_header_message().await,
            ));
        } else if !chain.is_filters_synced() {
            let filter_download = self.filter_sync_policy.read().await;
            if matches!(*filter_download, FilterSyncPolicy::Continue) {
                return Some(MainThreadMessage::GetFilters(
                    chain.next_filter_message().await,
                ));
            }
        }
        None
    }

    // We accepted a handshake with a peer but we may disconnect if they do not support CBF
    async fn handle_version(
        &self,
        nonce: u32,
        version_message: VersionMessage,
        best_height: u32,
    ) -> Result<MainThreadMessage, NodeError<H::Error, P::Error>> {
        let state = self.state.read().await;
        match *state {
            NodeState::Behind => (),
            _ => {
                if !version_message.services.has(ServiceFlags::COMPACT_FILTERS)
                    || !version_message.services.has(ServiceFlags::NETWORK)
                {
                    self.dialog.send_warning(Warning::NoCompactFilters).await;
                    return Ok(MainThreadMessage::Disconnect);
                }
            }
        }
        let mut peer_map = self.peer_map.lock().await;
        peer_map.tried(nonce).await;
        let mut chain = self.chain.lock().await;
        if chain.height().le(&best_height) {
            chain.set_best_known_height(best_height).await;
        }
        let needs_peers = peer_map.need_peers().await?;
        // First we signal for ADDRV2 support
        if version_message.version.gt(&ADDR_V2_VERSION) && needs_peers {
            peer_map
                .send_message(nonce, MainThreadMessage::GetAddrV2)
                .await;
        }
        peer_map
            .send_message(nonce, MainThreadMessage::Verack)
            .await;
        // Now we may request peers if required
        if needs_peers {
            self.dialog.send_dialog("Requesting new addresses").await;
            peer_map
                .send_message(nonce, MainThreadMessage::GetAddr)
                .await;
        }
        // Inform the user we are connected to all required peers
        if peer_map.live().eq(&self.required_peers) {
            self.dialog.send_data(NodeMessage::ConnectionsMet).await
        }
        // Even if we start the node as caught up in terms of height, we need to check for reorgs. So we can send this unconditionally.
        let next_headers = GetHeaderConfig {
            locators: chain.locators().await,
            stop_hash: None,
        };
        Ok(MainThreadMessage::GetHeaders(next_headers))
    }

    // Handle new addresses gossiped over the p2p network
    async fn handle_new_addrs(&self, new_peers: Vec<CombinedAddr>) {
        self.dialog
            .send_dialog(format!(
                "Adding {} new peers to the peer database",
                new_peers.len()
            ))
            .await;
        let mut peer_map = self.peer_map.lock().await;
        peer_map.add_gossiped_peers(new_peers).await;
    }

    // We always send headers to our peers, so our next message depends on our state
    async fn handle_headers(
        &self,
        peer_id: u32,
        headers: Vec<Header>,
    ) -> Option<MainThreadMessage> {
        let mut chain = self.chain.lock().await;
        if let Err(e) = chain.sync_chain(headers).await {
            match e {
                HeaderSyncError::EmptyMessage => {
                    if !chain.is_synced() {
                        return Some(MainThreadMessage::Disconnect);
                    }
                    return self.next_stateful_message(chain.deref_mut()).await;
                }
                HeaderSyncError::LessWorkFork => {
                    self.dialog
                        .send_warning(Warning::UnexpectedSyncError {
                            warning: "A peer sent us a fork with less work.".into(),
                        })
                        .await;
                    return Some(MainThreadMessage::Disconnect);
                }
                _ => {
                    self.dialog
                        .send_warning(Warning::UnexpectedSyncError {
                            warning: format!("Unexpected header syncing error: {}", e),
                        })
                        .await;
                    let mut lock = self.peer_map.lock().await;
                    lock.ban(peer_id).await;
                    return Some(MainThreadMessage::Disconnect);
                }
            }
        }
        self.next_stateful_message(chain.deref_mut()).await
    }

    // Compact filter headers may result in a number of outcomes, including the need to audit filters.
    async fn handle_cf_headers(
        &self,
        peer_id: u32,
        cf_headers: CFHeaders,
    ) -> Option<MainThreadMessage> {
        let mut chain = self.chain.lock().await;
        match chain.sync_cf_headers(peer_id, cf_headers).await {
            Ok(potential_message) => match potential_message {
                AppendAttempt::AddedToQueue => None,
                AppendAttempt::Extended => self.next_stateful_message(chain.deref_mut()).await,
                AppendAttempt::Conflict(_) => {
                    // TODO: Request the filter and block from the peer
                    self.dialog
                        .send_warning(Warning::UnexpectedSyncError {
                            warning: "Found a conflict while peers are sending filter headers"
                                .into(),
                        })
                        .await;
                    Some(MainThreadMessage::Disconnect)
                }
            },
            Err(e) => {
                self.dialog
                    .send_warning(Warning::UnexpectedSyncError {
                        warning: format!(
                            "Compact filter header syncing encountered an error: {}",
                            e
                        ),
                    })
                    .await;
                let mut lock = self.peer_map.lock().await;
                lock.ban(peer_id).await;
                Some(MainThreadMessage::Disconnect)
            }
        }
    }

    // Handle a new compact block filter
    async fn handle_filter(&self, peer_id: u32, filter: CFilter) -> Option<MainThreadMessage> {
        let mut chain = self.chain.lock().await;
        match chain.sync_filter(filter).await {
            Ok(potential_message) => potential_message.map(MainThreadMessage::GetFilters),
            Err(e) => {
                self.dialog
                    .send_warning(Warning::UnexpectedSyncError {
                        warning: format!("Compact filter syncing encountered an error: {}", e),
                    })
                    .await;
                match e {
                    CFilterSyncError::Filter(_) => Some(MainThreadMessage::Disconnect),
                    _ => {
                        let mut lock = self.peer_map.lock().await;
                        lock.ban(peer_id).await;
                        Some(MainThreadMessage::Disconnect)
                    }
                }
            }
        }
    }

    // Scan a block for transactions.
    async fn handle_block(&self, peer_id: u32, block: Block) -> Option<MainThreadMessage> {
        let mut chain = self.chain.lock().await;
        if let Err(e) = chain.check_send_block(block).await {
            self.dialog
                .send_warning(Warning::UnexpectedSyncError {
                    warning: format!("Unexpected block scanning error: {}", e),
                })
                .await;
            let mut lock = self.peer_map.lock().await;
            lock.ban(peer_id).await;
            return Some(MainThreadMessage::Disconnect);
        }
        None
    }

    // The block queue holds all the block hashes we may be interested in
    async fn pop_block_queue(&self) -> Option<MainThreadMessage> {
        let state = self.state.read().await;
        if matches!(
            *state,
            NodeState::FilterHeadersSynced | NodeState::FiltersSynced
        ) {
            let mut chain = self.chain.lock().await;
            let next_block_hash = chain.next_block();
            return match next_block_hash {
                Some(block_hash) => {
                    self.dialog
                        .send_dialog(format!("Next block in queue: {}", block_hash))
                        .await;
                    Some(MainThreadMessage::GetBlock(GetBlockConfig {
                        locator: block_hash,
                    }))
                }
                None => None,
            };
        }
        None
    }

    // If new inventory came in, we need to download the headers and update the node state
    async fn handle_inventory_blocks(
        &self,
        nonce: u32,
        blocks: Vec<BlockHash>,
    ) -> Option<MainThreadMessage> {
        let mut state = self.state.write().await;
        let mut chain = self.chain.lock().await;
        let mut peer_map = self.peer_map.lock().await;
        for block in blocks.iter() {
            peer_map.add_one_height(nonce);
            if !chain.contains_hash(*block) {
                self.dialog
                    .send_dialog(format!("New block: {}", block))
                    .await;
            }
        }
        let best_peer_height = *peer_map.best_height().unwrap_or(&0);
        if chain.height() < best_peer_height {
            chain.set_best_known_height(best_peer_height).await;
        }
        match *state {
            NodeState::Behind => None,
            _ => {
                if blocks.into_iter().any(|block| !chain.contains_hash(block)) {
                    self.dialog
                        .send_data(NodeMessage::StateChange(NodeState::Behind))
                        .await;
                    *state = NodeState::Behind;
                    let next_headers = GetHeaderConfig {
                        locators: chain.locators().await,
                        stop_hash: None,
                    };
                    chain.clear_compact_filter_queue();
                    Some(MainThreadMessage::GetHeaders(next_headers))
                } else {
                    None
                }
            }
        }
    }

    // Add more scripts to the chain to look for. Does not imply a rescan.
    async fn add_script(&self, script: ScriptBuf) {
        let mut chain = self.chain.lock().await;
        chain.put_script(script);
    }

    // Clear the filter hash cache and redownload the filters.
    async fn rescan(&self) -> Option<MainThreadMessage> {
        let mut state = self.state.write().await;
        let mut chain = self.chain.lock().await;
        match *state {
            NodeState::Behind => None,
            NodeState::HeadersSynced => None,
            _ => {
                chain.clear_filters().await;
                self.dialog
                    .send_data(NodeMessage::StateChange(NodeState::FilterHeadersSynced))
                    .await;
                *state = NodeState::FilterHeadersSynced;
                Some(MainThreadMessage::GetFilters(
                    chain.next_filter_message().await,
                ))
            }
        }
    }

    // Continue the filter syncing process by explicit command
    async fn start_filter_download(&self) -> Option<MainThreadMessage> {
        let mut download_policy = self.filter_sync_policy.write().await;
        *download_policy = FilterSyncPolicy::Continue;
        drop(download_policy);
        let current_state = self.state.read().await;
        match *current_state {
            NodeState::Behind => None,
            NodeState::HeadersSynced => None,
            _ => {
                let mut chain = self.chain.lock().await;
                self.next_stateful_message(chain.deref_mut()).await
            }
        }
    }

    // When the application starts, fetch any headers we know about from the database.
    async fn fetch_headers(&self) -> Result<(), NodeError<H::Error, P::Error>> {
        self.dialog
            .send_dialog("Attempting to load headers from the database")
            .await;
        let mut chain = self.chain.lock().await;
        chain
            .load_headers()
            .await
            .map_err(NodeError::HeaderDatabase)
    }
}

impl core::fmt::Display for NodeState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            NodeState::Behind => {
                write!(f, "Requesting block headers.")
            }
            NodeState::HeadersSynced => {
                write!(f, "Requesting compact filter headers.")
            }
            NodeState::FilterHeadersSynced => {
                write!(f, "Requesting compact block filters.")
            }
            NodeState::FiltersSynced => write!(f, "Downloading blocks with relevant transactions."),
            NodeState::TransactionsSynced => write!(f, "Fully synced to the highest block."),
        }
    }
}
