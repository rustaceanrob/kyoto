use crate::impl_sourceless_error;

/// Potential errors encountered by a persistence layer.
#[derive(Debug)]
pub enum DatabaseError {
    /// Loading a query or data from the database failed.
    Load(String),
    /// Writing a query or data from the database failed.
    Write(String),
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
            DatabaseError::Load(e) => {
                write!(f, "loading a query or data from the database failed: {e}")
            }
            DatabaseError::Write(e) => {
                write!(f, "writing a query or data from the database failed: {e}")
            }
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

/// Errors when initializing a SQL-based backend.
#[cfg(feature = "database")]
#[derive(Debug)]
pub enum SqlInitializationError {
    /// A file or directory could not be opened or created.
    IO(std::io::Error),
    /// An error occured performing a SQL operation.
    SQL(rusqlite::Error),
}

#[cfg(feature = "database")]
impl core::fmt::Display for SqlInitializationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SqlInitializationError::IO(e) => {
                write!(f, "a file or directory could not be opened or created: {e}")
            }
            SqlInitializationError::SQL(e) => {
                write!(f, "reading or writing from the database failed: {e}")
            }
        }
    }
}

#[cfg(feature = "database")]
impl std::error::Error for SqlInitializationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SqlInitializationError::IO(error) => Some(error),
            SqlInitializationError::SQL(error) => Some(error),
        }
    }
}

#[cfg(feature = "database")]
impl From<rusqlite::Error> for SqlInitializationError {
    fn from(value: rusqlite::Error) -> Self {
        Self::SQL(value)
    }
}

#[cfg(feature = "database")]
impl From<std::io::Error> for SqlInitializationError {
    fn from(value: std::io::Error) -> Self {
        Self::IO(value)
    }
}

/// Errors while reading or writing to and from a SQL-based backend.
#[cfg(feature = "database")]
#[derive(Debug)]
pub enum SqlError {
    /// A consensus critical data structure is malformed.
    Corruption,
    /// A string could not be deserialized into a known datatype.
    StringConversion,
    /// An error occured performing a SQL operation.
    SQL(rusqlite::Error),
}

#[cfg(feature = "database")]
impl core::fmt::Display for SqlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SqlError::StringConversion => {
                write!(
                    f,
                    "a string could not be deserialized into a known datatype."
                )
            }
            SqlError::SQL(e) => {
                write!(f, "reading or writing from the database failed: {e}")
            }
            SqlError::Corruption => write!(f, "a consensus critical data structure is malformed."),
        }
    }
}

#[cfg(feature = "database")]
impl std::error::Error for SqlError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SqlError::Corruption => None,
            SqlError::StringConversion => None,
            SqlError::SQL(error) => Some(error),
        }
    }
}

#[cfg(feature = "database")]
impl From<rusqlite::Error> for SqlError {
    fn from(value: rusqlite::Error) -> Self {
        Self::SQL(value)
    }
}

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
