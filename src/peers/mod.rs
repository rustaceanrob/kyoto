pub(crate) mod counter;
#[cfg(feature = "dns")]
pub(crate) mod dns;
pub(crate) mod error;
pub(crate) mod outbound_messages;
pub(crate) mod peer;
pub(crate) mod reader;
#[cfg(feature = "tor")]
pub(crate) mod tor;
pub(crate) mod traits;

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
