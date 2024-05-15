use thiserror::Error;

#[derive(Error, Debug)]
pub enum CFHeaderSyncError {
    #[error("empty headers message")]
    EmptyMessage,
    #[error("a stop hash recevied was not found in our chain")]
    UnknownStophash,
    #[error("the requested and received stop hashes do not match")]
    StopHashMismatch,
    #[error("we did not request this stop hash")]
    UnrequestedStophash,
    #[error("previous filter header mismatch")]
    PrevHeaderMismatch,
    #[error("indexed out of bounds on the header chain trying to find a block hash")]
    HeaderChainIndexOverflow,
    #[error("we already had a message from this peer staged in our queue")]
    UnexpectedCFHeaderMessage,
}

#[derive(Error, Debug)]
pub enum CFilterSyncError {
    #[error("a stop hash recevied was not found in our chain")]
    UnknownStophash,
    #[error("we did not request this stop hash")]
    UnrequestedStophash,
    #[error("we could not find the filter hash corresponding to that stop hash")]
    UnknownFilterHash,
    #[error("the filter hash from our header chain and this filter hash do not match")]
    MisalignedFilterHash,
}
