//! Usual sync on Signet.

use kyoto::core::messages::NodeMessage;
use kyoto::{chain::checkpoints::HeaderCheckpoint, core::builder::NodeBuilder};
use kyoto::{AddrV2, Address, Network, ServiceFlags, TrustedPeer};
use std::collections::HashSet;
use std::{
    net::{IpAddr, Ipv4Addr},
    str::FromStr,
};

const NETWORK: Network = Network::Signet;
const RECOVERY_HEIGHT: u32 = 190_000;
const ADDR: &str = "tb1q9pvjqz5u5sdgpatg3wn0ce438u5cyv85lly0pc";

#[tokio::main]
async fn main() {
    // Add third-party logging
    let subscriber = tracing_subscriber::FmtSubscriber::new();
    tracing::subscriber::set_global_default(subscriber).unwrap();
    // Use a predefined checkpoint
    let checkpoint = HeaderCheckpoint::closest_checkpoint_below_height(RECOVERY_HEIGHT, NETWORK);
    // Add Bitcoin scripts to scan the blockchain for
    let address = Address::from_str(ADDR)
        .unwrap()
        .require_network(NETWORK)
        .unwrap()
        .into();
    let mut addresses = HashSet::new();
    addresses.insert(address);
    // Add preferred peers to connect to
    let peer_1 = IpAddr::V4(Ipv4Addr::new(95, 217, 198, 121));
    let peer_2 = TrustedPeer::new(
        AddrV2::Ipv4(Ipv4Addr::new(23, 137, 57, 100)),
        None,
        ServiceFlags::P2P_V2,
    );
    // Create a new node builder
    let builder = NodeBuilder::new(NETWORK);
    // Add node preferences and build the node/client
    let (node, client) = builder
        // Add the peers
        .add_peers(vec![(peer_1, None).into(), peer_2])
        // The Bitcoin scripts to monitor
        .add_scripts(addresses)
        // Only scan blocks strictly after an anchor checkpoint
        .anchor_checkpoint(checkpoint)
        // The number of connections we would like to maintain
        .num_required_peers(2)
        // Create the node and client
        .build_node()
        .unwrap();
    // Check if the node is running. Another part of the program may be giving us the node.
    if !node.is_running() {
        tokio::task::spawn(async move { node.run().await });
    }
    // Split the client into components that send messages and listen to messages.
    // With this construction, different parts of the program can take ownership of
    // specific tasks.
    let (sender, mut receiver) = client.split();
    // Continually listen for events until the node is synced to its peers.
    loop {
        if let Ok(message) = receiver.recv().await {
            match message {
                NodeMessage::Dialog(d) => tracing::info!("{d}"),
                NodeMessage::Warning(e) => tracing::warn!("{e}"),
                NodeMessage::StateChange(s) => tracing::info!("State update: {s}"),
                NodeMessage::Block(b) => {
                    let block = b.block.block_hash();
                    tracing::info!("Received block: {block}");
                }
                NodeMessage::BlocksDisconnected(r) => {
                    let _ = r;
                }
                NodeMessage::TxSent(t) => {
                    tracing::info!("Transaction sent. TXID: {t}");
                }
                NodeMessage::TxBroadcastFailure(t) => {
                    tracing::error!("The transaction could not be broadcast. TXID: {}", t.txid);
                }
                NodeMessage::FeeFilter(fee) => {
                    tracing::info!("Fee filter received: {fee} kwu");
                }
                NodeMessage::Synced(update) => {
                    tracing::info!("Synced chain up to block {}", update.tip.height);
                    tracing::info!("Chain tip: {}", update.tip.hash);
                    let recent = update.recent_history;
                    let header = client.get_header(update.tip.height).await.unwrap().unwrap();
                    assert_eq!(header.block_hash(), update.tip.hash);
                    tracing::info!("Recent history:");
                    for (height, hash) in recent {
                        tracing::info!("Height: {}", height);
                        tracing::info!("Hash: {}", hash.block_hash());
                    }
                    break;
                }
                NodeMessage::ConnectionsMet => {
                    tracing::info!("Connected to all required peers");
                }
                _ => (),
            }
        }
    }
    let _ = sender.shutdown().await;
    tracing::info!("Shutting down");
}
