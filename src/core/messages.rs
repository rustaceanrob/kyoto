use std::{collections::BTreeMap, ops::Range, time::Duration};

#[cfg(feature = "filter-control")]
use bitcoin::BlockHash;
use bitcoin::{block::Header, p2p::message_network::RejectReason, FeeRate, ScriptBuf, Txid};

#[cfg(feature = "filter-control")]
use crate::IndexedFilter;
use crate::{
    chain::checkpoints::HeaderCheckpoint, DisconnectedHeader, IndexedBlock, TrustedPeer,
    TxBroadcast,
};

use super::{
    error::{FetchBlockError, FetchHeaderError},
    node::NodeState,
};

/// Informational messages emitted by a node
#[derive(Debug, Clone)]
pub enum Log {
    /// Human readable dialog of what the node is currently doing.
    Dialog(String),
    /// A warning that may effect the function of the node.
    Warning(Warning),
    /// The current state of the node in the syncing process.
    StateChange(NodeState),
    /// The node is connected to all required peers.
    ConnectionsMet,
    /// The progress of the node during the block filter download process.
    Progress(Progress),
    /// A transaction was sent to one or more connected peers.
    /// This does not guarentee the transaction will be relayed or accepted by the peers,
    /// only that the message was sent over the wire.
    TxSent(Txid),
}

impl core::fmt::Display for Log {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Log::Dialog(d) => write!(f, "{}", d),
            Log::Warning(w) => write!(f, "{}", w),
            Log::StateChange(s) => write!(f, "{}", s),
            Log::TxSent(txid) => write!(f, "Transaction sent: {}", txid),
            Log::ConnectionsMet => write!(f, "Required connections met"),
            Log::Progress(p) => {
                let progress_percent = p.percentage_complete();
                write!(f, "Percent complete: {}", progress_percent)
            }
        }
    }
}

/// Data and structures useful for a consumer, such as a wallet.
#[derive(Debug, Clone)]
pub enum Event {
    /// A relevant [`Block`](crate) based on the user provided scripts.
    /// Note that the block may not contain any transactions contained in the script set.
    /// This is due to block filters having a non-zero false-positive rate when compressing data.
    Block(IndexedBlock),
    /// The node is fully synced, having scanned the requested range.
    Synced(SyncUpdate),
    /// Blocks were reorganized out of the chain.
    BlocksDisconnected(Vec<DisconnectedHeader>),
    /// A problem occured sending a transaction. Either the remote node disconnected or the transaction was rejected.
    TxBroadcastFailure(RejectPayload),
    /// A compact block filter with associated height and block hash.
    #[cfg(feature = "filter-control")]
    IndexedFilter(IndexedFilter),
}

/// The node has synced to a new tip of the chain.
#[derive(Debug, Clone)]
pub struct SyncUpdate {
    /// Last known tip of the blockchain
    pub tip: HeaderCheckpoint,
    /// Ten recent headers ending with the tip
    pub recent_history: BTreeMap<u32, Header>,
}

impl SyncUpdate {
    pub(crate) fn new(tip: HeaderCheckpoint, recent_history: BTreeMap<u32, Header>) -> Self {
        Self {
            tip,
            recent_history,
        }
    }

    /// Get the tip of the blockchain after this sync.
    pub fn tip(&self) -> HeaderCheckpoint {
        self.tip
    }

    /// Get the ten most recent blocks in chronological order after this sync.
    /// For nodes that do not save any block header history, it is recommmended to use
    /// a block with significant depth, say 10 blocks deep, as the anchor for the
    /// next sync. This is so the node may gracefully handle block reorganizations,
    /// so long as they occur within 10 blocks of depth. This occurs at more than
    /// a 99% probability.
    pub fn recent_history(&self) -> &BTreeMap<u32, Header> {
        &self.recent_history
    }
}

/// The progress of the node during the block filter download process.

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Progress {
    /// The number of filter headers that have been assumed checked and downloaded.
    pub filter_headers: u32,
    /// The number of block filters that have been assumed checked and downloaded.
    pub filters: u32,
    /// The best known height to the tip of the chain.
    pub tip_height: u32,
}

impl Progress {
    pub(crate) fn new(filter_headers: u32, filters: u32, tip_height: u32) -> Self {
        Self {
            filter_headers,
            filters,
            tip_height,
        }
    }

    /// The total progress represented as a fraction.
    pub fn percentage_complete(&self) -> f32 {
        let total = (2 * self.tip_height) as f32;
        (self.filter_headers + self.filters) as f32 / total
    }
}

/// An attempt to broadcast a tranasction failed.
#[derive(Debug, Clone, Copy)]
pub struct RejectPayload {
    /// An enumeration of the reason for the transaction failure. If none is provided, the message could not be sent over the wire.
    pub reason: Option<RejectReason>,
    /// The transaction that was rejected or failed to broadcast.
    pub txid: Txid,
}

impl RejectPayload {
    pub(crate) fn from_txid(txid: Txid) -> Self {
        Self { reason: None, txid }
    }
}

