//! Strucutres and checkpoints related to the blockchain.
//!
//! Notably, [`checkpoints`] contains known Bitcoin block hashes and heights with significant work, so Kyoto nodes do not have to sync from genesis.

pub(crate) mod block_queue;
#[allow(clippy::module_inception)]
pub(crate) mod chain;
/// Expected block header checkpoints and corresponding structure.
pub mod checkpoints;
/// Errors associated with the blockchain representation.
#[allow(dead_code)]
pub(crate) mod error;
pub(crate) mod header_batch;
pub(crate) mod header_chain;
