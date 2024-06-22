use std::collections::VecDeque;

use bitcoin::BlockHash;

#[derive(Debug)]
pub(crate) struct BlockQueue {
    queue: VecDeque<BlockHash>,
    want: Option<BlockHash>,
}

impl BlockQueue {
    pub(crate) fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            want: None,
        }
    }

    pub(crate) fn add(&mut self, block: BlockHash) {
        if !self.contains(&block) {
            self.queue.push_front(block)
        }
    }

    pub(crate) fn contains(&mut self, block: &BlockHash) -> bool {
        self.queue.contains(block)
    }

    pub(crate) fn pop(&mut self) -> Option<BlockHash> {
        match self.want {
            Some(block) => Some(block),
            None => {
                let block = self.queue.pop_back();
                self.want = block;
                block
            }
        }
    }

    pub(crate) fn received(&mut self, hash: &BlockHash) -> bool {
        match self.want {
            Some(want) => {
                if want.eq(hash) {
                    self.want = None;
                    return true;
                }
                false
            }
            None => false,
        }
    }

    pub(crate) fn complete(&self) -> bool {
        self.want.is_none() && self.queue.is_empty()
    }
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn test_block_queue() {
        let hash_1 =
            BlockHash::from_str("0000007a93b953158a12aef32eb9cc4366eb1eea5892fb04afbeec421c29319d")
                .unwrap();
        let hash_2 =
            BlockHash::from_str("0000009e41d363546c5126c045bdef80e863324ac87f2bec88927a53662f6c0b")
                .unwrap();
        let hash_3 =
            BlockHash::from_str("000000254633c01d43534d80981c3d1e0f4f3541cce2af68084e7631832d2572")
                .unwrap();
        let mut queue = BlockQueue::new();
        queue.add(hash_1);
        queue.add(hash_2);
        queue.add(hash_3);
        queue.add(hash_1);
        assert_eq!(queue.queue.len(), 3);
        assert_eq!(queue.pop(), Some(hash_1));
        assert_eq!(queue.pop(), Some(hash_1));
        assert_eq!(queue.want, Some(hash_1));
        queue.received(&hash_1);
        assert_eq!(queue.want, None);
        assert_eq!(queue.pop(), Some(hash_2));
        assert_eq!(queue.want, Some(hash_2));
        queue.received(&hash_2);
        assert_eq!(queue.pop(), Some(hash_3));
        assert!(!queue.complete());
        assert_eq!(queue.pop(), Some(hash_3));
        assert!(!queue.complete());
        queue.received(&hash_2);
        assert_eq!(queue.pop(), Some(hash_3));
        assert!(!queue.complete());
        assert_eq!(queue.pop(), Some(hash_3));
        assert!(!queue.complete());
        queue.received(&hash_3);
        assert!(queue.complete());
        assert_eq!(queue.pop(), None);
    }
}
