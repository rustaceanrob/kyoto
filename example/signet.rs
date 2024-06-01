use bitcoin::BlockHash;
use kyoto_light_client::node::node_messages::NodeMessage;
use kyoto_light_client::{chain::checkpoints::HeaderCheckpoint, node::builder::NodeBuilder};
use std::{
    net::{IpAddr, Ipv4Addr},
    str::FromStr,
};

#[tokio::main]
async fn main() {
    // Enable logging
    let subscriber = tracing_subscriber::FmtSubscriber::new();
    tracing::subscriber::set_global_default(subscriber).unwrap();
    // Add Bitcoin scripts to scan the blockchain for
    let address = bitcoin::Address::from_str("tb1q9pvjqz5u5sdgpatg3wn0ce438u5cyv85lly0pc")
        .unwrap()
        .require_network(bitcoin::Network::Signet)
        .unwrap();
    let addresses = vec![address];
    // Add preferred peers to connect to
    let peer = IpAddr::V4(Ipv4Addr::new(170, 75, 163, 219));
    let peer_2 = IpAddr::V4(Ipv4Addr::new(23, 137, 57, 100));
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
        .num_required_peers(2)
        // Create the node and client
        .build_node()
        .await;
    // Check if the node is running. Another part of the program may be giving us the node.
    if !node.is_running() {
        let _ = tokio::task::spawn(async move { node.run().await }).await;
    }
    // Split the client into components that send messages and listen to messages.
    // With this construction, different parts of the program can take ownership of
    // specific tasks.
    let (mut sender, receiver) = client.split();
    // Continually listen for events until the node is synced to its peers.
    loop {
        if let Some(message) = receiver.recv().await {
            match message {
                NodeMessage::Dialog(d) => tracing::info!("{}", d),
                NodeMessage::Warning(e) => tracing::warn!("{}", e),
                NodeMessage::Transaction(t) => drop(t),
                NodeMessage::Block(b) => drop(b),
                NodeMessage::Synced(tip) => {
                    tracing::info!("Synced chain up to block {}", tip.height,);
                    tracing::info!("Chain tip: {}", tip.hash.to_string(),);
                    break;
                }
            }
        }
    }
    let _ = sender.shutdown().await;
    tracing::info!("Shutting down");
}
