use bitcoin::FilterHash;
use bitcoin_hashes::{sha256d, Hash};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Filter {
    contents: Vec<u8>,
}

impl Filter {
    pub fn new(contents: Vec<u8>) -> Self {
        Self { contents }
    }

    pub async fn filter_hash(&self) -> FilterHash {
        let hash = sha256d::Hash::hash(&self.contents);
        FilterHash::from_raw_hash(hash)
    }
}
