use std::fmt::{Debug, Display};

use crate::impl_sourceless_error;

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
            NodeError::HeaderDatabase(e) => write!(f, "block headers: {e}"),
            NodeError::PeerDatabase(e) => write!(f, "peer manager: {e}"),
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

/// Errors when managing persisted peers.
#[derive(Debug)]
pub enum PeerManagerError<P: Debug + Display> {
    /// DNS failed to respond.
    Dns,
    /// Reading or writing from the database failed.
    Database(P),
}

impl<P: Debug + Display> core::fmt::Display for PeerManagerError<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PeerManagerError::Dns => write!(f, "DNS servers failed to respond."),
            PeerManagerError::Database(e) => {
                write!(f, "database: {e}")
            }
        }
    }
}

impl<P: Debug + Display> std::error::Error for PeerManagerError<P> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

impl<P: Debug + Display> From<P> for PeerManagerError<P> {
    fn from(value: P) -> Self {
        PeerManagerError::Database(value)
    }
}

/// Errors with the block header representation that prevent the node from operating.
#[derive(Debug)]
pub enum HeaderPersistenceError<H: Debug + Display> {
    /// The block headers do not point to each other in a list.
    HeadersDoNotLink,
    /// Some predefined checkpoint does not match.
    MismatchedCheckpoints,
    /// A user tried to retrieve headers too far in the past for what is in their database.
    CannotLocateHistory,
    /// A database error.
    Database(H),
}

impl<H: Debug + Display> core::fmt::Display for HeaderPersistenceError<H> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HeaderPersistenceError::HeadersDoNotLink => write!(f, "the headers loaded from persistence do not link together."),
            HeaderPersistenceError::MismatchedCheckpoints => write!(f, "the headers loaded do not match a known checkpoint."),
            HeaderPersistenceError::CannotLocateHistory => write!(f, "the configured anchor checkpoint is too far in the past compared to previous syncs. The database cannot reconstruct the chain."),
            HeaderPersistenceError::Database(e) => write!(f, "database: {e}"),
        }
    }
}

impl<H: Debug + Display> std::error::Error for HeaderPersistenceError<H> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
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
