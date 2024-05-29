use std::{
    collections::HashMap,
    net::IpAddr,
    time::{SystemTime, UNIX_EPOCH},
};

use bitcoin::{p2p::ServiceFlags, Network};
use rand::{rngs::StdRng, seq::IteratorRandom, SeedableRng};
use tokio::{
    sync::mpsc::{self, Sender},
    task::JoinHandle,
};

use crate::{
    peers::peer::{Peer, PeerError},
    prelude::Median,
};

use super::channel_messages::{MainThreadMessage, PeerThreadMessage};

pub(crate) struct ManagedPeer {
    net_time: i64,
    service_flags: Option<ServiceFlags>,
    ptx: Sender<MainThreadMessage>,
    handle: JoinHandle<Result<(), PeerError>>,
}

pub(crate) struct PeerMap {
    num_peers: u32,
    heights: HashMap<u32, u32>,
    network: Network,
    mtx: Sender<PeerThreadMessage>,
    map: HashMap<u32, ManagedPeer>,
}

impl PeerMap {
    pub fn new(mtx: Sender<PeerThreadMessage>, network: Network) -> Self {
        Self {
            num_peers: 0,
            heights: HashMap::new(),
            network,
            mtx,
            map: HashMap::new(),
        }
    }

    pub async fn clean(&mut self) {
        self.map.retain(|_, peer| !peer.handle.is_finished());
        self.heights.retain(|peer, _| self.map.contains_key(peer));
    }

    pub fn live(&mut self) -> usize {
        self.map
            .values()
            .filter(|peer| !peer.handle.is_finished())
            .count()
    }

    pub fn num_cpf_peers(&mut self) -> usize {
        self.map
            .values()
            .filter(|peer| !peer.handle.is_finished())
            .filter(|peer| {
                if let Some(flags) = peer.service_flags {
                    flags.has(ServiceFlags::COMPACT_FILTERS)
                } else {
                    false
                }
            })
            .count()
    }

    pub fn median_time_adjustment(&self) -> i64 {
        if self.map.values().len() > 0 {
            let mut time_offsets: Vec<i64> = self.map.values().map(|peer| peer.net_time).collect();
            return time_offsets.median().unwrap();
        }
        0
    }

    pub fn set_offset(&mut self, peer: u32, time: i64) {
        if let Some(peer) = self.map.get_mut(&peer) {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time went backwards")
                .as_secs();
            peer.net_time = time - now as i64;
        }
    }

    pub async fn dispatch(&mut self, ip: IpAddr, port: Option<u16>) {
        let (ptx, prx) = mpsc::channel::<MainThreadMessage>(32);
        let peer_num = self.num_peers + 1;
        self.num_peers = peer_num;
        let mut peer = Peer::new(peer_num, ip, port, self.network, self.mtx.clone(), prx);
        let handle = tokio::spawn(async move { peer.connect().await });
        self.map.insert(
            peer_num,
            ManagedPeer {
                service_flags: None,
                net_time: 0,
                ptx,
                handle,
            },
        );
    }

    pub fn set_services(&mut self, nonce: u32, flags: ServiceFlags) {
        if let Some(peer) = self.map.get_mut(&nonce) {
            peer.service_flags = Some(flags)
        }
    }

    pub fn set_height(&mut self, nonce: u32, height: u32) {
        self.heights.insert(nonce, height);
    }

    pub fn add_one_height(&mut self, nonce: u32) {
        if let Some(height) = self.heights.get(&nonce) {
            self.heights.insert(nonce, height + 1);
        }
    }

    pub fn best_height(&self) -> Option<&u32> {
        self.heights.values().max()
    }

    pub async fn send_message(&mut self, nonce: u32, message: MainThreadMessage) {
        if let Some(peer) = self.map.get(&nonce) {
            let _ = peer.ptx.send(message).await;
        }
    }

    pub async fn broadcast(&mut self, message: MainThreadMessage) {
        let active = self.map.values().filter(|peer| !peer.handle.is_finished());
        for peer in active {
            let _ = peer.ptx.send(message.clone()).await;
        }
    }

    pub async fn send_random(&mut self, message: MainThreadMessage) {
        let mut rng = StdRng::from_entropy();
        if let Some((_, peer)) = self.map.iter().choose(&mut rng) {
            let _ = peer.ptx.send(message.clone()).await;
        }
    }
}
