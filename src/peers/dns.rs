extern crate alloc;
use bitcoin::Network;
use std::net::IpAddr;
use thiserror::Error;

const MIN_PEERS: usize = 10;

const SIGNET_SEEDS: &[&str; 2] = &["seed.dlsouza.lol", "seed.signet.bitcoin.sprovoost.nl"];

const TESTNET_SEEDS: &[&str; 4] = &[
    "testnet-seed.bitcoin.jonasschnelli.ch",
    "seed.tbtc.petertodd.org",
    "seed.testnet.bitcoin.sprovoost.nl",
    "testnet-seed.bluematt.me",
];

const MAINNET_SEEDS: &[&str; 9] = &[
    "seed.bitcoin.sipa.be",
    "dnsseed.bluematt.me",
    "dnsseed.bitcoin.dashjr.org",
    "seed.bitcoinstats.com",
    "seed.bitcoin.jonasschnelli.ch",
    "seed.btc.petertodd.org",
    "seed.bitcoin.sprovoost.nl",
    "dnsseed.emzy.de",
    "seed.bitcoin.wiz.biz",
];

pub(crate) struct Dns {}

impl Dns {
    pub async fn bootstrap(network: Network) -> Result<Vec<IpAddr>, DnsBootstrapError> {
        let seeds = match network {
            Network::Bitcoin => MAINNET_SEEDS.to_vec(),
            Network::Testnet => TESTNET_SEEDS.to_vec(),
            Network::Signet => SIGNET_SEEDS.to_vec(),
            Network::Regtest => Vec::with_capacity(0),
            _ => unreachable!(),
        };
        let mut ip_addrs: Vec<IpAddr> = vec![];

        for host in seeds {
            if let Ok(addrs) = dns_lookup::getaddrinfo(Some(host), None, None) {
                for addr in addrs.filter_map(Result::ok) {
                    ip_addrs.push(addr.sockaddr.ip().into());
                }
            }
        }

        // Arbitrary number for now
        if ip_addrs.len() < MIN_PEERS {
            return Err(DnsBootstrapError::NotEnoughPeersError);
        }

        Ok(ip_addrs)
    }
}

#[derive(Debug, Error)]
pub(crate) enum DnsBootstrapError {
    #[error("the async resolver could not be constructed")]
    ResolverError,
    #[error("most dns seeding failed")]
    NotEnoughPeersError,
}

#[cfg(test)]
mod test {
    use super::Dns;

    #[tokio::test]
    async fn dns_responds() {
        Dns::bootstrap(bitcoin::network::Network::Signet)
            .await
            .unwrap();
    }
}
