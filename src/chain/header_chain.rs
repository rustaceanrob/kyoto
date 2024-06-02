use bitcoin::{block::Header, BlockHash, Work};

use crate::prelude::MEDIAN_TIME_PAST;

use super::checkpoints::HeaderCheckpoint;

pub(crate) type Headers = Vec<Header>;

const LOCATOR_LOOKBACKS: &[usize] = &[1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024];
const MAX_LOOKBACK: usize = 1025;

#[derive(Debug)]
pub(crate) struct HeaderChain {
    anchor_checkpoint: HeaderCheckpoint,
    headers: Headers,
}

impl HeaderChain {
    pub(crate) fn new(checkpoint: HeaderCheckpoint, headers: Headers) -> Self {
        Self {
            anchor_checkpoint: checkpoint,
            headers,
        }
    }

    pub(crate) fn root(&self) -> BlockHash {
        match self.headers.first() {
            Some(header) => header.block_hash(),
            None => self.anchor_checkpoint.hash,
        }
    }

    // Top of the chain
    pub(crate) fn tip(&self) -> BlockHash {
        match self.headers.last() {
            Some(header) => header.block_hash(),
            None => self.anchor_checkpoint.hash,
        }
    }

    // The canoncial height of the chain, one less than the length
    pub(crate) fn height(&self) -> u32 {
        self.headers.len() as u32 + self.anchor_checkpoint.height
    }

    // The adjusted height with respect to the indexes of the underlying vector.
    // We do not actually have the header at the anchor height, so we need to offset
    // the height by 1 to reflect that requesting height 0, for instance, will not
    // yield a valid height.
    fn adjusted_height(&self, height: u32) -> Option<u32> {
        height.checked_sub(self.anchor_checkpoint.height + 1)
    }

    // The length of the chain we have interally
    pub(crate) fn inner_len(&self) -> usize {
        self.headers().len()
    }

    // All the headers of the chain
    pub(crate) fn headers(&self) -> &Headers {
        &self.headers
    }

    // This header chain contains a block hash
    pub(crate) fn contains_hash(&self, blockhash: BlockHash) -> bool {
        self.headers
            .iter()
            .any(|header| header.block_hash().eq(&blockhash))
            || blockhash.eq(&self.anchor_checkpoint.hash)
    }

    // The height of the blockhash in the chain, accounting for the chain offset
    pub(crate) async fn height_of_hash(&self, blockhash: BlockHash) -> Option<u32> {
        let offset_pos = self
            .headers
            .iter()
            .position(|header| header.block_hash().eq(&blockhash));
        offset_pos.map(|index| self.anchor_checkpoint.height + index as u32 + 1)
    }

    // This header chain contains a block hash
    pub(crate) fn header_at_height(&self, height: u32) -> Option<&Header> {
        let offset = self.adjusted_height(height);
        match offset {
            Some(index) => self.headers.get(index as usize),
            None => None,
        }
    }

    // This header chain contains a block hash
    pub(crate) fn contains_header(&self, header: Header) -> bool {
        self.headers.contains(&header)
    }

    // Compute the total work for the chain
    fn get_chainwork(&self, headers: &Headers) -> Work {
        let work = headers
            .iter()
            .map(|header| header.work())
            .reduce(|acc, next| acc + next);
        match work {
            Some(w) => w,
            None => Work::from_be_bytes([0; 32]),
        }
    }

    // Canoncial chainwork from the anchor checkpoint
    pub(crate) fn chainwork(&self) -> Work {
        self.get_chainwork(&self.headers)
    }

    // Calculate the chainwork after a fork height to evalutate the fork
    pub(crate) fn chainwork_after_height(&self, height: u32) -> Work {
        let offset_height = height.checked_sub(self.anchor_checkpoint.height);
        match offset_height {
            Some(index) => {
                let work = self
                    .headers
                    .iter()
                    .enumerate()
                    .filter(|(h, _)| (*h as u32).ge(&index))
                    .map(|(_, header)| header.work())
                    .reduce(|acc, next| acc + next);
                match work {
                    Some(work) => work,
                    // If the height is higher than the known chain, we don't have any work
                    None => Work::from_be_bytes([0; 32]),
                }
            }
            // If the height requested is below the checkpoint, just return the entire work of our chain
            None => self.chainwork(),
        }
    }

    pub(crate) fn chainwork_after_index(&self, index: usize) -> Work {
        let work = self
            .headers
            .iter()
            .enumerate()
            .filter(|(h, _)| h.gt(&index))
            .map(|(_, header)| header.work())
            .reduce(|acc, next| acc + next);
        work.unwrap_or(self.chainwork())
    }

    // Human readable chainwork
    pub(crate) fn log2_work(&self) -> f64 {
        let work = self
            .headers
            .iter()
            .map(|header| header.work().log2())
            .reduce(|acc, next| acc + next);
        work.unwrap_or(0.0)
    }

    // The last 11 headers, if we have that many
    pub(crate) fn last_median_time_past_window(&self) -> Headers {
        self.headers
            .iter()
            .rev()
            .take(MEDIAN_TIME_PAST)
            .rev()
            .copied()
            .collect()
    }

    // The block locators are a way to inform our peer of blocks we know about
    pub(crate) fn locators(&self) -> Vec<BlockHash> {
        let mut locators = Vec::new();
        let rev: Vec<BlockHash> = self
            .headers
            .iter()
            .rev()
            .take(MAX_LOOKBACK)
            .map(|header| header.block_hash())
            .collect();
        locators.push(self.tip());
        for locator in LOCATOR_LOOKBACKS {
            match rev.get(*locator) {
                Some(hash) => locators.push(*hash),
                None => break,
            }
        }
        locators
    }

