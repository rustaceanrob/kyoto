use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::{SystemTime, UNIX_EPOCH},
};

use bip324::{PacketType, PacketWriter};
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
    BlockHash, Network, Transaction, Wtxid,
};

use crate::{core::channel_messages::GetBlockConfig, prelude::default_port_from_network};

use super::{
    error::PeerError, traits::MessageGenerator, KYOTO_VERSION, PROTOCOL_VERSION,
    RUST_BITCOIN_VERSION,
};

pub(crate) struct V1OutboundMessage {
    network: Network,
}

impl V1OutboundMessage {
    pub(crate) fn new(network: Network) -> Self {
        Self { network }
    }
}

fn make_version(port: Option<u16>, network: &Network) -> VersionMessage {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_secs();
    let default_port = default_port_from_network(network);
    let ip = SocketAddr::new(
        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        port.unwrap_or(default_port),
    );
    let from_and_recv = Address::new(&ip, ServiceFlags::NONE);
    VersionMessage {
        version: PROTOCOL_VERSION,
        services: ServiceFlags::NONE,
        timestamp: now as i64,
        receiver: from_and_recv.clone(),
        sender: from_and_recv,
        nonce: 1,
        user_agent: format!(
            "Kyoto Light Client / {KYOTO_VERSION} / rust-bitcoin {RUST_BITCOIN_VERSION}"
        ),
        start_height: 0,
        relay: false,
    }
}

fn get_block_from_cfg(config: GetBlockConfig) -> Inventory {
    if cfg!(feature = "filter-control") {
        Inventory::WitnessBlock(config.locator)
    } else {
        Inventory::Block(config.locator)
    }
}

impl MessageGenerator for V1OutboundMessage {
    fn version_message(&mut self, port: Option<u16>) -> Result<Vec<u8>, PeerError> {
        let msg = make_version(port, &self.network);
        let data = RawNetworkMessage::new(self.network.magic(), NetworkMessage::Version(msg));
        Ok(serialize(&data))
    }

    fn verack(&mut self) -> Result<Vec<u8>, PeerError> {
        let data = RawNetworkMessage::new(self.network.magic(), NetworkMessage::Verack);
        Ok(serialize(&data))
    }

    fn addr(&mut self) -> Result<Vec<u8>, PeerError> {
        let data = RawNetworkMessage::new(self.network.magic(), NetworkMessage::GetAddr);
        Ok(serialize(&data))
    }

    fn addrv2(&mut self) -> Result<Vec<u8>, PeerError> {
        let data = RawNetworkMessage::new(self.network.magic(), NetworkMessage::SendAddrV2);
        Ok(serialize(&data))
    }

    fn wtxid_relay(&mut self) -> Result<Vec<u8>, PeerError> {
        let data = RawNetworkMessage::new(self.network.magic(), NetworkMessage::WtxidRelay);
        Ok(serialize(&data))
    }

    fn headers(
        &mut self,
        locator_hashes: Vec<BlockHash>,
        stop_hash: Option<BlockHash>,
    ) -> Result<Vec<u8>, PeerError> {
        let msg =
            GetHeadersMessage::new(locator_hashes, stop_hash.unwrap_or(BlockHash::all_zeros()));
        let data = RawNetworkMessage::new(self.network.magic(), NetworkMessage::GetHeaders(msg));
        Ok(serialize(&data))
    }

    fn cf_headers(&mut self, message: GetCFHeaders) -> Result<Vec<u8>, PeerError> {
        let data =
            RawNetworkMessage::new(self.network.magic(), NetworkMessage::GetCFHeaders(message));
        Ok(serialize(&data))
    }

    fn filters(&mut self, message: GetCFilters) -> Result<Vec<u8>, PeerError> {
        let data =
            RawNetworkMessage::new(self.network.magic(), NetworkMessage::GetCFilters(message));
        Ok(serialize(&data))
    }

    fn block(&mut self, config: GetBlockConfig) -> Result<Vec<u8>, PeerError> {
        let inv = get_block_from_cfg(config);
        let data = RawNetworkMessage::new(self.network.magic(), NetworkMessage::GetData(vec![inv]));
        Ok(serialize(&data))
    }

    fn pong(&mut self, nonce: u64) -> Result<Vec<u8>, PeerError> {
        let msg = NetworkMessage::Pong(nonce);
        let data = RawNetworkMessage::new(self.network.magic(), msg);
        Ok(serialize(&data))
    }

