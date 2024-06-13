use std::collections::HashSet;

use bitcoin::ScriptBuf;

use crate::{
    chain::checkpoints::HeaderCheckpoint, DisconnectedHeader, IndexedBlock, IndexedTransaction,
    TxBroadcast,
};

/// Messages receivable by a running node.
#[derive(Debug, Clone)]
pub enum NodeMessage {
    /// A human readable dialog of what the node is currently doing
    Dialog(String),
    /// A human readable warning that may effect the function of the node
    Warning(String),
    /// A relevant transaction based on the user provided scripts
    Transaction(IndexedTransaction),
    /// A relevant [`crate::Block`] based on the user provided scripts
    Block(IndexedBlock),
    /// The node is fully synced, having scanned the requested range
    Synced(HeaderCheckpoint),
    /// Blocks were reorganized out of the chain
    BlocksDisconnected(Vec<DisconnectedHeader>),
    /// A problem occured sending a transaction.
    TxBroadcastFailure,
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
