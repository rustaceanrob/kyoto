use async_trait::async_trait;
use bitcoin::{BlockHash, ScriptBuf, Transaction};

use super::{error::TransactionStoreError, types::IndexedTransaction};

#[async_trait]
pub trait TransactionStore {
    async fn add_transaction(
        &mut self,
        script: &ScriptBuf,
        transaction: &Transaction,
        height: Option<usize>,
        hash: &BlockHash,
    ) -> Result<(), TransactionStoreError>;

    async fn all_transactions(&self) -> Vec<IndexedTransaction>;
}
