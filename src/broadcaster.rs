use std::collections::{HashMap, HashSet};

use bitcoin::{Transaction, Txid, Wtxid};
use tokio::sync::oneshot;

use crate::Package;

#[derive(Debug)]
pub(crate) struct BroadcastQueue {
    // There are the transactions that a peer should receive first. In the case of 1p1c, these are
    // the `Wtxid` of the child transaction in the package.
    advertise: HashSet<Wtxid>,
    // Notify the user when:
    // 1. a singleton transaction was broadcast
    // 2. the final transaction in a package was broadcast
    callbacks: HashMap<Wtxid, (oneshot::Sender<Wtxid>, Wtxid)>,
    // These transactions will be fetched by the usual `Wtxid`.
    witness_data: HashMap<Wtxid, Transaction>,
    // These transactions represent missing inputs to a previously broadcast transaction. Because
    // the inputs use the legacy `Txid` in the outpoint, these transactions are indexed by `Txid`.
    legacy_data: HashMap<Txid, Transaction>,
}

impl BroadcastQueue {
    pub(crate) fn new() -> Self {
        Self {
            advertise: HashSet::new(),
            callbacks: HashMap::new(),
            witness_data: HashMap::new(),
            legacy_data: HashMap::new(),
        }
    }

    pub(crate) fn add_to_queue(&mut self, package: Package, oneshot: oneshot::Sender<Wtxid>) {
        let advertise_wtxid = package.advertise_package();
        self.advertise.insert(advertise_wtxid);
        let parent = package.parent();
        let parent_txid = parent.compute_txid();
        let parent_wtxid = parent.compute_wtxid();
        match package.child() {
            Some(child) => {
                let child_wtxid = child.compute_wtxid();
                // Only confirm once the parent is confirmed to have been requested.
                self.callbacks.insert(parent_wtxid, (oneshot, child_wtxid));
                self.witness_data.insert(child_wtxid, child);
                // The only way a peer can feasibly request this transaction is by `Txid`, as it is
                // never advertised explicitly.
                self.legacy_data.insert(parent_txid, parent);
            }
            None => {
                self.callbacks.insert(parent_wtxid, (oneshot, parent_wtxid));
                self.witness_data.insert(parent_wtxid, parent);
            }
        }
    }

    pub(crate) fn fetch_tx(&self, id: impl Into<TxIdentifier>) -> Option<Transaction> {
        let id = id.into();
        match id {
            TxIdentifier::Legacy(txid) => self.legacy_data.get(&txid).cloned(),
            TxIdentifier::Witness(wtxid) => self.witness_data.get(&wtxid).cloned(),
        }
    }

    pub(crate) fn sent_transaction_payload(&mut self, wtxid: Wtxid) {
        if let Some((callback, child)) = self.callbacks.remove(&wtxid) {
            self.advertise.remove(&child);
            let _ = callback.send(child);
        }
    }

    pub(crate) fn pending_wtxid(&self) -> Vec<Wtxid> {
        self.advertise.iter().copied().collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, std::hash::Hash)]
pub(crate) enum TxIdentifier {
    Legacy(Txid),
    Witness(Wtxid),
}

impl From<Txid> for TxIdentifier {
    fn from(value: Txid) -> Self {
        Self::Legacy(value)
    }
}

impl From<Wtxid> for TxIdentifier {
    fn from(value: Wtxid) -> Self {
        Self::Witness(value)
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use bitcoin::Transaction;
    use corepc_node::serde_json;

    use super::BroadcastQueue;

    #[derive(Debug, Clone)]
    struct HexTx(Transaction);
    crate::impl_deserialize!(HexTx, Transaction);

    #[derive(Debug, Clone, serde::Deserialize)]
    struct TransactionFile {
        transactions: Vec<HexTx>,
    }

    #[test]
    fn test_broadcast_queue_works() {
        // Sourced from BIP 174 test vectors
        let tx_file = File::open("./tests/data/transactions.json").unwrap();
        let tx_data: TransactionFile = serde_json::from_reader(&tx_file).unwrap();
        let transaction_1: Transaction = tx_data.transactions[0].clone().0;
        let transaction_2: Transaction = tx_data.transactions[1].clone().0;
        let mut queue = BroadcastQueue::new();
        let (tx, _) = tokio::sync::oneshot::channel();
        queue.add_to_queue(transaction_1.clone().into(), tx);
        let (tx, _) = tokio::sync::oneshot::channel();
        queue.add_to_queue(transaction_2.clone().into(), tx);
        assert_eq!(queue.pending_wtxid().len(), 2);
        queue.sent_transaction_payload(transaction_1.compute_wtxid());
        assert_eq!(queue.pending_wtxid().len(), 1);
        assert!(queue.fetch_tx(transaction_1.compute_wtxid()).is_some());
        assert!(queue.fetch_tx(transaction_2.compute_wtxid()).is_some());
        queue.sent_transaction_payload(transaction_2.compute_wtxid());
        assert_eq!(queue.pending_wtxid().len(), 0);
    }
}
