use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum HeaderSyncError {
    #[error("empty headers message")]
    EmptyMessage,
    #[error("the headers received do not connect")]
    HeadersNotConnected,
    #[error("one or more headers does not match its own PoW target")]
    InvalidHeaderWork,
    #[error("one or more headers does not have a valid block time")]
    InvalidHeaderTimes,
    #[error("the sync peer sent us a discontinuous chain")]
    PreCheckpointFork,
    #[error("a checkpoint in the chain did not match")]
    InvalidCheckpoint,
    #[error("a computed difficulty adjustment did not match")]
    MiscalculatedDifficulty,
    #[error("the peer sent us a chain that does not connect to any header of ours")]
    FloatingHeaders,
    #[error("less work fork")]
    LessWorkFork,
}

#[derive(Error, Debug)]
pub enum HeaderPersistenceError {
    #[error("the headers loaded from the persistence layer do not match the network")]
    GenesisMismatch,
    #[error("the headers loaded from persistence do not link together")]
    HeadersDoNotLink,
    #[error("the headers loaded do not match a known checkpoint")]
    MismatchedCheckpoints,
    #[error("the headers could not be loaded from sqlite")]
    SQLite,
}

#[derive(Error, Debug)]
pub enum BlockScanError {
    #[error("unknown block hash")]
    NoBlockHash,
}
