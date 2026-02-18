use bitcoin::{Amount, Transaction, Wtxid};
use bitcoin::{BlockHash, FeeRate};
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::oneshot;

use crate::chain::block_subsidy;
use crate::messages::ClientRequest;
use crate::node::Node;
use crate::{Event, Info, TrustedPeer, Warning};

use super::{error::ClientError, messages::ClientMessage};
use super::{error::FetchBlockError, IndexedBlock};

/// Client state when idle.
pub struct Idle;
/// Client state when active.
pub struct Active;

mod sealed {
    pub trait Sealed {}
}

impl sealed::Sealed for Idle {}
impl sealed::Sealed for Active {}

/// State of the client
pub trait State: sealed::Sealed {}

impl State for Idle {}
impl State for Active {}

/// Wrapper type for the channels that will receive events.
#[derive(Debug)]
pub struct EventListeners {
    /// Receive informational messages from the node.
    pub info_rx: mpsc::Receiver<Info>,
    /// Receive warning messages from a node.
    pub warn_rx: mpsc::UnboundedReceiver<Warning>,
    /// Receive [`Event`] from a node to act on.
    pub event_rx: mpsc::UnboundedReceiver<Event>,
}

impl EventListeners {
    fn new(
        info_rx: mpsc::Receiver<Info>,
        warn_rx: mpsc::UnboundedReceiver<Warning>,
        event_rx: mpsc::UnboundedReceiver<Event>,
    ) -> Self {
        Self {
            info_rx,
            warn_rx,
            event_rx,
        }
    }
}

/// A [`Client`] allows for communication with a running node.
#[derive(Debug)]
pub struct Client<S: State> {
    /// Send events to a node, such as broadcasting a transaction.
    ntx: UnboundedSender<ClientMessage>,
    /// Receive informational messages from the node.
    events: Option<EventListeners>,
    /// Internal node structure.
    node: Option<Node>,
    /// Marker for state.
    _marker: core::marker::PhantomData<S>,
}

impl Client<Idle> {
    pub(crate) fn new(
        info_rx: mpsc::Receiver<Info>,
        warn_rx: mpsc::UnboundedReceiver<Warning>,
        event_rx: mpsc::UnboundedReceiver<Event>,
        ntx: UnboundedSender<ClientMessage>,
        node: Node,
    ) -> Client<Idle> {
        Client {
            ntx,
            events: Some(EventListeners::new(info_rx, warn_rx, event_rx)),
            node: Some(node),
            _marker: core::marker::PhantomData,
        }
    }
    /// Start the underlying node on a [`tokio::task`]. This assumes there is a runtime present to
    /// execute the task.
    pub fn run(mut self) -> (Client<Active>, EventListeners) {
        let events = core::mem::take(&mut self.events).expect("cannot call run twice.");
        let node = core::mem::take(&mut self.node).expect("cannot call run twice.");
        tokio::task::spawn(async move { node.run().await });
        (
            Client {
                ntx: self.ntx,
                events: None,
                node: None,
                _marker: core::marker::PhantomData,
            },
            events,
        )
    }

    /// Run on a detached operating system thread. This method is useful in the case where the
    /// majority of your application code is blocking, and you do not have a
    /// [`tokio::runtime::Runtime`] available. This method will implicitly create a runtime which
    /// runs the data fetching process.
    pub fn run_detached(mut self) -> (Client<Active>, EventListeners) {
        let events = core::mem::take(&mut self.events).expect("cannot call run twice.");
        let node = core::mem::take(&mut self.node).expect("cannot call run twice.");
        std::thread::spawn(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async move {
                    let _ = node.run().await;
                })
        });
        let client = Client {
            ntx: self.ntx,
            events: None,
            node: None,
            _marker: core::marker::PhantomData,
        };
        (client, events)
    }

    /// Run the node with an existing [`tokio::runtime::Runtime`].
    pub fn run_with_runtime(
        mut self,
        rt: impl AsRef<tokio::runtime::Runtime>,
    ) -> (Client<Active>, EventListeners) {
        let rt = rt.as_ref();
        let events = core::mem::take(&mut self.events).expect("cannot call run twice.");
        let node = core::mem::take(&mut self.node).expect("cannot call run twice.");
        rt.spawn(async move { node.run().await });
        let client = Client {
            ntx: self.ntx,
            events: None,
            node: None,
            _marker: core::marker::PhantomData,
        };
        (client, events)
    }
}

impl Client<Active> {
    /// Tell the node to shut down.
    ///
    /// # Errors
    ///
    /// If the node has already stopped running.
    pub fn shutdown(&self) -> Result<(), ClientError> {
        self.ntx
            .send(ClientMessage::Shutdown)
            .map_err(|_| ClientError::SendError)
    }

