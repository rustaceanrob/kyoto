use crate::{db::error::DatabaseError, impl_sourceless_error};
use core::fmt::Display;

#[derive(Debug, PartialEq)]
pub(crate) enum HeaderSyncError {
    EmptyMessage,
    HeadersNotConnected,
    InvalidHeaderWork,
    InvalidHeaderTimes,
    PreCheckpointFork,
    InvalidCheckpoint,
    MiscalculatedDifficulty,
    FloatingHeaders,
    LessWorkFork,
    DbError,
}

impl Display for HeaderSyncError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            HeaderSyncError::EmptyMessage => write!(f, "empty headers message."),
            HeaderSyncError::HeadersNotConnected => {
                write!(f, "the headers received do not connect.")
            }
            HeaderSyncError::InvalidHeaderWork => {
                write!(f, "one or more headers does not match its own PoW target.")
            }
            HeaderSyncError::InvalidHeaderTimes => {
                write!(f, "one or more headers does not have a valid block time.")
            }
            HeaderSyncError::PreCheckpointFork => {
                write!(f, "the sync peer sent us a discontinuous chain.")
            }
            HeaderSyncError::InvalidCheckpoint => {
                write!(f, "a checkpoint in the chain did not match.")
            }
            HeaderSyncError::MiscalculatedDifficulty => {
                write!(f, "a computed difficulty adjustment did not match.")
            }
            HeaderSyncError::FloatingHeaders => write!(
                f,
                "the peer sent us a chain that does not connect to any header of ours."
            ),
            HeaderSyncError::LessWorkFork => {
                write!(f, "a peer sent us a fork with less work than our chain.")
            }
            HeaderSyncError::DbError => write!(f, "the database could not load a fork."),
        }
    }
}

impl_sourceless_error!(HeaderSyncError);

/// Errors with the block header representation that prevent the node from operating.
#[derive(Debug)]
pub enum HeaderPersistenceError {
    /// The block headers do not point to each other in a list.
    HeadersDoNotLink,
    /// Some predefined checkpoint does not match.
    MismatchedCheckpoints,
    /// A user tried to retrieve headers too far in the past for what is in their database.
    CannotLocateHistory,
    /// A database error.
    Database(DatabaseError),
}

impl core::fmt::Display for HeaderPersistenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HeaderPersistenceError::HeadersDoNotLink => write!(f, "the headers loaded from persistence do not link together."),
            HeaderPersistenceError::MismatchedCheckpoints => write!(f, "the headers loaded do not match a known checkpoint."),
            HeaderPersistenceError::CannotLocateHistory => write!(f, "the configured anchor checkpoint is too far in the past compared to previous syncs. The database cannot reconstruct the chain."),
            HeaderPersistenceError::Database(e) => write!(f, "the headers could not be loaded from sqlite. {e}"),
        }
    }
}

impl std::error::Error for HeaderPersistenceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            HeaderPersistenceError::Database(e) => Some(e),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub(crate) enum BlockScanError {
    NoBlockHash,
    InvalidMerkleRoot,
}

impl Display for BlockScanError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            BlockScanError::NoBlockHash => {
                write!(f, "the block sent to us does not have a known hash.")
            }
            BlockScanError::InvalidMerkleRoot => {
                write!(f, "the block sent to us does not have a merkle root that matches its header commitment.")
            }
        }
    }
}

impl_sourceless_error!(BlockScanError);
