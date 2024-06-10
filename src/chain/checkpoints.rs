use std::{collections::VecDeque, str::FromStr};

use bitcoin::{BlockHash, Network};

/// Known Testnet3 block hashes.
pub const TESTNET_HEADER_CP: &[(u32, &str)] = &[(
    546,
    "000000002a936ca763904c3c35fce2f3556c559c0214345d31b1bcebf76acb70",
)];

/// Known block hashes for Regtest. Only the genesis hash.
pub const REGTEST_HEADER_CP: &[(u32, &str)] = &[(
    0,
    "0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206",
)];

/// Known block hashes for Signet.
pub const SIGNET_HEADER_CP: &[(u32, &str)] = &[
    (
        0,
        "00000008819873e925422c1ff0f99f7cc9bbb232af63a077a480a3633bee1ef6",
    ),
    (
        10000,
        "000000ade699ac51fe9f23005115eccafe986e9d0c97f87403579698d31f1692",
    ),
    (
        20000,
        "000000d86368960eddbf7e127f8ba93a56efe71420b5dd8dbf8b0a68fa9ebbd1",
    ),
    (
        30000,
        "00000018629ddf1cef19d764cab5fc630dadffca9819d8ff9ae93d4bd76729f0",
    ),
    (
        40000,
        "0000014086ddfe6836bd52179c2ce1ce5eb8a9b85aee87c18be05c605723793c",
    ),
    (
        50000,
        "000000f43b569ea4bdce85a92e8140e90049d6efbffd95c1b6e80de4e397cb01",
    ),
    (
        60000,
        "00000130ab66a74ee232acb3f7ae35f9763dcec197b82b22f482ad0ea4c801f1",
    ),
    (
        70000,
        "0000009266d061c81d88492a405e2e84f55d9a953b8f10cbbb9ea77cfd52a3b1",
    ),
    (
        80000,
        "0000011f3e29efed437f858f1757a7a8ac406194cc80ea2948ed543994962416",
    ),
    (
        90000,
        "0000012965e7e5d073e39cd2efc782054109d9bd359a9560f955f68eff227ef5",
    ),
    (
        100000,
        "0000008753108390007b3f5c26e5d924191567e147876b84489b0c0cf133a0bf",
    ),
    (
        110000,
        "000000e154c6a6d8cea49a3311e590f65898a17a274f15def91692885c61b48e",
    ),
    (
        120000,
        "0000011b6acd2af1a7dc04a2c88a7b2c3980ebd9375dc8d52d331e715deeffcb",
    ),
    (
        130000,
        "000000e62e1e6694ef877dadcca7c8a7eb3474fd2a92bae953a9d263e28a7590",
    ),
    (
        140000,
        "000000cea409c45ccffc69a9a958d4108961a4f0ada1e7b0b40114443a8d36c5",
    ),
    (
        150000,
        "0000013d778ba3f914530f11f6b69869c9fab54acff85acd7b8201d111f19b7f",
    ),
    (
        160000,
        "0000003ca3c99aff040f2563c2ad8f8ec88bd0fd6b8f0895cfaf1ef90353a62c",
    ),
    (
        170000,
        "00000041c812a89f084f633e4cf47e819a2f6b1c0a15162355a930410522c99d",
    ),
    (
        180000,
        "0000000870f15246ba23c16e370a7ffb1fc8a3dcf8cb4492882ed4b0e3d4cd26",
    ),
    (
        190000,
        "0000013a6143b7360b7ba3834316b3265ee9072dde440bd45f99c01c42abaef2",
    ),
];

/// A known block hash in the chain of most work.
#[derive(Debug, Clone, Copy)]
pub struct HeaderCheckpoint {
    /// The index of the block hash.
    pub height: u32,
    /// The Bitcoin block hash expected at this height
    pub hash: BlockHash,
}

impl HeaderCheckpoint {
    pub fn new(height: u32, hash: BlockHash) -> Self {
        HeaderCheckpoint { height, hash }
    }
}

#[derive(Debug)]
pub(crate) struct HeaderCheckpoints {
    checkpoints: VecDeque<HeaderCheckpoint>,
    last: HeaderCheckpoint,
}

impl HeaderCheckpoints {
    pub fn new(network: &Network) -> Self {
        let mut checkpoints: VecDeque<HeaderCheckpoint> = VecDeque::new();
        let cp_list = match network {
            Network::Bitcoin => panic!("unimplemented network"),
            Network::Testnet => TESTNET_HEADER_CP.to_vec(),
            Network::Signet => SIGNET_HEADER_CP.to_vec(),
            Network::Regtest => REGTEST_HEADER_CP.to_vec(),
            _ => unreachable!(),
        };
        cp_list.iter().for_each(|(height, hash)| {
            checkpoints.push_back(HeaderCheckpoint {
                height: *height,
                hash: BlockHash::from_str(hash).unwrap(),
            })
        });
        let last = *checkpoints.back().unwrap();
        HeaderCheckpoints { checkpoints, last }
    }

    pub fn next(&self) -> Option<&HeaderCheckpoint> {
        self.checkpoints.front()
    }

    pub fn advance(&mut self) {
        self.checkpoints.pop_front();
    }

    pub fn is_exhausted(&self) -> bool {
        self.checkpoints.is_empty()
    }

    pub fn last(&self) -> HeaderCheckpoint {
        self.last
    }

    pub fn prune_up_to(&mut self, checkpoint: HeaderCheckpoint) {
        while let Some(header_checkpoint) = self.next() {
            if header_checkpoint.height.le(&checkpoint.height) {
                self.advance()
            } else {
                return;
            }
        }
    }

    pub fn skip_all(&mut self) {
        while !self.is_exhausted() {
            self.advance()
        }
    }
}
