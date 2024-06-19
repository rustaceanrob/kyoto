use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("loading a query or data from the database failed")]
    LoadError,
    #[error("writing a query or data from the database failed")]
    WriteError,
}

#[derive(Error, Debug)]
pub enum PeerManagerError {
    #[error("DNS failed to respond")]
    Dns,
    #[error("reading or writing from the database failed")]
    Database(DatabaseError),
}
