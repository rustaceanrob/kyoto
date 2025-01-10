use std::{
    collections::HashSet,
    net::{IpAddr, Ipv4Addr, SocketAddrV4},
    path::PathBuf,
    str::FromStr,
    time::Duration,
};

use bitcoin::{address::NetworkChecked, ScriptBuf};
use corepc_node::serde_json;
use corepc_node::{anyhow, exe_path};
use kyoto::{
    chain::checkpoints::HeaderCheckpoint,
    core::{
        client::{Client, Receiver},
        node::Node,
    },
    BlockHash, Event, Log, NodeState, ServiceFlags, SqliteHeaderDb, SqlitePeerDb, TrustedPeer,
};
use tokio::sync::mpsc::UnboundedReceiver;

// Start the bitcoin daemon either through an environment variable or by download
fn start_bitcoind(with_v2_transport: bool) -> anyhow::Result<(corepc_node::Node, SocketAddrV4)> {
    let path = exe_path()?;
    let mut conf = corepc_node::Conf::default();
    conf.p2p = corepc_node::P2P::Yes;
    conf.args.push("--txindex");
    conf.args.push("--blockfilterindex");
    conf.args.push("--peerblockfilters");
    conf.args.push("--rest=1");
    conf.args.push("--server=1");
    conf.args.push("--listen=1");
    conf.tmpdir = Some(tempfile::TempDir::new().unwrap().into_path());
    if with_v2_transport {
        conf.args.push("--v2transport=1")
    } else {
        conf.args.push("--v2transport=0");
    }
    let bitcoind = corepc_node::Node::with_conf(path, &conf)?;
    let socket_addr = bitcoind.params.p2p_socket.unwrap();
    Ok((bitcoind, socket_addr))
}

async fn new_node(addrs: HashSet<ScriptBuf>, socket_addr: SocketAddrV4) -> (Node<(), ()>, Client) {
    let host = (IpAddr::V4(*socket_addr.ip()), Some(socket_addr.port()));
    let builder = kyoto::core::builder::NodeBuilder::new(bitcoin::Network::Regtest);
    let (node, client) = builder
        .add_peer(host)
        .add_scripts(addrs)
        .build_with_databases((), ());
    (node, client)
}

async fn new_node_sql(
    addrs: HashSet<ScriptBuf>,
    socket_addr: SocketAddrV4,
    tempdir_path: PathBuf,
) -> (Node<SqliteHeaderDb, SqlitePeerDb>, Client) {
    let host = (IpAddr::V4(*socket_addr.ip()), Some(socket_addr.port()));
    let mut trusted: TrustedPeer = host.into();
    trusted.set_services(ServiceFlags::P2P_V2);
    let builder = kyoto::core::builder::NodeBuilder::new(bitcoin::Network::Regtest);
    let (node, client) = builder
        .add_peer(host)
        .add_scripts(addrs)
        .add_data_dir(tempdir_path)
        .build_node()
        .unwrap();
    (node, client)
}

async fn new_node_anchor_sql(
    addrs: HashSet<ScriptBuf>,
    checkpoint: HeaderCheckpoint,
    socket_addr: SocketAddrV4,
    tempdir_path: PathBuf,
) -> (Node<SqliteHeaderDb, SqlitePeerDb>, Client) {
    let addr = (IpAddr::V4(*socket_addr.ip()), Some(socket_addr.port()));
    let mut trusted: TrustedPeer = addr.into();
    trusted.set_services(ServiceFlags::P2P_V2);
    let builder = kyoto::core::builder::NodeBuilder::new(bitcoin::Network::Regtest);
    let (node, client) = builder
        .add_peer(trusted)
        .add_scripts(addrs)
        .add_data_dir(tempdir_path)
        .anchor_checkpoint(checkpoint)
        .build_node()
        .unwrap();
    (node, client)
}

fn num_blocks(rpc: &corepc_node::Client) -> i64 {
    rpc.get_blockchain_info().unwrap().blocks
}

