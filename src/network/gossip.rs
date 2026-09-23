use std::collections::HashSet;

use bitcoin::{OutPoint, ScriptBuf, Transaction};
use tokio::sync::mpsc::Sender;

#[derive(Debug, Clone, Copy)]
pub(crate) enum TransactionCheck {
    NoMatch,
    SentToClient,
    Failed,
}

#[derive(Debug)]
pub(crate) struct GossipMonitor {
    scripts: HashSet<ScriptBuf>,
    txins: HashSet<OutPoint>,
    sender: Sender<Transaction>,
}

impl GossipMonitor {
    pub(crate) fn new(
        scripts: HashSet<ScriptBuf>,
        txins: HashSet<OutPoint>,
        sender: Sender<Transaction>,
    ) -> Self {
        Self {
            scripts,
            txins,
            sender,
        }
    }

    pub(crate) fn extend(
        &mut self,
        scripts: impl IntoIterator<Item = ScriptBuf>,
        txins: impl IntoIterator<Item = OutPoint>,
    ) {
        self.scripts.extend(scripts);
        self.txins.extend(txins);
    }

    fn matches(&self, tx: &Transaction) -> bool {
        tx.input
            .iter()
            .any(|txin| self.txins.contains(&txin.previous_output))
            || tx
                .output
                .iter()
                .any(|txout| self.scripts.contains(&txout.script_pubkey))
    }

    pub(crate) fn check_tx(&self, tx: Transaction) -> TransactionCheck {
        if self.matches(&tx) {
            let sent = self.sender.try_send(tx).is_ok();
            if sent {
                TransactionCheck::SentToClient
            } else {
                TransactionCheck::Failed
            }
        } else {
            TransactionCheck::NoMatch
        }
    }
}
