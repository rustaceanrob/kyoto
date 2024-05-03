use std::sync::{Arc, Mutex};

use bitcoin::{
    block::Header, constants::genesis_block, network, params::Params, BlockHash, Network,
};
use log::info;
use rand::{seq::sample_slice, thread_rng, Rng};
use tokio::sync::mpsc;

use crate::{
    headers::header_chain::HeaderChain,
    peers::{dns::Dns, peer::Peer},
};

use super::channel_messages::{GetHeaderConfig, MainThreadMessage, PeerMessage, PeerThreadMessage};

pub enum NodeState {
    Behind,
    HeadersSynced,
    FilterHeadersSynced,
    FiltersSynced,
}

pub struct Node {
    state: Arc<NodeState>,
    header_chain: Arc<Mutex<HeaderChain>>,
    // fill filter headers, etc
    best_known_height: u32,
    best_known_hash: Option<BlockHash>,
    network: Network,
}

impl Node {
    pub fn new(network: Network) -> Self {
        let state = Arc::new(NodeState::Behind);
        let genesis = match network {
            Network::Bitcoin => panic!("unimplemented"),
            Network::Testnet => panic!("unimplemented"),
            Network::Signet => genesis_block(Params::new(network)).header,
            Network::Regtest => panic!("unimplemented"),
            _ => unreachable!(),
        };
        let header_chain = Arc::new(Mutex::new(HeaderChain::new(genesis, &network)));
        let best_known_height = 0;
        let best_known_hash = None;
        Self {
            state,
            header_chain,
            best_known_height,
            best_known_hash,
            network,
        }
    }
    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error + '_>> {
        println!("Starting node");
        let dns_peers = Dns::bootstrap(self.network).await.unwrap();
        let mut rng = thread_rng();
        let ip = sample_slice(&mut rng, &dns_peers, 1)[0];
        println!("Connecting to peer: {}", ip);
        let (mtx, mut mrx) = mpsc::channel::<PeerThreadMessage>(32);
        let (ptx, prx) = mpsc::channel::<MainThreadMessage>(32);
        let mut peer = Peer::new(rng.next_u32(), ip, None, self.network, mtx, prx);
        tokio::spawn(async move { peer.connect().await });
        loop {
            if let Some(thd) = mrx.recv().await {
                match thd.message {
                    PeerMessage::Version(_) => {
                        // add the peer version to our tried db
                        println!("Requesting addresses from new peer");
                        let _ = ptx.send(MainThreadMessage::GetAddr).await;
                        continue;
                    }
                    PeerMessage::Addr(addresses) => {
                        for addr in addresses {
                            println!("Received addresses");
                            println!("{}", addr.ip.to_string())
                        }
                    }
                    PeerMessage::Headers(headers) => {
                        let mut guard = self.header_chain.lock()?;
                        guard.sync_chain(headers).await?;
                        let next_headers = GetHeaderConfig {
                            locators: vec![guard.tip().block_hash()],
                            stop_hash: None,
                        };
                        ptx.send(MainThreadMessage::GetHeaders(next_headers))
                            .await?;
                    }
                    PeerMessage::Disconnect => return Ok(()),
                    _ => continue,
                }
            }
        }
    }
}
