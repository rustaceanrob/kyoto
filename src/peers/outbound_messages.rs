use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::{SystemTime, UNIX_EPOCH},
};

use bitcoin::{
    consensus::serialize,
    hashes::Hash,
    p2p::{
        message::{NetworkMessage, RawNetworkMessage},
        message_blockdata::{GetHeadersMessage, Inventory},
        message_filter::{GetCFHeaders, GetCFilters},
        message_network::VersionMessage,
        Address, ServiceFlags,
    },
    BlockHash, Network, Transaction,
};

use crate::{node::channel_messages::GetBlockConfig, prelude::default_port_from_network};

use super::traits::MessageGenerator;

pub const PROTOCOL_VERSION: u32 = 70016;

pub(crate) struct V1OutboundMessage {
    network: Network,
}

impl V1OutboundMessage {
    pub(crate) fn new(network: Network) -> Self {
        Self { network }
    }
}

impl MessageGenerator for V1OutboundMessage {
    fn version_message(&mut self, port: Option<u16>) -> Vec<u8> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_secs();
        let default_port = default_port_from_network(&self.network);
        let ip = SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            port.unwrap_or(default_port),
        );
        let from_and_recv = Address::new(&ip, ServiceFlags::NONE);
        let msg = VersionMessage {
            version: PROTOCOL_VERSION,
            services: ServiceFlags::NONE,
            timestamp: now as i64,
            receiver: from_and_recv.clone(),
            sender: from_and_recv,
            nonce: 1,
            user_agent: "Kyoto Light Client / 0.1.0 / rust-bitcoin 0.32".to_string(),
            start_height: 0,
            relay: false,
        };
        let data = RawNetworkMessage::new(self.network.magic(), NetworkMessage::Version(msg));
        serialize(&data)
    }

    fn verack(&mut self) -> Vec<u8> {
        let data = RawNetworkMessage::new(self.network.magic(), NetworkMessage::Verack);
        serialize(&data)
    }

    fn addr(&mut self) -> Vec<u8> {
        let data = RawNetworkMessage::new(self.network.magic(), NetworkMessage::GetAddr);
        serialize(&data)
    }

    fn addrv2(&mut self) -> Vec<u8> {
        let data = RawNetworkMessage::new(self.network.magic(), NetworkMessage::SendAddrV2);
        serialize(&data)
    }

    fn headers(&mut self, locator_hashes: Vec<BlockHash>, stop_hash: Option<BlockHash>) -> Vec<u8> {
        let msg =
            GetHeadersMessage::new(locator_hashes, stop_hash.unwrap_or(BlockHash::all_zeros()));
        let data =
            &mut RawNetworkMessage::new(self.network.magic(), NetworkMessage::GetHeaders(msg));
        serialize(&data)
    }

    fn cf_headers(&mut self, message: GetCFHeaders) -> Vec<u8> {
        let data = &mut RawNetworkMessage::new(
            self.network.magic(),
            NetworkMessage::GetCFHeaders(message),
        );
        serialize(&data)
    }

    fn filters(&mut self, message: GetCFilters) -> Vec<u8> {
        let data =
            &mut RawNetworkMessage::new(self.network.magic(), NetworkMessage::GetCFilters(message));
        serialize(&data)
    }

    fn block(&mut self, config: GetBlockConfig) -> Vec<u8> {
        let inv = Inventory::Block(config.locator);
        let data =
            &mut RawNetworkMessage::new(self.network.magic(), NetworkMessage::GetData(vec![inv]));
        serialize(&data)
    }

    fn pong(&mut self, nonce: u64) -> Vec<u8> {
        let msg = NetworkMessage::Pong(nonce);
        let data = &mut RawNetworkMessage::new(self.network.magic(), msg);
        serialize(&data)
    }

    fn transaction(&mut self, transaction: Transaction) -> Vec<u8> {
        let msg = NetworkMessage::Tx(transaction);
        let data = &mut RawNetworkMessage::new(self.network.magic(), msg);
        serialize(&data)
    }
}
