pub const CF_HEADER_BATCH_SIZE: usize = 1_999;
pub const FILTER_BATCH_SIZE: usize = 10;

pub(crate) mod cfheader_batch;
pub(crate) mod cfheader_chain;
pub(crate) mod error;
pub(crate) mod filter;
pub(crate) mod filter_chain;
