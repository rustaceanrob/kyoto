use std::{collections::HashSet, net::IpAddr, path::PathBuf};

use bitcoin::ScriptBuf;

use crate::chain::checkpoints::HeaderCheckpoint;

pub(crate) struct NodeConfig {
    pub required_peers: u8,
    pub white_list: Option<Vec<(IpAddr, u16)>>,
    pub addresses: HashSet<ScriptBuf>,
    pub data_path: Option<PathBuf>,
    pub header_checkpoint: Option<HeaderCheckpoint>,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            required_peers: 1,
            white_list: Default::default(),
            addresses: Default::default(),
            data_path: Default::default(),
            header_checkpoint: Default::default(),
        }
    }
}
