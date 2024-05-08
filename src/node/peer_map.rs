use std::{collections::BTreeMap, net::IpAddr};

use bitcoin::Network;
use rand::{thread_rng, RngCore};
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
    ptx: Sender<MainThreadMessage>,
    handle: JoinHandle<Result<(), PeerError>>,
}

pub(crate) struct PeerMap {
    network: Network,
    mtx: Sender<PeerThreadMessage>,
    map: BTreeMap<u32, ManagedPeer>,
}

impl PeerMap {
    pub fn new(mtx: Sender<PeerThreadMessage>, network: Network) -> Self {
        Self {
            network,
            mtx,
            map: BTreeMap::new(),
        }
    }

    pub fn clean(&mut self) {
        self.map.retain(|_, peer| !peer.handle.is_finished())
    }

    pub fn live(&mut self) -> usize {
        self.map
            .values()
            .filter(|peer| !peer.handle.is_finished())
            .count()
    }

    pub fn median_time_adjustment(&self) -> i64 {
        if self.map.values().len() > 0 {
            let mut time_offsets: Vec<i64> = self.map.values().map(|peer| peer.net_time).collect();
            return time_offsets.median().unwrap();
        }
        0
    }

    pub fn set_offset(&mut self, peer: u32, offset: i64) {
        if let Some(peer) = self.map.get_mut(&peer) {
            peer.net_time = offset;
        }
    }

    pub fn dispatch(&mut self, ip: IpAddr, port: Option<u16>) {
        let (ptx, prx) = mpsc::channel::<MainThreadMessage>(32);
        let mut rng = thread_rng();
        let nonce = rng.next_u32();
        let mut peer = Peer::new(nonce, ip, port, self.network, self.mtx.clone(), prx);
        let handle = tokio::spawn(async move { peer.connect().await });
        self.map.insert(
            nonce,
            ManagedPeer {
                net_time: 0,
                ptx,
                handle,
            },
        );
    }

    pub async fn send_message(&mut self, nonce: u32, message: MainThreadMessage) {
        if let Some(peer) = self.map.get(&nonce) {
            let _ = peer.ptx.send(message).await;
        }
    }
}
