use thiserror::Error;

#[derive(Error, Debug)]
pub enum CFHeaderSyncError {
    #[error("Empty headers message.")]
    EmptyMessage,
    #[error("A stop hash recevied was not found in our chain.")]
    UnknownStophash,
    #[error("The requested and received stop hashes do not match.")]
    StopHashMismatch,
    #[error("We did not request this stop hash.")]
    UnrequestedStophash,
    #[error("Previous filter header mismatch.")]
    PrevHeaderMismatch,
    #[error("Indexed out of bounds on the header chain trying to find a block hash.")]
    HeaderChainIndexOverflow,
    #[error("We already had a message from this peer staged in our queue.")]
    UnexpectedCFHeaderMessage,
}

#[derive(Error, Debug)]
pub enum CFilterSyncError {
    #[error("A stop hash recevied was not found in our chain.")]
    UnknownStophash,
    #[error("We did not request this stop hash.")]
    UnrequestedStophash,
    #[error("We could not find the filter hash corresponding to that stop hash.")]
    UnknownFilterHash,
    #[error("The filter hash from our header chain and this filter hash do not match.")]
    MisalignedFilterHash,
    #[error("The filter experienced an IO error checking for Script inclusions.")]
    Filter(FilterError),
}

#[derive(Error, Debug)]
pub enum FilterError {
    #[error("Unable to read from the contents buffer.")]
    IORead,
}
