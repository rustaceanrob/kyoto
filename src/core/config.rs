use std::{collections::HashSet, path::PathBuf, time::Duration};

use bitcoin::ScriptBuf;

use crate::{chain::checkpoints::HeaderCheckpoint, ConnectionType, TrustedPeer};

use super::FilterSyncPolicy;

const TARGET_PEER_DB_SIZE: u32 = 4096;
const REQUIRED_PEERS: u8 = 1;
const TIMEOUT_SECS: u64 = 5;

pub(crate) struct NodeConfig {
    pub required_peers: u8,
    pub white_list: Vec<TrustedPeer>,
    pub addresses: HashSet<ScriptBuf>,
    pub data_path: Option<PathBuf>,
    pub header_checkpoint: Option<HeaderCheckpoint>,
    pub connection_type: ConnectionType,
    pub target_peer_size: u32,
    pub response_timeout: Duration,
    pub filter_sync_policy: FilterSyncPolicy,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            required_peers: REQUIRED_PEERS,
            white_list: Default::default(),
            addresses: Default::default(),
            data_path: Default::default(),
            header_checkpoint: Default::default(),
            connection_type: Default::default(),
            target_peer_size: TARGET_PEER_DB_SIZE,
            response_timeout: Duration::from_secs(TIMEOUT_SECS),
            filter_sync_policy: Default::default(),
        }
    }
}