    /// Broadcast a new transaction to the network, waiting for at least one peer to request it.
    ///
    /// # Note
    ///
    /// When broadcasting a one-parent one-child (TRUC) package,
    /// broadcast the child first, followed by the parent.
    ///
    /// Package relay is under-development at the time of writing.
    ///
    /// For more information, see BIP-431 and BIP-331.
    ///
    /// # Errors
    ///
    /// If the node has stopped running.
    pub async fn broadcast_tx(&self, transaction: Transaction) -> Result<Wtxid, ClientError> {
        let (tx, rx) = tokio::sync::oneshot::channel::<Wtxid>();
        let client_request = ClientRequest::new(transaction, tx);
        self.ntx
            .send(ClientMessage::Broadcast(client_request))
            .map_err(|_| ClientError::SendError)?;
        rx.await.map_err(|_| ClientError::RecvError)
    }

    /// A connection has a minimum transaction fee requirement to enter its mempool. For proper transaction propagation,
    /// transactions should have a fee rate at least as high as the maximum fee filter received.
    /// This method returns the maximum fee rate requirement of all connected peers.
    ///
    /// For more information, refer to BIP133
    ///
    /// # Errors
    ///
    /// If the node has stopped running.
    pub async fn broadcast_min_feerate(&self) -> Result<FeeRate, ClientError> {
        let (tx, rx) = tokio::sync::oneshot::channel::<FeeRate>();
        let request = ClientRequest::new((), tx);
        self.ntx
            .send(ClientMessage::GetBroadcastMinFeeRate(request))
            .map_err(|_| ClientError::SendError)?;
        rx.await.map_err(|_| ClientError::RecvError)
    }

    /// Request a block be fetched. Note that this method will request a block
    /// from a connected peer's inventory, and may take an indefinite amount of
    /// time, until a peer responds.
    ///
    /// # Errors
    ///
    /// If the node has stopped running.
    pub async fn get_block(&self, block_hash: BlockHash) -> Result<IndexedBlock, FetchBlockError> {
        let (tx, rx) = tokio::sync::oneshot::channel::<Result<IndexedBlock, FetchBlockError>>();
        let message = ClientRequest::new(block_hash, tx);
        self.ntx
            .send(ClientMessage::GetBlock(message))
            .map_err(|_| FetchBlockError::SendError)?;
        rx.await.map_err(|_| FetchBlockError::RecvError)?
    }

    /// Request a block be fetched and receive a [`tokio::sync::oneshot::Receiver`]
    /// to await the resulting block.
    ///
    /// # Errors
    ///
    /// If the node has stopped running.
    pub fn request_block(
        &self,
        block_hash: BlockHash,
    ) -> Result<oneshot::Receiver<Result<IndexedBlock, FetchBlockError>>, FetchBlockError> {
        let (tx, rx) = tokio::sync::oneshot::channel::<Result<IndexedBlock, FetchBlockError>>();
        let message = ClientRequest::new(block_hash, tx);
        self.ntx
            .send(ClientMessage::GetBlock(message))
            .map_err(|_| FetchBlockError::SendError)?;
        Ok(rx)
    }

    /// Fetch the average fee rate for the given block hash.
    ///
    /// Computed by taking (`coinbase output amount` - `block subsidy`) / `block weight`. Note that
    /// this value may provide skewed estimates, as averages are more effected by outliers than
    /// medians. For a rudimentary estimation of the fee rate required to enter the next block,
    /// this method may suffice.
    pub async fn average_fee_rate(
        &self,
        block_hash: BlockHash,
    ) -> Result<FeeRate, FetchBlockError> {
        let (tx, rx) = tokio::sync::oneshot::channel::<Result<IndexedBlock, FetchBlockError>>();
        let message = ClientRequest::new(block_hash, tx);
        self.ntx
            .send(ClientMessage::GetBlock(message))
            .map_err(|_| FetchBlockError::SendError)?;
        let indexed_block = rx.await.map_err(|_| FetchBlockError::RecvError)??;
        let subsidy = block_subsidy(indexed_block.height);
        let weight = indexed_block.block.weight();
        let revenue = indexed_block
            .block
            .txdata
            .first()
            .map(|tx| tx.output.iter().map(|txout| txout.value).sum())
            .unwrap_or(Amount::ZERO);
        let block_fees = revenue.checked_sub(subsidy).unwrap_or(Amount::ZERO);
        let fee_rate = block_fees.to_sat() / weight.to_kwu_floor();
        Ok(FeeRate::from_sat_per_kwu(fee_rate))
    }

    /// Starting after the configured checkpoint, re-emit all block filters.
    ///
    /// # Errors
    ///
    /// If the node has stopped running.
    pub fn rescan(&self) -> Result<(), ClientError> {
        self.ntx
            .send(ClientMessage::Rescan)
            .map_err(|_| ClientError::SendError)
    }

    /// Add another known peer to connect to.
    ///
    /// # Errors
    ///
    /// If the node has stopped running.
    pub fn add_peer(&self, peer: impl Into<TrustedPeer>) -> Result<(), ClientError> {
        self.ntx
            .send(ClientMessage::AddPeer(peer.into()))
            .map_err(|_| ClientError::SendError)
    }

    /// Check if the node is running.
    pub fn is_running(&self) -> bool {
        self.ntx.send(ClientMessage::NoOp).is_ok()
    }
}

impl<T> From<mpsc::error::SendError<T>> for ClientError {
    fn from(_: mpsc::error::SendError<T>) -> Self {
        ClientError::SendError
    }
}
