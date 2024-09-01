use std::collections::HashSet;

use bitcoin::{Script, ScriptBuf};
use silentpayments::receiving::Receiver;
use silentpayments::utils::receiving::calculate_ecdh_shared_secret;
use silentpayments::utils::Network;

pub use silentpayments::receiving::Label;
pub use silentpayments::secp256k1::{PublicKey, Secp256k1, SecretKey};
pub use silentpayments::Error;

/// Create a silent payments client that can scan for incoming silent payments.
/// Read more about silent payments [here](https://silentpayments.xyz/docs/explained/), or read the specification [here](https://github.com/bitcoin/bips/blob/master/bip-0352.mediawiki).
pub struct SilentPaymentsScanner {
    scan_secret_key: SecretKey,
    receiver: Receiver,
}

impl SilentPaymentsScanner {
    /// Create a new client to scan for silent payment outputs.
    ///
    /// *Why do I need to pass a [`SecretKey`]?*
    ///
    /// Secret keys are required to calculate an elliptic curve Diffie-Hellman shared-secret.
    /// Read more about the security considerations of the silent payment specification [here](https://github.com/bitcoin/bips/blob/master/bip-0352.mediawiki#overview).
    pub fn new(
        spend_public_key: PublicKey,
        scan_secret_key: SecretKey,
        network: bitcoin::Network,
        label: Label,
    ) -> Result<Self, Error> {
        let network = convert_from_bitcoin_network(network);
        let receiver = Receiver::new(
            0,
            scan_secret_key.public_key(&Secp256k1::new()),
            spend_public_key,
            label,
            network,
        )?;
        Ok(Self {
            scan_secret_key,
            receiver,
        })
    }

    /// Calculate potential [`ScriptBuf`] based on a set of input public key tweaks.
    /// Presumably, these are sourced from a separate server that may provide these tweaks.
    ///
    /// A "tweak" here is defined with:
    ///
    /// `input_hash = hash_0352(outpoint_L || A)` where `A` is the sum of all input public keys and `outpoint_L` is the smallest outpoint sorted lexographically.
    /// `tweak = input_hash * A`
    pub fn scripts_from_tweak_data(&self, tweaks: &[PublicKey]) -> HashSet<ScriptBuf> {
        // Are these input_hash * A?
        let mut scripts = HashSet::new();
        for tweak in tweaks {
            let shared_secret = calculate_ecdh_shared_secret(tweak, &self.scan_secret_key);
            let potential_scripts = self.receiver.get_spks_from_shared_secret(&shared_secret);
            if let Ok(potential_scripts) = potential_scripts {
                potential_scripts
                    .values()
                    .into_iter()
                    .map(|buf| Script::from_bytes(buf).to_owned())
                    .for_each(|script| {
                        scripts.insert(script);
                    });
            }
        }
        scripts
    }
}

fn convert_from_bitcoin_network(network: bitcoin::Network) -> Network {
    match network {
        bitcoin::Network::Bitcoin => Network::Mainnet,
        bitcoin::Network::Testnet => Network::Testnet,
        bitcoin::Network::Signet => Network::Testnet,
        bitcoin::Network::Regtest => Network::Regtest,
        _ => unreachable!(),
    }
}