    // Extend the current chain, potentially rewriting history
    pub(crate) fn extend(&mut self, batch: &Vec<Header>) -> Vec<Header> {
        let mut reorged = Vec::new();
        // We cannot extend from nothing
        if batch.is_empty() {
            return reorged;
        }
        // This is the usual case where the headers link
        if self.tip().eq(&batch
            .first()
            .expect("cannot extend from empty batch")
            .prev_blockhash)
        {
            self.headers.extend(batch);
        } else {
            // Panic if we don't contain the hash. Something went wrong further up the call stack.
            assert!(self.contains_hash(
                batch
                    .first()
                    .expect("cannot extend from empty batch")
                    .prev_blockhash
            ));
            // Remove items from the top of the chain until they link.
            while self.tip().ne(&batch
                .first()
                .expect("cannot extend from empty batch")
                .prev_blockhash)
            {
                if let Some(header) = self.headers.pop() {
                    reorged.push(header)
                }
            }
            self.headers.extend(batch);
        }
        reorged.iter().rev().copied().collect()
    }

    // Clear all the headers from our chain. Only to be used when a peer has feed us faulty checkpoints
    pub(crate) fn clear_all(&mut self) {
        self.headers.clear()
    }
}

#[cfg(test)]
mod tests {
    use bitcoin::consensus::deserialize;

    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_empty_chain() {
        let chain = HeaderChain::new(
            HeaderCheckpoint::new(
                190_000,
                BlockHash::from_str(
                    "0000013a6143b7360b7ba3834316b3265ee9072dde440bd45f99c01c42abaef2",
                )
                .unwrap(),
            ),
            Vec::new(),
        );
        assert_eq!(chain.chainwork(), Work::from_be_bytes([0; 32]));
        assert_eq!(
            chain.chainwork_after_height(189_999),
            Work::from_be_bytes([0; 32])
        );
        assert_eq!(chain.height(), 190_000);
        assert_eq!(chain.tip(), BlockHash::from_str(
            "0000013a6143b7360b7ba3834316b3265ee9072dde440bd45f99c01c42abaef2",
        )
        .unwrap());
        assert_eq!(chain.root(), BlockHash::from_str(
            "0000013a6143b7360b7ba3834316b3265ee9072dde440bd45f99c01c42abaef2",
        )
        .unwrap());
        assert!(
            chain.contains_hash(
                BlockHash::from_str(
                    "0000013a6143b7360b7ba3834316b3265ee9072dde440bd45f99c01c42abaef2",
                )
                .unwrap()
            )
        );
        assert_eq!(chain.inner_len(), 0);
    }

    #[test]
    fn test_nonempty_chain() {
        let block_190_001: Header = deserialize(&hex::decode("00000020f2aeab421cc0995fd40b44de2d07e95e26b3164383a37b0b36b743613a0100001f521e92a0bdc70b68554351f2a72c7476204f312bce7f084768b7054ad915f4f41110669d41011ec816bf00").unwrap()).unwrap();
        let block_190_002: Header = deserialize(&hex::decode("0000002060553eca679219d7c61307fd1e01922416ee1d2dfa09e2a5062aa08ef800000063b913b2d4fde38d46f01bac8b9b177ae5c23c938736dcafc31c3d10c282cc20ff1510669d41011e55671a00").unwrap()).unwrap();
        let block_190_003: Header = deserialize(&hex::decode("0000002042c5fa907f5d28affaa72b430f2732052d7a19f203be794fea39153e7e0000009c8705706dce105bbaf42a9a692d3bdcca1d7e34399e8cc7684700da439bf144291810669d41011ed0241501").unwrap()).unwrap();
        let batch_1 = vec![block_190_001];
        let batch_2 = vec![block_190_002, block_190_003];
        let mut chain = HeaderChain::new(
            HeaderCheckpoint::new(
                190_000,
                BlockHash::from_str(
                    "0000013a6143b7360b7ba3834316b3265ee9072dde440bd45f99c01c42abaef2",
                )
                .unwrap(),
            ),
            Vec::new(),
        );
        let reorg = chain.extend(&batch_1);
        assert!(reorg.is_empty());
        assert_eq!(chain.height(), 190_001);
        assert_eq!(chain.inner_len(), 1);
        assert_eq!(chain.chainwork(), block_190_001.work());
        assert_eq!(chain.header_at_height(190_001).unwrap(), &block_190_001);
        assert_eq!(chain.chainwork_after_height(190_000), block_190_001.work());
        assert_eq!(chain.chainwork_after_height(189_999), block_190_001.work());
        assert_eq!(
            chain.chainwork_after_height(190_001),
            Work::from_be_bytes([0; 32])
        );
        let reorg = chain.extend(&batch_2);
        assert!(reorg.is_empty());
        assert_eq!(chain.height(), 190_003);
        assert_eq!(chain.inner_len(), 3);
        assert_eq!(chain.header_at_height(190_003).unwrap(), &block_190_003);
        assert_eq!(
            chain.chainwork_after_height(190_001),
            block_190_002.work() + block_190_003.work()
        );
        assert_eq!(chain.chainwork_after_index(1), block_190_003.work());
        assert_eq!(chain.tip(), block_190_003.block_hash());
    }

    #[test]
    fn test_reorganized_chain() {}
}
