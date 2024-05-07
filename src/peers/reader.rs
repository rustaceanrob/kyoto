use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use bitcoin::consensus::deserialize;
use bitcoin::consensus::deserialize_partial;
use bitcoin::consensus::Decodable;
use bitcoin::io::BufRead;
use bitcoin::p2p::message::NetworkMessage;
use bitcoin::p2p::message::RawNetworkMessage;
use bitcoin::p2p::Address;
use bitcoin::p2p::Magic;
use bitcoin::p2p::ServiceFlags;
use bitcoin::Network;
use thiserror::Error;
use tokio::io::AsyncReadExt;
use tokio::net::tcp::OwnedReadHalf;
use tokio::sync::mpsc::Sender;

use crate::node::channel_messages::PeerMessage;
use crate::node::channel_messages::RemoteVersion;

const ONE_MONTH: u64 = 2_500_000;

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
            let mut message_buf = vec![0_u8; 24];
            let _ = self
                .stream
                .read_exact(&mut message_buf)
                .await
                .map_err(|_| PeerReadError::ReadBufferError)?;
            let header: V1Header = deserialize_partial(&message_buf)
                .map_err(|_| PeerReadError::DeserializationError)?
                .0;
            let mut contents_buf = vec![0_u8; header.length as usize];
            let _ = self.stream.read_exact(&mut contents_buf).await.unwrap();
            message_buf.extend_from_slice(&contents_buf);
            let message: RawNetworkMessage =
                deserialize(&message_buf).map_err(|_| PeerReadError::DeserializationError)?;
            let cleaned_message = parse_message(message.payload());
            match cleaned_message {
                Some(message) => self
                    .tx
                    .send(message)
                    .await
                    .map_err(|_| PeerReadError::MpscSenderError)?,
                None => continue,
            }
        }
    }
}

fn parse_message(message: &NetworkMessage) -> Option<PeerMessage> {
    println!("[Peer message]: {}", message.cmd());
    match message {
        NetworkMessage::Version(version) => Some(PeerMessage::Version(RemoteVersion {
            service_flags: version.services,
            timestamp: version.timestamp,
            height: version.start_height,
        })),
        NetworkMessage::Verack => Some(PeerMessage::Verack),
        NetworkMessage::Addr(addresses) => {
            let last_month = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time went backwards")
                .as_secs()
                - ONE_MONTH;
            let addresses: Vec<Address> = addresses
                .iter()
                .filter(|f| {
                    f.1.services.has(ServiceFlags::COMPACT_FILTERS)
                        && f.1.services.has(ServiceFlags::WITNESS)
                })
                .filter(|f| f.1.socket_addr().is_ok())
                .filter(|f| f.0 > last_month as u32)
                .map(|(_, addr)| addr.clone())
                .collect();
            Some(PeerMessage::Addr(addresses))
        }
        NetworkMessage::Inv(_) => None,
        NetworkMessage::GetData(_) => None,
        NetworkMessage::NotFound(_) => None,
        NetworkMessage::GetBlocks(_) => None,
        NetworkMessage::GetHeaders(_) => None,
        NetworkMessage::MemPool => None,
        NetworkMessage::Tx(_) => None,
        NetworkMessage::Block(_) => None,
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
        NetworkMessage::CFilter(_) => None,
        NetworkMessage::GetCFHeaders(_) => None,
        NetworkMessage::CFHeaders(_) => None,
        NetworkMessage::GetCFCheckpt(_) => None,
        NetworkMessage::CFCheckpt(_) => None,
        NetworkMessage::SendCmpct(_) => None,
        NetworkMessage::CmpctBlock(_) => None,
        NetworkMessage::GetBlockTxn(_) => None,
        NetworkMessage::BlockTxn(_) => None,
        NetworkMessage::Alert(_) => None,
        NetworkMessage::Reject(_) => None,
        NetworkMessage::FeeFilter(_) => None,
        NetworkMessage::WtxidRelay => None,
        NetworkMessage::AddrV2(addresses) => {
            let last_month = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time went backwards")
                .as_secs()
                - ONE_MONTH;
            let addresses: Vec<Address> = addresses
                .iter()
                .filter(|f| {
                    f.services.has(ServiceFlags::COMPACT_FILTERS)
                        && f.services.has(ServiceFlags::WITNESS)
                })
                .filter(|f| f.socket_addr().is_ok())
                .filter(|f| f.time > last_month as u32)
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
    #[error("reading bytes off the stream failed")]
    ReadBufferError,
    #[error("the message could not be properly deserialized")]
    DeserializationError,
    #[error("sending over the channel failed")]
    MpscSenderError,
}
