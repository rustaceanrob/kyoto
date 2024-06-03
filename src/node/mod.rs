/// Convenient way to build a compact filters node.
pub mod builder;
pub(crate) mod channel_messages;
/// Structures to communicate with a node.
pub mod client;
/// Node configuration options.
pub mod config;
pub(crate) mod dialog;
/// Errors associated with a node.
pub mod error;
#[allow(clippy::module_inception)]
/// The structure that communicates with the Bitcoin P2P network and collects data.
pub mod node;
/// Messages the node may send a client.
pub mod node_messages;
mod peer_map;
