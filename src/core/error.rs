use std::fmt::{Debug, Display};

use crate::{
    chain::error::HeaderPersistenceError, db::error::PeerManagerError, impl_sourceless_error,
};

use super::messages::FailurePayload;

/// Errors that prevent the node from running.
#[derive(Debug)]
pub enum NodeError<H: Debug + Display, P: Debug + Display> {
    /// The persistence layer experienced a critical error.
    HeaderDatabase(HeaderPersistenceError<H>),
    /// The persistence layer experienced a critical error.
    PeerDatabase(PeerManagerError<P>),
}

impl<H: Debug + Display, P: Debug + Display> core::fmt::Display for NodeError<H, P> {
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

impl<H: Debug + Display, P: Debug + Display> std::error::Error for NodeError<H, P> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

impl<H: Debug + Display, P: Debug + Display> From<HeaderPersistenceError<H>> for NodeError<H, P> {
    fn from(value: HeaderPersistenceError<H>) -> Self {
        NodeError::HeaderDatabase(value)
    }
}

impl<H: Debug + Display, P: Debug + Display> From<PeerManagerError<P>> for NodeError<H, P> {
    fn from(value: PeerManagerError<P>) -> Self {
        NodeError::PeerDatabase(value)
    }
}

/// Errors occuring when the client is talking to the node.
#[derive(Debug)]
pub enum ClientError {
    /// The channel to the node was likely closed and dropped from memory.
    SendError,
    /// The transaction was not broadcast to any peers.
    BroadcastFailure(FailurePayload),
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
