use bitcoin::{BlockHash, Transaction};

#[derive(Debug, Clone)]
pub struct IndexedTransaction {
    pub transaction: Transaction,
    pub height: Option<usize>,
    pub hash: BlockHash,
}

impl IndexedTransaction {
    pub fn new(transaction: Transaction, height: Option<usize>, hash: BlockHash) -> Self {
        Self {
            transaction,
            height,
            hash,
        }
    }
}
