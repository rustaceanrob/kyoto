use std::collections::BTreeMap;

use bitcoin::{block::Header, BlockHash, Work};

use crate::{prelude::MEDIAN_TIME_PAST, DisconnectedHeader};

use super::checkpoints::HeaderCheckpoint;

pub(crate) type Headers = BTreeMap<u32, Header>;

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

    // Top of the chain
    pub(crate) fn tip(&self) -> BlockHash {
        match self.headers.values().last() {
            Some(header) => header.block_hash(),
            None => self.anchor_checkpoint.hash,
        }
    }

    // The canoncial height of the chain, one less than the length
    pub(crate) fn height(&self) -> u32 {
        self.headers.len() as u32 + self.anchor_checkpoint.height
    }

    // The length of the chain we have interally
    pub(crate) fn inner_len(&self) -> usize {
        self.headers().len()
    }

    // All the headers of the chain
    pub(crate) fn headers(&self) -> &Headers {
        &self.headers
    }

    // All the headers of the chain
    pub(crate) fn values(&self) -> Vec<Header> {
        self.headers.values().copied().collect()
    }

    // This header chain contains a block hash
    pub(crate) fn contains_hash(&self, blockhash: BlockHash) -> bool {
        self.headers
            .values()
            .any(|header| header.block_hash().eq(&blockhash))
            || blockhash.eq(&self.anchor_checkpoint.hash)
    }

    // The height of the blockhash in the chain
    pub(crate) async fn height_of_hash(&self, blockhash: BlockHash) -> Option<u32> {
        for (height, header) in self.headers.iter().rev() {
            if header.block_hash().eq(&blockhash) {
                return Some(*height);
            }
        }
        None
    }

    // This header chain contains a block hash
    pub(crate) fn header_at_height(&self, height: u32) -> Option<&Header> {
        self.headers.get(&height)
    }

    // This header chain contains a block hash
    pub(crate) fn contains_header(&self, other: &Header) -> bool {
        self.headers.values().any(|header| header.eq(other))
    }

    // Compute the total work for the chain
    fn get_chainwork(&self, headers: &Headers) -> Work {
        let work = headers
            .values()
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
        let work = self
            .headers
            .iter()
            .filter(|(h, _)| h.gt(&&height))
            .map(|(_, header)| header.work())
            .reduce(|acc, next| acc + next);
        match work {
            Some(work) => work,
            // If the height is higher than the known chain, we don't have any work
            None => Work::from_be_bytes([0; 32]),
        }
    }

    // Human readable chainwork
    pub(crate) fn log2_work(&self) -> f64 {
        let work = self
            .headers
            .values()
            .map(|header| header.work().log2())
            .reduce(|acc, next| acc + next);
        work.unwrap_or(0.0)
    }

    // The last 11 headers, if we have that many
    pub(crate) fn last_median_time_past_window(&self) -> Vec<Header> {
        self.headers
            .values()
            .rev()
            .take(MEDIAN_TIME_PAST)
            .rev()
            .copied()
            .collect()
    }

    // The block locators are a way to inform our peer of blocks we know about
    pub(crate) fn locators(&mut self) -> Vec<BlockHash> {
        let mut locators = Vec::new();
        let rev: Vec<BlockHash> = self
            .headers
            .values()
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

    // Extend the current chain, potentially rewriting history. Higher order functions should decide what we extend
    pub(crate) fn extend(&mut self, batch: &[Header]) -> Vec<DisconnectedHeader> {
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
            let current_anchor = self.height();
            for (index, header) in batch.iter().enumerate() {
                self.headers
                    .insert(current_anchor + 1 + index as u32, *header);
            }
        } else {
            // Panic if we don't contain the hash. Something went wrong further up the call stack.
            assert!(self.contains_hash(
                batch
                    .first()
                    .expect("cannot extend from empty batch")
                    .prev_blockhash
            ));
            // Supporting MSRV of 1.63 requires this round-about way of removing the headers instead of `pop_last`
            // Find the header that matches the new batch prev block hash, collecting the disconnected headers
            for (height, header) in self.headers.iter().rev() {
                if header.block_hash().ne(&batch
                    .first()
                    .expect("cannot extend from empty batch")
                    .prev_blockhash)
                {
                    reorged.push(DisconnectedHeader::new(*height, *header));
                } else {
                    break;
                }
            }
            // Actually remove anything that should no longer be connected
            for removal in reorged.iter() {
                self.remove(&removal.height);
            }
            // Add back the new headers, starting at the proper link
            let current_anchor = self.height();
            for (index, header) in batch.iter().enumerate() {
                self.headers
                    .insert(current_anchor + 1 + index as u32, *header);
            }
        }
        reorged
    }

    fn remove(&mut self, height: &u32) {
        self.headers.remove(height);
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
            BTreeMap::new(),
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
            BTreeMap::new(),
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
        assert_eq!(chain.tip(), block_190_003.block_hash());
    }

    #[test]
    fn test_depth_one_reorg() {
        let block_8: Header = deserialize(&hex::decode("0000002016fe292517eecbbd63227d126a6b1db30ebc5262c61f8f3a4a529206388fc262dfd043cef8454f71f30b5bbb9eb1a4c9aea87390f429721e435cf3f8aa6e2a9171375166ffff7f2000000000").unwrap()).unwrap();
        let block_9: Header = deserialize(&hex::decode("000000205708a90197d93475975545816b2229401ccff7567cb23900f14f2bd46732c605fd8de19615a1d687e89db365503cdf58cb649b8e935a1d3518fa79b0d408704e71375166ffff7f2000000000").unwrap()).unwrap();
        let block_10: Header = deserialize(&hex::decode("000000201d062f2162835787db536c55317e08df17c58078c7610328bdced198574093790c9f554a7780a6043a19619d2a4697364bb62abf6336c0568c31f1eedca3c3e171375166ffff7f2000000000").unwrap()).unwrap();
        let batch_1 = vec![block_8, block_9, block_10];
        let new_block_10: Header = deserialize(&hex::decode("000000201d062f2162835787db536c55317e08df17c58078c7610328bdced198574093792151c0e9ce4e4c789ca98427d7740cc7acf30d2ca0c08baef266bf152289d814567e5e66ffff7f2001000000").unwrap()).unwrap();
        let block_11: Header = deserialize(&hex::decode("00000020efcf8b12221fccc735b9b0b657ce15b31b9c50aff530ce96a5b4cfe02d8c0068496c1b8a89cf5dec22e46c35ea1035f80f5b666a1b3aa7f3d6f0880d0061adcc567e5e66ffff7f2001000000").unwrap()).unwrap();
        let fork = vec![new_block_10, block_11];
        let mut chain = HeaderChain::new(
            HeaderCheckpoint::new(
                7,
                BlockHash::from_str(
                    "62c28f380692524a3a8f1fc66252bc0eb31d6b6a127d2263bdcbee172529fe16",
                )
                .unwrap(),
            ),
            BTreeMap::new(),
        );
        let reorg = chain.extend(&batch_1);
        assert!(reorg.is_empty());
        assert_eq!(chain.height(), 10);
        assert_eq!(
            chain.chainwork_after_height(7),
            block_8.work() + block_9.work() + block_10.work()
        );
        assert_eq!(
            chain.chainwork_after_height(8),
            block_9.work() + block_10.work()
        );
        let reorg = chain.extend(&fork);
        assert_eq!(reorg.len(), 1);
        assert_eq!(reorg.first().unwrap().header, block_10);
        assert_eq!(
            vec![block_8, block_9, new_block_10, block_11],
            chain.values()
        );
        assert_eq!(chain.header_at_height(12), None);
        assert_eq!(chain.header_at_height(10), Some(&new_block_10));
    }

    #[test]
    fn test_depth_two_reorg() {
        let block_1: Header = deserialize(&hex::decode("0000002006226e46111a0b59caaf126043eb5bbf28c34f3a5e332a1fc7b2b73cf188910f047eb4d0fe76345e307d0e020a079cedfa37101ee7ac84575cf829a611b0f84bc4805e66ffff7f2001000000").unwrap()).unwrap();
        let block_2: Header = deserialize(&hex::decode("00000020299e41732deb76d869fcdb5f72518d3784e99482f572afb73068d52134f1f75e1f20f5da8d18661d0f13aa3db8fff0f53598f7d61f56988a6d66573394b2c6ffc5805e66ffff7f2001000000").unwrap()).unwrap();
        let block_3: Header = deserialize(&hex::decode("00000020b96feaa82716f11befeb608724acee4743e0920639a70f35f1637a88b8b6ea3471f1dbedc283ce6a43a87ed3c8e6326dae8d3dbacce1b2daba08e508054ffdb697815e66ffff7f2001000000").unwrap()).unwrap();
        let block_4: Header = deserialize(&hex::decode("0000002052ff614fa461ff38b4a5c101d04fdcac2f34722e60bd43d12c8de0a394fe0c60444fb24b7e9885f60fed9927d27f23854ecfab29287163ef2b868d5d626f82ed97815e66ffff7f2002000000").unwrap()).unwrap();

        let batch_1 = vec![block_1, block_2, block_3, block_4];
        let new_block_3: Header = deserialize(&hex::decode("00000020b96feaa82716f11befeb608724acee4743e0920639a70f35f1637a88b8b6ea349c6240c5d0521966771808950f796c9c04088bc9551a828b64f1cf06831705dfbc835e66ffff7f2000000000").unwrap()).unwrap();
        let new_block_4: Header = deserialize(&hex::decode("00000020d2a1c6ba2be393f405fe2f4574565f9ee38ac68d264872fcd82b030970d0232ce882eb47c3dd138587120f1ad97dd0e73d1e30b79559ad516cb131f83dcb87e9bc835e66ffff7f2002000000").unwrap()).unwrap();
        let fork = vec![new_block_3, new_block_4];
        let mut chain = HeaderChain::new(
            HeaderCheckpoint::new(
                0,
                BlockHash::from_str(
                    "0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206",
                )
                .unwrap(),
            ),
            BTreeMap::new(),
        );
        chain.extend(&batch_1);
        let reorged = chain.extend(&fork);
        assert_eq!(reorged.len(), 2);
        assert_eq!(
            vec![block_1, block_2, new_block_3, new_block_4],
            chain.values()
        );
    }

    #[tokio::test]
    async fn reorg_is_stump() {
        let block_1: Header = deserialize(&hex::decode("0000002006226e46111a0b59caaf126043eb5bbf28c34f3a5e332a1fc7b2b73cf188910f575b313ad3ef825cfc204c34da8f3c1fd1784e2553accfa38001010587cb57241f855e66ffff7f2000000000").unwrap()).unwrap();
        let block_2: Header = deserialize(&hex::decode("00000020c81cedd6a989939936f31448e49d010a13c2e750acf02d3fa73c9c7ecfb9476e798da2e5565335929ad303fc746acabc812ee8b06139bcf2a4c0eb533c21b8c420855e66ffff7f2000000000").unwrap()).unwrap();
        let batch_1 = vec![block_1, block_2];
        let new_block_1: Header = deserialize(&hex::decode("0000002006226e46111a0b59caaf126043eb5bbf28c34f3a5e332a1fc7b2b73cf188910f575b313ad3ef825cfc204c34da8f3c1fd1784e2553accfa38001010587cb5724d5855e66ffff7f2004000000").unwrap()).unwrap();
        let new_block_2: Header = deserialize(&hex::decode("00000020d1d80f53343a084bd0da6d6ab846f9fe4a133de051ea00e7cae16ed19f601065798da2e5565335929ad303fc746acabc812ee8b06139bcf2a4c0eb533c21b8c4d6855e66ffff7f2000000000").unwrap()).unwrap();
        let fork = vec![new_block_1, new_block_2];
        let block_3: Header = deserialize(&hex::decode("0000002080f38c14e898d6646dd426428472888966e0d279d86453f42edc56fdb143241aa66c8fa8837d95be3f85d53f22e86a0d6d456b1ab348e073da4d42a39f50637423865e66ffff7f2000000000").unwrap()).unwrap();
        let block_4: Header = deserialize(&hex::decode("000000204877fed370af64c0a1f7a76f6944e1127aad965b1865f99ecfdf8fa72ae23377f51921d01ff1131bd589500a8ca142884297ceeb1aa762ad727249e9a23f2cb023865e66ffff7f2000000000").unwrap()).unwrap();
        let batch_2 = vec![block_3, block_4];
        let mut chain = HeaderChain::new(
            HeaderCheckpoint::new(
                0,
                BlockHash::from_str(
                    "0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206",
                )
                .unwrap(),
            ),
            BTreeMap::new(),
        );
        chain.extend(&batch_1);
        let reorged = chain.extend(&fork);
        assert_eq!(reorged.len(), 2);
        assert_eq!(
            reorged.iter().map(|f| f.header).collect::<Vec<Header>>(),
            vec![block_2, block_1]
        );
        assert_eq!(vec![new_block_1, new_block_2], chain.values());
        let no_org = chain.extend(&batch_2);
        assert_eq!(no_org.len(), 0);
        assert_eq!(
            vec![new_block_1, new_block_2, block_3, block_4],
            chain.values()
        );
        assert_eq!(chain.header_at_height(3), Some(&block_3));
        let want = chain.height_of_hash(new_block_2.block_hash()).await;
        assert_eq!(Some(2), want);
    }
}
