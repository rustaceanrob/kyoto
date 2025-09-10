use std::path::PathBuf;

use crate::{
    chain::checkpoints::HeaderCheckpoint,
    network::{dns::DnsResolver, ConnectionType},
    PeerStoreSizeConfig, PeerTimeoutConfig, TrustedPeer,
};

const REQUIRED_PEERS: u8 = 1;

pub(crate) struct NodeConfig {
    pub required_peers: u8,
    pub white_list: Vec<TrustedPeer>,
    pub dns_resolver: DnsResolver,
    pub data_path: Option<PathBuf>,
    pub header_checkpoint: Option<HeaderCheckpoint>,
    pub connection_type: ConnectionType,
    pub target_peer_size: PeerStoreSizeConfig,
    pub peer_timeout_config: PeerTimeoutConfig,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            required_peers: REQUIRED_PEERS,
            white_list: Default::default(),
            dns_resolver: DnsResolver::default(),
            data_path: Default::default(),
            header_checkpoint: Default::default(),
            connection_type: Default::default(),
            target_peer_size: PeerStoreSizeConfig::default(),
            peer_timeout_config: PeerTimeoutConfig::default(),
        }
    }
}