fn best_hash(rpc: &corepc_node::Client) -> BlockHash {
    rpc.get_best_block_hash().unwrap().block_hash().unwrap()
}

async fn mine_blocks(
    rpc: &corepc_node::Client,
    miner: &bitcoin::Address<NetworkChecked>,
    num_blocks: usize,
    time: u64,
) {
    rpc.generate_to_address(num_blocks, miner).unwrap();
    tokio::time::sleep(Duration::from_secs(time)).await;
}

async fn invalidate_block(rpc: &corepc_node::Client, hash: &bitcoin::BlockHash) {
    let value = serde_json::to_value(hash).unwrap();
    rpc.call::<()>("invalidateblock", &[value]).unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
}

async fn sync_assert(
    best: &bitcoin::BlockHash,
    channel: &mut UnboundedReceiver<Event>,
    log: &mut Receiver<Log>,
) {
    loop {
        tokio::select! {
            event = channel.recv() => {
                if let Some(Event::Synced(update)) = event {
                    assert_eq!(update.tip().hash, *best);
                    println!("Correct sync");
                    break;
                };
            }
            log = log.recv() => {
                if let Some(log) = log {
                    match log {
                        Log::Dialog(d) => println!("{d}"),
                        Log::Warning(warning) => println!("{warning}"),
                        _ => (),
                    }
                }
            }
        }
    }
}

#[tokio::test]
async fn test_reorg() {
    let rpc_result = start_bitcoind(false);
    // If we can't fetch the genesis block then bitcoind is not running. Just exit.
    if rpc_result.is_err() {
        println!("Bitcoin Core is not running. Skipping this test...");
        return;
    }
    let (bitcoind, socket_addr) = rpc_result.unwrap();
    let rpc = &bitcoind.client;
    // Mine some blocks
    let miner = rpc.new_address().unwrap();
    mine_blocks(rpc, &miner, 10, 1).await;
    let best = best_hash(rpc);
    // Build and run a node
    let mut scripts = HashSet::new();
    let other = rpc.new_address().unwrap();
    scripts.insert(other.into());
    let (node, client) = new_node(scripts.clone(), socket_addr).await;
    tokio::task::spawn(async move { node.run().await });
    let Client {
        sender,
        log_rx: mut log,
        event_rx: mut channel,
    } = client;
    sync_assert(&best, &mut channel, &mut log).await;
    // Reorganize the blocks
    let old_best = best;
    let old_height = num_blocks(rpc);
    invalidate_block(rpc, &best).await;
    mine_blocks(rpc, &miner, 2, 1).await;
    let best = best_hash(rpc);
    // Make sure the reorg was caught
    while let Some(message) = channel.recv().await {
        match message {
            kyoto::core::messages::Event::BlocksDisconnected(blocks) => {
                assert_eq!(blocks.len(), 1);
                assert_eq!(blocks.first().unwrap().header.block_hash(), old_best);
                assert_eq!(old_height as u32, blocks.first().unwrap().height);
            }
            kyoto::core::messages::Event::Synced(update) => {
                assert_eq!(update.tip().hash, best);
                sender.shutdown().await.unwrap();
                break;
            }
            _ => {}
        }
    }
    sender.shutdown().await.unwrap();
    rpc.stop().unwrap();
}

