use thiserror::Error;

use crate::{chain::error::HeaderPersistenceError, db::error::PeerManagerError};

use super::messages::RejectPayload;

/// Errors that prevent the node from running.
#[derive(Error, Debug)]
pub enum NodeError {
    /// The persistence layer experienced a critical error.
    #[error("The header database encountered an error and cannot recover.")]
    HeaderDatabase(HeaderPersistenceError),
    /// The persistence layer experienced a critical error.
    #[error("The peer database encountered an error and cannot recover.")]
    PeerDatabase(PeerManagerError),
}

/// Errors occuring when the client is talking to the node.
#[derive(Error, Debug)]
pub enum ClientError {
    /// The channel to the node was likely closed and dropped from memory.
    #[error("The receiver of this message was dropped from memory.")]
    SendError,
    /// The transaction was not broadcast to any peers.
    #[error("The transaction was not broadcast to any peers.")]
    BroadcastFailure(RejectPayload),
}
