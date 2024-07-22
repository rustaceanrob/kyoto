use std::net::IpAddr;

use bitcoin::{params::Params, Network};

pub const MAX_FUTURE_BLOCK_TIME: i64 = 60 * 60 * 2;
pub const MEDIAN_TIME_PAST: usize = 11;

#[macro_export]
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
    fn median(&mut self) -> Option<T>;
}

impl Median<i64> for Vec<i64> {
    fn median(&mut self) -> Option<i64> {
        self.sort();
        let len = self.len();
        if len == 0 {
            None
        } else if len % 2 == 1 {
            Some(self[len / 2])
        } else {
            let mid = len / 2;
            Some((self[mid - 1] + self[mid]) / 2)
        }
    }
}

impl Median<u64> for Vec<u64> {
    fn median(&mut self) -> Option<u64> {
        self.sort();
        let len = self.len();
        if len == 0 {
            None
        } else if len % 2 == 1 {
            Some(self[len / 2])
        } else {
            let mid = len / 2;
            Some((self[mid - 1] + self[mid]) / 2)
        }
    }
}

impl Median<u32> for Vec<u32> {
    fn median(&mut self) -> Option<u32> {
        self.sort();
        let len = self.len();
        if len == 0 {
            None
        } else if len % 2 == 1 {
            Some(self[len / 2])
        } else {
            let mid = len / 2;
            Some((self[mid - 1] + self[mid]) / 2)
        }
    }
}

pub trait Netgroup {
    fn netgroup(&mut self) -> String;
}

impl Netgroup for IpAddr {
    fn netgroup(&mut self) -> String {
        if self.is_ipv4() {
            self.to_string()
                .split('.')
                .take(2)
                .collect::<Vec<&str>>()
                .join(".")
        } else {
            "V6ADDR".to_string()
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
