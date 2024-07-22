use crate::impl_sourceless_error;

/// Potential errors encountered by a persistence layer.
#[derive(Debug)]
pub enum DatabaseError {
    /// Loading a query or data from the database failed.
    Load,
    /// Writing a query or data from the database failed.
    Write,
    /// The data loading is corrupted.
    Corruption,
    /// Serializing an object to write to the database failed.
    Serialization,
    /// Deserializing an object after loading from the database failed.
    Deserialization,
    /// Opening the database file failed.
    Open,
}

impl core::fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DatabaseError::Load => write!(f, "loading a query or data from the database failed."),
            DatabaseError::Write => write!(f, "writing a query or data from the database failed."),
            DatabaseError::Corruption => write!(f, "loaded data has been corrupted."),
            DatabaseError::Serialization => {
                write!(f, "serializing an object to write to the database failed.")
            }
            DatabaseError::Deserialization => write!(
                f,
                "deserializing an object after loading from the database failed."
            ),
            DatabaseError::Open => write!(f, "opening the database file failed."),
        }
    }
}

impl_sourceless_error!(DatabaseError);

/// Errors when managing persisted peers.
#[derive(Debug)]
pub enum PeerManagerError {
    /// DNS failed to respond.
    Dns,
    /// Reading or writing from the database failed.
    Database(DatabaseError),
}

impl core::fmt::Display for PeerManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PeerManagerError::Dns => write!(f, "DNS servers failed to respond."),
            PeerManagerError::Database(e) => {
                write!(f, "reading or writing from the database failed: {e}")
            }
        }
    }
}

impl std::error::Error for PeerManagerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PeerManagerError::Database(e) => Some(e),
            _ => None,
        }
    }
}
