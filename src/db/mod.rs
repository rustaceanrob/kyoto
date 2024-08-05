use core::future::Future;
use core::pin::Pin;

use bitcoin::p2p::address::AddrV2;
use bitcoin::p2p::ServiceFlags;

/// Errors a database backend may produce.
pub mod error;
/// In-memory persistence trait implementations for light-weight nodes running on constrained or semi-trusted setups.
pub mod memory;
/// Persistence traits defined with SQL Lite to store data between sessions.
#[cfg(feature = "database")]
pub mod sqlite;
/// Traits that define the header and peer databases.
pub mod traits;

pub(crate) type FutureResult<'a, T, E> = Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'a>>;

/// A peer that will be saved to the [`traits::PeerStore`].
#[derive(Debug, Clone, PartialEq)]
pub struct PersistedPeer {
    /// Canonical IP address of this peer.
    pub addr: AddrV2,
    /// The port believed to be listening for connections.
    pub port: u16,
    /// The services this peer may offer.
    pub services: ServiceFlags,
    /// A new, tried, or banned status.
    pub status: PeerStatus,
}

impl PersistedPeer {
    pub fn new(addr: AddrV2, port: u16, services: ServiceFlags, status: PeerStatus) -> Self {
        Self {
            addr,
            port,
            services,
            status,
        }
    }
}

impl From<PersistedPeer> for (AddrV2, u16) {
    fn from(value: PersistedPeer) -> Self {
        (value.addr, value.port)
    }
}

/// The status of a peer in the database.
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum PeerStatus {
    /// A newly found peer from DNS or the peer-to-peer network.
    New,
    /// The node successfully connected to this peer.
    Tried,
    /// A connected peer responded with faulty or malicious behavior.
    Ban,
}
