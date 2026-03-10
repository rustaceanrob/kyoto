use std::collections::HashMap;

use bitcoin::{Transaction, Wtxid};
use tokio::sync::oneshot;

/// One Parent One Child (1P1C) package for TRUC policy
#[derive(Debug)]
pub(crate) struct OneParentOneChild {
    pub(crate) child: Transaction,
    pub(crate) parent: Transaction,
    child_broadcast: bool,
    parent_broadcast: bool,
    sender: Option<oneshot::Sender<Wtxid>>,
    parent_wtxid: Wtxid,
}

impl OneParentOneChild {
    pub(crate) fn new(child: Transaction, parent: Transaction, sender: oneshot::Sender<Wtxid>) -> Self {
        let parent_wtxid = parent.compute_wtxid();
        Self {
            child,
            parent,
            child_broadcast: false,
            parent_broadcast: false,
            sender: Some(sender),
            parent_wtxid,
        }
    }

    pub(crate) fn child_wtxid(&self) -> Wtxid {
        self.child.compute_wtxid()
    }

    pub(crate) fn parent_wtxid(&self) -> Wtxid {
        self.parent_wtxid
    }

    /// Mark the child as broadcast (GETDATA'd by a peer)
    fn child_success(&mut self) -> bool {
        self.child_broadcast = true;
        self.check_complete()
    }

    /// Mark the parent as broadcast (GETDATA'd by a peer)
    fn parent_success(&mut self) -> bool {
        self.parent_broadcast = true;
        self.check_complete()
    }

    /// Check if both child and parent have been broadcast
    fn check_complete(&mut self) -> bool {
        if self.child_broadcast && self.parent_broadcast {
            if let Some(sender) = self.sender.take() {
                let _ = sender.send(self.parent_wtxid);
            }
            true
        } else {
            false
        }
    }
}

#[derive(Debug)]
pub(crate) struct BroadcastQueue {
    pending: HashMap<Wtxid, oneshot::Sender<Wtxid>>,
    data: HashMap<Wtxid, Transaction>,
    /// 1P1C packages keyed by child wtxid
    pending_1p1c: HashMap<Wtxid, OneParentOneChild>,
}

impl BroadcastQueue {
    pub(crate) fn new() -> Self {
        Self {
            pending: HashMap::new(),
            data: HashMap::new(),
            pending_1p1c: HashMap::new(),
        }
    }

    pub(crate) fn add_to_queue(&mut self, tx: Transaction, oneshot: oneshot::Sender<Wtxid>) {
        let wtxid = tx.compute_wtxid();
        self.pending.insert(wtxid, oneshot);
        self.data.insert(wtxid, tx);
    }

    pub(crate) fn add_1p1c(&mut self, child: Transaction, parent: Transaction, oneshot: oneshot::Sender<Wtxid>) {
        // Compute wtxids before moving transactions
        let child_wtxid = child.compute_wtxid();
        let parent_wtxid = parent.compute_wtxid();

        let package = OneParentOneChild::new(child, parent, oneshot);

        // Store both transactions in data
        self.data.insert(child_wtxid, package.child.clone());
        self.data.insert(parent_wtxid, package.parent.clone());

        // Store the 1P1C package keyed by child wtxid
        self.pending_1p1c.insert(child_wtxid, package);
    }

    pub(crate) fn fetch_tx(&self, wtxid: Wtxid) -> Option<Transaction> {
        self.data.get(&wtxid).cloned()
    }

    /// Mark a transaction as successful (GETDATA'd by a peer).
    /// Returns the child_wtxid if a 1P1C package was completed, None otherwise.
    pub(crate) fn successful(&mut self, wtxid: Wtxid) -> Option<Wtxid> {
        // First check if this is a 1P1C package (keyed by child wtxid)
        if let Some(package) = self.pending_1p1c.get_mut(&wtxid) {
            // Determine if this is the child or parent based on which wtxid matches
            let completed = if wtxid == package.child_wtxid() {
                package.child_success()
            } else if wtxid == package.parent_wtxid() {
                package.parent_success()
            } else {
                return None;
            };
            // If both are done, return the child wtxid so caller can clean up
            if completed {
                return Some(package.child_wtxid());
            }
            return None;
        }

        // Regular single transaction
        if let Some(pending) = self.pending.remove(&wtxid) {
            let _ = pending.send(wtxid);
        }
        None
    }

    /// Remove a completed 1P1C package from the queue
    pub(crate) fn successful_completed(&mut self, child_wtxid: Wtxid) {
        if let Some(package) = self.pending_1p1c.remove(&child_wtxid) {
            // Remove both transactions from data
            self.data.remove(&package.child_wtxid());
            self.data.remove(&package.parent_wtxid());
        }
    }

    pub(crate) fn pending_wtxid(&self) -> Vec<Wtxid> {
        let mut wtxids = self.pending.keys().copied().collect::<Vec<_>>();
        // Add child wtxids from 1P1C packages
        for package in self.pending_1p1c.values() {
            wtxids.push(package.child_wtxid());
        }
        wtxids
    }

    /// Check if a wtxid belongs to a 1P1C package
    pub(crate) fn is_1p1c(&self, wtxid: Wtxid) -> bool {
        self.pending_1p1c.contains_key(&wtxid)
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
        queue.add_to_queue(transaction_1.clone(), tx);
        let (tx, _) = tokio::sync::oneshot::channel();
        queue.add_to_queue(transaction_2.clone(), tx);
        assert_eq!(queue.pending_wtxid().len(), 2);
        queue.successful(transaction_1.compute_wtxid());
        assert_eq!(queue.pending_wtxid().len(), 1);
        assert!(queue.fetch_tx(transaction_1.compute_wtxid()).is_some());
        assert!(queue.fetch_tx(transaction_2.compute_wtxid()).is_some());
        queue.successful(transaction_2.compute_wtxid());
        assert_eq!(queue.pending_wtxid().len(), 0);
    }
}