#[tokio::test]
async fn test_mine_after_reorg() {
    let rpc_result = start_bitcoind(false);
    // If we can't fetch the genesis block then bitcoind is not running. Just exit.
    if rpc_result.is_err() {
        println!("Bitcoin Core is not running. Skipping this test...");
        return;
    }
    let (bitcoind, socket_addr) = rpc_result.unwrap();
    let rpc = &bitcoind.client;
    // Mine some blocks
    let miner = rpc.new_address().unwrap();
    mine_blocks(rpc, &miner, 10, 1).await;
    let best = best_hash(rpc);
    // Build and run a node
    let mut scripts = HashSet::new();
    let other = rpc.new_address().unwrap();
    scripts.insert(other.into());
    let (node, client) = new_node(scripts.clone(), socket_addr).await;
    tokio::task::spawn(async move { node.run().await });
    let Client {
        sender,
        log_rx: mut log,
        event_rx: mut channel,
    } = client;
    sync_assert(&best, &mut channel, &mut log).await;
    // Reorganize the blocks
    let old_best = best;
    let old_height = num_blocks(rpc);
    let fetched_header = sender.get_header(10).await.unwrap();
    assert_eq!(old_best, fetched_header.block_hash());
    invalidate_block(rpc, &best).await;
    mine_blocks(rpc, &miner, 2, 1).await;
    let best = best_hash(rpc);
    // Make sure the reorg was caught
    while let Some(message) = channel.recv().await {
        match message {
            kyoto::core::messages::Event::BlocksDisconnected(blocks) => {
                assert_eq!(blocks.len(), 1);
                assert_eq!(blocks.first().unwrap().header.block_hash(), old_best);
                assert_eq!(old_height as u32, blocks.first().unwrap().height);
            }
            kyoto::core::messages::Event::Synced(update) => {
                assert_eq!(update.tip().hash, best);
                break;
            }
            _ => {}
        }
    }
    mine_blocks(rpc, &miner, 2, 1).await;
    let best = best_hash(rpc);
    sync_assert(&best, &mut channel, &mut log).await;
    sender.shutdown().await.unwrap();
    rpc.stop().unwrap();
}

#[tokio::test]
async fn test_various_client_methods() {
    let rpc_result = start_bitcoind(false);
    // If we can't fetch the genesis block then bitcoind is not running. Just exit.
    if rpc_result.is_err() {
        println!("Bitcoin Core is not running. Skipping this test...");
        return;
    }
    let (bitcoind, socket_addr) = rpc_result.unwrap();
    let rpc = &bitcoind.client;
    // Mine a lot of blocks
    let miner = rpc.new_address().unwrap();
    mine_blocks(rpc, &miner, 500, 15).await;
    let best = best_hash(rpc);
    let mut scripts = HashSet::new();
    let other = rpc.new_address().unwrap();
    scripts.insert(other.into());
    let (node, client) = new_node(scripts.clone(), socket_addr).await;
    tokio::task::spawn(async move { node.run().await });
    let Client {
        sender,
        log_rx: mut log,
        event_rx: mut channel,
    } = client;
    sync_assert(&best, &mut channel, &mut log).await;
    let batch = sender.get_header_range(10_000..10_002).await.unwrap();
    let _ = sender.broadcast_min_feerate().await.unwrap();
    assert!(batch.is_empty());
    sender.shutdown().await.unwrap();
    rpc.stop().unwrap();
}

