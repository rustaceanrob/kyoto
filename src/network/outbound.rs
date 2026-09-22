use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::{SystemTime, UNIX_EPOCH},
};

use bip324::{OutboundCipher, PacketType};
use bitcoin::{
    consensus::serialize,
    p2p::{
        message::{NetworkMessage, RawNetworkMessage},
        message_blockdata::Inventory,
        message_network::VersionMessage,
        Address, ServiceFlags,
    },
    BlockHash, Network, Transaction, Wtxid,
};

use crate::{default_port_from_network, network::RelayPolicy, BlockType};

use super::{KYOTO_VERSION, PROTOCOL_VERSION, RUST_BITCOIN_VERSION};

// Responsible for serializing messages to write over the wire, either encrypted or plaintext.
pub(in crate::network) struct MessageGenerator {
    pub network: Network,
    pub transport: Transport,
    pub block_type: BlockType,
}

pub(in crate::network) enum Transport {
    V1,
    V2 { encryptor: OutboundCipher },
}

impl MessageGenerator {
    pub(in crate::network) fn serialize(&mut self, msg: NetworkMessage) -> Vec<u8> {
        match &mut self.transport {
            Transport::V1 => {
                let data = RawNetworkMessage::new(self.network.magic(), msg);
                serialize(&data)
            }
            Transport::V2 { encryptor } => {
                let plaintext = serialize_network_message(msg);
                encrypt_plaintext(encryptor, plaintext)
            }
        }
    }

    pub(in crate::network) fn version_message(&mut self, port: Option<u16>, relay_policy: RelayPolicy) -> Vec<u8> {
        let msg = NetworkMessage::Version(make_version(port, relay_policy, &self.network));
        self.serialize(msg)
    }

    pub(in crate::network) fn block(&mut self, hash: BlockHash) -> Vec<u8> {
        let inv = match self.block_type {
            BlockType::Legacy => Inventory::Block(hash),
            BlockType::Witness => Inventory::WitnessBlock(hash),
        };
        let msg = NetworkMessage::GetData(vec![inv]);
        self.serialize(msg)
    }

    pub(in crate::network) fn announce_transactions(&mut self, wtxids: Vec<Wtxid>) -> Vec<u8> {
        let msg = NetworkMessage::Inv(wtxids.into_iter().map(Inventory::WTx).collect());
        self.serialize(msg)
    }

    pub(in crate::network) fn broadcast_transaction(
        &mut self,
        transaction: Transaction,
    ) -> Vec<u8> {
        let msg = NetworkMessage::Tx(transaction);
        self.serialize(msg)
    }
}

fn serialize_network_message(message: NetworkMessage) -> Vec<u8> {
    bip324::serde::serialize(message)
}

fn encrypt_plaintext(encryptor: &mut OutboundCipher, plaintext: Vec<u8>) -> Vec<u8> {
    encryptor.encrypt_to_vec(&plaintext, PacketType::Genuine, None)
}

pub(in crate::network) fn make_version(
    port: Option<u16>,
    relay_policy: RelayPolicy,
    network: &Network,
) -> VersionMessage {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_secs();
    let default_port = default_port_from_network(network);
    let ip = SocketAddr::new(
        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        port.unwrap_or(default_port),
    );
    let relay = match relay_policy {
        RelayPolicy::BlocksOnly => false,
        RelayPolicy::Transactions => true,
    };
    let from_and_recv = Address::new(&ip, ServiceFlags::NONE);
    VersionMessage {
        version: PROTOCOL_VERSION,
        services: ServiceFlags::NONE,
        timestamp: now as i64,
        receiver: from_and_recv.clone(),
        sender: from_and_recv,
        nonce: 1,
        user_agent: format!("/Rust BIP-157:{KYOTO_VERSION}/rust-bitcoin:{RUST_BITCOIN_VERSION}/"),
        start_height: 0,
        relay,
    }
}
