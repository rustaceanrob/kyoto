pub(crate) const CF_HEADER_BATCH_SIZE: u32 = 1_999;
pub(crate) const FILTER_BATCH_SIZE: u32 = 99;

pub(crate) mod cfheader_batch;
pub(crate) mod cfheader_chain;
#[allow(dead_code)]
pub(crate) mod error;
pub(crate) mod filter_chain;

use std::collections::HashSet;

use bitcoin::{bip158::BlockFilter, BlockHash, FilterHash, ScriptBuf};
use bitcoin_hashes::{sha256d, Hash};

use error::FilterError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Filter {
    filter_hash: FilterHash,
    block_hash: BlockHash,
    block_filter: BlockFilter,
}

#[allow(dead_code)]
impl Filter {
    pub fn new(contents: Vec<u8>, block_hash: BlockHash) -> Self {
        let hash = sha256d::Hash::hash(&contents);
        let filter_hash = FilterHash::from_raw_hash(hash);
        let block_filter = BlockFilter::new(&contents);
        Self {
            filter_hash,
            block_hash,
            block_filter,
        }
    }

    pub fn filter_hash(&self) -> &FilterHash {
        &self.filter_hash
    }

    pub fn block_hash(&self) -> &BlockHash {
        &self.block_hash
    }

    pub async fn contains_any(
        &mut self,
        scripts: &HashSet<ScriptBuf>,
    ) -> Result<bool, FilterError> {
        self.block_filter
            .match_any(
                &self.block_hash,
                &mut scripts.iter().map(|script| script.to_bytes()),
            )
            .map_err(|_| FilterError::IORead)
    }
}
