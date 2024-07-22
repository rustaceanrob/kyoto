use bitcoin::consensus::{deserialize, deserialize_partial, Decodable};
use bitcoin::io::BufRead;
use bitcoin::p2p::{
    message::{NetworkMessage, RawNetworkMessage},
    message_blockdata::Inventory,
    Address, Magic, ServiceFlags,
};
use bitcoin::{Network, Txid};
use tokio::io::AsyncReadExt;
use tokio::net::tcp::OwnedReadHalf;
use tokio::sync::mpsc::Sender;

use crate::impl_sourceless_error;
use crate::node::channel_messages::{PeerMessage, RemoteVersion};
use crate::node::messages::RejectPayload;

const ONE_MONTH: u64 = 2_500_000;
const ONE_MINUTE: u64 = 60;
const MAX_MESSAGE_BYTES: u32 = 1024 * 1024 * 32;
// From Bitcoin Core PR #29575
const MAX_ADDR: usize = 1_000;
const MAX_INV: usize = 50_000;
const MAX_HEADERS: usize = 2_000;

pub(crate) struct Reader {
    stream: OwnedReadHalf,
    tx: Sender<PeerMessage>,
    network: Network,
}

impl Reader {
    pub fn new(stream: OwnedReadHalf, tx: Sender<PeerMessage>, network: Network) -> Self {
        Self {
            stream,
            tx,
            network,
        }
    }

    pub(crate) async fn read_from_remote(&mut self) -> Result<(), PeerReadError> {
        loop {
            // V1 headers are 24 bytes
            let mut message_buf = vec![0_u8; 24];
            let _ = self
                .stream
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
            let _ = self.stream.read_exact(&mut contents_buf).await.unwrap();
            message_buf.extend_from_slice(&contents_buf);
            let message: RawNetworkMessage =
                deserialize(&message_buf).map_err(|_| PeerReadError::Deserialization)?;
            let cleaned_message = parse_message(message.payload());
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
}

fn parse_message(message: &NetworkMessage) -> Option<PeerMessage> {
    match message {
        NetworkMessage::Version(version) => Some(PeerMessage::Version(RemoteVersion {
            service_flags: version.services,
            timestamp: version.timestamp,
            height: version.start_height,
        })),
        NetworkMessage::Verack => Some(PeerMessage::Verack),
        NetworkMessage::Addr(addresses) => {
            if addresses.len() > MAX_ADDR {
                return Some(PeerMessage::Disconnect);
            }
            let addresses: Vec<Address> = addresses
                .iter()
                .filter(|f| f.1.services.has(ServiceFlags::COMPACT_FILTERS))
                .filter(|f| f.1.socket_addr().is_ok())
                .map(|(_, addr)| addr.clone())
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
            let addresses: Vec<Address> = addresses
                .iter()
                .filter(|f| f.services.has(ServiceFlags::COMPACT_FILTERS))
                .filter(|f| f.socket_addr().is_ok())
                .map(|addr| match addr.socket_addr().unwrap().ip() {
                    std::net::IpAddr::V4(ip) => Address {
                        services: addr.services,
                        address: ip.to_ipv6_mapped().segments(),
                        port: addr.port,
                    },
                    std::net::IpAddr::V6(ip) => Address {
                        services: addr.services,
                        address: ip.segments(),
                        port: addr.port,
                    },
                })
                .collect();
            Some(PeerMessage::Addr(addresses))
        }
        NetworkMessage::SendAddrV2 => None,
        #[allow(unused)]
        NetworkMessage::Unknown { command, payload } => Some(PeerMessage::Disconnect),
    }
}

pub struct V1Header {
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

#[derive(Debug)]
pub enum PeerReadError {
    ReadBuffer,
    Deserialization,
    TooManyMessages,
    PeerTimeout,
    MpscChannel,
}

impl core::fmt::Display for PeerReadError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            PeerReadError::ReadBuffer => write!(f, "reading bytes off the stream failed."),
            PeerReadError::Deserialization => {
                write!(f, "the message could not be properly deserialized.")
            }
            PeerReadError::TooManyMessages => write!(f, "DOS protection."),
            PeerReadError::PeerTimeout => write!(f, "peer timeout."),
            PeerReadError::MpscChannel => write!(f, "sending over the channel failed."),
        }
    }
}

impl_sourceless_error!(PeerReadError);
