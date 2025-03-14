use bitcoin::{
    consensus::Decodable,
    io::Read,
    p2p::{message::CommandString, Magic},
};
use std::time::Duration;

pub(crate) mod counter;
pub(crate) mod dns;
#[allow(dead_code)]
pub(crate) mod error;
pub(crate) mod outbound_messages;
pub(crate) mod parsers;
pub(crate) mod peer;
pub(crate) mod peer_map;
#[allow(dead_code)]
pub(crate) mod reader;
#[cfg(feature = "tor")]
pub(crate) mod tor;
pub(crate) mod traits;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PeerId(pub(crate) u32);

impl PeerId {
    pub(crate) fn increment(&mut self) {
        self.0 = self.0.wrapping_add(1)
    }
}

impl From<u32> for PeerId {
    fn from(value: u32) -> Self {
        PeerId(value)
    }
}

impl std::fmt::Display for PeerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Peer {}", self.0)
    }
}

/// Configuration for peer connection timeouts
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct PeerTimeoutConfig {
    /// How long to wait for a peer to respond to a request
    pub(crate) response_timeout: Duration,
    /// Maximum time to maintain a connection with a peer
    pub(crate) max_connection_time: Duration,
}

impl PeerTimeoutConfig {
    /// Create a new peer timeout configuration
    pub fn new(response_timeout: Duration, max_connection_time: Duration) -> Self {
        Self {
            response_timeout,
            max_connection_time,
        }
    }
}

pub const PROTOCOL_VERSION: u32 = 70016;
pub const KYOTO_VERSION: &str = "0.8.0";
pub const RUST_BITCOIN_VERSION: &str = "0.32.4";

pub(crate) struct V1Header {
    magic: Magic,
    _command: CommandString,
    length: u32,
    _checksum: u32,
}

impl Decodable for V1Header {
    fn consensus_decode<R: Read + ?Sized>(
        reader: &mut R,
    ) -> Result<Self, bitcoin::consensus::encode::Error> {
        let magic = Magic::consensus_decode(reader)?;
        let _command = CommandString::consensus_decode(reader)?;
        let length = u32::consensus_decode(reader)?;
        let _checksum = u32::consensus_decode(reader)?;
        Ok(Self {
            magic,
            _command,
            length,
            _checksum,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use bitcoin::p2p::address::AddrV2;

    use crate::prelude::Netgroup;

    #[test]
    fn test_sixteen() {
        let peer = AddrV2::Ipv4(Ipv4Addr::new(95, 217, 198, 121));
        assert_eq!("95.217".to_string(), peer.netgroup());
    }
}
