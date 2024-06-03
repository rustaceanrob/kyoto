use async_trait::async_trait;
use bitcoin::{BlockHash, Transaction};

use crate::IndexedTransaction;

use super::error::TransactionStoreError;

#[async_trait]
pub trait TransactionStore {
    async fn add_transaction(
        &mut self,
        transaction: &Transaction,
        height: u32,
        hash: &BlockHash,
    ) -> Result<(), TransactionStoreError>;

    async fn all_transactions(&self) -> Vec<IndexedTransaction>;
}
