//! Kyoto is a limited, private, and small Bitcoin client built in accordance
//! with the [BIP-157](https://github.com/bitcoin/bips/blob/master/bip-0157.mediawiki) and [BIP-158](https://github.com/bitcoin/bips/blob/master/bip-0158.mediawiki)
//! standards. _Limited_, as in Kyoto makes very little assumptions about the underlying resources of the
//! device running the software. _Private_, as in the Bitcoin nodes that serve Kyoto data do not know what transactions the
//! client is querying for, only the entire Bitcoin block. _Small_, as in the dependencies are meant to remain limited
//! and vetted.

#![allow(dead_code)]
/// Strucutres and checkpoints related to the blockchain.
pub mod chain;
/// Traits and structures that define the data persistence required for a node.
pub mod db;
mod filters;
/// Tools to build and run a compact block filters node.
pub mod node;
mod peers;
mod prelude;

pub use bitcoin::block::Header;
pub use bitcoin::p2p::message_network::RejectReason;
pub use bitcoin::{Address, Block, BlockHash, Network, ScriptBuf, Transaction, Txid};

/// A Bitcoin [`Transaction`] with additional context.
#[derive(Debug, Clone)]
pub struct IndexedTransaction {
    /// The Bitcoin transaction.
    pub transaction: Transaction,
    /// The height of the block in the chain that includes this transaction.
    pub height: u32,
    /// The hash of the block.
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

/// A block [`Header`] that was disconnected from the chain of most work along with its previous height.
#[derive(Debug, Clone, Copy)]
pub struct DisconnectedHeader {
    /// The height where this header used to be in the chain.
    pub height: u32,
    /// The reorganized header.
    pub header: Header,
}

impl DisconnectedHeader {
    pub(crate) fn new(height: u32, header: Header) -> Self {
        Self { height, header }
    }
}

/// A Bitcoin [`Block`] with associated height.
#[derive(Debug, Clone)]
pub struct IndexedBlock {
    /// The height or index in the chain.
    pub height: u32,
    /// The Bitcoin block with some matching script.
    pub block: Block,
}

impl IndexedBlock {
    pub(crate) fn new(height: u32, block: Block) -> Self {
        Self { height, block }
    }
}

/// Broadcast a [`Transaction`] to a set of connected peers.
#[derive(Debug, Clone)]
pub struct TxBroadcast {
    /// The presumably valid Bitcoin transaction.
    pub tx: Transaction,
    /// The strategy for how this transaction should be shared with the network
    pub broadcast_policy: TxBroadcastPolicy,
}

impl TxBroadcast {
    pub fn new(tx: Transaction, broadcast_policy: TxBroadcastPolicy) -> Self {
        Self {
            tx,
            broadcast_policy,
        }
    }
}

/// The strategy for how this transaction should be shared with the network
#[derive(Debug, Clone)]
pub enum TxBroadcastPolicy {
    /// Broadcast the transaction to all peers at the same time.
    AllPeers,
    /// Broadcast the transaction to a single random peer, optimal for user privacy.
    RandomPeer,
}
