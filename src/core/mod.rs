//! Tools to build and run a compact block filters node.
//!
//! All logic for syncing with the Bitcoin network occurs within a [`Node`](node::Node). Nodes emit events of relevance
//! by sending [`NodeMessage`](messages::NodeMessage), and these events may be consumed by a [`Client`](client::Client). A client may also send
//! messages to a node to add more Bitcoin scripts, broadcast transactions, and more.
//!
//! To build a [`Node`](node::Node) and [`Client`](client::Client), please refer to the [`NodeBuilder`](builder::NodeBuilder), which allows for node
//! configuration.

use std::time::Duration;

use tokio::time::Instant;

mod broadcaster;
/// Convenient way to build a compact filters node.
pub mod builder;
pub(crate) mod channel_messages;
/// Structures to communicate with a node.
pub mod client;
/// Node configuration options.
pub(crate) mod config;
pub(crate) mod dialog;
/// Errors associated with a node.
pub mod error;
/// Messages the node may send a client.
pub mod messages;
/// The structure that communicates with the Bitcoin P2P network and collects data.
pub mod node;
mod peer_map;
#[cfg(feature = "filter-control")]
use crate::IndexedBlock;
#[cfg(feature = "filter-control")]
use error::FetchBlockError;

/// Receive an [`IndexedBlock`] from a request.
#[cfg(feature = "filter-control")]
pub type BlockReceiver = tokio::sync::oneshot::Receiver<Result<IndexedBlock, FetchBlockError>>;

const THIRTY_MINS: u64 = 60 * 30;

// This struct detects for stale tips and requests headers if no blocks were found after 30 minutes of wait time.
pub(crate) struct LastBlockMonitor {
    last_block: Option<Instant>,
}

impl LastBlockMonitor {
    pub(crate) fn new() -> Self {
        Self { last_block: None }
    }

    pub(crate) fn reset(&mut self) {
        self.last_block = Some(Instant::now())
    }

    pub(crate) fn stale(&self) -> bool {
        if let Some(time) = self.last_block {
            return Instant::now().duration_since(time) > Duration::from_secs(THIRTY_MINS);
        }
        false
    }
}

/// Should the node immediately download filters or wait for a command
#[derive(Debug, Default)]
pub enum FilterSyncPolicy {
    /// The node will wait for an explicit command to start downloading and checking filters
    Halt,
    /// Filters are downloaded immediately after CBF headers are synced.
    #[default]
    Continue,
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub(crate) struct PeerTimeoutConfig {
    pub(crate) response_timeout: Duration,
    pub(crate) max_connection_time: Duration,
}

impl PeerTimeoutConfig {
    fn new(response_timeout: Duration, max_connection_time: Duration) -> Self {
        Self {
            response_timeout,
            max_connection_time,
        }
    }
}
