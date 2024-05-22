use thiserror::Error;

#[derive(Error, Debug)]
pub enum NodeError {
    #[error("persistence failed")]
    LoadError(PersistenceError),
    #[error("dns bootstrap failed")]
    DnsFailure,
}

#[derive(Error, Debug)]
pub enum PersistenceError {
    #[error("there was an error loading the headers from persistence")]
    HeaderLoadError,
    #[error("there was an error loading peers from the database")]
    PeerLoadFailure,
}
