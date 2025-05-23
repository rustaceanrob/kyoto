//! Kyoto supports checking filters directly, as protocols like silent payments will have
//! many possible scripts to check. Enable the `filter-control` feature to check filters
//! manually in your program.

use kyoto::messages::Event;
use kyoto::{builder::NodeBuilder, chain::checkpoints::HeaderCheckpoint, Client};
use kyoto::{AddrV2, Address, BlockHash, LogLevel, Network, ServiceFlags, TrustedPeer};
use std::collections::HashSet;
use std::{net::Ipv4Addr, str::FromStr};

const NETWORK: Network = Network::Signet;
const RECOVERY_HEIGHT: u32 = 201_000;
const RECOVERY_HASH: &str = "0000002238d05b522875f9edc4c9f418dd89ccfde7e4c305e8448a87a5dc71b7";
const ADDR: &str = "tb1q9pvjqz5u5sdgpatg3wn0ce438u5cyv85lly0pc";

#[tokio::main]
async fn main() {
    // Add third-party logging
    let subscriber = tracing_subscriber::FmtSubscriber::new();
    tracing::subscriber::set_global_default(subscriber).unwrap();
    // Use a predefined checkpoint
    let checkpoint =
        HeaderCheckpoint::new(RECOVERY_HEIGHT, BlockHash::from_str(RECOVERY_HASH).unwrap());
    // Add Bitcoin scripts to scan the blockchain for
    let address = Address::from_str(ADDR)
        .unwrap()
        .require_network(NETWORK)
        .unwrap()
        .into();
    let mut addresses = HashSet::new();
    addresses.insert(address);
    // Add preferred peers to connect to
    let peer = TrustedPeer::new(
        AddrV2::Ipv4(Ipv4Addr::new(23, 137, 57, 100)),
        None,
        ServiceFlags::P2P_V2,
    );
    // Create a new node builder
    let builder = NodeBuilder::new(NETWORK);
    // Add node preferences and build the node/client
    let (node, client) = builder
        // Add the peers
        .add_peer(peer)
        // Only scan blocks strictly after a checkpoint
        .after_checkpoint(checkpoint)
        // The number of connections we would like to maintain
        .required_peers(1)
        // Omit debug messages
        .log_level(LogLevel::Info)
        // Create the node and client
        .build()
        .unwrap();

    tokio::task::spawn(async move { node.run().await });

    let Client {
        requester,
        log_rx: _,
        mut info_rx,
        mut warn_rx,
        mut event_rx,
    } = client;

    // Continually listen for events until the node is synced to its peers.
    loop {
        tokio::select! {
            info = info_rx.recv() => {
                if let Some(info) = info {
                    tracing::info!("{info}");
                }
            }
            warn = warn_rx.recv() => {
                if let Some(warn) = warn {
                    tracing::warn!("{warn}");
                }
            }
            event = event_rx.recv() => {
                if let Some(event) = event {
                    match event {
                        Event::IndexedFilter(filter) => {
                            let height = filter.height();
                            tracing::info!("Checking filter: {height}");
                            if filter.contains_any(addresses.iter()) {
                                let hash = *filter.block_hash();
                                tracing::info!("Found script at {}!", hash);
                                let indexed_block = requester.get_block(hash).await.unwrap();
                                let coinbase = indexed_block.block.txdata.first().unwrap().compute_txid();
                                tracing::info!("Coinbase transaction ID: {}", coinbase);
                                break;
                            }
                        },
                        Event::Synced(_) => break,
                        _ => (),
                    }
                }
            }
        }
    }
    let _ = requester.shutdown();
    tracing::info!("Shutting down");
}
