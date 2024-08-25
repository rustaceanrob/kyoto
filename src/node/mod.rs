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
#[allow(clippy::module_inception)]
/// The structure that communicates with the Bitcoin P2P network and collects data.
pub mod node;
mod peer_map;

const THIRTY_MINS: u64 = 60 * 30;

// This struct detects for stale tips and requests headers if no blocks were found after 30 minutes of wait time.
pub(crate) struct LastBlockMonitor {
    last_block: Option<Instant>,
}

impl LastBlockMonitor {
    pub(crate) fn new() -> Self {
        Self { last_block: None }
    }

    pub(crate) fn update(&mut self) {
        self.last_block = Some(Instant::now())
    }

    pub(crate) fn stale(&self) -> bool {
        if let Some(time) = self.last_block {
            return Instant::now().duration_since(time) > Duration::from_secs(THIRTY_MINS);
        }
        false
    }
}
