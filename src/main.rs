#[allow(unused)]
use bitcoin::BlockHash;
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
    // let address_1 = bitcoin::Address::from_str("tb1q9pvjqz5u5sdgpatg3wn0ce438u5cyv85lly0pc")
    //     .unwrap()
    //     .require_network(bitcoin::Network::Signet)
    //     .unwrap();
    // let address_2 = bitcoin::Address::from_str("tb1qjcpcmt7heskyqys2pw7ajj9xgz2dlz83keg3j3")
    //     .unwrap()
    //     .require_network(bitcoin::Network::Signet)
    //     .unwrap();
    // let mut addresses = vec![address_1];
    // for _ in 0..99 {
    //     addresses.push(address_2.clone())
    // }
    let pref_peer = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
    let builder = NodeBuilder::new(bitcoin::Network::Regtest);
    let (mut node, mut client) = builder
        .add_peers(vec![(pref_peer, 18444)])
        .build_node()
        .await;
    let _ = tokio::task::spawn(async move { node.run().await });
    client.print_log_stream().await;
}