    fn announce_transaction(&mut self, wtxid: Wtxid) -> Result<Vec<u8>, PeerError> {
        let msg = NetworkMessage::Inv(vec![Inventory::WTx(wtxid)]);
        let data = RawNetworkMessage::new(self.network.magic(), msg);
        Ok(serialize(&data))
    }

    fn broadcast_transaction(&mut self, transaction: Transaction) -> Result<Vec<u8>, PeerError> {
        let msg = NetworkMessage::Tx(transaction);
        let data = RawNetworkMessage::new(self.network.magic(), msg);
        Ok(serialize(&data))
    }
}

pub(crate) struct V2OutboundMessage {
    network: Network,
    encryptor: PacketWriter,
}

impl V2OutboundMessage {
    pub(crate) fn new(network: Network, encryptor: PacketWriter) -> Self {
        Self { network, encryptor }
    }

    fn serialize_network_message(&self, message: NetworkMessage) -> Result<Vec<u8>, PeerError> {
        bip324::serde::serialize(message).map_err(|_| PeerError::MessageSerialization)
    }

    fn encrypt_plaintext(&mut self, plaintext: Vec<u8>) -> Result<Vec<u8>, PeerError> {
        self.encryptor
            .encrypt_packet(&plaintext, None, PacketType::Genuine)
            .map_err(|_| PeerError::MessageEncryption)
    }
}

impl MessageGenerator for V2OutboundMessage {
    fn version_message(&mut self, port: Option<u16>) -> Result<Vec<u8>, PeerError> {
        let msg = make_version(port, &self.network);
        let plaintext = self.serialize_network_message(NetworkMessage::Version(msg))?;
        self.encrypt_plaintext(plaintext)
    }

    fn verack(&mut self) -> Result<Vec<u8>, PeerError> {
        let plaintext = self.serialize_network_message(NetworkMessage::Verack)?;
        self.encrypt_plaintext(plaintext)
    }

    fn addr(&mut self) -> Result<Vec<u8>, PeerError> {
        let plaintext = self.serialize_network_message(NetworkMessage::GetAddr)?;
        self.encrypt_plaintext(plaintext)
    }

    fn addrv2(&mut self) -> Result<Vec<u8>, PeerError> {
        let plaintext = self.serialize_network_message(NetworkMessage::SendAddrV2)?;
        self.encrypt_plaintext(plaintext)
    }

    fn wtxid_relay(&mut self) -> Result<Vec<u8>, PeerError> {
        let plaintext = self.serialize_network_message(NetworkMessage::WtxidRelay)?;
        self.encrypt_plaintext(plaintext)
    }

    fn headers(
        &mut self,
        locator_hashes: Vec<BlockHash>,
        stop_hash: Option<BlockHash>,
    ) -> Result<Vec<u8>, PeerError> {
        let msg =
            GetHeadersMessage::new(locator_hashes, stop_hash.unwrap_or(BlockHash::all_zeros()));
        let plaintext = self.serialize_network_message(NetworkMessage::GetHeaders(msg))?;
        self.encrypt_plaintext(plaintext)
    }

    fn cf_headers(&mut self, message: GetCFHeaders) -> Result<Vec<u8>, PeerError> {
        let plaintext = self.serialize_network_message(NetworkMessage::GetCFHeaders(message))?;
        self.encrypt_plaintext(plaintext)
    }

    fn filters(&mut self, message: GetCFilters) -> Result<Vec<u8>, PeerError> {
        let plaintext = self.serialize_network_message(NetworkMessage::GetCFilters(message))?;
        self.encrypt_plaintext(plaintext)
    }

    fn block(&mut self, config: GetBlockConfig) -> Result<Vec<u8>, PeerError> {
        let inv = get_block_from_cfg(config);
        let plaintext = self.serialize_network_message(NetworkMessage::GetData(vec![inv]))?;
        self.encrypt_plaintext(plaintext)
    }

    fn pong(&mut self, nonce: u64) -> Result<Vec<u8>, PeerError> {
        let plaintext = self.serialize_network_message(NetworkMessage::Pong(nonce))?;
        self.encrypt_plaintext(plaintext)
    }

    fn announce_transaction(&mut self, wtxid: Wtxid) -> Result<Vec<u8>, PeerError> {
        let msg = NetworkMessage::Inv(vec![Inventory::WTx(wtxid)]);
        let plaintext = self.serialize_network_message(msg)?;
        self.encrypt_plaintext(plaintext)
    }

    fn broadcast_transaction(&mut self, transaction: Transaction) -> Result<Vec<u8>, PeerError> {
        let plaintext = self.serialize_network_message(NetworkMessage::Tx(transaction))?;
        self.encrypt_plaintext(plaintext)
    }
}
