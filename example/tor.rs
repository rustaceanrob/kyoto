//! This example demonstrates a limited resource device running with no default features.
//! Note that, without DNS enabled, at least one peer must be provided when building the node.

use kyoto::chain::checkpoints::SIGNET_HEADER_CP;
use kyoto::core::messages::NodeMessage;
use kyoto::db::memory::peers::StatelessPeerStore;
use kyoto::db::sqlite::headers::SqliteHeaderDb;
use kyoto::{chain::checkpoints::HeaderCheckpoint, core::builder::NodeBuilder};
use kyoto::{BlockHash, ConnectionType, TorClient, TorClientConfig, TrustedPeer};
use std::collections::HashSet;
use std::str::FromStr;

const PEER_ONION: [u8; 32] = [
    122, 158, 138, 248, 80, 128, 65, 182, 7, 162, 120, 132, 58, 231, 181, 235, 247, 78, 128, 81,
    77, 117, 148, 234, 156, 5, 51, 150, 136, 144, 21, 22,
];

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
    let peer = TrustedPeer::from_tor_v3(PEER_ONION);
    // Limited devices may not save any peers to disk
    let peer_store = StatelessPeerStore::new();
    // To handle reorgs, it is still recommended to store block headers
    let header_store = SqliteHeaderDb::new(bitcoin::Network::Signet, None).unwrap();
    // Create a new node builder
    let builder = NodeBuilder::new(bitcoin::Network::Signet);
    // Add node preferences and build the node/client
    let (node, client) = builder
        // Add the peer
        .add_peer(peer)
        // The Bitcoin scripts to monitor
        .add_scripts(addresses)
        // Only scan blocks strictly after an anchor checkpoint
        .anchor_checkpoint(anchor)
        // The number of connections we would like to maintain
        .num_required_peers(2)
        // We only maintain a list of 32 peers in memory
        .peer_db_size(256)
        // Connect to peers over Tor
        .set_connection_type(ConnectionType::Tor(tor))
        // Build without the default databases
        .build_with_databases(peer_store, header_store);
    // Run the node
    tokio::task::spawn(async move { node.run().await });
    // Split the client into components that send messages and listen to messages.
    // With this construction, different parts of the program can take ownership of
    // specific tasks.
    let (sender, mut receiver) = client.split();
    // Continually listen for events until the node is synced to its peers.
    while let Ok(message) = receiver.recv().await {
        match message {
            NodeMessage::Dialog(d) => tracing::info!("{}", d),
            NodeMessage::Warning(e) => tracing::warn!("{}", e),
            NodeMessage::StateChange(_) => (),
            NodeMessage::Transaction(t) => drop(t),
            NodeMessage::Block(b) => drop(b),
            NodeMessage::BlocksDisconnected(r) => {
                let _ = r;
            }
            NodeMessage::TxSent(t) => {
                tracing::info!("Transaction sent. TXID: {t}");
            }
            NodeMessage::TxBroadcastFailure(t) => {
                tracing::error!("The transaction could not be broadcast. TXID: {}", t.txid);
            }
            NodeMessage::Synced(update) => {
                tracing::info!("Synced chain up to block {}", update.tip().height,);
                tracing::info!("Chain tip: {}", update.tip().hash.to_string(),);
                break;
            }
            NodeMessage::ConnectionsMet => tracing::info!("Connected to all required peers"),
        }
    }
    let _ = sender.shutdown().await;
    tracing::info!("Shutting down");
}
