use std::sync::{Arc, Mutex};

use bitcoin::{block::Header, constants::genesis_block, params::Params, BlockHash, Network};
use rand::{seq::sample_slice, thread_rng, Rng};
use thiserror::Error;
use tokio::sync::mpsc;

use crate::{
    headers::header_chain::{HeaderChain, HeaderSyncError},
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
            Network::Testnet => genesis_block(Params::new(network)).header,
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
        let dns_peers = Dns::bootstrap(self.network).await?;
        let mut rng = thread_rng();
        let ip = sample_slice(&mut rng, &dns_peers, 1)[0];
        println!("Connecting to peer: {}", ip);
        let (mtx, mut mrx) = mpsc::channel::<PeerThreadMessage>(32);
        let (ptx, prx) = mpsc::channel::<MainThreadMessage>(32);
        let mut peer = Peer::new(rng.next_u32(), ip, None, self.network, mtx, prx);
        tokio::spawn(async move { peer.connect().await });
        loop {
            // rehydrate on peers when lower than a threshold
            // try to update the state of our node periodically
            if let Some(peer_thread) = mrx.recv().await {
                match peer_thread.message {
                    PeerMessage::Version(_) => {
                        // add the peer version to our tried db
                        // add the node to a BTreeMap
                        // start asking for headers from where our tip is (add check if we are already caught up)
                        let guard = self
                            .header_chain
                            .lock()
                            .map_err(|_| MainThreadError::PoisonedGuard)?;
                        let next_headers = GetHeaderConfig {
                            // should be done a little smarter
                            locators: vec![guard.tip().block_hash()],
                            stop_hash: None,
                        };
                        let response = MainThreadMessage::GetHeaders(next_headers);
                        let _ = ptx.send(response).await;
                    }
                    PeerMessage::Addr(addresses) => {
                        for _addr in addresses {
                            // add peers to persistence
                        }
                    }
                    PeerMessage::Headers(headers) => match self.handle_headers(headers).await {
                        Ok(response) => {
                            let _ = ptx.send(response).await;
                            continue;
                        }
                        // remove node from the BTreeMap
                        Err(_) => continue,
                    },
                    PeerMessage::Disconnect => {
                        // remove the node from the BTreeMap
                        return Ok(());
                    }
                    _ => continue,
                }
            }
        }
    }

    async fn handle_headers(
        &mut self,
        headers: Vec<Header>,
    ) -> Result<MainThreadMessage, MainThreadError> {
        let mut guard = self
            .header_chain
            .lock()
            .map_err(|_| MainThreadError::PoisonedGuard)?;
        let next_headers = GetHeaderConfig {
            // should be done a little smarter
            locators: vec![guard.tip().block_hash()],
            stop_hash: None,
        };
        if let Err(e) = guard.sync_chain(headers).await {
            match e {
                HeaderSyncError::EmptyMessage => {
                    return Ok(MainThreadMessage::GetHeaders(next_headers))
                }
                _ => {
                    println!("{}", e.to_string());
                    return Ok(MainThreadMessage::Disconnect);
                }
            }
        }
        Ok(MainThreadMessage::GetHeaders(next_headers))
    }
}

#[derive(Error, Debug)]
pub enum MainThreadError {
    #[error("the lock acquired on the mutex may have left data in an indeterminant state")]
    PoisonedGuard,
}