#[tokio::test]
async fn test_sql_reorg() {
    let rpc_result = start_bitcoind(true);
    // If we can't fetch the genesis block then bitcoind is not running. Just exit.
    if rpc_result.is_err() {
        println!("Bitcoin Core is not running. Skipping this test...");
        return;
    }
    let (bitcoind, socket_addr) = rpc_result.unwrap();
    let rpc = &bitcoind.client;
    let tempdir = tempfile::TempDir::new().unwrap().into_path();
    // Mine some blocks.
    let miner = rpc.new_address().unwrap();
    mine_blocks(rpc, &miner, 10, 1).await;
    let best = best_hash(rpc);
    let mut scripts = HashSet::new();
    let other = rpc.new_address().unwrap();
    scripts.insert(other.into());
    let (node, client) = new_node_sql(scripts.clone(), socket_addr, tempdir.clone()).await;
    tokio::task::spawn(async move { node.run().await });
    let Client {
        sender,
        log_rx: mut log,
        event_rx: mut channel,
    } = client;
    sync_assert(&best, &mut channel, &mut log).await;
    let batch = sender.get_header_range(0..10).await.unwrap();
    assert!(!batch.is_empty());
    sender.shutdown().await.unwrap();
    // Reorganize the blocks
    let old_best = best;
    let old_height = num_blocks(rpc);
    invalidate_block(rpc, &best).await;
    mine_blocks(rpc, &miner, 2, 1).await;
    let best = best_hash(rpc);
    // Spin up the node on a cold start
    let (node, client) = new_node_sql(scripts.clone(), socket_addr, tempdir.clone()).await;
    tokio::task::spawn(async move { node.run().await });
    let Client {
        sender,
        log_rx: _,
        event_rx: mut channel,
    } = client;
    // Make sure the reorganization is caught after a cold start
    while let Some(message) = channel.recv().await {
        match message {
            kyoto::core::messages::Event::BlocksDisconnected(blocks) => {
                assert_eq!(blocks.len(), 1);
                assert_eq!(blocks.first().unwrap().header.block_hash(), old_best);
                assert_eq!(old_height as u32, blocks.first().unwrap().height);
            }
            kyoto::core::messages::Event::Synced(update) => {
                println!("Done");
                assert_eq!(update.tip().hash, best);
                break;
            }
            _ => {}
        }
    }
    sender.shutdown().await.unwrap();
    // Mine more blocks
    mine_blocks(rpc, &miner, 2, 1).await;
    let best = best_hash(rpc);
    // Make sure the node does not have any corrupted headers
    let (node, client) = new_node_sql(scripts.clone(), socket_addr, tempdir).await;
    tokio::task::spawn(async move { node.run().await });
    let Client {
        sender,
        log_rx: mut log,
        event_rx: mut channel,
    } = client;
    // The node properly syncs after persisting a reorg
    sync_assert(&best, &mut channel, &mut log).await;
    sender.shutdown().await.unwrap();
    rpc.stop().unwrap();
}

#[tokio::test]
async fn test_two_deep_reorg() {
    let rpc_result = start_bitcoind(true);
    // If we can't fetch the genesis block then bitcoind is not running. Just exit.
    if rpc_result.is_err() {
        println!("Bitcoin Core is not running. Skipping this test...");
        return;
    }
    let (bitcoind, socket_addr) = rpc_result.unwrap();
    let rpc = &bitcoind.client;
    let tempdir = tempfile::TempDir::new().unwrap().into_path();
    // Mine some blocks.
    let miner = rpc.new_address().unwrap();
    mine_blocks(rpc, &miner, 10, 1).await;
    let best = best_hash(rpc);
    let mut scripts = HashSet::new();
    let other = rpc.new_address().unwrap();
    scripts.insert(other.into());
    let (node, client) = new_node_sql(scripts.clone(), socket_addr, tempdir.clone()).await;
    tokio::task::spawn(async move { node.run().await });
    let Client {
        sender,
        log_rx: mut log,
        event_rx: mut channel,
    } = client;
    sync_assert(&best, &mut channel, &mut log).await;
    sender.shutdown().await.unwrap();
    // Reorganize the blocks
    let old_height = num_blocks(rpc);
    let old_best = best;
    invalidate_block(rpc, &best).await;
    let best = best_hash(rpc);
    invalidate_block(rpc, &best).await;
    mine_blocks(rpc, &miner, 3, 1).await;
    let best = best_hash(rpc);
    // Make sure the reorganization is caught after a cold start
    let (node, client) = new_node_sql(scripts.clone(), socket_addr, tempdir.clone()).await;
    tokio::task::spawn(async move { node.run().await });
    let Client {
        sender,
        log_rx: _,
        event_rx: mut channel,
    } = client;
    while let Some(message) = channel.recv().await {
        match message {
            kyoto::core::messages::Event::BlocksDisconnected(blocks) => {
                assert_eq!(blocks.len(), 2);
                assert_eq!(blocks.last().unwrap().header.block_hash(), old_best);
                assert_eq!(old_height as u32, blocks.last().unwrap().height);
            }
            kyoto::core::messages::Event::Synced(update) => {
                println!("Done");
                assert_eq!(update.tip().hash, best);
                break;
            }
            _ => {}
        }
    }
    sender.shutdown().await.unwrap();
    // Mine more blocks
    mine_blocks(rpc, &miner, 2, 1).await;
    let best = best_hash(rpc);
    // Make sure the node does not have any corrupted headers
    let (node, client) = new_node_sql(scripts.clone(), socket_addr, tempdir).await;
    tokio::task::spawn(async move { node.run().await });
    let Client {
        sender,
        log_rx: mut log,
        event_rx: mut channel,
    } = client;
    // The node properly syncs after persisting a reorg
    sync_assert(&best, &mut channel, &mut log).await;
    sender.shutdown().await.unwrap();
    rpc.stop().unwrap();
}

