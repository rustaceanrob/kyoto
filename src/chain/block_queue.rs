use std::collections::VecDeque;

use bitcoin::BlockHash;

#[derive(Debug)]
pub(crate) struct BlockQueue {
    queue: VecDeque<BlockHash>,
    want: usize,
}

impl BlockQueue {
    pub(crate) fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            want: 0,
        }
    }

    pub(crate) fn add(&mut self, block: BlockHash) {
        if !self.contains(&block) {
            self.want += 1;
            self.queue.push_front(block)
        }
    }

    pub(crate) fn contains(&mut self, block: &BlockHash) -> bool {
        self.queue.contains(block)
    }

    pub(crate) fn pop(&mut self) -> Option<BlockHash> {
        self.queue.pop_back()
    }

    pub(crate) fn receive_one(&mut self) {
        self.want = self.want.saturating_sub(1);
    }

    pub(crate) fn complete(&self) -> bool {
        self.want.eq(&0) && self.queue.is_empty()
    }
}
