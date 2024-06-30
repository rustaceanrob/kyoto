use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use bitcoin::consensus::deserialize;
use bitcoin::consensus::deserialize_partial;
use bitcoin::consensus::Decodable;
use bitcoin::io::BufRead;
use bitcoin::p2p::message::NetworkMessage;
use bitcoin::p2p::message::RawNetworkMessage;
use bitcoin::p2p::message_blockdata::Inventory;
use bitcoin::p2p::Address;
use bitcoin::p2p::Magic;
use bitcoin::p2p::ServiceFlags;
use bitcoin::Network;
use bitcoin::Txid;
use thiserror::Error;
use tokio::io::AsyncReadExt;
use tokio::net::tcp::OwnedReadHalf;
use tokio::sync::mpsc::Sender;

use crate::node::channel_messages::PeerMessage;
use crate::node::channel_messages::RemoteVersion;
use crate::node::messages::RejectPayload;

const ONE_MONTH: u64 = 2_500_000;
const ONE_MINUTE: u64 = 60;
// The peer must have sent at least 10 messages to trigger DOS
const MINIMUM_DOS_THRESHOLD: u64 = 10;
// We allow up to 5000 messages per second
const RATE_LIMIT: u64 = 5000;

pub(crate) struct Reader {
    num_messages: u64,
    start_time: u64,
    stream: OwnedReadHalf,
    tx: Sender<PeerMessage>,
    network: Network,
}

impl Reader {
    pub fn new(stream: OwnedReadHalf, tx: Sender<PeerMessage>, network: Network) -> Self {
        let start_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_secs();
        Self {
            num_messages: 0,
            start_time,
            stream,
            tx,
            network,
        }
    }

    pub(crate) async fn read_from_remote(&mut self) -> Result<(), PeerReadError> {
        loop {
            // v1 headers are 24 bytes
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
            if header.length > (1024 * 1024 * 32) as u32 {
                return Err(PeerReadError::Deserialization);
            }
            // DOS protection
            self.num_messages += 1;
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time went backwards")
                .as_secs();
            let duration = now - self.start_time;
            if self.num_messages > MINIMUM_DOS_THRESHOLD
                && self.num_messages.checked_div(duration).unwrap_or(0) > RATE_LIMIT
            {
                return Err(PeerReadError::TooManyMessages);
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
            let addresses: Vec<Address> = addresses
                .iter()
                .filter(|f| f.1.services.has(ServiceFlags::COMPACT_FILTERS))
                .filter(|f| f.1.socket_addr().is_ok())
                .map(|(_, addr)| addr.clone())
                .collect();
            Some(PeerMessage::Addr(addresses))
        }
        NetworkMessage::Inv(inventory) => {
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
        NetworkMessage::Headers(headers) => Some(PeerMessage::Headers(headers.clone())),
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

#[derive(Error, Debug)]
pub enum PeerReadError {
    #[error("Reading bytes off the stream failed.")]
    ReadBuffer,
    #[error("The message could not be properly deserialized.")]
    Deserialization,
    #[error("DOS protection.")]
    TooManyMessages,
    #[error("Peer timeout.")]
    PeerTimeout,
    #[error("Sending over the channel failed.")]
    MpscChannel,
}
