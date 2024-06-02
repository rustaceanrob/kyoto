use bitcoin::{BlockHash, Transaction};

#[derive(Debug, Clone)]
pub struct IndexedTransaction {
    pub transaction: Transaction,
    pub height: Option<u32>,
    pub hash: BlockHash,
}

impl IndexedTransaction {
    pub fn new(transaction: Transaction, height: Option<u32>, hash: BlockHash) -> Self {
        Self {
            transaction,
            height,
            hash,
        }
    }
}
