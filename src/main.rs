use bitcoin::Network;
use light_client::node::node::Node;

#[tokio::main]
async fn main() {
    let mut node = Node::new(bitcoin::Network::Signet);
    node.run().await.unwrap();
    // let response = vec![
    //     72, 84, 84, 80, 47, 49, 46, 49, 32, 52, 48, 48, 32, 66, 97, 100, 32, 82, 101, 113, 117,
    //     101, 115, 116,
    // ];
    // println!("{}", response.len());
    // println!("{:?}", response);
    // println!("{}", hex::encode(response));
    // println!("Bitcoin magic {:?}", Network::Bitcoin.magic().to_bytes());
    // println!("Testnet magic {:?}", Network::Testnet.magic().to_bytes());
    // println!("Signet magic {:?}", Network::Signet.magic().to_bytes());
    // println!("Regtest magic {:?}", Network::Regtest.magic().to_bytes());
}
