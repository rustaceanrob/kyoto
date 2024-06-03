pub use bitcoin::{Block, Transaction};

use crate::{
    chain::checkpoints::HeaderCheckpoint, DisconnectedHeader, IndexedBlock, IndexedTransaction,
};

/// Messages receivable by a running node
#[derive(Debug, Clone)]
pub enum NodeMessage {
    /// A human readable dialog
    Dialog(String),
    /// A human readable warning that may effect the function of the node
    Warning(String),
    /// A relevant transaction based on the user provided scripts
    Transaction(IndexedTransaction),
    /// A relevant [`Block`] based on the user provided scripts
    Block(IndexedBlock),
    /// The node is fully synced, having scanned the requested range
    Synced(HeaderCheckpoint),
    /// Blocks were reorganized out of the chain
    BlocksDisconnected(Vec<DisconnectedHeader>),
}

/// Commands to issue a node
#[derive(Debug, Clone)]
pub enum ClientMessage {
    /// Stop the node
    Shutdown,
    /// Broadcast a [`Transaction`]
    Broadcast(Transaction),
}
