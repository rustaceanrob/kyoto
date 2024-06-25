use std::{collections::HashSet, net::IpAddr, path::PathBuf};

use bitcoin::{Network, ScriptBuf};

use crate::db::error::DatabaseError;
use crate::{
    chain::checkpoints::HeaderCheckpoint,
    db::traits::{HeaderStore, PeerStore},
};

use super::{client::Client, config::NodeConfig, node::Node};

/// Build a [`Node`] in an additive way.
pub struct NodeBuilder {
    config: NodeConfig,
    network: Network,
}

impl NodeBuilder {
    /// Create a new [`NodeBuilder`].
    pub fn new(network: Network) -> Self {
        Self {
            config: NodeConfig::default(),
            network,
        }
    }

    /// Add preferred peers to try to connect to.
    pub fn add_peers(mut self, whitelist: Vec<(IpAddr, u16)>) -> Self {
        self.config.white_list = Some(whitelist);
        self
    }

    /// Add Bitcoin scripts to monitor for. You may add more later with the [`Client`].
    pub fn add_scripts(mut self, addresses: HashSet<ScriptBuf>) -> Self {
        self.config.addresses = addresses;
        self
    }

    /// Add a path to the directory where data should be stored.
    pub fn add_data_dir(mut self, path: PathBuf) -> Self {
        self.config.data_path = Some(path);
        self
    }

    /// Add the minimum number of peer connections that should be maintained by the node.
    /// Adding more connections increases the node's anonymity, but requires waiting for more responses,
    /// higher bandwidth, and higher memory requirements.
    pub fn num_required_peers(mut self, num_peers: u8) -> Self {
        self.config.required_peers = num_peers;
        self
    }

    /// Set the desired number of peers for the database to keep track of. For limited or in-memory peer storage,
    /// this number may be small, however a sufficient margin of peers should be set so the node can try many options
    /// when downloading compact block filters. For nodes that store peers on disk, more peers will typically result in
    /// fewer errors.
    pub fn peer_db_size(mut self, target_num: u32) -> Self {
        self.config.target_peer_size = target_num;
        self
    }

    /// Add a checkpoint for the node to look for relevant blocks _strictly after_ the given height.
    /// This may be from the same [`HeaderCheckpoint`] every time the node is ran, or from the last known sync height.
    /// In the case of a block reorganization, the node may scan for blocks below the given block height
    /// to accurately reflect which relevant blocks are in the best chain.
    pub fn anchor_checkpoint(mut self, checkpoint: HeaderCheckpoint) -> Self {
        self.config.header_checkpoint = Some(checkpoint);
        self
    }

    /// Consume the node builder and receive a [`Node`] and [`Client`].
    #[cfg(feature = "database")]
    pub async fn build_node(&self) -> (Node, Client) {
        use crate::db::sqlite::{headers::SqliteHeaderDb, peers::SqlitePeerDb};
        let peer_store = SqlitePeerDb::new(self.network, self.config.data_path.clone()).unwrap();
        let header_store =
            SqliteHeaderDb::new(self.network, self.config.data_path.clone()).unwrap();
        Node::new_from_config(&self.config, self.network, peer_store, header_store)
            .await
            .map_err(|_| DatabaseError::LoadError)
            .unwrap()
    }

    pub async fn build_with_databases(
        &self,
        peer_store: impl PeerStore + Send + Sync + 'static,
        header_store: impl HeaderStore + Send + Sync + 'static,
    ) -> (Node, Client) {
        Node::new_from_config(&self.config, self.network, peer_store, header_store)
            .await
            .map_err(|_| DatabaseError::LoadError)
            .unwrap()
    }
}
