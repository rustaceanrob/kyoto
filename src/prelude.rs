use bitcoin::{hex::DisplayHex, p2p::address::AddrV2, Network, Work};

#[allow(dead_code)]
pub const MAX_FUTURE_BLOCK_TIME: i64 = 60 * 60 * 2;
#[allow(unused)]
pub const MEDIAN_TIME_PAST: usize = 11;

macro_rules! impl_sourceless_error {
    ($e:ident) => {
        impl std::error::Error for $e {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                None
            }
        }
    };
}

pub(crate) use impl_sourceless_error;

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

pub(crate) fn default_port_from_network(network: &Network) -> u16 {
    match network {
        Network::Bitcoin => 8333,
        Network::Testnet => 18333,
        Network::Testnet4 => 48333,
        Network::Signet => 38333,
        Network::Regtest => 18444,
    }
}

pub(crate) fn encode_qname(domain: &str, filter: Option<&str>) -> Vec<u8> {
    let mut qname = Vec::new();
    if let Some(filter) = filter {
        qname.push(filter.len() as u8);
        qname.extend(filter.as_bytes());
    }
    for label in domain.split('.') {
        qname.push(label.len() as u8);
        qname.extend(label.as_bytes());
    }
    qname.push(0x00);
    qname
}

pub(crate) trait ZerolikeExt {
    fn zero() -> Self;
}

impl ZerolikeExt for Work {
    fn zero() -> Self {
        Self::from_be_bytes([0; 32])
    }
}

#[cfg(test)]
macro_rules! impl_deserialize {
    ($t:ident, $for:ident) => {
        impl<'de> serde::Deserialize<'de> for $t {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let s = String::deserialize(deserializer)?;
                let bytes = hex::decode(&s).map_err(serde::de::Error::custom)?;
                let bitcoin_type: $for =
                    bitcoin::consensus::deserialize(&bytes).map_err(serde::de::Error::custom)?;
                Ok($t(bitcoin_type))
            }
        }
    };
}

#[cfg(test)]
pub(crate) use impl_deserialize;
