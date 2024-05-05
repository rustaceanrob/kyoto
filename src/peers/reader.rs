use bitcoin::consensus::deserialize;
use bitcoin::consensus::deserialize_partial;
use bitcoin::consensus::Decodable;
use bitcoin::io::BufRead;
use bitcoin::p2p::message::NetworkMessage;
use bitcoin::p2p::message::RawNetworkMessage;
use bitcoin::p2p::Magic;
use bitcoin::p2p::ServiceFlags;
use bitcoin::Network;
use thiserror::Error;
use tokio::io::AsyncReadExt;
use tokio::net::tcp::OwnedReadHalf;
use tokio::sync::mpsc::Sender;

use crate::node::channel_messages::PeerMessage;
use crate::node::channel_messages::RemotePeerAddr;
use crate::node::channel_messages::RemoteVersion;

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
            let addresses: Vec<RemotePeerAddr> = addresses
                .iter()
                .filter(|f| f.1.socket_addr().is_ok())
                .map(|(last_seen, ip_addr)| RemotePeerAddr {
                    last_seen: *last_seen,
                    ip: ip_addr.socket_addr().unwrap().ip(),
                    port: ip_addr.port,
                })
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
            let addresses: Vec<RemotePeerAddr> = addresses
                .iter()
                .filter(|f| {
                    f.services.has(ServiceFlags::COMPACT_FILTERS)
                        && f.services.has(ServiceFlags::WITNESS)
                })
                .filter(|f| f.socket_addr().is_ok())
                .map(|ip_addr| RemotePeerAddr {
                    last_seen: ip_addr.time,
                    ip: ip_addr.socket_addr().unwrap().ip(),
                    port: ip_addr.port,
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
