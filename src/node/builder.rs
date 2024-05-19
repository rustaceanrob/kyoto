use std::net::IpAddr;

use bitcoin::Network;

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

    pub async fn build_node(&self) -> (Node, Client) {
        Node::new(
            self.network,
            self.config.white_list.clone(),
            self.config.addresses.clone(),
        )
        .await
        .unwrap()
    }
}
