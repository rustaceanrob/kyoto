use std::collections::{HashMap, HashSet};

use bitcoin::{bip158::BlockFilter, Block, BlockHash, FilterHash, ScriptBuf};
use bitcoin_hashes::{sha256d, Hash};

use super::error::FilterError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Filter {
    contents: Vec<u8>,
    block_hash: BlockHash,
    block_filter: BlockFilter,
}

impl Filter {
    pub fn new(contents: Vec<u8>, block_hash: BlockHash) -> Self {
        let block_filter = BlockFilter::new(&contents);
        Self {
            contents,
            block_hash,
            block_filter,
        }
    }

    pub async fn filter_hash(&self) -> FilterHash {
        let hash = sha256d::Hash::hash(&self.contents);
        FilterHash::from_raw_hash(hash)
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

    // pub async fn filter_hash_from_block(block: &Block) -> FilterHash {
    //     let mut txmap = HashMap::new();
    //     for tx in block.txdata.iter().skip(1) {
    //         for input in tx.input.iter() {
    //             txmap.insert(input.previous_output, );
    //         }
    //     }
    //     let block_filter = BlockFilter::new_script_filter(block, |o| {
    //         if let Some(s) = txmap.get(o) {
    //             Ok(s.clone())
    //         }
    //     });
    //     let hash = sha256d::Hash::hash(&self.contents);
    //     FilterHash::from_raw_hash(hash)
    // }
}
