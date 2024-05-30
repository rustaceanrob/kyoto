use thiserror::Error;

#[derive(Error, Debug)]
pub enum HeaderDatabaseError {
    #[error("loading a query or data from sqlite failed")]
    LoadError,
    #[error("writing a query or data from sqlite failed")]
    WriteError,
}
