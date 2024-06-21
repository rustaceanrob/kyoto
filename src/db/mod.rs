use std::net::IpAddr;

use bitcoin::p2p::ServiceFlags;

/// Errors a database backend may produce.
pub mod error;
/// In-memory persistence trait implementations for light-weight nodes running on constrained or semi-trusted setups.
pub mod memory;
pub(crate) mod peer_man;
#[cfg(feature = "database")]
pub(crate) mod sqlite;
/// Traits that define the header and peer databases.
pub mod traits;

/// A peer that will be saved to the [`traits::PeerStore`].
#[derive(Debug, Clone, PartialEq)]
pub struct PersistedPeer {
    /// Canonical IP address of this peer.
    pub addr: IpAddr,
    /// The port believed to be listening for connections.
    pub port: u16,
    /// The services this peer may offer.
    pub services: ServiceFlags,
    /// Have we tried this peer before.
    pub tried: bool,
    /// Did we ban this peer for faulty behavior.
    pub banned: bool,
}

impl PersistedPeer {
    pub fn new(addr: IpAddr, port: u16, services: ServiceFlags, tried: bool, banned: bool) -> Self {
        Self {
            addr,
            port,
            services,
            tried,
            banned,
        }
    }
}
