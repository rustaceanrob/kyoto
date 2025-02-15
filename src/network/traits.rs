use core::fmt::Debug;
use std::{net::IpAddr, time::Duration};

use bitcoin::{
    p2p::{
        address::AddrV2,
        message::NetworkMessage,
        message_filter::{GetCFHeaders, GetCFilters},
    },
    BlockHash, Transaction, Wtxid,
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
    sync::Mutex,
};

use crate::{core::channel_messages::GetBlockConfig, prelude::FutureResult};

use super::error::{PeerError, PeerReadError};

const CONNECTION_TIMEOUT: u64 = 2;

pub(crate) type StreamReader = Mutex<Box<dyn AsyncRead + Send + Unpin>>;
pub(crate) type StreamWriter = Mutex<Box<dyn AsyncWrite + Send + Unpin>>;

// Responsible for serializing messages to write over the wire, either encrypted or plaintext.
pub(crate) trait MessageGenerator: Send + Sync {
    fn version_message(&mut self, port: Option<u16>) -> Result<Vec<u8>, PeerError>;

    fn verack(&mut self) -> Result<Vec<u8>, PeerError>;

    fn addr(&mut self) -> Result<Vec<u8>, PeerError>;

    fn addrv2(&mut self) -> Result<Vec<u8>, PeerError>;

    fn wtxid_relay(&mut self) -> Result<Vec<u8>, PeerError>;

    fn headers(
        &mut self,
        locator_hashes: Vec<BlockHash>,
        stop_hash: Option<BlockHash>,
    ) -> Result<Vec<u8>, PeerError>;

    fn cf_headers(&mut self, message: GetCFHeaders) -> Result<Vec<u8>, PeerError>;

    fn filters(&mut self, message: GetCFilters) -> Result<Vec<u8>, PeerError>;

    fn block(&mut self, config: GetBlockConfig) -> Result<Vec<u8>, PeerError>;

    fn pong(&mut self, nonce: u64) -> Result<Vec<u8>, PeerError>;

    fn announce_transaction(&mut self, wtxid: Wtxid) -> Result<Vec<u8>, PeerError>;

    fn broadcast_transaction(&mut self, transaction: Transaction) -> Result<Vec<u8>, PeerError>;
}

// Responsible for parsing plaintext or encrypted messages off of the  wire.
pub(crate) trait MessageParser: Send + Sync {
    fn read_message(&mut self) -> FutureResult<Option<NetworkMessage>, PeerReadError>;
}

// Establishes connections based on the network configuration.
pub(crate) trait NetworkConnector {
    fn can_connect(&self, addr: &AddrV2) -> bool;

    fn connect(
        &mut self,
        addr: AddrV2,
        port: u16,
    ) -> FutureResult<(StreamReader, StreamWriter), PeerError>;
}

pub(crate) struct ClearNetConnection {}

impl ClearNetConnection {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

impl NetworkConnector for ClearNetConnection {
    fn can_connect(&self, addr: &AddrV2) -> bool {
        matches!(addr, AddrV2::Ipv4(_) | AddrV2::Ipv6(_))
    }

    fn connect(
        &mut self,
        addr: AddrV2,
        port: u16,
    ) -> FutureResult<(StreamReader, StreamWriter), PeerError> {
        async fn do_impl(
            addr: AddrV2,
            port: u16,
        ) -> Result<(StreamReader, StreamWriter), PeerError> {
            let socket_addr = match addr {
                AddrV2::Ipv4(ip) => IpAddr::V4(ip),
                AddrV2::Ipv6(ip) => IpAddr::V6(ip),
                _ => return Err(PeerError::UnreachableSocketAddr),
            };
            let timeout = tokio::time::timeout(
                Duration::from_secs(CONNECTION_TIMEOUT),
                TcpStream::connect((socket_addr, port)),
            )
            .await
            .map_err(|_| PeerError::ConnectionFailed)?;
            match timeout {
                Ok(stream) => {
                    let (reader, writer) = stream.into_split();
                    Ok((Mutex::new(Box::new(reader)), Mutex::new(Box::new(writer))))
                }
                Err(_) => Err(PeerError::ConnectionFailed),
            }
        }
        Box::pin(do_impl(addr, port))
    }
}

impl Debug for dyn NetworkConnector + Send + Sync {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Generic connection. Either TCP, Tor, or something else concrete"
        )
    }
}
