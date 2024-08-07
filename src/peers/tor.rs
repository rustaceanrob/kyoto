use arti_client::{TorAddr, TorClient};
use bitcoin::p2p::address::AddrV2;
use tokio::sync::Mutex;
use tor_rtcompat::PreferredRuntime;

use crate::prelude::FutureResult;

use super::{
    error::PeerError,
    traits::{NetworkConnector, StreamReader, StreamWriter},
};

pub(crate) struct TorConnection {
    client: TorClient<PreferredRuntime>,
}

impl TorConnection {
    pub(crate) fn new(client: TorClient<PreferredRuntime>) -> Self {
        Self { client }
    }
}

impl NetworkConnector for TorConnection {
    fn can_connect(&self, addr: &AddrV2) -> bool {
        matches!(
            addr,
            AddrV2::Ipv4(_) | AddrV2::Ipv6(_) | AddrV2::TorV2(_) | AddrV2::TorV3(_)
        )
    }

    fn connect(
        &mut self,
        addr: AddrV2,
        port: u16,
    ) -> FutureResult<(StreamReader, StreamWriter), PeerError> {
        async fn do_impl(
            client: &mut TorClient<PreferredRuntime>,
            addr: AddrV2,
            port: u16,
        ) -> Result<(StreamReader, StreamWriter), PeerError> {
            match addr {
                AddrV2::Ipv4(ip) => {
                    let tor_address = TorAddr::dangerously_from((ip, port))
                        .map_err(|_| PeerError::UnreachableSocketAddr)?;
                    let res = client
                        .connect(tor_address)
                        .await
                        .map_err(|_| PeerError::ConnectionFailed)?;
                    let (reader, writer) = res.split();
                    Ok((Mutex::new(Box::new(reader)), Mutex::new(Box::new(writer))))
                }
                AddrV2::Ipv6(ip) => {
                    let tor_address = TorAddr::dangerously_from((ip, port))
                        .map_err(|_| PeerError::UnreachableSocketAddr)?;
                    let res = client
                        .connect(tor_address)
                        .await
                        .map_err(|_| PeerError::ConnectionFailed)?;
                    let (reader, writer) = res.split();
                    Ok((Mutex::new(Box::new(reader)), Mutex::new(Box::new(writer))))
                }
                AddrV2::TorV2(_onion) => Err(PeerError::UnreachableSocketAddr),
                AddrV2::TorV3(_onion) => Err(PeerError::UnreachableSocketAddr),
                _ => Err(PeerError::UnreachableSocketAddr),
            }
        }
        Box::pin(do_impl(&mut self.client, addr, port))
    }
}
