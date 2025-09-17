use bitcoin::Network;

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

pub(crate) fn default_port_from_network(network: &Network) -> u16 {
    match network {
        Network::Bitcoin => 8333,
        Network::Testnet => 18333,
        Network::Testnet4 => 48333,
        Network::Signet => 38333,
        Network::Regtest => 18444,
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
