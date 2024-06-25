use thiserror::Error;

/// Potential errors encountered by a persistence layer.
#[derive(Error, Debug)]
pub enum DatabaseError {
    /// Loading a query or data from the database failed.
    #[error("Loading a query or data from the database failed.")]
    Load,
    /// Writing a query or data from the database failed.
    #[error("Writing a query or data from the database failed.")]
    Write,
    /// The data loading is corrupted.
    #[error("Loaded data has been corrupted.")]
    Corruption,
    /// Serializing an object to write to the database failed.
    #[error("Serializing an object to write to the database failed.")]
    Serialization,
    /// Deserializing an object after loading from the database failed.
    #[error("Deserializing an object after loading from the database failed.")]
    Deserialization,
    /// Opening the database file failed.
    #[error("Opening the database file failed.")]
    Open,
}

/// Errors when managing persisted peers.
#[derive(Error, Debug)]
pub enum PeerManagerError {
    /// DNS failed to respond.
    #[error("DNS failed to respond.")]
    Dns,
    /// Reading or writing from the database failed.
    #[error("Reading or writing from the database failed.")]
    Database(DatabaseError),
}
