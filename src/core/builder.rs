#[cfg(not(feature = "filter-control"))]
use std::collections::HashSet;
use std::{path::PathBuf, time::Duration};

use bitcoin::Network;
#[cfg(not(feature = "filter-control"))]
use bitcoin::ScriptBuf;

use super::{client::Client, config::NodeConfig, node::Node, FilterSyncPolicy};
#[cfg(feature = "database")]
use crate::db::error::SqlInitializationError;
#[cfg(feature = "database")]
use crate::db::sqlite::{headers::SqliteHeaderDb, peers::SqlitePeerDb};
use crate::{
    chain::checkpoints::HeaderCheckpoint,
    db::traits::{HeaderStore, PeerStore},
};
use crate::{ConnectionType, PeerStoreSizeConfig, TrustedPeer};

#[cfg(feature = "database")]
/// The default node returned from the [`NodeBuilder`](crate::core).
pub type NodeDefault = Node<SqliteHeaderDb, SqlitePeerDb>;

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
/// let checkpoint = HeaderCheckpoint::closest_checkpoint_below_height(170_000, Network::Signet);
///
/// let builder = NodeBuilder::new(Network::Signet);
/// // Add node preferences and build the node/client
/// let (mut node, client) = builder
///     // The Bitcoin scripts to monitor
///     .add_scripts(script_set)
///     // Only scan blocks strictly after an anchor checkpoint
///     .anchor_checkpoint(checkpoint)
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
        self.config.white_list = whitelist;
        self
    }

    /// Add a preferred peer to try to connect to.
    pub fn add_peer(mut self, trusted_peer: impl Into<TrustedPeer>) -> Self {
        self.config.white_list.push(trusted_peer.into());
        self
    }

    /// Add Bitcoin scripts to monitor for. You may add more later with the [`Client`].
    #[cfg(not(feature = "filter-control"))]
    pub fn add_scripts(mut self, addresses: HashSet<ScriptBuf>) -> Self {
        self.config.addresses = addresses;
        self
    }

    /// Add a path to the directory where data should be stored. If none is provided, the current
    /// working directory will be used.
    pub fn add_data_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.data_path = Some(path.into());
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
    /// fewer errors. If none is provided, no limit to the size of the store will be introduced.
    pub fn peer_db_size(mut self, target: PeerStoreSizeConfig) -> Self {
        self.config.target_peer_size = target;
        self
    }

    /// Add a checkpoint for the node to look for relevant blocks _strictly after_ the given height.
    /// This may be from the same [`HeaderCheckpoint`] every time the node is ran, or from the last known sync height.
    /// In the case of a block reorganization, the node may scan for blocks below the given block height
    /// to accurately reflect which relevant blocks are in the best chain.
    /// If none is provided, the _most recent_ checkpoint will be used.
    pub fn anchor_checkpoint(mut self, checkpoint: impl Into<HeaderCheckpoint>) -> Self {
        self.config.header_checkpoint = Some(checkpoint.into());
        self
    }

    /// Add a starting height for filters, only height is specified (no block hash).
    /// This works in conjunction with `anchor_checkpoint`:
    /// if `anchor_checkpoint` is set, that will also be used as starting point for filters, and
    /// if `filter_startpoint` is set, the closest earlier known checkpoint will be used as anchor checkpoint.
    pub fn filter_startpoint(mut self, start_height: u32) -> Self {
        self.config.filter_startpoint = Some(start_height);
        // This also implies a default checkpoint (if not yet set)
        if self.config.header_checkpoint.is_none() {
            let checkpoint =
                HeaderCheckpoint::closest_checkpoint_below_height(start_height, self.network);
            return self.anchor_checkpoint(checkpoint);
        }
        self
    }

    /// Set the desired communication channel. Either directly over TCP or over the Tor network.
    pub fn set_connection_type(mut self, connection_type: ConnectionType) -> Self {
        self.config.connection_type = connection_type;
        self
    }

    /// Set the time duration a peer has to respond to a message from the local node.
    ///
    /// ## Note
    ///
    /// Both bandwidth and computing time should be considered when configuring this timeout.
    /// On test networks, this value may be quite short, however on the Bitcoin network,
    /// nodes may be slower to respond while processing blocks and transactions.
    ///
    /// If none is provided, a timeout of 5 seconds will be used.
    pub fn set_response_timeout(mut self, response_timeout: Duration) -> Self {
        self.config.response_timeout = response_timeout;
        self
    }

    /// The maximum connection time that will be maintained with a remote peer, regardless of
    /// the quality of the peer.
    ///
    /// ## Note
    ///
    /// This value is configurable as some developers may be satisfied with a peer
    /// as long as the peer responds promptly. Other implementations may value finding
    /// new a reliable peers faster, so the maximum connection time may be shorter.
    ///
    /// If none is provided, a maximum connection time of two hours will be used.
    pub fn set_maximum_connection_time(mut self, max_connection_time: Duration) -> Self {
        self.config.max_connection_time = max_connection_time;
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
    pub fn build_node(&mut self) -> Result<(NodeDefault, Client), SqlInitializationError> {
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
    pub fn build_with_databases<H: HeaderStore + 'static, P: PeerStore + 'static>(
        &mut self,
        peer_store: P,
        header_store: H,
    ) -> (Node<H, P>, Client) {
        Node::new_from_config(
            core::mem::take(&mut self.config),
            self.network,
            peer_store,
            header_store,
        )
    }
}
