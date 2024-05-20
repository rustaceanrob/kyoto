use std::{net::IpAddr, path::PathBuf};

pub struct NodeConfig {
    pub required_peers: u8,
    pub white_list: Option<Vec<(IpAddr, u16)>>,
    pub addresses: Vec<bitcoin::Address>,
    pub data_path: Option<PathBuf>,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            required_peers: 1,
            white_list: Default::default(),
            addresses: Default::default(),
            data_path: Default::default(),
        }
    }
}
