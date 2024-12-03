use bitcoin::{
    consensus::Decodable,
    io::Read,
    p2p::{message::CommandString, Magic},
};

pub(crate) mod counter;
#[cfg(feature = "dns")]
pub(crate) mod dns;
#[allow(dead_code)]
pub(crate) mod error;
pub(crate) mod outbound_messages;
pub(crate) mod parsers;
pub(crate) mod peer;
#[allow(dead_code)]
pub(crate) mod reader;
#[cfg(feature = "tor")]
pub(crate) mod tor;
pub(crate) mod traits;

pub const PROTOCOL_VERSION: u32 = 70013;
pub const KYOTO_VERSION: &str = "0.6.0";
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
