#[allow(unused)]
use bitcoin::BlockHash;
use light_client::node::node_messages::NodeMessage;
#[allow(unused)]
use light_client::{chain::checkpoints::HeaderCheckpoint, node::builder::NodeBuilder};
#[allow(unused)]
use std::{
    net::{IpAddr, Ipv4Addr},
    str::FromStr,
};

#[tokio::main]
async fn main() {
    // console_subscriber::init();
    let address_1 = bitcoin::Address::from_str("tb1q9pvjqz5u5sdgpatg3wn0ce438u5cyv85lly0pc")
        .unwrap()
        .require_network(bitcoin::Network::Signet)
        .unwrap();
    let address_2 = bitcoin::Address::from_str("tb1qjcpcmt7heskyqys2pw7ajj9xgz2dlz83keg3j3")
        .unwrap()
        .require_network(bitcoin::Network::Signet)
        .unwrap();
    let mut addresses = vec![address_1];
    for _ in 0..99 {
        addresses.push(address_2.clone())
    }
    let peer = IpAddr::V4(Ipv4Addr::new(170, 75, 163, 219));
    let peer_2 = IpAddr::V4(Ipv4Addr::new(23, 137, 57, 100));
    let builder = NodeBuilder::new(bitcoin::Network::Signet);
    let (mut node, mut client) = builder
        // .add_peers(vec![(pref_peer, 18444)])
        .anchor_checkpoint(HeaderCheckpoint {
            height: 197_652,
            hash: BlockHash::from_str(
                "0000006067eb002b26cf8411eae04082435bfce44144831938b4b24abcd1fca3",
            )
            .unwrap(),
        })
        .add_peers(vec![(peer, 38333), (peer_2, 38333)])
        .add_scripts(addresses)
        .num_required_peers(2)
        .build_node()
        .await;
    let _ = tokio::task::spawn(async move { node.run().await });
    let (mut sender, receiver) = client.split();
    loop {
        if let Some(message) = receiver.recv().await {
            match message {
                NodeMessage::Dialog(d) => println!("\x1b[32mInfo\x1b[0m {}", d),
                NodeMessage::Warning(e) => println!("\x1b[93mWarn\x1b[0m {}", e),
                NodeMessage::Transaction(t) => drop(t),
                NodeMessage::Block(b) => drop(b),
                NodeMessage::Synced(tip) => {
                    println!(
                        "\x1b[32mInfo\x1b[0m Synced chain up to height {} with hash {}",
                        tip.height,
                        tip.hash.to_string()
                    );
                    break;
                }
            }
        }
    }
    let _ = sender.shutdown().await;
    println!("Shutting down");
}
