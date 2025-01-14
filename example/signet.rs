//! Usual sync on Signet.

use kyoto::{chain::checkpoints::HeaderCheckpoint, core::builder::NodeBuilder};
use kyoto::{AddrV2, Address, Client, Event, Log, Network, ServiceFlags, TrustedPeer};
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
    // Run the node on a separate task
    tokio::task::spawn(async move { node.run().await });
    // Split the client into components that send messages and listen to messages.
    // With this construction, different parts of the program can take ownership of
    // specific tasks.
    let Client {
        requester,
        mut log_rx,
        mut event_rx,
    } = client;
    // Continually listen for events until the node is synced to its peers.
    loop {
        tokio::select! {
            event = event_rx.recv() => {
                if let Some(event) = event {
                    match event {
                        Event::Synced(update) => {
                            tracing::info!("Synced chain up to block {}",update.tip().height);
                            tracing::info!("Chain tip: {}",update.tip().hash);
                            // Request information from the node
                            let fee = requester.broadcast_min_feerate().await.unwrap();
                            tracing::info!("Minimum transaction broadcast fee rate: {}", fee);
                            break;
                        },
                        Event::Block(indexed_block) => {
                            let hash = indexed_block.block.block_hash();
                            tracing::info!("Received block: {}", hash);
                        },
                        Event::BlocksDisconnected(_) => {
                            tracing::warn!("Some blocks were reorganized")
                        },
                        _ => (),
                    }
                }
            }
            log = log_rx.recv() => {
                if let Some(log) = log {
                    match log {
                        Log::Dialog(d)=> tracing::info!("{d}"),
                        Log::Warning(warning)=> tracing::warn!("{warning}"),
                        Log::StateChange(node_state) => tracing::info!("{node_state}"),
                        Log::ConnectionsMet => tracing::info!("All required connections met"),
                        _ => (),
                    }
                }
            }
        }
    }
    let _ = requester.shutdown().await;
    tracing::info!("Shutting down");
}
