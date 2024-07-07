use thiserror::Error;

use crate::db::error::DatabaseError;

#[derive(Error, Debug, PartialEq)]
pub(crate) enum HeaderSyncError {
    #[error("Empty headers message.")]
    EmptyMessage,
    #[error("The headers received do not connect.")]
    HeadersNotConnected,
    #[error("One or more headers does not match its own PoW target.")]
    InvalidHeaderWork,
    #[error("One or more headers does not have a valid block time.")]
    InvalidHeaderTimes,
    #[error("The sync peer sent us a discontinuous chain.")]
    PreCheckpointFork,
    #[error("A checkpoint in the chain did not match.")]
    InvalidCheckpoint,
    #[error("A computed difficulty adjustment did not match.")]
    MiscalculatedDifficulty,
    #[error("The peer sent us a chain that does not connect to any header of ours.")]
    FloatingHeaders,
    #[error("A peer sent us a fork with less work than our chain.")]
    LessWorkFork,
    #[error("The database could not load a fork.")]
    DbError,
}

/// Errors with the block header representation that prevent the node from operating.
#[derive(Error, Debug)]
pub enum HeaderPersistenceError {
    /// The block headers do not point to each other in a list.
    #[error("The headers loaded from persistence do not link together.")]
    HeadersDoNotLink,
    /// Some predefined checkpoint does not match.
    #[error("The headers loaded do not match a known checkpoint.")]
    MismatchedCheckpoints,
    /// A user tried to retrieve headers too far in the past for what is in their database.
    #[error("The configured anchor checkpoint is too far in the past compared to previous syncs. The database cannot reconstruct the chain.")]
    CannotLocateHistory,
    /// A database error.
    #[error("The headers could not be loaded from sqlite.")]
    Database(DatabaseError),
}

#[derive(Error, Debug)]
pub(crate) enum BlockScanError {
    #[error("The block sent to us does not have a known hash.")]
    NoBlockHash,
}
