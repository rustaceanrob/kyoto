use std::collections::HashSet;

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

    fn block_hash(&self) -> &BlockHash {
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

    pub async fn is_filter_for_block(&mut self, block: &Block) -> Result<bool, FilterError> {
        // Skip the coinbase transaction
        for tx in block.txdata.iter().skip(1) {
            let scripts = tx
                .output
                .iter()
                .filter(|output| !output.script_pubkey.is_op_return())
                .map(|output| output.script_pubkey.clone());
            if !self
                .block_filter
                .match_all(
                    &self.block_hash,
                    &mut scripts.map(|script| script.to_bytes()),
                )
                .map_err(|_| FilterError::IORead)?
            {
                return Ok(false);
            }
            // The filter should not contain OP_RETURN outputs
            // let scripts = tx
            //     .output
            //     .iter()
            //     .filter(|output| output.script_pubkey.is_op_return())
            //     .map(|output| output.script_pubkey.clone());
            // if self
            //     .block_filter
            //     .match_any(
            //         &self.block_hash,
            //         &mut scripts.map(|script| script.to_bytes()),
            //     )
            //     .map_err(|_| FilterError::IORead)?
            // {
            //     return Ok(false);
            // }
        }
        Ok(true)
    }
}
