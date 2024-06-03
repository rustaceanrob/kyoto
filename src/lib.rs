#![allow(dead_code)]
/// Strucutres related to the blockchain
pub mod chain;
mod db;
mod filters;
/// Tools to build and run a compact block filters node
pub mod node;
mod peers;
mod prelude;
/// Bitcoin transactions and metadata
pub mod tx;

pub use bitcoin::block::Header;
pub use bitcoin::{Block, BlockHash, Transaction};

/// A Bitcoin [`Transaction`] with additional context
#[derive(Debug, Clone)]
pub struct IndexedTransaction {
    /// The Bitcoin transaction
    pub transaction: Transaction,
    /// The height of the block in the chain that includes this transaction
    pub height: u32,
    /// The hash of the block in the chain that includes this transaction
    pub hash: BlockHash,
}

impl IndexedTransaction {
    pub(crate) fn new(transaction: Transaction, height: u32, hash: BlockHash) -> Self {
        Self {
            transaction,
            height,
            hash,
        }
    }
}

/// A block [`Header`] that was disconnected from the chain of most work along with its previous height
#[derive(Debug, Clone, Copy)]
pub struct DisconnectedHeader {
    /// The height where this header used to be in the chain
    height: u32,
    /// The reorganized header
    header: Header,
}

impl DisconnectedHeader {
    pub(crate) fn new(height: u32, header: Header) -> Self {
        Self { height, header }
    }
}

/// A Bitcoin [`Block`] with associated height
#[derive(Debug, Clone)]
pub struct IndexedBlock {
    /// The height or index in the chain
    height: u32,
    /// The Bitcoin block with some matching script
    block: Block,
}

impl IndexedBlock {
    pub(crate) fn new(height: u32, block: Block) -> Self {
        Self { height, block }
    }
}
