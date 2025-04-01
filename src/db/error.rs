/// Errors when initializing a SQL-based backend.
#[cfg(feature = "rusqlite")]
#[derive(Debug)]
pub enum SqlInitializationError {
    /// A file or directory could not be opened or created.
    IO(std::io::Error),
    /// An error occured performing a SQL operation.
    SQL(rusqlite::Error),
}

#[cfg(feature = "rusqlite")]
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

#[cfg(feature = "rusqlite")]
impl std::error::Error for SqlInitializationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SqlInitializationError::IO(error) => Some(error),
            SqlInitializationError::SQL(error) => Some(error),
        }
    }
}

#[cfg(feature = "rusqlite")]
impl From<rusqlite::Error> for SqlInitializationError {
    fn from(value: rusqlite::Error) -> Self {
        Self::SQL(value)
    }
}

#[cfg(feature = "rusqlite")]
impl From<std::io::Error> for SqlInitializationError {
    fn from(value: std::io::Error) -> Self {
        Self::IO(value)
    }
}

/// Errors while reading or writing to and from a SQL-based peer backend.
#[cfg(feature = "rusqlite")]
#[derive(Debug)]
pub enum SqlPeerStoreError {
    /// A consensus critical data structure is malformed.
    Deserialize(bitcoin::consensus::encode::Error),
    /// There are no known peers in the database.
    Empty,
    /// An error occured performing a SQL operation.
    SQL(rusqlite::Error),
}

#[cfg(feature = "rusqlite")]
impl core::fmt::Display for SqlPeerStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SqlPeerStoreError::Deserialize(e) => {
                write!(
                    f,
                    "a byte array could not be deserialized into a known datatype: {e}"
                )
            }
            Self::Empty => {
                write!(f, "there are no known peers in the database.")
            }
            SqlPeerStoreError::SQL(e) => {
                write!(f, "reading or writing from the database failed: {e}")
            }
        }
    }
}

#[cfg(feature = "rusqlite")]
impl std::error::Error for SqlPeerStoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SqlPeerStoreError::Deserialize(error) => Some(error),
            SqlPeerStoreError::Empty => None,
            SqlPeerStoreError::SQL(error) => Some(error),
        }
    }
}

#[cfg(feature = "rusqlite")]
impl From<rusqlite::Error> for SqlPeerStoreError {
    fn from(value: rusqlite::Error) -> Self {
        Self::SQL(value)
    }
}

#[cfg(feature = "rusqlite")]
impl From<bitcoin::consensus::encode::Error> for SqlPeerStoreError {
    fn from(value: bitcoin::consensus::encode::Error) -> Self {
        Self::Deserialize(value)
    }
}

/// Errors while reading or writing to and from a SQL-based block header backend.
#[cfg(feature = "rusqlite")]
#[derive(Debug)]
pub enum SqlHeaderStoreError {
    /// The headers do not link together.
    Corruption,
    /// Consensus deserialization failed.
    Deserialize(bitcoin::consensus::encode::Error),
    /// An error occured performing a SQL operation.
    SQL(rusqlite::Error),
}

#[cfg(feature = "rusqlite")]
impl core::fmt::Display for SqlHeaderStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SqlHeaderStoreError::SQL(e) => {
                write!(f, "reading or writing from the database failed: {e}")
            }
            SqlHeaderStoreError::Deserialize(e) => {
                write!(f, "consensus decoding failed {e}")
            }
            SqlHeaderStoreError::Corruption => {
                write!(f, "a consensus critical data structure is malformed.")
            }
        }
    }
}

#[cfg(feature = "rusqlite")]
impl std::error::Error for SqlHeaderStoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SqlHeaderStoreError::Corruption => None,
            SqlHeaderStoreError::SQL(error) => Some(error),
            SqlHeaderStoreError::Deserialize(error) => Some(error),
        }
    }
}

#[cfg(feature = "rusqlite")]
impl From<rusqlite::Error> for SqlHeaderStoreError {
    fn from(value: rusqlite::Error) -> Self {
        Self::SQL(value)
    }
}

#[cfg(feature = "rusqlite")]
impl From<bitcoin::consensus::encode::Error> for SqlHeaderStoreError {
    fn from(value: bitcoin::consensus::encode::Error) -> Self {
        Self::Deserialize(value)
    }
}
