use std::{collections::HashSet, path::PathBuf, time::Duration};

use bitcoin::ScriptBuf;

use crate::{
    chain::checkpoints::HeaderCheckpoint, ConnectionType, PeerStoreSizeConfig, TrustedPeer,
};

use super::FilterSyncPolicy;

const REQUIRED_PEERS: u8 = 1;
const TIMEOUT_SECS: u64 = 5;
//                    sec  min  hour
const TWO_HOUR: u64 = 60 * 60 * 2;

pub(crate) struct NodeConfig {
    pub required_peers: u8,
    pub white_list: Vec<TrustedPeer>,
    pub addresses: HashSet<ScriptBuf>,
    pub data_path: Option<PathBuf>,
    pub header_checkpoint: Option<HeaderCheckpoint>,
    pub filter_startpoint: Option<u32>,
    pub connection_type: ConnectionType,
    pub target_peer_size: PeerStoreSizeConfig,
    pub response_timeout: Duration,
    pub max_connection_time: Duration,
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
            filter_startpoint: Default::default(),
            connection_type: Default::default(),
            target_peer_size: PeerStoreSizeConfig::default(),
            response_timeout: Duration::from_secs(TIMEOUT_SECS),
            max_connection_time: Duration::from_secs(TWO_HOUR),
            filter_sync_policy: Default::default(),
        }
    }
}
