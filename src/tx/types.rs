use bitcoin::{BlockHash, Transaction};

#[derive(Debug, Clone)]
pub struct IndexedTransaction {
    pub transaction: Transaction,
    pub height: Option<usize>,
    pub hash: BlockHash,
}
