extern crate alloc;
use bitcoin::Network;
use std::{
    net::{IpAddr, SocketAddr},
    time::Duration,
};

use super::{socks::resolve_socks5, ConnectionType};

const SIGNET_SEEDS: &[&str; 3] = &[
    "seed.dlsouza.lol",
    "seed.signet.bitcoin.sprovoost.nl",
    "seed.signet.achownodes.xyz",
];

const TESTNET_SEEDS: &[&str; 4] = &[
    "testnet-seed.bitcoin.jonasschnelli.ch",
    "seed.tbtc.petertodd.net",
    "testnet-seed.bluematt.me",
    "seed.testnet.achownodes.xyz",
];

const MAINNET_SEEDS: &[&str; 8] = &[
    "dnsseed.bluematt.me",
    "seed.bitcoin.jonasschnelli.ch",
    "seed.btc.petertodd.net",
    "seed.bitcoin.sprovoost.nl",
    "dnsseed.emzy.de",
    "seed.bitcoin.wiz.biz",
    "seed.mainnet.achownodes.xyz",
    "seed.rustaceanrob.com",
];

const TESTNET4_SEEDS: &[&str; 2] = &[
    "seed.testnet4.bitcoin.sprovoost.nl",
    "seed.testnet4.wiz.biz",
];

pub(crate) const CBF_SERVICE_BIT_PREFIX: &str = "x49"; // Compact Filters, Node Network
pub(crate) const CBF_V2T_SERVICE_BIT_PREFIX: &str = "x849"; // Compact Filters, Node Network, P2P V2

const SERVICE_BITS_PREFIX: &[&str; 2] = &[CBF_SERVICE_BIT_PREFIX, CBF_V2T_SERVICE_BIT_PREFIX];

pub(crate) const DNS_RESOLVER_PORT: u16 = 53;

const SOCKS_RESOLVE_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) async fn bootstrap_dns(network: Network, connector: &ConnectionType) -> Vec<IpAddr> {
    let seeds = match network {
        Network::Bitcoin => MAINNET_SEEDS.to_vec(),
        Network::Testnet => TESTNET_SEEDS.to_vec(),
        Network::Signet => SIGNET_SEEDS.to_vec(),
        Network::Regtest => Vec::with_capacity(0),
        Network::Testnet4 => TESTNET4_SEEDS.to_vec(),
    };
    let mut ip_addrs: Vec<IpAddr> = vec![];
    for host in seeds {
        let hosts = match connector {
            ConnectionType::ClearNet => lookup_hostname(host).await,
            ConnectionType::Socks5Proxy(proxy) => lookup_hostname_socks5(proxy.0, host).await,
        };
        ip_addrs.extend(hosts);
    }
    ip_addrs
}

pub(crate) async fn lookup_hostname(host: &str) -> Vec<IpAddr> {
    let hostnames = [
        format!("{host}:{DNS_RESOLVER_PORT}"),
        format!("{}.{host}:{DNS_RESOLVER_PORT}", SERVICE_BITS_PREFIX[0]),
        format!("{}.{host}:{DNS_RESOLVER_PORT}", SERVICE_BITS_PREFIX[1]),
    ];
    let mut ip_addrs = Vec::new();
    for hostname in hostnames {
        if let Ok(peers) = tokio::net::lookup_host(hostname).await {
            ip_addrs.extend(peers.map(|ip| ip.ip()));
        }
    }
    ip_addrs
}

async fn lookup_hostname_socks5(proxy: SocketAddr, host: &str) -> Vec<IpAddr> {
    let hostnames = [
        host.to_string(),
        format!("{}.{host}", SERVICE_BITS_PREFIX[0]),
        format!("{}.{host}", SERVICE_BITS_PREFIX[1]),
    ];
    let mut ip_addrs = Vec::new();
    for hostname in hostnames {
        if let Some(ip) = resolve_hostname_socks5(proxy, &hostname).await {
            ip_addrs.push(ip);
        }
    }
    ip_addrs
}

pub(crate) async fn resolve_hostname_socks5(proxy: SocketAddr, host: &str) -> Option<IpAddr> {
    tokio::time::timeout(SOCKS_RESOLVE_TIMEOUT, resolve_socks5(proxy, host))
        .await
        .ok()?
        .ok()
}
