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

#[cfg(feature = "tor")]
pub use arti_client::{TorClient, TorClientConfig};
#[cfg(feature = "tor")]
use tor_rtcompat::PreferredRuntime;

pub use bitcoin::block::Header;
pub use bitcoin::p2p::address::AddrV2;
pub use bitcoin::p2p::message_network::RejectReason;
pub use bitcoin::p2p::ServiceFlags;
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
#[derive(Debug, Default, Clone)]
pub enum TxBroadcastPolicy {
    /// Broadcast the transaction to all peers at the same time.
    AllPeers,
    /// Broadcast the transaction to a single random peer, optimal for user privacy.
    #[default]
    RandomPeer,
}

/// A peer on the Bitcoin P2P network
#[derive(Debug, Clone)]
pub struct TrustedPeer {
    /// The IP address of the remote node to connect to.
    pub address: AddrV2,
    /// The port to establish a TCP connection. If none is provided, the typical Bitcoin Core port is used as the default.
    pub port: Option<u16>,
    /// The services this peer is known to offer before starting the node.
    pub known_services: ServiceFlags,
}

impl TrustedPeer {
    /// Create a new trusted peer.
    pub fn new(address: AddrV2, port: Option<u16>, services: ServiceFlags) -> Self {
        Self {
            address,
            port,
            known_services: services,
        }
    }

    /// Create a new peer from a TorV3 service and port.
    #[cfg(feature = "tor")]
    pub fn new_from_tor_v3(
        public_key: [u8; 32],
        port: Option<u16>,
        services: ServiceFlags,
    ) -> Self {
        let address = AddrV2::TorV3(public_key);
        Self {
            address,
            port,
            known_services: services,
        }
    }

    /// Create a new trusted peer using the default port for the network.
    pub fn from_ip(ip_addr: IpAddr) -> Self {
        let address = match ip_addr {
            IpAddr::V4(ip) => AddrV2::Ipv4(ip),
            IpAddr::V6(ip) => AddrV2::Ipv6(ip),
        };
        Self {
            address,
            port: None,
            known_services: ServiceFlags::NONE,
        }
    }

    /// Create a new peer from a TorV3 service.
    #[cfg(feature = "tor")]
    pub fn from_tor_v3(public_key: [u8; 32]) -> Self {
        let address = AddrV2::TorV3(public_key);
        Self {
            address,
            port: None,
            known_services: ServiceFlags::NONE,
        }
    }

    /// The IP address of the trusted peer.
    pub fn address(&self) -> AddrV2 {
        self.address.clone()
    }

    /// A recommended port to connect to, if there is one.
    pub fn port(&self) -> Option<u16> {
        self.port
    }

    /// The services this peer is known to offer.
    pub fn services(&self) -> ServiceFlags {
        self.known_services
    }

    /// Set the known services for this trusted peer.
    pub fn set_services(&mut self, services: ServiceFlags) {
        self.known_services = services;
    }
}

impl From<(IpAddr, Option<u16>)> for TrustedPeer {
    fn from(value: (IpAddr, Option<u16>)) -> Self {
        let address = match value.0 {
            IpAddr::V4(ip) => AddrV2::Ipv4(ip),
            IpAddr::V6(ip) => AddrV2::Ipv6(ip),
        };
        TrustedPeer::new(address, value.1, ServiceFlags::NONE)
    }
}

impl From<TrustedPeer> for (AddrV2, Option<u16>) {
    fn from(value: TrustedPeer) -> Self {
        (value.address(), value.port())
    }
}
/// How to connect to peers on the peer-to-peer network
#[derive(Default, Clone)]
#[non_exhaustive]
pub enum ConnectionType {
    /// Version one peer-to-peer connections
    #[default]
    ClearNet,
    /// Connect to peers over Tor
    #[cfg(feature = "tor")]
    Tor(TorClient<PreferredRuntime>),
}
