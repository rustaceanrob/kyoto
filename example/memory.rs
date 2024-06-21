//! This example demonstrates a limited resource device connecting to a single peer.
//! While this configuration limits dependencies and requires practically no storage on
//! the underlying machine, there is no privacy if the connected node is also used to broadcast
//! transactions to the Bitcoin P2P network.

use kyoto::chain::checkpoints::SIGNET_HEADER_CP;
use kyoto::db::memory::peers::StatelessPeerStore;
use kyoto::node::messages::NodeMessage;
use kyoto::BlockHash;
use kyoto::{chain::checkpoints::HeaderCheckpoint, node::builder::NodeBuilder};
use std::collections::HashSet;
use std::{
    net::{IpAddr, Ipv4Addr},
    str::FromStr,
};

#[tokio::main]
async fn main() {
    // Add third-party logging
    let subscriber = tracing_subscriber::FmtSubscriber::new();
    tracing::subscriber::set_global_default(subscriber).unwrap();
    // Add Bitcoin scripts to scan the blockchain for
    let address = bitcoin::Address::from_str("tb1q9pvjqz5u5sdgpatg3wn0ce438u5cyv85lly0pc")
        .unwrap()
        .require_network(bitcoin::Network::Signet)
        .unwrap()
        .into();
    let mut addresses = HashSet::new();
    addresses.insert(address);
    // If you don't have any checkpoint stored yet, you can use a predefined header.
    let (height, hash) = SIGNET_HEADER_CP.last().unwrap();
    let anchor = HeaderCheckpoint::new(*height, BlockHash::from_str(hash).unwrap());
    // Define a peer to connect to
    let peer = IpAddr::V4(Ipv4Addr::new(95, 217, 198, 121));
    // Limited devices may not save any peers to disk
    let peer_store = StatelessPeerStore::new();
    // Create a new node builder
    let builder = NodeBuilder::new(bitcoin::Network::Signet);
    // Add node preferences and build the node/client
    let (mut node, mut client) = builder
        // Add the peer
        .add_peers(vec![(peer, 38333)])
        // The Bitcoin scripts to monitor
        .add_scripts(addresses)
        // Only scan blocks strictly after an anchor checkpoint
        .anchor_checkpoint(anchor)
        // The number of connections we would like to maintain
        .num_required_peers(2)
        // We only maintain a list of 32 peers in memory
        .peer_db_size(32)
        // Build without the default databases
        .build_node_with_custom_databases(peer_store, ())
        .await;
    // Run the node
    tokio::task::spawn(async move { node.run().await });
    // Split the client into components that send messages and listen to messages.
    // With this construction, different parts of the program can take ownership of
    // specific tasks.
    let (mut sender, mut receiver) = client.split();
    // Continually listen for events until the node is synced to its peers.
    loop {
        if let Ok(message) = receiver.recv().await {
            match message {
                NodeMessage::Dialog(d) => tracing::info!("{}", d),
                NodeMessage::Warning(e) => tracing::warn!("{}", e),
                NodeMessage::Transaction(t) => drop(t),
                NodeMessage::Block(b) => drop(b),
                NodeMessage::BlocksDisconnected(r) => {
                    let _ = r;
                }
                NodeMessage::TxBroadcastFailure => {
                    tracing::error!("The transaction could not be broadcast.")
                }
                NodeMessage::Synced(update) => {
                    tracing::info!("Synced chain up to block {}", update.tip().height,);
                    tracing::info!("Chain tip: {}", update.tip().hash.to_string(),);
                    break;
                }
            }
        }
    }
    let _ = sender.shutdown().await;
    tracing::info!("Shutting down");
}
