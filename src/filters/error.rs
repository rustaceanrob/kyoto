use thiserror::Error;

#[derive(Error, Debug)]
pub enum CFHeaderSyncError {
    #[error("empty headers message")]
    EmptyMessage,
    #[error("a StopHash recevied was not found in our chain")]
    UnknownStophash,
    #[error("the requested and received stop hashes do not match")]
    StopHashMismatch,
    #[error("we did not request this stop hash")]
    UnrequestedStopHash,
    #[error("previous filter header mismatch")]
    PrevHeaderMismatch,
    #[error("indexed out of bounds on the header chain trying to find a block hash")]
    HeaderChainIndexOverflow,
    #[error("we already had a message from this peer staged in our queue")]
    UnexpectedCFHeaderMessage,
}
