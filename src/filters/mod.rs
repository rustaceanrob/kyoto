pub(crate) const CF_HEADER_BATCH_SIZE: u32 = 1_999;
pub(crate) const FILTER_BATCH_SIZE: u32 = 99;

pub(crate) mod cfheader_batch;
pub(crate) mod cfheader_chain;
pub(crate) mod error;
pub(crate) mod filter;
pub(crate) mod filter_chain;
