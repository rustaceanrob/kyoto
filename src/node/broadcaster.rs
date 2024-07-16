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
        let ret = self.queue.clone();
        self.queue = Vec::new();
        ret
    }

    pub(crate) fn next(&mut self) -> Option<TxBroadcast> {
        self.queue.pop()
    }
}
