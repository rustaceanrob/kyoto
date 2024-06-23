use thiserror::Error;

/// Errors that prevent the node from running.
#[derive(Error, Debug)]
pub enum NodeError {
    /// The persistence layer experienced a critical error.
    #[error("persistence failed")]
    LoadError(PersistenceError),
    /// No peers were found in the database and DNS did not respond.
    #[error("dns bootstrap failed")]
    DnsFailure,
}

/// Database errors.
#[derive(Error, Debug)]
pub enum PersistenceError {
    /// There was a problem loading the headers or they were corrupted in some way.
    #[error("there was an error loading the headers from persistence")]
    HeaderLoadError,
    /// There was some problem loading peers from the database.
    #[error("there was an error loading peers from the database")]
    PeerLoadFailure,
}

/// Errors occuring when the client is talking to the node.
#[derive(Error, Debug)]
pub enum ClientError {
    /// The channel to the node was likely closed and dropped from memory.
    #[error("the receiver of this message was dropped from memory")]
    SendError,
    /// The transaction was not broadcast to any peers.
    #[error("the transaction was not broadcast to any peers")]
    BroadcastFailure,
}
