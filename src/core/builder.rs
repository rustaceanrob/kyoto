use std::{collections::HashSet, path::PathBuf, time::Duration};

use bitcoin::{Network, ScriptBuf};

use super::{client::Client, config::NodeConfig, node::Node, FilterSyncPolicy};
#[cfg(feature = "database")]
use crate::db::error::SqlInitializationError;
#[cfg(feature = "database")]
use crate::db::sqlite::{headers::SqliteHeaderDb, peers::SqlitePeerDb};
use crate::{
    chain::checkpoints::HeaderCheckpoint,
    db::traits::{HeaderStore, PeerStore},
};
use crate::{ConnectionType, TrustedPeer};

/// Build a [`Node`] in an additive way.
///
/// # Examples
///
/// Nodes may be built with minimal configuration.
///
/// ```rust
/// use std::net::{IpAddr, Ipv4Addr};
/// use std::collections::HashSet;
/// use kyoto::{NodeBuilder, Network};
///
/// let host = (IpAddr::from(Ipv4Addr::new(0, 0, 0, 0)), None);
/// let builder = NodeBuilder::new(Network::Regtest);
/// let (node, client) = builder
///     .add_peers(vec![host.into()])
///     .build_node()
///     .unwrap();
/// ```
///
/// Or, more pratically, known Bitcoin scripts to monitor for may be added
/// as the node is built.
///
/// ```rust
/// use std::collections::HashSet;
/// use std::str::FromStr;
/// use kyoto::{HeaderCheckpoint, NodeBuilder, Network, Address, BlockHash};
///
/// let address = Address::from_str("tb1q9pvjqz5u5sdgpatg3wn0ce438u5cyv85lly0pc")
///     .unwrap()
///     .require_network(Network::Signet)
///     .unwrap()
///     .into();
/// let mut script_set = HashSet::new();
/// script_set.insert(address);
/// let builder = NodeBuilder::new(Network::Signet);
/// // Add node preferences and build the node/client
/// let (mut node, client) = builder
///     // The Bitcoin scripts to monitor
///     .add_scripts(script_set)
///     // Only scan blocks strictly after an anchor checkpoint
///     .anchor_checkpoint(HeaderCheckpoint::new(
///         170_000,
///         BlockHash::from_str("00000041c812a89f084f633e4cf47e819a2f6b1c0a15162355a930410522c99d")
///             .unwrap(),
///     ))
///     // The number of connections we would like to maintain
///     .num_required_peers(2)
///     .build_node()
///     .unwrap();
/// ```
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
    pub fn add_peers(mut self, whitelist: Vec<TrustedPeer>) -> Self {
        self.config.white_list = Some(whitelist);
        self
    }

    /// Add Bitcoin scripts to monitor for. You may add more later with the [`Client`].
    #[cfg(not(feature = "silent-payments"))]
    pub fn add_scripts(mut self, addresses: HashSet<ScriptBuf>) -> Self {
        self.config.addresses = addresses;
        self
    }

    /// Add a path to the directory where data should be stored. If none is provided, the current
    /// working directory will be used.
    pub fn add_data_dir(mut self, path: PathBuf) -> Self {
        self.config.data_path = Some(path);
        self
    }

    /// Add the minimum number of peer connections that should be maintained by the node.
    /// Adding more connections increases the node's anonymity, but requires waiting for more responses,
    /// higher bandwidth, and higher memory requirements. If none is provided, a single connection will be maintained.
    pub fn num_required_peers(mut self, num_peers: u8) -> Self {
        self.config.required_peers = num_peers;
        self
    }

    /// Set the desired number of peers for the database to keep track of. For limited or in-memory peer storage,
    /// this number may be small, however a sufficient margin of peers should be set so the node can try many options
    /// when downloading compact block filters. For nodes that store peers on disk, more peers will typically result in
    /// fewer errors. If none is provided, a limit of 4096 will be used.
    pub fn peer_db_size(mut self, target_num: u32) -> Self {
        self.config.target_peer_size = target_num;
        self
    }

    /// Add a checkpoint for the node to look for relevant blocks _strictly after_ the given height.
    /// This may be from the same [`HeaderCheckpoint`] every time the node is ran, or from the last known sync height.
    /// In the case of a block reorganization, the node may scan for blocks below the given block height
    /// to accurately reflect which relevant blocks are in the best chain.
    /// If none is provided, the _most recent_ checkpoint will be used.
    pub fn anchor_checkpoint(mut self, checkpoint: HeaderCheckpoint) -> Self {
        self.config.header_checkpoint = Some(checkpoint);
        self
    }

    /// Set the desired communication channel. Either directly over TCP or over the Tor network.
    pub fn set_connection_type(mut self, connection_type: ConnectionType) -> Self {
        self.config.connection_type = connection_type;
        self
    }

    /// Set the time duration a peer has to respond to a message from the local node.
    /// Note that, on test networks, new block propagation is empirically faster than
    /// the Bitcoin network. Longer durations may allow blocks to propagate and for
    /// new `inv` messages to be sent. If none is provided, a timeout of 5 seconds will be used.
    pub fn set_response_timeout(mut self, timeout: Duration) -> Self {
        self.config.response_timeout = timeout;
        self
    }

    /// Stop the node from downloading and checking compact block filters until an explicit command by the client is made.
    /// This is only useful if the scripts to check for may not be known do to some expensive computation, like in a silent
    /// payments context.
    pub fn halt_filter_download(mut self) -> Self {
        self.config.filter_sync_policy = FilterSyncPolicy::Halt;
        self
    }

    /// Consume the node builder and receive a [`Node`] and [`Client`].
    ///
    /// # Errors
    ///
    /// Building a node and client will error if a database connection is denied or cannot be found.
    #[cfg(feature = "database")]
    pub fn build_node(&mut self) -> Result<(Node<SqliteHeaderDb>, Client), SqlInitializationError> {
        let peer_store = SqlitePeerDb::new(self.network, self.config.data_path.clone())?;
        let header_store = SqliteHeaderDb::new(self.network, self.config.data_path.clone())?;
        Ok(Node::new_from_config(
            core::mem::take(&mut self.config),
            self.network,
            peer_store,
            header_store,
        ))
    }

    /// Consume the node builder by using custom database implementations, receiving a [`Node`] and [`Client`].
    pub fn build_with_databases<H: HeaderStore + 'static>(
        &mut self,
        peer_store: impl PeerStore + Send + Sync + 'static,
        header_store: H,
    ) -> (Node<H>, Client) {
        Node::new_from_config(
            core::mem::take(&mut self.config),
            self.network,
            peer_store,
            header_store,
        )
    }
}
