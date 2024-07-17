use crate::TxBroadcast;

#[derive(Debug, Clone)]
pub(crate) struct Broadcaster {
    queue: Vec<TxBroadcast>,
}

impl Broadcaster {
    pub(crate) fn new() -> Self {
        Self { queue: Vec::new() }
    }

    pub(crate) fn add(&mut self, tx: TxBroadcast) {
        self.queue.push(tx)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub(crate) fn queue(&mut self) -> Vec<TxBroadcast> {
        core::mem::take(&mut self.queue)
    }

    pub(crate) fn next(&mut self) -> Option<TxBroadcast> {
        self.queue.pop()
    }
}

#[cfg(test)]
mod tests {
    use bitcoin::{consensus::deserialize, Transaction};

    use crate::TxBroadcast;

    use super::Broadcaster;

    #[test]
    fn test_broadcast_queue_works() {
        // Sourced from BIP 174 test vectors
        let transaction_1: Transaction = deserialize(&hex::decode("0200000000010158e87a21b56daf0c23be8e7070456c336f7cbaa5c8757924f545887bb2abdd7501000000171600145f275f436b09a8cc9a2eb2a2f528485c68a56323feffffff02d8231f1b0100000017a914aed962d6654f9a2b36608eb9d64d2b260db4f1118700c2eb0b0000000017a914b7f5faf40e3d40a5a459b1db3535f2b72fa921e88702483045022100a22edcc6e5bc511af4cc4ae0de0fcd75c7e04d8c1c3a8aa9d820ed4b967384ec02200642963597b9b1bc22c75e9f3e117284a962188bf5e8a74c895089046a20ad770121035509a48eb623e10aace8bfd0212fdb8a8e5af3c94b0b133b95e114cab89e4f7965000000").unwrap()).unwrap();
        let transaction_2: Transaction = deserialize(&hex::decode("0200000001aad73931018bd25f84ae400b68848be09db706eac2ac18298babee71ab656f8b0000000048473044022058f6fc7c6a33e1b31548d481c826c015bd30135aad42cd67790dab66d2ad243b02204a1ced2604c6735b6393e5b41691dd78b00f0c5942fb9f751856faa938157dba01feffffff0280f0fa020000000017a9140fb9463421696b82c833af241c78c17ddbde493487d0f20a270100000017a91429ca74f8a08f81999428185c97b5d852e4063f618765000000").unwrap()).unwrap();
        let tx_1 = TxBroadcast::new(transaction_1, crate::TxBroadcastPolicy::AllPeers);
        let tx_2 = TxBroadcast::new(transaction_2, crate::TxBroadcastPolicy::AllPeers);
        let mut queue = Broadcaster::new();
        assert!(queue.is_empty());
        queue.add(tx_1.clone());
        assert!(!queue.is_empty());
        let tx = queue.next();
        assert!(tx.is_some());
        assert!(queue.is_empty());
        queue.add(tx_1);
        queue.add(tx_2.clone());
        assert!(!queue.is_empty());
        let txs = queue.queue();
        assert_eq!(txs.len(), 2);
        assert!(queue.is_empty());
        queue.add(tx_2);
        assert!(!queue.is_empty());
    }
}
