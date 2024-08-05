use core::fmt::Debug;
use std::{net::IpAddr, time::Duration};

use bitcoin::{
    p2p::{
        address::AddrV2,
        message_filter::{GetCFHeaders, GetCFilters},
    },
    BlockHash, Transaction,
};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};

use crate::{node::channel_messages::GetBlockConfig, prelude::FutureResult};

use super::error::PeerError;

const CONNECTION_TIMEOUT: u64 = 2;

type Reader = Box<dyn AsyncRead + Send + Sync + Unpin>;
type Writer = Box<dyn AsyncWrite + Send + Sync + Unpin>;

pub(crate) trait MessageGenerator {
    fn version_message(&mut self, port: Option<u16>) -> Vec<u8>;

    fn verack(&mut self) -> Vec<u8>;

    fn addr(&mut self) -> Vec<u8>;

    fn addrv2(&mut self) -> Vec<u8>;

    fn headers(&mut self, locator_hashes: Vec<BlockHash>, stop_hash: Option<BlockHash>) -> Vec<u8>;

    fn cf_headers(&mut self, message: GetCFHeaders) -> Vec<u8>;

    fn filters(&mut self, message: GetCFilters) -> Vec<u8>;

    fn block(&mut self, config: GetBlockConfig) -> Vec<u8>;

    fn pong(&mut self, nonce: u64) -> Vec<u8>;

    fn transaction(&mut self, transaction: Transaction) -> Vec<u8>;
}

pub(crate) trait NetworkConnector {
    fn can_connect(&self, addr: &AddrV2) -> bool;

    fn connect(&mut self, addr: AddrV2, port: u16) -> FutureResult<(Reader, Writer), PeerError>;
}

pub(crate) struct ClearNetConnection {}

impl ClearNetConnection {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

impl NetworkConnector for ClearNetConnection {
    fn can_connect(&self, addr: &AddrV2) -> bool {
        match addr {
            AddrV2::Ipv4(_) => true,
            AddrV2::Ipv6(_) => true,
            _ => false,
        }
    }

    fn connect(&mut self, addr: AddrV2, port: u16) -> FutureResult<(Reader, Writer), PeerError> {
        async fn do_impl(addr: AddrV2, port: u16) -> Result<(Reader, Writer), PeerError> {
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
                    Ok((Box::new(reader), Box::new(writer)))
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