#[tokio::test]
async fn test_sql_stale_anchor() {
    let rpc_result = start_bitcoind(true);
    // If we can't fetch the genesis block then bitcoind is not running. Just exit.
    if rpc_result.is_err() {
        println!("Bitcoin Core is not running. Skipping this test...");
        return;
    }
    let (bitcoind, socket_addr) = rpc_result.unwrap();
    let rpc = &bitcoind.client;
    let tempdir = tempfile::TempDir::new().unwrap().into_path();
    let miner = rpc.new_address().unwrap();
    mine_blocks(rpc, &miner, 17, 3).await;
    let best = best_hash(rpc);
    let mut scripts = HashSet::new();
    let other = rpc.new_address().unwrap();
    scripts.insert(other.into());
    let (node, client) = new_node_sql(scripts.clone(), socket_addr, tempdir.clone()).await;
    tokio::task::spawn(async move { node.run().await });
    let Client {
        sender,
        log_rx: mut log,
        event_rx: mut channel,
    } = client;
    sync_assert(&best, &mut channel, &mut log).await;
    sender.shutdown().await.unwrap();
    // Reorganize the blocks
    let old_best = best;
    let old_height = num_blocks(rpc);
    invalidate_block(rpc, &best).await;
    mine_blocks(rpc, &miner, 2, 1).await;
    let best = best_hash(rpc);
    // Spin up the node on a cold start with a stale tip
    let (node, client) = new_node_anchor_sql(
        scripts.clone(),
        HeaderCheckpoint::new(old_height as u32, old_best),
        socket_addr,
        tempdir.clone(),
    )
    .await;
    tokio::task::spawn(async move { node.run().await });
    let Client {
        sender,
        log_rx: _,
        event_rx: mut channel,
    } = client;
    // Ensure SQL is able to catch the fork by loading in headers from the database
    while let Some(message) = channel.recv().await {
        match message {
            kyoto::core::messages::Event::BlocksDisconnected(blocks) => {
                assert_eq!(blocks.len(), 1);
                assert_eq!(blocks.first().unwrap().header.block_hash(), old_best);
                assert_eq!(old_height as u32, blocks.first().unwrap().height);
            }
            kyoto::core::messages::Event::Synced(update) => {
                println!("Done");
                assert_eq!(update.tip().hash, best);
                break;
            }
            _ => {}
        }
    }
    sender.shutdown().await.unwrap();
    // Don't do anything, but reload the node from the checkpoint
    let cp = best_hash(rpc);
    let old_height = num_blocks(rpc);
    let best = best_hash(rpc);
    // Make sure the node does not have any corrupted headers
    let (node, client) = new_node_anchor_sql(
        scripts.clone(),
        HeaderCheckpoint::new(old_height as u32, cp),
        socket_addr,
        tempdir.clone(),
    )
    .await;
    tokio::task::spawn(async move { node.run().await });
    let Client {
        sender,
        log_rx: mut log,
        event_rx: mut channel,
    } = client;
    // The node properly syncs after persisting a reorg
    sync_assert(&best, &mut channel, &mut log).await;
    sender.shutdown().await.unwrap();
    // Mine more blocks and reload from the checkpoint
    let cp = best_hash(rpc);
    let old_height = num_blocks(rpc);
    mine_blocks(rpc, &miner, 2, 1).await;
    let best = best_hash(rpc);
    // Make sure the node does not have any corrupted headers
    let (node, client) = new_node_anchor_sql(
        scripts.clone(),
        HeaderCheckpoint::new(old_height as u32, cp),
        socket_addr,
        tempdir,
    )
    .await;
    tokio::task::spawn(async move { node.run().await });
    let Client {
        sender,
        log_rx: mut log,
        event_rx: mut channel,
    } = client;
    // The node properly syncs after persisting a reorg
    sync_assert(&best, &mut channel, &mut log).await;
    sender.shutdown().await.unwrap();
    rpc.stop().unwrap();
}

