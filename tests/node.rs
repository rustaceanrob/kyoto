use std::{
    collections::HashSet,
    net::{IpAddr, Ipv4Addr},
    time::Duration,
};

use bitcoin::ScriptBuf;
use bitcoincore_rpc::RpcApi;
use kyoto::node::{client::Client, node::Node};

const RPC_USER: &str = "test";
const RPC_PASSWORD: &str = "kyoto";
const WALLET: &str = "test_kyoto";
const HOST: &str = "http://localhost:18443";
const PORT: u16 = 18444;
const NUM_BLOCKS: u64 = 10;

fn initialize_client() -> Result<bitcoincore_rpc::Client, bitcoincore_rpc::Error> {
    let rpc = bitcoincore_rpc::Client::new(
        HOST,
        bitcoincore_rpc::Auth::UserPass(RPC_USER.into(), RPC_PASSWORD.into()),
    )?;
    // Do a call that will only fail if we are not connected to RPC.
    let _ = rpc.get_best_block_hash()?;
    match rpc.load_wallet(WALLET) {
        Ok(_) => Ok(rpc),
        Err(_) => {
            rpc.create_wallet(WALLET, None, None, None, None)?;
            Ok(rpc)
        }
    }
}

async fn new_node(addrs: HashSet<ScriptBuf>) -> (Node, Client) {
    let host = (IpAddr::from(Ipv4Addr::new(0, 0, 0, 0)), PORT);
    let builder = kyoto::node::builder::NodeBuilder::new(bitcoin::Network::Regtest);
    let (node, client) = builder
        .add_peers(vec![host])
        .add_scripts(addrs)
        .build_with_databases((), ())
        .await;
    (node, client)
}

#[tokio::test]
async fn reorg() {
    let rpc_result = initialize_client();
    // If we can't fetch the genesis block then bitcoind is not running. Just exit.
    if let Err(_) = rpc_result {
        return;
    }
    let rpc = rpc_result.unwrap();
    let miner = rpc.get_new_address(None, None).unwrap().assume_checked();
    rpc.generate_to_address(NUM_BLOCKS, &miner).unwrap();
    let best = rpc.get_best_block_hash().unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;
    // Make sure we sync up to the tip under usual conditions.
    let mut scripts = HashSet::new();
    let other = rpc.get_new_address(None, None).unwrap().assume_checked();
    scripts.insert(other.into());
    let (mut node, mut client) = new_node(scripts.clone()).await;
    tokio::task::spawn(async move { node.run().await });
    let (mut sender, mut recv) = client.split();
    while let Ok(message) = recv.recv().await {
        match message {
            kyoto::node::messages::NodeMessage::Dialog(d) => println!("{d}"),
            kyoto::node::messages::NodeMessage::Warning(e) => println!("{e}"),
            kyoto::node::messages::NodeMessage::Synced(update) => {
                assert_eq!(update.tip().hash, best);
                break;
            }
            _ => {}
        }
    }
    rpc.invalidate_block(&best).unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
    rpc.generate_to_address(2, &miner).unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
    let best = rpc.get_best_block_hash().unwrap();
    while let Ok(message) = recv.recv().await {
        match message {
            kyoto::node::messages::NodeMessage::Dialog(d) => println!("{d}"),
            kyoto::node::messages::NodeMessage::Warning(e) => println!("{e}"),
            kyoto::node::messages::NodeMessage::Synced(update) => {
                assert_eq!(update.tip().hash, best);
                sender.shutdown().await.unwrap();
                break;
            }
            _ => {}
        }
    }
    client.shutdown().await.unwrap();
    rpc.stop().unwrap();
}
