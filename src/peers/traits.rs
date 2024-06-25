use bitcoin::{
    p2p::message_filter::{GetCFHeaders, GetCFilters},
    BlockHash, Transaction,
};

use crate::node::channel_messages::GetBlockConfig;

pub(crate) trait MessageGenerator {
    fn version_message(&mut self, port: Option<u16>) -> Vec<u8>;

    fn verack(&mut self) -> Vec<u8>;

    fn get_addr(&mut self) -> Vec<u8>;

    fn get_headers(
        &mut self,
        locator_hashes: Vec<BlockHash>,
        stop_hash: Option<BlockHash>,
    ) -> Vec<u8>;

    fn cf_headers(&mut self, message: GetCFHeaders) -> Vec<u8>;

    fn filters(&mut self, message: GetCFilters) -> Vec<u8>;

    fn block(&mut self, config: GetBlockConfig) -> Vec<u8>;

    fn pong(&mut self, nonce: u64) -> Vec<u8>;

    fn transaction(&mut self, transaction: Transaction) -> Vec<u8>;
}
