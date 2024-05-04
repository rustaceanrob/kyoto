use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::{SystemTime, UNIX_EPOCH},
};

use bitcoin::{
    consensus::serialize,
    hashes::Hash,
    p2p::{
        message::{NetworkMessage, RawNetworkMessage},
        message_blockdata::GetHeadersMessage,
        message_network::VersionMessage,
        Address, ServiceFlags,
    },
    BlockHash, Network,
};

pub const PROTOCOL_VERSION: u32 = 70012;

pub(crate) struct V1OutboundMessage {
    network: Network,
}

impl V1OutboundMessage {
    pub(crate) fn new(network: Network) -> Self {
        Self { network }
    }

    pub(crate) fn new_version_message(&self, port: Option<u16>) -> Vec<u8> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_secs();
        let default_port = match self.network {
            Network::Bitcoin => 8332,
            Network::Testnet => 18332,
            Network::Signet => 38332,
            Network::Regtest => panic!("unimplemented"),
            _ => unreachable!(),
        };
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
            user_agent: "kyoto".to_string(),
            start_height: 0,
            relay: false,
        };
        let data = RawNetworkMessage::new(self.network.magic(), NetworkMessage::Version(msg));
        serialize(&data)
    }

    pub(crate) fn new_verack(&self) -> Vec<u8> {
        let data = RawNetworkMessage::new(self.network.magic(), NetworkMessage::Verack);
        serialize(&data)
    }

    pub(crate) fn new_get_addr(&self) -> Vec<u8> {
        let data = RawNetworkMessage::new(self.network.magic(), NetworkMessage::GetAddr);
        serialize(&data)
    }

    pub(crate) fn new_get_headers(
        &self,
        locator_hashes: Vec<BlockHash>,
        stop_hash: Option<BlockHash>,
    ) -> Vec<u8> {
        let msg =
            GetHeadersMessage::new(locator_hashes, stop_hash.unwrap_or(BlockHash::all_zeros()));
        let data =
            &mut RawNetworkMessage::new(self.network.magic(), NetworkMessage::GetHeaders(msg));
        serialize(&data)
    }

    pub(crate) fn new_pong(&self, nonce: u64) -> Vec<u8> {
        let msg = NetworkMessage::Pong(nonce);
        let data = &mut RawNetworkMessage::new(self.network.magic(), msg);
        serialize(&data)
    }
}
