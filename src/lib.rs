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

use std::net::IpAddr;

pub use bitcoin::block::Header;
pub use bitcoin::p2p::message_network::RejectReason;
pub use bitcoin::{Address, Block, BlockHash, Network, ScriptBuf, Transaction, Txid};
/// Build a light client and node.
pub use node::builder::NodeBuilder;
/// A structured way to send messages to a running node.
pub use node::client::{Client, ClientSender};
/// A Bitcoin light client according to BIP 157/158.
pub use node::node::Node;
pub use tokio::sync::broadcast::Receiver;

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

/// A peer on the Bitcoin P2P network
#[derive(Debug, Clone)]
pub struct TrustedPeer {
    /// The IP address of the remote node to connect to.
    pub ip: IpAddr,
    /// The port to establish a TCP connection. If none is provided, the typical Bitcoin Core port is used as the default.
    pub port: Option<u16>,
}

impl TrustedPeer {
    /// Create a new trusted peer.
    pub fn new(ip_addr: IpAddr, port: Option<u16>) -> Self {
        Self { ip: ip_addr, port }
    }

    /// Create a new trusted peer using the default port for the network.
    pub fn from_ip(ip_addr: IpAddr) -> Self {
        Self {
            ip: ip_addr,
            port: None,
        }
    }

    /// The IP address of the trusted peer.
    pub fn ip(&self) -> IpAddr {
        self.ip
    }

    /// A recommended port to connect to, if there is one.
    pub fn port(&self) -> Option<u16> {
        self.port
    }
}

impl From<(IpAddr, Option<u16>)> for TrustedPeer {
    fn from(value: (IpAddr, Option<u16>)) -> Self {
        TrustedPeer::new(value.0, value.1)
    }
}

impl From<TrustedPeer> for (IpAddr, Option<u16>) {
    fn from(value: TrustedPeer) -> Self {
        (value.ip(), value.port())
    }
}

/// How to connect to peers on the peer-to-peer network
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum ConnectionType {
    /// Version one peer-to-peer connections
    ClearNet,
}

impl Default for ConnectionType {
    fn default() -> Self {
        ConnectionType::ClearNet
    }
}
