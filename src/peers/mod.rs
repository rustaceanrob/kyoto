pub(crate) mod counter;
#[cfg(feature = "dns")]
pub(crate) mod dns;
pub(crate) mod outbound_messages;
pub(crate) mod peer;
pub(crate) mod reader;
pub(crate) mod traits;

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use crate::prelude::Netgroup;

    #[test]
    fn test_sixteen() {
        let mut peer = IpAddr::V4(Ipv4Addr::new(95, 217, 198, 121));
        assert_eq!("95.217".to_string(), peer.netgroup());
    }
}
