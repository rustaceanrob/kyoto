use std::collections::{BTreeMap, HashSet};

use bitcoin::{block::Header, p2p::message_network::RejectReason, ScriptBuf, Txid};

use crate::{
    chain::checkpoints::HeaderCheckpoint, DisconnectedHeader, IndexedBlock, IndexedTransaction,
    TxBroadcast,
};

use super::node::NodeState;

/// Messages receivable by a running node.
#[derive(Debug, Clone)]
pub enum NodeMessage {
    /// Human readable dialog of what the node is currently doing
    Dialog(String),
    /// A warning that may effect the function of the node
    Warning(Warning),
    /// The current state of the node in the syncing process
    StateChange(NodeState),
    /// A relevant transaction based on the user provided scripts
    Transaction(IndexedTransaction),
    /// A relevant [`crate::Block`] based on the user provided scripts
    Block(IndexedBlock),
    /// The node is fully synced, having scanned the requested range
    Synced(SyncUpdate),
    /// Blocks were reorganized out of the chain
    BlocksDisconnected(Vec<DisconnectedHeader>),
    /// A transaction was sent to one or more connected peers.
    /// This does not guarentee the transaction will be relayed or accepted by the peers,
    /// only that the message was sent over the wire.
    TxSent(Txid),
    /// A problem occured sending a transaction.
    TxBroadcastFailure(RejectPayload),
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

/// An attempt to broadcast a tranasction failed.
#[derive(Debug, Clone, Copy)]
pub struct RejectPayload {
    /// An enumeration of the reason for the transaction failure.
    pub reason: RejectReason,
    /// The transaction that was rejected.
    pub txid: Txid,
}

/// Commands to issue a node.
#[derive(Debug, Clone)]
pub enum ClientMessage {
    /// Stop the node.
    Shutdown,
    /// Broadcast a [`crate::Transaction`] with a [`crate::TxBroadcastPolicy`].
    Broadcast(TxBroadcast),
    /// Add more Bitcoin [`ScriptBuf`] to look for.
    AddScripts(HashSet<ScriptBuf>),
    /// Starting at the configured anchor checkpoint, look for block inclusions with newly added scripts.
    Rescan,
}

/// Warnings a node may issue while running.
#[derive(Debug, Clone)]
pub enum Warning {
    /// The node is looking for connections to peers.
    NotEnoughConnections,
    /// A connection to a peer timed out.
    PeerTimedOut,
    /// A peer sent us a peer-to-peer message the node did not request.
    UnsolicitedMessage,
    /// The provided anchor is deeper than the database history.
    UnlinkableAnchor,
    /// The headers in the database do not link together.
    CorruptedHeaders,
    /// A transaction got rejected.
    TransactionRejected,
    /// A database failed to persist some data.
    FailedPersistance(String),
    /// The peer sent us a potential fork
    EvaluatingFork,
    /// The peer database has no values.
    EmptyPeerDatabase,
    /// An unexpected error occured processing a peer-to-peer message.
    UnexpectedSyncError(String),
}

impl std::fmt::Display for Warning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Warning::NotEnoughConnections => {
                write!(f, "The node is looking for connections to peers.")
            }
            Warning::UnlinkableAnchor => write!(
                f,
                "The provided anchor is deeper than the database history."
            ),
            Warning::TransactionRejected => write!(f, "A transaction got rejected."),
            Warning::FailedPersistance(s) => {
                write!(f, "A database failed to persist some data: {}", s)
            }
            Warning::EvaluatingFork => write!(f, "The peer sent us a potential fork."),
            Warning::EmptyPeerDatabase => write!(f, "The peer database has no values."),
            Warning::UnexpectedSyncError(s) => write!(f, "Error handling a P2P message: {}", s),
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
        }
    }
}
