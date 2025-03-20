use std::collections::VecDeque;

use bitcoin::BlockHash;
use tokio::time::Instant;

#[cfg(feature = "filter-control")]
use crate::messages::BlockRequest;
use crate::messages::BlockSender;

const SPAM_LIMIT: u64 = 5;

#[derive(Debug)]
pub(crate) struct BlockQueue {
    queue: VecDeque<Request>,
    want: Option<Request>,
    last_req: Instant,
}

impl BlockQueue {
    pub(crate) fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            want: None,
            last_req: Instant::now(),
        }
    }

    pub(crate) fn add(&mut self, request: impl Into<Request>) {
        let request: Request = request.into();
        if !self.contains(&request.hash) {
            self.queue.push_front(request)
        }
    }

    pub(crate) fn contains(&mut self, block: &BlockHash) -> bool {
        self.queue.iter().any(|request| request.hash.eq(block))
            || self
                .want
                .as_ref()
                .map_or(false, |request| request.hash.eq(block))
    }

    pub(crate) fn pop(&mut self) -> Option<BlockHash> {
        match self.want.as_mut() {
            Some(request) => {
                if Instant::now().duration_since(self.last_req).as_secs() < SPAM_LIMIT {
                    None
                } else {
                    self.last_req = Instant::now();
                    Some(request.hash)
                }
            }
            None => {
                self.last_req = Instant::now();
                let request = self.queue.pop_back();
                let hash = request.as_ref().map(|request| request.hash);
                self.want = request;
                hash
            }
        }
    }

    pub(crate) fn need(&self, block: &BlockHash) -> bool {
        self.want
            .as_ref()
            .map_or(false, |request| request.hash.eq(block))
    }

    pub(crate) fn receive(&mut self, hash: &BlockHash) -> Option<BlockSender> {
        if let Some(request) = self.want.take() {
            if request.hash.eq(hash) {
                self.want = None;
                return request.sender;
            } else {
                self.want = Some(request);
                return None;
            }
        }
        None
    }

    pub(crate) fn complete(&self) -> bool {
        self.want.is_none() && self.queue.is_empty()
    }

    pub(crate) fn remove(&mut self, hashes: &[BlockHash]) {
        self.queue.retain(|request| !hashes.contains(&request.hash));
        if let Some(want) = self.want.as_ref() {
            if hashes.contains(&want.hash) {
                self.want = None;
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct Request {
    hash: BlockHash,
    sender: Option<BlockSender>,
}

impl Request {
    fn new(hash: BlockHash) -> Self {
        Self { hash, sender: None }
    }

    #[cfg(feature = "filter-control")]
    fn from_block_request(block_request: BlockRequest) -> Self {
        Self {
            hash: block_request.hash,
            sender: Some(block_request.oneshot),
        }
    }
}

impl From<BlockHash> for Request {
    fn from(value: BlockHash) -> Self {
        Request::new(value)
    }
}

#[cfg(feature = "filter-control")]
impl From<BlockRequest> for Request {
    fn from(value: BlockRequest) -> Self {
        Request::from_block_request(value)
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
        assert_eq!(queue.pop(), None);
        assert_eq!(
            queue.want.as_ref().map(|request| request.hash),
            Some(hash_1)
        );
        assert!(queue.need(&hash_1));
        queue.receive(&hash_1);
        assert_eq!(queue.want.as_ref().map(|request| request.hash), None);
        assert_eq!(queue.pop(), Some(hash_2));
        assert_eq!(
            queue.want.as_ref().map(|request| request.hash),
            Some(hash_2)
        );
        assert!(queue.need(&hash_2));
        queue.receive(&hash_2);
        assert_eq!(queue.pop(), Some(hash_3));
        assert!(!queue.complete());
        assert_eq!(queue.pop(), None);
        assert!(!queue.complete());
        assert!(queue.need(&hash_3));
        queue.receive(&hash_2);
        assert!(queue.need(&hash_3));
        assert!(!queue.complete());
        queue.receive(&hash_3);
        assert!(queue.complete());
        assert!(!queue.need(&hash_3));
        assert_eq!(queue.pop(), None);
    }

    #[tokio::test]
    async fn test_laggy_peer() {
        use std::time::Duration;
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
        tokio::time::sleep(Duration::from_secs(6)).await;
        assert_eq!(queue.pop(), Some(hash_1));
        assert_eq!(
            queue.want.as_ref().map(|request| request.hash),
            Some(hash_1)
        );
        assert!(queue.need(&hash_1));
        queue.receive(&hash_1);
        assert!(!queue.need(&hash_1));
        assert_eq!(queue.want.as_ref().map(|request| request.hash), None);
        assert_eq!(queue.pop(), Some(hash_2));
        assert_eq!(
            queue.want.as_ref().map(|request| request.hash),
            Some(hash_2)
        );
        assert!(queue.need(&hash_2));
        queue.receive(&hash_2);
        assert_eq!(queue.pop(), Some(hash_3));
        assert!(!queue.complete());
        assert_eq!(queue.pop(), None);
        assert!(!queue.complete());
        queue.receive(&hash_2);
        assert!(!queue.complete());
        tokio::time::sleep(Duration::from_secs(6)).await;
        assert_eq!(queue.pop(), Some(hash_3));
        assert!(queue.need(&hash_3));
        assert!(!queue.complete());
        queue.receive(&hash_3);
        assert!(queue.complete());
        assert_eq!(queue.pop(), None);
    }

    #[test]
    fn test_blocks_removed() {
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
        assert_eq!(
            queue.want.as_ref().map(|request| request.hash),
            Some(hash_1)
        );
        queue.remove(&[hash_1]);
        assert!(!queue.need(&hash_1));
        assert_eq!(queue.want.as_ref().map(|request| request.hash), None);
        queue.remove(&[hash_2]);
        assert_eq!(queue.queue.len(), 1);
        assert_eq!(queue.pop(), Some(hash_3));
    }
}
