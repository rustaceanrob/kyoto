use async_trait::async_trait;
use bitcoin::{BlockHash, ScriptBuf, Transaction};

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
        script: &ScriptBuf,
        transaction: &Transaction,
        height: Option<usize>,
        hash: &BlockHash,
    ) -> Result<(), TransactionStoreError> {
        self.transactions.push(IndexedTransaction {
            script: script.clone(),
            transaction: transaction.clone(),
            height,
            hash: hash.clone(),
        });
        Ok(())
    }

    async fn all_transactions(&self) -> Vec<IndexedTransaction> {
        self.transactions.clone()
    }
}
