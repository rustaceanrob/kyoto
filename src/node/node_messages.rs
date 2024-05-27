pub use bitcoin::{Block, Transaction};

use crate::tx::types::IndexedTransaction;

/// Messages receivable by a running node
#[derive(Debug)]
pub enum NodeMessage {
    /// A human readable dialog
    Dialog(String),
    /// A human readable warning that may effect the function of the node
    Warning(String),
    /// A relevant transaction based on the user provided scripts
    Transaction(IndexedTransaction),
    /// A relevant [`Block`] based on the user provided scripts
    Block(Block),
    /// The node is fully synced, having scanned the requested range
    Synced,
}

/// Commands to issue a node
#[derive(Debug)]
pub enum ClientMessage {
    /// Stop the node
    Shutdown,
    /// Broadcast a [`Transaction`]
    Broadcast(Transaction),
}
