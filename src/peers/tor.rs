use arti_client::{config::BoolOrAuto, DataStream, HsId, StreamPrefs, TorAddr, TorClient};
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

    // FIXME: (@leonardo) If we receive the AddressV2Message, we wouldn't need the port parameter, and could use the `socket_addr` method too.
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
            let data_stream: DataStream = match addr {
                AddrV2::Ipv4(ip_addr) => {
                    let tor_addr = TorAddr::dangerously_from((ip_addr, port))
                        .map_err(|_e| PeerError::UnreachableSocketAddr)?;

                    client
                        .connect(tor_addr)
                        .await
                        .map_err(|_e| PeerError::ConnectionFailed)?
                }
                AddrV2::Ipv6(ip_addr) => {
                    let tor_addr = TorAddr::dangerously_from((ip_addr, port))
                        .map_err(|_e| PeerError::UnreachableSocketAddr)?;

                    client
                        .connect(tor_addr)
                        .await
                        .map_err(|_e| PeerError::ConnectionFailed)?
                }
                AddrV2::TorV3(pk) => {
                    let hs_id = HsId::from(pk);
                    let tor_addr = TorAddr::from(hs_id.to_string())
                        .map_err(|_e| PeerError::UnreachableSocketAddr)?;

                    let mut stream_prefs = StreamPrefs::default();
                    stream_prefs.connect_to_onion_services(BoolOrAuto::Explicit(true));

                    client
                        .connect_with_prefs(tor_addr, &stream_prefs)
                        .await
                        .map_err(|_e| PeerError::ConnectionFailed)?
                }
                _ => return Err(PeerError::UnreachableSocketAddr),
            };

            let (reader, writer) = data_stream.split();
            Ok((Mutex::new(Box::new(reader)), Mutex::new(Box::new(writer))))
        }
        Box::pin(do_impl(&mut self.client, addr, port))
    }
}
