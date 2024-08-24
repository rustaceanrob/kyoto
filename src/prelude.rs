use core::{future::Future, pin::Pin};

use bitcoin::{hex::DisplayHex, p2p::address::AddrV2, params::Params, Network};

#[allow(dead_code)]
pub const MAX_FUTURE_BLOCK_TIME: i64 = 60 * 60 * 2;
pub const MEDIAN_TIME_PAST: usize = 11;

pub(crate) type FutureResult<'a, T, E> = Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'a>>;

#[macro_export]
/// Implement std::error::Error for an error with no sources.
macro_rules! impl_sourceless_error {
    ($e:ident) => {
        impl std::error::Error for $e {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                None
            }
        }
    };
}

pub trait Median<T> {
    fn median(&mut self) -> T;
}

impl Median<i64> for Vec<i64> {
    fn median(&mut self) -> i64 {
        self.sort();
        let len = self.len();
        if len == 0 {
            0
        } else if len % 2 == 1 {
            self[len / 2]
        } else {
            let mid = len / 2;
            (self[mid - 1] + self[mid]) / 2
        }
    }
}

impl Median<u64> for Vec<u64> {
    fn median(&mut self) -> u64 {
        self.sort();
        let len = self.len();
        if len == 0 {
            0
        } else if len % 2 == 1 {
            self[len / 2]
        } else {
            let mid = len / 2;
            (self[mid - 1] + self[mid]) / 2
        }
    }
}

impl Median<u32> for Vec<u32> {
    fn median(&mut self) -> u32 {
        self.sort();
        let len = self.len();
        if len == 0 {
            0
        } else if len % 2 == 1 {
            self[len / 2]
        } else {
            let mid = len / 2;
            (self[mid - 1] + self[mid]) / 2
        }
    }
}

pub trait Netgroup {
    fn netgroup(&self) -> String;
}

impl Netgroup for AddrV2 {
    fn netgroup(&self) -> String {
        match self {
            AddrV2::Ipv4(ip) => ip
                .to_string()
                .split('.')
                .take(2)
                .collect::<Vec<&str>>()
                .join("."),
            AddrV2::Ipv6(ip) => ip
                .to_string()
                .replace("::", ".")
                .split('.')
                .take(4)
                .collect::<Vec<&str>>()
                .join("::"),
            AddrV2::TorV2(t) => t.to_lower_hex_string(),
            AddrV2::TorV3(t) => t.to_lower_hex_string(),
            AddrV2::I2p(t) => t.to_lower_hex_string(),
            AddrV2::Cjdns(cj) => cj
                .to_string()
                .replace("::", ".")
                .split('.')
                .take(4)
                .collect::<Vec<&str>>()
                .join("::"),
            AddrV2::Unknown(_, _) => "UNKNOWN".to_owned(),
        }
    }
}

pub(crate) fn params_from_network(network: &Network) -> Params {
    match network {
        Network::Bitcoin => Params::new(*network),
        Network::Testnet => panic!("unimplemented network"),
        Network::Signet => Params::new(*network),
        Network::Regtest => Params::new(*network),
        _ => unreachable!(),
    }
}

pub(crate) fn default_port_from_network(network: &Network) -> u16 {
    match network {
        Network::Bitcoin => 8333,
        Network::Testnet => 18333,
        Network::Signet => 38333,
        Network::Regtest => 18444,
        _ => unreachable!(),
    }
}

pub(crate) fn encode_qname(domain: &str) -> Vec<u8> {
    let mut qname = Vec::new();
    for label in domain.split('.') {
        qname.push(label.len() as u8);
        qname.extend(label.as_bytes());
    }
    qname.push(0x00);
    qname
}
