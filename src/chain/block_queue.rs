use std::{
    collections::{HashSet, VecDeque},
    time::Duration,
};

use bitcoin::BlockHash;
use tokio::{sync::oneshot, time::Instant};

use crate::{error::FetchBlockError, messages::BlockRequest, IndexedBlock};

const SPAM_LIMIT: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub(crate) struct BlockQueue {
    queue: VecDeque<Request>,
    want: Option<Request>,
    last_req: Instant,
    completed: HashSet<BlockHash>,
}

impl BlockQueue {
    pub(crate) fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            want: None,
            last_req: Instant::now(),
            completed: HashSet::new(),
        }
    }

    pub(crate) fn add(&mut self, request: impl Into<Request>) {
        let request: Request = request.into();
        self.queue.push_front(request)
    }

    pub(crate) fn pop(&mut self) -> Option<BlockHash> {
        match self.want.as_mut() {
            Some(request) => {
                if self.last_req.elapsed() < SPAM_LIMIT {
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

    pub(crate) fn process_block(&mut self, block: &BlockHash) -> ProcessBlockResponse {
        if let Some(request) = self.want.take() {
            if request.hash.eq(block) {
                self.want = None;
                self.completed.insert(*block);
                return ProcessBlockResponse::Accepted {
                    block_recipient: request.recipient,
                };
            // We still need whatever hash is in the queue
            } else if self.completed.contains(block) {
                self.want = Some(request);
                return ProcessBlockResponse::LateResponse;
            } else {
                self.want = Some(request);
                return ProcessBlockResponse::UnknownHash;
            }
        }
        if self.completed.contains(block) {
            return ProcessBlockResponse::LateResponse;
        }
        ProcessBlockResponse::UnknownHash
    }

    #[allow(unused)]
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
    recipient: BlockRecipient,
}

impl Request {
    fn new(hash: BlockHash) -> Self {
        Self {
            hash,
            recipient: BlockRecipient::Event,
        }
    }

    fn from_block_request(block_request: BlockRequest) -> Self {
        Self {
            hash: block_request.hash,
            recipient: BlockRecipient::Client(block_request.oneshot),
        }
    }
}

impl From<BlockHash> for Request {
    fn from(value: BlockHash) -> Self {
        Request::new(value)
    }
}

impl From<BlockRequest> for Request {
    fn from(value: BlockRequest) -> Self {
        Request::from_block_request(value)
    }
}

#[derive(Debug)]
pub(crate) enum BlockRecipient {
    Client(oneshot::Sender<Result<IndexedBlock, FetchBlockError>>),
    Event,
}

#[derive(Debug)]
pub(crate) enum ProcessBlockResponse {
    Accepted { block_recipient: BlockRecipient },
    LateResponse,
    UnknownHash,
}

#[cfg(test)]
mod test {
    use std::str::FromStr;
    use std::time::Duration;

    use super::*;

    fn three_block_hashes() -> [BlockHash; 3] {
        let hash_1 =
            BlockHash::from_str("0000007a93b953158a12aef32eb9cc4366eb1eea5892fb04afbeec421c29319d")
                .unwrap();
        let hash_2 =
            BlockHash::from_str("0000009e41d363546c5126c045bdef80e863324ac87f2bec88927a53662f6c0b")
                .unwrap();
        let hash_3 =
            BlockHash::from_str("000000254633c01d43534d80981c3d1e0f4f3541cce2af68084e7631832d2572")
                .unwrap();
        [hash_1, hash_2, hash_3]
    }

    #[test]
    fn test_block_queue() {
        let [hash_1, hash_2, hash_3] = three_block_hashes();
        let mut queue = BlockQueue::new();
        queue.add(hash_1);
        queue.add(hash_2);
        queue.add(hash_3);
        queue.add(hash_1);
        assert_eq!(queue.queue.len(), 4);
        assert_eq!(queue.pop(), Some(hash_1));
        assert_eq!(queue.pop(), None);
        assert_eq!(
            queue.want.as_ref().map(|request| request.hash),
            Some(hash_1)
        );
        queue.process_block(&hash_1);
        assert_eq!(queue.want.as_ref().map(|request| request.hash), None);
        assert_eq!(queue.pop(), Some(hash_2));
        assert_eq!(
            queue.want.as_ref().map(|request| request.hash),
            Some(hash_2)
        );
        queue.process_block(&hash_2);
        assert_eq!(queue.pop(), Some(hash_3));
        assert!(!queue.complete());
        assert_eq!(queue.pop(), None);
        assert!(!queue.complete());
        queue.process_block(&hash_2);
        assert!(!queue.complete());
        queue.process_block(&hash_3);
        assert!(!queue.complete());
        assert_eq!(queue.pop(), Some(hash_1));
        queue.process_block(&hash_1);
        assert!(queue.complete());
    }

    #[tokio::test(start_paused = true)]
    async fn test_laggy_peer() {
        let [hash_1, hash_2, hash_3] = three_block_hashes();
        let mut queue = BlockQueue::new();
        queue.add(hash_1);
        queue.add(hash_2);
        queue.add(hash_3);
        assert_eq!(queue.queue.len(), 3);
        assert_eq!(queue.pop(), Some(hash_1));
        tokio::time::sleep(Duration::from_secs(6)).await;
        assert_eq!(queue.pop(), Some(hash_1));
        assert_eq!(
            queue.want.as_ref().map(|request| request.hash),
            Some(hash_1)
        );
        queue.process_block(&hash_1);
        assert_eq!(queue.want.as_ref().map(|request| request.hash), None);
        assert_eq!(queue.pop(), Some(hash_2));
        assert_eq!(
            queue.want.as_ref().map(|request| request.hash),
            Some(hash_2)
        );
        queue.process_block(&hash_2);
        assert_eq!(queue.pop(), Some(hash_3));
        assert!(!queue.complete());
        assert_eq!(queue.pop(), None);
        assert!(!queue.complete());
        let response = queue.process_block(&hash_2);
        assert!(matches!(response, ProcessBlockResponse::LateResponse));
        assert!(!queue.complete());
        tokio::time::sleep(Duration::from_secs(6)).await;
        assert_eq!(queue.pop(), Some(hash_3));
        assert!(!queue.complete());
        queue.process_block(&hash_3);
        assert!(queue.complete());
        assert_eq!(queue.pop(), None);
        let response = queue.process_block(&hash_3);
        assert!(matches!(response, ProcessBlockResponse::LateResponse));
    }

    #[test]
    fn test_blocks_removed() {
        let [hash_1, hash_2, hash_3] = three_block_hashes();
        let mut queue = BlockQueue::new();
        queue.add(hash_1);
        queue.add(hash_2);
        queue.add(hash_3);
        queue.add(hash_1);
        assert_eq!(queue.queue.len(), 4);
        assert_eq!(queue.pop(), Some(hash_1));
        assert_eq!(
            queue.want.as_ref().map(|request| request.hash),
            Some(hash_1)
        );
        queue.remove(&[hash_1]);
        assert_eq!(queue.want.as_ref().map(|request| request.hash), None);
        queue.remove(&[hash_2]);
        assert_eq!(queue.queue.len(), 1);
        assert_eq!(queue.pop(), Some(hash_3));
    }
}
