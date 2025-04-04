use crate::impl_sourceless_error;
use core::fmt::Display;
use std::fmt::Debug;

#[derive(Debug, PartialEq)]
pub(crate) enum HeaderSyncError {
    EmptyMessage,
    HeadersNotConnected,
    InvalidHeaderWork,
    InvalidHeaderTimes,
    InvalidCheckpoint,
    MiscalculatedDifficulty,
    InvalidBits,
    FloatingHeaders,
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
            HeaderSyncError::DbError => write!(f, "the database could not load a fork."),
            HeaderSyncError::InvalidBits => write!(
                f,
                "the target work does not adhere to basic transition requirements."
            ),
        }
    }
}

impl_sourceless_error!(HeaderSyncError);

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
