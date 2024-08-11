use std::{collections::HashSet, path::PathBuf};

use bitcoin::ScriptBuf;

use crate::{chain::checkpoints::HeaderCheckpoint, ConnectionType, TrustedPeer};

const TARGET_PEER_DB_SIZE: u32 = 256;

pub(crate) struct NodeConfig {
    pub required_peers: u8,
    pub white_list: Option<Vec<TrustedPeer>>,
    pub addresses: HashSet<ScriptBuf>,
    pub data_path: Option<PathBuf>,
    pub header_checkpoint: Option<HeaderCheckpoint>,
    pub connection_type: ConnectionType,
    pub target_peer_size: u32,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            required_peers: 1,
            white_list: Default::default(),
            addresses: Default::default(),
            data_path: Default::default(),
            header_checkpoint: Default::default(),
            connection_type: Default::default(),
            target_peer_size: TARGET_PEER_DB_SIZE,
        }
    }
}
