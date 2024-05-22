use std::{net::IpAddr, path::PathBuf};

use bitcoin::Network;

use crate::chain::checkpoints::HeaderCheckpoint;

use super::{client::Client, config::NodeConfig, node::Node};

pub struct NodeBuilder {
    config: NodeConfig,
    network: Network,
}

impl NodeBuilder {
    pub fn new(network: Network) -> Self {
        Self {
            config: NodeConfig::default(),
            network,
        }
    }

    pub fn add_peers(mut self, whitelist: Vec<(IpAddr, u16)>) -> Self {
        self.config.white_list = Some(whitelist);
        self
    }

    pub fn add_scripts(mut self, addresses: Vec<bitcoin::Address>) -> Self {
        self.config.addresses = addresses;
        self
    }

    pub fn add_data_dir(mut self, path: PathBuf) -> Self {
        self.config.data_path = Some(path);
        self
    }

    pub fn num_required_peers(mut self, num_peers: u8) -> Self {
        self.config.required_peers = num_peers;
        self
    }

    pub fn from_checkpoint(mut self, checkpoint: HeaderCheckpoint) -> Self {
        self.config.header_checkpoint = Some(checkpoint);
        self
    }

    pub async fn build_node(&self) -> (Node, Client) {
        Node::new_from_config(&self.config, self.network)
            .await
            .unwrap()
    }
}
