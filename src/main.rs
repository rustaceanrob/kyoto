use light_client::node::node::Node;

#[tokio::main]
async fn main() {
    // console_subscriber::init();
    let mut node = Node::new(bitcoin::Network::Signet).unwrap();
    node.run().await.unwrap();
}
