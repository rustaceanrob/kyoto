//! Sync a simple script with the Bitcoin network. This example is intended to demonstrate the
//! expected sync time on your machine and in your region.

use kyoto::builder::NodeBuilder;
use kyoto::{Client, Event, Network, ScriptBuf};
use std::collections::HashSet;
use tokio::time::Instant;

const NETWORK: Network = Network::Bitcoin;

#[tokio::main]
async fn main() {
    // Add third-party logging
    let subscriber = tracing_subscriber::FmtSubscriber::new();
    tracing::subscriber::set_global_default(subscriber).unwrap();
    let now = Instant::now();
    // Add Bitcoin scripts to scan the blockchain for
    let address = ScriptBuf::new_op_return(b"Kyoto light client");
    let mut addresses = HashSet::new();
    addresses.insert(address);
    // Create a new node builder
    let builder = NodeBuilder::new(NETWORK);
    // Add node preferences and build the node/client
    let (node, client) = builder
        // The number of connections we would like to maintain
        .required_peers(2)
        // Create the node and client
        .build()
        .unwrap();
    // Run the node on a separate task
    tokio::task::spawn(async move { node.run().await });
    // Split the client into components that send messages and listen to messages.
    // With this construction, different parts of the program can take ownership of
    // specific tasks.
    let Client {
        requester,
        mut info_rx,
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
                            tracing::info!("Chain tip: {}",update.tip().hash);
                            // Request information from the node
                            let fee = requester.broadcast_min_feerate().await.unwrap();
                            tracing::info!("Minimum transaction broadcast fee rate: {:#}", fee);
                            let sync_time = now.elapsed().as_secs_f32();
                            tracing::info!("Total sync time: {sync_time} seconds");
                            let avg_fee_rate = requester.average_fee_rate(update.tip().hash).await.unwrap();
                            tracing::info!("Last block average fee rate: {:#}", avg_fee_rate);
                            break;
                        },
                        Event::BlocksDisconnected { accepted: _, disconnected: _} => {
                            tracing::warn!("Some blocks were reorganized")
                        },
                        _ => (),
                    }
                }
            }
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
        }
    }
    let _ = requester.shutdown();
    tracing::info!("Shutting down");
}
