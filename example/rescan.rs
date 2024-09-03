//! When adding new scripts that have a previous history, we can rescan
//! the filters for inclusions of these scripts and download the relevant
//! blocks.

use bitcoin::BlockHash;
use kyoto::core::messages::NodeMessage;
use kyoto::{chain::checkpoints::HeaderCheckpoint, core::builder::NodeBuilder};
use std::collections::HashSet;
use std::str::FromStr;

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
    // Create a new node builder
    let builder = NodeBuilder::new(bitcoin::Network::Signet);
    // Add node preferences and build the node/client
    let (node, client) = builder
        // The Bitcoin scripts to monitor
        .add_scripts(addresses)
        // Only scan blocks strictly after an anchor checkpoint
        .anchor_checkpoint(HeaderCheckpoint::new(
            170_000,
            BlockHash::from_str("00000041c812a89f084f633e4cf47e819a2f6b1c0a15162355a930410522c99d")
                .unwrap(),
        ))
        // The number of connections we would like to maintain
        .num_required_peers(2)
        .build_node()
        .unwrap();
    // Run the node and wait for the sync message;
    tokio::task::spawn(async move { node.run().await });
    tracing::info!("Running the node and waiting for a sync message. Please wait a minute!");
    // Split the client into components that send messages and listen to messages
    let (sender, mut receiver) = client.split();
    // Sync with the single script added
    loop {
        if let Ok(message) = receiver.recv().await {
            match message {
                NodeMessage::Dialog(d) => tracing::info!("{}", d),
                NodeMessage::Warning(e) => tracing::warn!("{}", e),
                NodeMessage::Synced(update) => {
                    tracing::info!("Synced chain up to block {}", update.tip().height);
                    tracing::info!("Chain tip: {}", update.tip().hash);
                    break;
                }
                _ => (),
            }
        }
    }
    // Add new scripts to the node.
    let mut new_scripts = HashSet::new();
    let new_script = bitcoin::Address::from_str(
        "tb1par6ufhp0t448t908kyyvkp3a48r42qcjmg0z9p6a0zuakc44nn2seh63jr",
    )
    .unwrap()
    .require_network(bitcoin::Network::Signet)
    .unwrap()
    .into();
    new_scripts.insert(new_script);
    sender.add_scripts(new_scripts).await.unwrap();
    // // Tell the node to look for these new scripts
    sender.rescan().await.unwrap();
    tracing::info!("Starting rescan");
    loop {
        if let Ok(message) = receiver.recv().await {
            match message {
                NodeMessage::Dialog(d) => tracing::info!("{}", d),
                NodeMessage::Warning(e) => tracing::warn!("{}", e),
                NodeMessage::Synced(update) => {
                    tracing::info!("Synced chain up to block {}", update.tip.height);
                    tracing::info!("Chain tip: {}", update.tip.hash);
                    break;
                }
                _ => (),
            }
        }
    }
    let _ = sender.shutdown().await;
    tracing::info!("Shutting down");
}
