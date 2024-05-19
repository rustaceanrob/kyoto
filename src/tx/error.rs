use thiserror::Error;

#[derive(Error, Debug)]
pub enum TransactionStoreError {
    #[error("unable to read from the tx store")]
    Read,
    #[error("unable to write to the tx store")]
    Write,
}
