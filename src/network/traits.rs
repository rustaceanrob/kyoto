use bitcoin::{
    p2p::message_filter::{GetCFHeaders, GetCFilters},
    BlockHash, Transaction, Wtxid,
};

use crate::channel_messages::GetBlockConfig;

use super::error::PeerError;

// Responsible for serializing messages to write over the wire, either encrypted or plaintext.
pub(crate) trait MessageGenerator: Send + Sync {
    fn version_message(&mut self, port: Option<u16>) -> Result<Vec<u8>, PeerError>;

    fn verack(&mut self) -> Result<Vec<u8>, PeerError>;

    fn addr(&mut self) -> Result<Vec<u8>, PeerError>;

    fn addrv2(&mut self) -> Result<Vec<u8>, PeerError>;

    fn wtxid_relay(&mut self) -> Result<Vec<u8>, PeerError>;

    fn headers(
        &mut self,
        locator_hashes: Vec<BlockHash>,
        stop_hash: Option<BlockHash>,
    ) -> Result<Vec<u8>, PeerError>;

    fn cf_headers(&mut self, message: GetCFHeaders) -> Result<Vec<u8>, PeerError>;

    fn filters(&mut self, message: GetCFilters) -> Result<Vec<u8>, PeerError>;

    fn block(&mut self, config: GetBlockConfig) -> Result<Vec<u8>, PeerError>;

    fn pong(&mut self, nonce: u64) -> Result<Vec<u8>, PeerError>;

    fn announce_transaction(&mut self, wtxid: Wtxid) -> Result<Vec<u8>, PeerError>;

    fn broadcast_transaction(&mut self, transaction: Transaction) -> Result<Vec<u8>, PeerError>;
}
