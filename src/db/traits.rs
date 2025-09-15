use std::fmt::Debug;
use std::ops::RangeBounds;
use std::{collections::BTreeMap, fmt::Display};

use bitcoin::{block::Header, BlockHash};

use crate::prelude::FutureResult;

use super::BlockHeaderChanges;

/// Methods required to persist the chain of block headers.
pub trait HeaderStore: Debug + Send + Sync {
    /// Errors that may occur within a [`HeaderStore`].
    type Error: Debug + Display;
    /// Load the headers of the canonical chain for the specified range.
    fn load<'a>(
        &'a mut self,
        range: impl RangeBounds<u32> + Send + Sync + 'a,
    ) -> FutureResult<'a, BTreeMap<u32, Header>, Self::Error>;

    /// Stage changes to the chain to be written in the future.
    fn stage(&mut self, changes: BlockHeaderChanges);

    /// Commit the changes by writing them to disk.
    fn write(&mut self) -> FutureResult<'_, (), Self::Error>;

    /// Return the height of a block hash in the database, if it exists.
    fn height_of<'a>(
        &'a mut self,
        hash: &'a BlockHash,
    ) -> FutureResult<'a, Option<u32>, Self::Error>;

    /// Return the hash at the height in the database, if it exists.
    fn hash_at(&mut self, height: u32) -> FutureResult<'_, Option<BlockHash>, Self::Error>;

    /// Return the header at the height in the database, if it exists.
    fn header_at(&mut self, height: u32) -> FutureResult<'_, Option<Header>, Self::Error>;
}

#[cfg(test)]
mod test {
    use super::*;
    use std::convert::Infallible;

    impl HeaderStore for () {
        type Error = Infallible;
        fn load<'a>(
            &'a mut self,
            _range: impl RangeBounds<u32> + Send + Sync + 'a,
        ) -> FutureResult<'a, BTreeMap<u32, Header>, Self::Error> {
            async fn do_load() -> Result<BTreeMap<u32, Header>, Infallible> {
                Ok(BTreeMap::new())
            }
            Box::pin(do_load())
        }

        fn stage(&mut self, _changes: BlockHeaderChanges) {}

        fn write(&mut self) -> FutureResult<'_, (), Self::Error> {
            async fn do_write() -> Result<(), Infallible> {
                Ok(())
            }
            Box::pin(do_write())
        }

        fn height_of<'a>(
            &'a mut self,
            _block_hash: &'a BlockHash,
        ) -> FutureResult<'a, Option<u32>, Self::Error> {
            async fn do_height_of() -> Result<Option<u32>, Infallible> {
                Ok(None)
            }
            Box::pin(do_height_of())
        }

        fn hash_at(&mut self, _height: u32) -> FutureResult<'_, Option<BlockHash>, Self::Error> {
            async fn do_hast_at() -> Result<Option<BlockHash>, Infallible> {
                Ok(None)
            }
            Box::pin(do_hast_at())
        }

        fn header_at(&mut self, _height: u32) -> FutureResult<'_, Option<Header>, Self::Error> {
            async fn do_header_at() -> Result<Option<Header>, Infallible> {
                Ok(None)
            }
            Box::pin(do_header_at())
        }
    }
}
