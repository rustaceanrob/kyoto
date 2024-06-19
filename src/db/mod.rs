use std::net::IpAddr;

use bitcoin::p2p::ServiceFlags;

pub(crate) mod error;
pub(crate) mod peer_man;
pub(crate) mod sqlite;
pub(crate) mod traits;

#[derive(Debug, Clone)]
pub struct PersistedPeer {
    pub addr: IpAddr,
    pub port: u16,
    pub services: ServiceFlags,
    pub tried: bool,
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
