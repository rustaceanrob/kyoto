use bitcoin::{BlockHash, ScriptBuf, Transaction};

#[derive(Debug, Clone)]
pub struct IndexedTransaction {
    pub script: ScriptBuf,
    pub transaction: Transaction,
    pub height: Option<usize>,
    pub hash: BlockHash,
}
