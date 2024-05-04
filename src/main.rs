use light_client::node::node::Node;

#[tokio::main]
async fn main() {
    let mut node = Node::new(bitcoin::Network::Signet);
    node.run().await.unwrap();
}
