use bitcoin::BlockHash;

#[derive(Debug)]
pub(crate) struct BlockQueue {
    queue: Vec<BlockHash>,
    received: usize,
}

impl BlockQueue {
    pub(crate) fn new() -> Self {
        Self {
            queue: Vec::new(),
            received: 0,
        }
    }

    pub(crate) fn add(&mut self, block: BlockHash) {
        if !self.contains(&block) {
            self.received += 1;
            self.queue.push(block)
        }
    }

    pub(crate) fn contains(&mut self, block: &BlockHash) -> bool {
        self.queue.contains(block)
    }

    pub(crate) fn pop(&mut self) -> Option<BlockHash> {
        self.queue.pop()
    }

    pub(crate) fn receive_one(&mut self) {
        self.received = self.received.saturating_sub(1);
    }

    pub(crate) fn complete(&self) -> bool {
        self.received.eq(&0) && self.queue.is_empty()
    }
}