/// Commands to issue a node.
#[derive(Debug)]
pub(crate) enum ClientMessage {
    /// Stop the node.
    Shutdown,
    /// Broadcast a [`crate::Transaction`] with a [`crate::TxBroadcastPolicy`].
    Broadcast(TxBroadcast),
    /// Add more Bitcoin [`ScriptBuf`] to look for.
    AddScript(ScriptBuf),
    /// Starting at the configured anchor checkpoint, look for block inclusions with newly added scripts.
    Rescan,
    /// If the [`FilterSyncPolicy`](crate) is set to `Halt`, issuing this command will
    /// start the filter download and checking process. Otherwise, this command will not have any effect
    /// on node operation.
    ContinueDownload,
    /// Explicitly request a block from the node.
    #[cfg(feature = "filter-control")]
    GetBlock(BlockRequest),
    /// Set a new connection timeout.
    SetDuration(Duration),
    /// Add another known peer to connect to.
    AddPeer(TrustedPeer),
    /// Request a header from a specified height.
    GetHeader(HeaderRequest),
    /// Request a range of headers.
    GetHeaderBatch(BatchHeaderRequest),
    /// Request the broadcast minimum fee rate.
    GetBroadcastMinFeeRate(FeeRateSender),
    /// Send an empty message to see if the node is running.
    NoOp,
}

type HeaderSender = tokio::sync::oneshot::Sender<Result<Header, FetchHeaderError>>;

#[derive(Debug)]
pub(crate) struct HeaderRequest {
    pub(crate) oneshot: HeaderSender,
    pub(crate) height: u32,
}

impl HeaderRequest {
    pub(crate) fn new(oneshot: HeaderSender, height: u32) -> Self {
        Self { oneshot, height }
    }
}

type BatchHeaderSender =
    tokio::sync::oneshot::Sender<Result<BTreeMap<u32, Header>, FetchHeaderError>>;

#[derive(Debug)]
pub(crate) struct BatchHeaderRequest {
    pub(crate) oneshot: BatchHeaderSender,
    pub(crate) range: Range<u32>,
}

impl BatchHeaderRequest {
    pub(crate) fn new(oneshot: BatchHeaderSender, range: Range<u32>) -> Self {
        Self { oneshot, range }
    }
}

pub(crate) type BlockSender = tokio::sync::oneshot::Sender<Result<IndexedBlock, FetchBlockError>>;

pub(crate) type FeeRateSender = tokio::sync::oneshot::Sender<FeeRate>;

#[cfg(feature = "filter-control")]
#[derive(Debug)]
pub(crate) struct BlockRequest {
    pub(crate) oneshot: BlockSender,
    pub(crate) hash: BlockHash,
}

#[cfg(feature = "filter-control")]
impl BlockRequest {
    pub(crate) fn new(oneshot: BlockSender, hash: BlockHash) -> Self {
        Self { oneshot, hash }
    }
}

/// Warnings a node may issue while running.
#[derive(Debug, Clone)]
pub enum Warning {
    /// The node is looking for connections to peers.
    NotEnoughConnections,
    /// A connection to a peer timed out.
    PeerTimedOut,
    /// The node was unable to connect to a peer in the database.
    CouldNotConnect,
    /// A connection was maintained, but the peer does not signal for compact block filers.
    NoCompactFilters,
    /// The node has been waiting for new `inv` and will find new peers to avoid block withholding.
    PotentialStaleTip,
    /// A peer sent us a peer-to-peer message the node did not request.
    UnsolicitedMessage,
    /// The provided anchor is deeper than the database history.
    /// Recoverable by deleting the headers from the database or starting from a higher point in the chain.
    UnlinkableAnchor,
    /// The headers in the database do not link together. Recoverable by deleting the database.
    CorruptedHeaders,
    /// A transaction got rejected, likely for being an insufficient fee or non-standard transaction.
    TransactionRejected,
    /// A database failed to persist some data.
    FailedPersistance {
        /// Additional context for the persistance failure.
        warning: String,
    },
    /// The peer sent us a potential fork.
    EvaluatingFork,
    /// The peer database has no values.
    EmptyPeerDatabase,
    /// An unexpected error occured processing a peer-to-peer message.
    UnexpectedSyncError {
        /// Additional context as to why block syncing failed.
        warning: String,
    },
    /// A channel that was supposed to receive a message was dropped.
    ChannelDropped,
}

impl core::fmt::Display for Warning {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Warning::NotEnoughConnections => {
                write!(f, "Looking for connections to peers.")
            }
            Warning::UnlinkableAnchor => write!(
                f,
                "The provided anchor is deeper than the database history."
            ),
            Warning::CouldNotConnect => {
                write!(f, "An attempted connection failed or timed out.")
            }
            Warning::NoCompactFilters => {
                write!(f, "A connected peer does not serve compact block filters.")
            }
            Warning::PotentialStaleTip => {
                write!(
                    f,
                    "The node has been running for a long duration without receiving new blocks."
                )
            }
            Warning::TransactionRejected => write!(f, "A transaction got rejected."),
            Warning::FailedPersistance { warning } => {
                write!(f, "A database failed to persist some data: {}", warning)
            }
            Warning::EvaluatingFork => write!(f, "Peer sent us a potential fork."),
            Warning::EmptyPeerDatabase => write!(f, "The peer database has no values."),
            Warning::UnexpectedSyncError { warning } => {
                write!(f, "Error handling a P2P message: {}", warning)
            }
            Warning::CorruptedHeaders => {
                write!(f, "The headers in the database do not link together.")
            }
            Warning::PeerTimedOut => {
                write!(f, "A connection to a peer timed out.")
            }
            Warning::UnsolicitedMessage => {
                write!(
                    f,
                    "A peer sent us a peer-to-peer message the node did not request."
                )
            }
            Warning::ChannelDropped => {
                write!(
                    f,
                    "A channel that was supposed to receive a message was dropped."
                )
            }
        }
    }
}
