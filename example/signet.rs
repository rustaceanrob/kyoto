//! The following configuration is likely a common set-up for most users.
//! New peers are stored in a local database, and the node connects to multiple peers
//! to improve the anonymity set when broadcasting transactions to the Bitcoin network.

use bitcoin::BlockHash;
use kyoto::node::messages::NodeMessage;
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
    // Add preferred peers to connect to
    let peer = IpAddr::V4(Ipv4Addr::new(23, 137, 57, 100));
    let peer_2 = IpAddr::V4(Ipv4Addr::new(95, 217, 198, 121));
    // Create a new node builder
    let builder = NodeBuilder::new(bitcoin::Network::Signet);
    // Add node preferences and build the node/client
    let (mut node, mut client) = builder
        // Add the peers
        .add_peers(vec![(peer, 38333), (peer_2, 38333)])
        // The Bitcoin scripts to monitor
        .add_scripts(addresses)
        // Only scan blocks strictly after an anchor checkpoint
        .anchor_checkpoint(HeaderCheckpoint::new(
            190_000,
            BlockHash::from_str("0000013a6143b7360b7ba3834316b3265ee9072dde440bd45f99c01c42abaef2")
                .unwrap(),
        ))
        // The number of connections we would like to maintain
        .num_required_peers(3)
        // Create the node and client
        .build_node()
        .await;
    // Check if the node is running. Another part of the program may be giving us the node.
    if !node.is_running() {
        tokio::task::spawn(async move { node.run().await });
    }
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
                NodeMessage::TxSent => {
                    tracing::info!("Transaction sent");
                }
                NodeMessage::TxBroadcastFailure => {
                    tracing::error!("The transaction could not be broadcast.")
                }
                NodeMessage::Synced(update) => {
                    tracing::info!("Synced chain up to block {}", update.tip.height);
                    tracing::info!("Chain tip: {}", update.tip.hash);
                    let recent = update.recent_history;
                    tracing::info!("Recent history:");
                    for (height, hash) in recent {
                        tracing::info!("Synced chain up to block {}", height);
                        tracing::info!("Chain tip: {}", hash.block_hash());
                    }
                    break;
                }
            }
        }
    }
    let _ = sender.shutdown().await;
    tracing::info!("Shutting down");
}
