use std::{collections::BTreeMap, net::IpAddr};

use bitcoin::{p2p::ServiceFlags, Network};
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
    network: Network,
    mtx: Sender<PeerThreadMessage>,
    map: BTreeMap<u32, ManagedPeer>,
}

impl PeerMap {
    pub fn new(mtx: Sender<PeerThreadMessage>, network: Network) -> Self {
        Self {
            num_peers: 0,
            network,
            mtx,
            map: BTreeMap::new(),
        }
    }

    pub async fn clean(&mut self) {
        self.map.retain(|_, peer| !peer.handle.is_finished())
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

    pub fn set_offset(&mut self, peer: u32, offset: i64) {
        if let Some(peer) = self.map.get_mut(&peer) {
            peer.net_time = offset;
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
}
