use async_trait::async_trait;
use bitcoin::{BlockHash, Transaction};

use super::{error::TransactionStoreError, store::TransactionStore, types::IndexedTransaction};

#[derive(Debug)]
pub struct MemoryTransactionCache {
    transactions: Vec<IndexedTransaction>,
}

impl MemoryTransactionCache {
    pub fn new() -> Self {
        Self {
            transactions: vec![],
        }
    }
}

#[async_trait]
impl TransactionStore for MemoryTransactionCache {
    async fn add_transaction(
        &mut self,
        transaction: &Transaction,
        height: Option<u32>,
        hash: &BlockHash,
    ) -> Result<(), TransactionStoreError> {
        self.transactions.push(IndexedTransaction {
            transaction: transaction.clone(),
            height,
            hash: *hash,
        });
        Ok(())
    }

    async fn all_transactions(&self) -> Vec<IndexedTransaction> {
        self.transactions.clone()
    }
}
