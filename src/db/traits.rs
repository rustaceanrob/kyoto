use std::fmt::Debug;
use std::ops::RangeBounds;
use std::{collections::BTreeMap, fmt::Display};

use bitcoin::{block::Header, BlockHash};

use crate::prelude::FutureResult;

use super::{BlockHeaderChanges, PersistedPeer};

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
    fn write(&mut self) -> FutureResult<(), Self::Error>;

    /// Return the height of a block hash in the database, if it exists.
    fn height_of<'a>(
        &'a mut self,
        hash: &'a BlockHash,
    ) -> FutureResult<'a, Option<u32>, Self::Error>;

    /// Return the hash at the height in the database, if it exists.
    fn hash_at(&mut self, height: u32) -> FutureResult<Option<BlockHash>, Self::Error>;

    /// Return the header at the height in the database, if it exists.
    fn header_at(&mut self, height: u32) -> FutureResult<Option<Header>, Self::Error>;
}

/// Methods that define a list of peers on the Bitcoin P2P network.
pub trait PeerStore: Debug + Send + Sync {
    /// Errors that may occur within a [`PeerStore`].
    type Error: Debug + Display;
    /// Add a peer to the database, defining if it should be replaced or not.
    fn update(&mut self, peer: PersistedPeer) -> FutureResult<(), Self::Error>;

    /// Get any peer from the database, selected at random. If no peers exist, an error is thrown.
    fn random(&mut self) -> FutureResult<PersistedPeer, Self::Error>;

    /// The number of peers in the database that are not marked as banned.
    fn num_unbanned(&mut self) -> FutureResult<u32, Self::Error>;
}

#[cfg(test)]
mod test {
    use super::*;
    use std::convert::Infallible;

    /// Errors for the [`PeerStore`](crate) of unit type.
    #[derive(Debug)]
    pub enum UnitPeerStoreError {
        /// There were no peers found.
        NoPeers,
    }

    impl core::fmt::Display for UnitPeerStoreError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                UnitPeerStoreError::NoPeers => write!(f, "no peers in unit database."),
            }
        }
    }

    impl PeerStore for () {
        type Error = UnitPeerStoreError;
        fn update(&mut self, _peer: PersistedPeer) -> FutureResult<(), Self::Error> {
            async fn do_update() -> Result<(), UnitPeerStoreError> {
                Ok(())
            }
            Box::pin(do_update())
        }

        fn random(&mut self) -> FutureResult<PersistedPeer, Self::Error> {
            async fn do_random() -> Result<PersistedPeer, UnitPeerStoreError> {
                Err(UnitPeerStoreError::NoPeers)
            }
            Box::pin(do_random())
        }

        fn num_unbanned(&mut self) -> FutureResult<u32, Self::Error> {
            async fn do_num_unbanned() -> Result<u32, UnitPeerStoreError> {
                Ok(0)
            }
            Box::pin(do_num_unbanned())
        }
    }

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

        fn write(&mut self) -> FutureResult<(), Self::Error> {
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

        fn hash_at(&mut self, _height: u32) -> FutureResult<Option<BlockHash>, Self::Error> {
            async fn do_hast_at() -> Result<Option<BlockHash>, Infallible> {
                Ok(None)
            }
            Box::pin(do_hast_at())
        }

        fn header_at(&mut self, _height: u32) -> FutureResult<Option<Header>, Self::Error> {
            async fn do_header_at() -> Result<Option<Header>, Infallible> {
                Ok(None)
            }
            Box::pin(do_header_at())
        }
    }
}
