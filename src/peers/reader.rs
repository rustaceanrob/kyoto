use std::net::IpAddr;
use std::ops::DerefMut;

use bitcoin::consensus::{deserialize, deserialize_partial, Decodable};
use bitcoin::io::BufRead;
use bitcoin::p2p::address::AddrV2;
use bitcoin::p2p::{
    message::{NetworkMessage, RawNetworkMessage},
    message_blockdata::Inventory,
    Magic, ServiceFlags,
};
use bitcoin::{Network, Txid};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;

use crate::node::channel_messages::{CombinedAddr, PeerMessage};
use crate::node::messages::RejectPayload;

use super::error::PeerReadError;

const ONE_MONTH: u64 = 2_500_000;
const ONE_MINUTE: u64 = 60;
const MAX_MESSAGE_BYTES: u32 = 1024 * 1024 * 32;
// From Bitcoin Core PR #29575
const MAX_ADDR: usize = 1_000;
const MAX_INV: usize = 50_000;
const MAX_HEADERS: usize = 2_000;

pub(crate) struct Reader {
    stream: Mutex<Box<dyn AsyncRead + Send + Unpin>>,
    tx: Sender<PeerMessage>,
    network: Network,
}

impl Reader {
    pub fn new(
        stream: Mutex<Box<dyn AsyncRead + Send + Unpin>>,
        tx: Sender<PeerMessage>,
        network: Network,
    ) -> Self {
        Self {
            stream,
            tx,
            network,
        }
    }

    pub(crate) async fn read_from_remote(&mut self) -> Result<(), PeerReadError> {
        let mut lock = self.stream.lock().await;
        let stream = lock.deref_mut();
        loop {
            // V1 headers are 24 bytes
            let mut message_buf = vec![0_u8; 24];
            let _ = stream
                .read_exact(&mut message_buf)
                .await
                .map_err(|_| PeerReadError::ReadBuffer)?;
            let header: V1Header = deserialize_partial(&message_buf)
                .map_err(|_| PeerReadError::Deserialization)?
                .0;
            // Nonsense for our network
            if header.magic != self.network.magic() {
                return Err(PeerReadError::Deserialization);
            }
            // Message is too long
            if header.length > MAX_MESSAGE_BYTES {
                return Err(PeerReadError::Deserialization);
            }
            let mut contents_buf = vec![0_u8; header.length as usize];
            let _ = stream.read_exact(&mut contents_buf).await.unwrap();
            message_buf.extend_from_slice(&contents_buf);
            let message: RawNetworkMessage =
                deserialize(&message_buf).map_err(|_| PeerReadError::Deserialization)?;
            let cleaned_message = self.parse_message(message.payload());
            match cleaned_message {
                Some(message) => self
                    .tx
                    .send(message)
                    .await
                    .map_err(|_| PeerReadError::MpscChannel)?,
                None => continue,
            }
        }
    }

