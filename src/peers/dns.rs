extern crate alloc;
use bitcoin::Network;
use std::net::IpAddr;
use thiserror::Error;
// need to get rid of this at some point
use trust_dns_resolver::AsyncResolver;

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
        println!("Bootstrapping peers with DNS seeds");
        let seeds = match network {
            Network::Bitcoin => MAINNET_SEEDS.to_vec(),
            Network::Testnet => TESTNET_SEEDS.to_vec(),
            Network::Signet => SIGNET_SEEDS.to_vec(),
            Network::Regtest => panic!("unimplemented network"),
            _ => panic!("unreachable"),
        };

        let resolver = AsyncResolver::tokio_from_system_conf()
            .map_err(|_| DnsBootstrapError::ResolverError)?;

        let mut ip_addrs: Vec<IpAddr> = vec![];

        for host in seeds {
            let ips = match resolver.lookup_ip(host).await {
                Ok(ips) => ips,
                // ignore individual errors for DNS queries
                Err(_) => continue,
            };
            ips.iter().for_each(|ip| ip_addrs.push(ip));
        }

        // arbitrary number for now
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
    async fn it_works() {
        Dns::bootstrap(bitcoin::network::Network::Signet)
            .await
            .unwrap();
    }
}
