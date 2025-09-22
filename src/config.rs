use std::path::PathBuf;

use crate::{
    chain::ChainState,
    network::{ConnectionType, PeerTimeoutConfig},
    TrustedPeer,
};

const REQUIRED_PEERS: u8 = 1;

#[derive(Debug)]
pub(crate) struct NodeConfig {
    pub required_peers: u8,
    pub white_list: Vec<TrustedPeer>,
    pub data_path: Option<PathBuf>,
    pub chain_state: Option<ChainState>,
    pub connection_type: ConnectionType,
    pub peer_timeout_config: PeerTimeoutConfig,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            required_peers: REQUIRED_PEERS,
            white_list: Default::default(),
            data_path: Default::default(),
            chain_state: Default::default(),
            connection_type: Default::default(),
            peer_timeout_config: PeerTimeoutConfig::default(),
        }
    }
}
