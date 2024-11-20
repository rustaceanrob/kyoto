use kyoto::core::messages::NodeMessage;
use kyoto::{chain::checkpoints::HeaderCheckpoint, core::builder::NodeBuilder};
use kyoto::{Address, Network, PeerStoreSizeConfig, TrustedPeer};
use std::collections::HashSet;
use std::{net::Ipv4Addr, str::FromStr};

const NETWORK: Network = Network::Testnet4;
const RECOVERY_HEIGHT: u32 = 0;

#[tokio::main]
async fn main() {
    // Add third-party logging
    let subscriber = tracing_subscriber::FmtSubscriber::new();
    tracing::subscriber::set_global_default(subscriber).unwrap();
    // Use a predefined checkpoint
    let checkpoint = HeaderCheckpoint::closest_checkpoint_below_height(RECOVERY_HEIGHT, NETWORK);
    // Add Bitcoin scripts to scan the blockchain for
    let address = Address::from_str("tb1qkqkt3ra8d44lt90t9thgy3lgucsjrtywwgq8yp")
        .unwrap()
        .require_network(NETWORK)
        .unwrap()
        .into();
    let mut addresses = HashSet::new();
    addresses.insert(address);
    // Add preferred peers to connect to
    let peer = TrustedPeer::from_ip(Ipv4Addr::new(18, 189, 156, 102));
    // Create a new node builder
    let builder = NodeBuilder::new(NETWORK);
    // Add node preferences and build the node/client
    let (node, client) = builder
        // Add the peers
        .add_peer(peer)
        // The Bitcoin scripts to monitor
        .add_scripts(addresses)
        // Only scan blocks strictly after an anchor checkpoint
        .anchor_checkpoint(checkpoint)
        // Store a limited number of peers
        .peer_db_size(PeerStoreSizeConfig::Limit(256))
        // The number of connections we would like to maintain
        .num_required_peers(1)
        // Create the node and client
        .build_node()
        .unwrap();

    // Run the node on a new task
    tokio::task::spawn(async move { node.run().await });

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
                NodeMessage::Block(b) => drop(b),
                NodeMessage::BlocksDisconnected(r) => {
                    for dc in r {
                        let warning = format!("Block disconnected {}", dc.height);
                        tracing::warn!(warning);
                    }
                }
                NodeMessage::Synced(update) => {
                    tracing::info!("Synced chain up to block {}", update.tip.height);
                    tracing::info!("Chain tip: {}", update.tip.hash);
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
