use crate::{
    chain::error::HeaderPersistenceError, db::error::PeerManagerError, impl_sourceless_error,
};

use super::messages::RejectPayload;

/// Errors that prevent the node from running.
#[derive(Debug)]
pub enum NodeError {
    /// The persistence layer experienced a critical error.
    HeaderDatabase(HeaderPersistenceError),
    /// The persistence layer experienced a critical error.
    PeerDatabase(PeerManagerError),
}

impl core::fmt::Display for NodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeError::HeaderDatabase(e) => write!(
                f,
                "the header database encountered an error and cannot recover: {e}"
            ),
            NodeError::PeerDatabase(e) => write!(
                f,
                "the peer database encountered an error and cannot recover: {e}"
            ),
        }
    }
}

impl std::error::Error for NodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            NodeError::HeaderDatabase(e) => Some(e),
            NodeError::PeerDatabase(e) => Some(e),
        }
    }
}

/// Errors occuring when the client is talking to the node.
#[derive(Debug)]
pub enum ClientError {
    /// The channel to the node was likely closed and dropped from memory.
    SendError,
    /// The transaction was not broadcast to any peers.
    BroadcastFailure(RejectPayload),
}

impl core::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::SendError => {
                write!(f, "the receiver of this message was dropped from memory.")
            }
            ClientError::BroadcastFailure(fail) => write!(
                f,
                "the transaction was not broadcast to any peers. REASON CODE: {}",
                fail.txid
            ),
        }
    }
}

impl_sourceless_error!(ClientError);