    fn parse_message(&self, message: &NetworkMessage) -> Option<PeerMessage> {
        match message {
            NetworkMessage::Version(version) => Some(PeerMessage::Version(version.clone())),
            NetworkMessage::Verack => Some(PeerMessage::Verack),
            NetworkMessage::Addr(addresses) => {
                if addresses.len() > MAX_ADDR {
                    return Some(PeerMessage::Disconnect);
                }
                let addresses: Vec<CombinedAddr> = addresses
                    .iter()
                    .filter(|f| f.1.services.has(ServiceFlags::COMPACT_FILTERS))
                    .filter(|f| f.1.socket_addr().is_ok())
                    .map(|(_, addr)| {
                        let ip = match addr.socket_addr().unwrap().ip() {
                            IpAddr::V4(ip) => AddrV2::Ipv4(ip),
                            IpAddr::V6(ip) => AddrV2::Ipv6(ip),
                        };
                        let mut addr = CombinedAddr::new(ip, addr.port);
                        addr.services(addr.services);
                        addr
                    })
                    .collect();
                Some(PeerMessage::Addr(addresses))
            }
            NetworkMessage::Inv(inventory) => {
                if inventory.len() > MAX_INV {
                    return Some(PeerMessage::Disconnect);
                }
                let mut hashes = Vec::new();
                for i in inventory {
                    match i {
                        Inventory::Block(hash) => hashes.push(*hash),
                        Inventory::CompactBlock(hash) => hashes.push(*hash),
                        Inventory::WitnessBlock(hash) => hashes.push(*hash),
                        _ => continue,
                    }
                }
                if !hashes.is_empty() {
                    Some(PeerMessage::NewBlocks(hashes))
                } else {
                    None
                }
            }
            NetworkMessage::GetData(_) => None,
            NetworkMessage::NotFound(_) => None,
            NetworkMessage::GetBlocks(_) => None,
            NetworkMessage::GetHeaders(_) => None,
            NetworkMessage::MemPool => None,
            NetworkMessage::Tx(_) => None,
            NetworkMessage::Block(block) => Some(PeerMessage::Block(block.clone())),
            NetworkMessage::Headers(headers) => {
                if headers.len() > MAX_HEADERS {
                    return Some(PeerMessage::Disconnect);
                }
                Some(PeerMessage::Headers(headers.clone()))
            }
            NetworkMessage::SendHeaders => None,
            NetworkMessage::GetAddr => None,
            NetworkMessage::Ping(nonce) => Some(PeerMessage::Ping(*nonce)),
            NetworkMessage::Pong(nonce) => Some(PeerMessage::Pong(*nonce)),
            NetworkMessage::MerkleBlock(_) => None,
            NetworkMessage::FilterLoad(_) => None,
            NetworkMessage::FilterAdd(_) => None,
            NetworkMessage::FilterClear => None,
            NetworkMessage::GetCFilters(_) => None,
            NetworkMessage::CFilter(filter) => Some(PeerMessage::Filter(filter.clone())),
            NetworkMessage::GetCFHeaders(_) => None,
            NetworkMessage::CFHeaders(cf_headers) => {
                Some(PeerMessage::FilterHeaders(cf_headers.clone()))
            }
            NetworkMessage::GetCFCheckpt(_) => None,
            NetworkMessage::CFCheckpt(_) => None,
            NetworkMessage::SendCmpct(_) => None,
            NetworkMessage::CmpctBlock(_) => None,
            NetworkMessage::GetBlockTxn(_) => None,
            NetworkMessage::BlockTxn(_) => None,
            NetworkMessage::Alert(_) => None,
            NetworkMessage::Reject(rejection) => {
                let txid = Txid::from(rejection.hash);
                Some(PeerMessage::Reject(RejectPayload {
                    reason: rejection.ccode,
                    txid,
                }))
            }
            NetworkMessage::FeeFilter(_) => None,
            NetworkMessage::WtxidRelay => None,
            NetworkMessage::AddrV2(addresses) => {
                if addresses.len() > MAX_ADDR {
                    return Some(PeerMessage::Disconnect);
                }
                let addresses: Vec<CombinedAddr> = addresses
                    .iter()
                    .filter(|f| f.services.has(ServiceFlags::COMPACT_FILTERS))
                    .map(|addr| {
                        let mut ip = CombinedAddr::new(addr.addr.clone(), addr.port);
                        ip.services(addr.services);
                        ip
                    })
                    .collect();
                Some(PeerMessage::Addr(addresses))
            }
            NetworkMessage::SendAddrV2 => None,
            #[allow(unused)]
            NetworkMessage::Unknown { command, payload } => Some(PeerMessage::Disconnect),
        }
    }
}

struct V1Header {
    magic: Magic,
    _command: [u8; 12],
    length: u32,
    _checksum: u32,
}

impl Decodable for V1Header {
    fn consensus_decode<R: BufRead + ?Sized>(
        reader: &mut R,
    ) -> Result<Self, bitcoin::consensus::encode::Error> {
        let magic = Magic::consensus_decode(reader)?;
        let _command = <[u8; 12]>::consensus_decode(reader)?;
        let length = u32::consensus_decode(reader)?;
        let _checksum = u32::consensus_decode(reader)?;
        Ok(Self {
            magic,
            _command,
            length,
            _checksum,
        })
    }
}
