use std::net::IpAddr;

pub struct NodeConfig {
    pub required_peers: usize,
    pub white_list: Option<Vec<(IpAddr, u16)>>,
    pub addresses: Vec<bitcoin::Address>,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            required_peers: 1,
            white_list: Default::default(),
            addresses: Default::default(),
        }
    }
}