#[tokio::test]
#[allow(clippy::collapsible_match)]
async fn test_halting_works() {
    let rpc_result = start_bitcoind(true);
    // If we can't fetch the genesis block then bitcoind is not running. Just exit.
    if rpc_result.is_err() {
        println!("Bitcoin Core is not running. Skipping this test...");
        return;
    }
    let (bitcoind, socket_addr) = rpc_result.unwrap();
    let rpc = &bitcoind.client;

    let miner = rpc.new_address().unwrap();
    mine_blocks(rpc, &miner, 10, 1).await;
    let best = best_hash(rpc);
    let mut scripts = HashSet::new();
    let other = rpc.new_address().unwrap();
    scripts.insert(other.into());

    let host = (IpAddr::V4(*socket_addr.ip()), Some(socket_addr.port()));
    let builder = kyoto::core::builder::NodeBuilder::new(bitcoin::Network::Regtest);
    let (node, client) = builder
        .add_peers(vec![host.into()])
        .add_scripts(scripts)
        .halt_filter_download()
        .build_with_databases((), ());

    tokio::task::spawn(async move { node.run().await });
    let Client {
        sender,
        log_rx: mut log,
        event_rx: mut channel,
    } = client;
    // Ensure SQL is able to catch the fork by loading in headers from the database
    while let Some(message) = log.recv().await {
        match message {
            Log::Dialog(d) => println!("{d}"),
            Log::Warning(warning) => println!("{warning}"),
            Log::StateChange(node_state) => {
                if let NodeState::FilterHeadersSynced = node_state {
                    println!("Sleeping for one second...");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    sender.continue_download().await.unwrap();
                    break;
                }
            }
            _ => (),
        }
    }
    while let Some(message) = channel.recv().await {
        if let kyoto::core::messages::Event::Synced(update) = message {
            println!("Done");
            assert_eq!(update.tip().hash, best);
            break;
        }
    }
    sender.shutdown().await.unwrap();
    rpc.stop().unwrap();
}

#[tokio::test]
async fn test_signet_syncs() {
    let address = bitcoin::Address::from_str("tb1q9pvjqz5u5sdgpatg3wn0ce438u5cyv85lly0pc")
        .unwrap()
        .require_network(bitcoin::Network::Signet)
        .unwrap()
        .into();
    let mut set = HashSet::new();
    set.insert(address);
    let host = (IpAddr::from(Ipv4Addr::new(68, 47, 229, 218)), None);
    let builder = kyoto::core::builder::NodeBuilder::new(bitcoin::Network::Signet);
    let (node, client) = builder
        .add_peer(host)
        .add_scripts(set)
        .build_with_databases((), ());
    tokio::task::spawn(async move { node.run().await });
    async fn print_and_sync(mut client: Client) {
        loop {
            tokio::select! {
                event = client.event_rx.recv() => {
                    if let Some(Event::Synced(update)) = event {
                        println!("Synced chain up to block {}", update.tip().height);
                        println!("Chain tip: {}", update.tip().hash);
                        break;
                    }
                }
                log = client.log_rx.recv() => {
                    if let Some(log) = log {
                        match log {
                            Log::Dialog(d) => println!("{d}"),
                            Log::Warning(warning) => tracing::warn!("{warning}"),
                            _ => (),
                        }
                    }
                }
            }
        }
    }
    let timeout = tokio::time::timeout(Duration::from_secs(180), print_and_sync(client)).await;
    assert!(timeout.is_ok());
}
