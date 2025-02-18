//! Kyoto allows for use of the Tor protocol to connect to peers.
//! To do so, the `arti` project is used to build a Tor client.
//! Enable Tor connections via the `tor` feature.

use kyoto::{
    Client, ConnectionType, Event, HeaderCheckpoint, Log, Network, NodeBuilder,
    PeerStoreSizeConfig, TorClient, TorClientConfig, TrustedPeer,
};
use std::collections::HashSet;
use std::str::FromStr;

const PEER_ONION: [u8; 32] = [
    122, 158, 138, 248, 80, 128, 65, 182, 7, 162, 120, 132, 58, 231, 181, 235, 247, 78, 128, 81,
    77, 117, 148, 234, 156, 5, 51, 150, 136, 144, 21, 22,
];
const RECOVERY_HEIGHT: u32 = 220_000;
const ADDR: &str = "tb1qmfjfv35csd200t0cfpckvx4ccw6w7ytkvga2gn";
const NETWORK: Network = Network::Signet;

#[tokio::main]
async fn main() {
    // Add third-party logging
    let subscriber = tracing_subscriber::FmtSubscriber::new();
    tracing::subscriber::set_global_default(subscriber).unwrap();
    // Start a TorClient
    let tor = TorClient::create_bootstrapped(TorClientConfig::default())
        .await
        .unwrap();
    // Add Bitcoin scripts to scan the blockchain for
    let address = bitcoin::Address::from_str(ADDR)
        .unwrap()
        .require_network(NETWORK)
        .unwrap()
        .into();
    let mut addresses = HashSet::new();
    addresses.insert(address);
    // If you don't have any checkpoint stored yet, you can use a predefined header.
    let anchor = HeaderCheckpoint::closest_checkpoint_below_height(RECOVERY_HEIGHT, NETWORK);
    // Define a peer to connect to
    let peer = TrustedPeer::from_tor_v3(PEER_ONION);
    // Create a new node builder
    let builder = NodeBuilder::new(NETWORK);
    // Add node preferences and build the node/client
    let (node, client) = builder
        // Add the peer
        .add_peer(peer)
        // The Bitcoin scripts to monitor
        .add_scripts(addresses)
        // Only scan blocks strictly after an anchor checkpoint
        .anchor_checkpoint(anchor)
        // The number of connections we would like to maintain
        .required_peers(2)
        // We only maintain a list of 256 peers in memory
        .peer_db_size(PeerStoreSizeConfig::Limit(256))
        // Connect to peers over Tor
        .connection_type(ConnectionType::Tor(tor))
        // Build without the default databases
        .build()
        .unwrap();
    // Run the node
    tokio::task::spawn(async move { node.run().await });

    let Client {
        requester,
        mut log_rx,
        mut warn_rx,
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
                    }
                }
            }
            log = log_rx.recv() => {
                if let Some(log) = log {
                    match log {
                        Log::Dialog(d)=> tracing::info!("{d}"),
                        Log::StateChange(node_state) => tracing::info!("{node_state}"),
                        Log::ConnectionsMet => tracing::info!("All required connections met"),
                        _ => (),
                    }
                }
            }
            warn = warn_rx.recv() => {
                if let Some(warn) = warn {
                    tracing::warn!("{warn}");
                }
            }
        }
    }
    let _ = requester.shutdown().await;
}
