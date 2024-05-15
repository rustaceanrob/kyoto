use std::net::{IpAddr, Ipv4Addr};

use light_client::node::node::Node;

#[tokio::main]
async fn main() {
    console_subscriber::init();
    let pref_peer = IpAddr::V4(Ipv4Addr::new(135, 181, 215, 237));
    let mut node = Node::new(bitcoin::Network::Signet, Some(vec![(pref_peer, 38333)])).unwrap();
    node.run().await.unwrap();
}
