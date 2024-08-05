use std::collections::BTreeMap;

use bitcoin::{block::Header, BlockHash};

use crate::prelude::FutureResult;

use super::{error::DatabaseError, PersistedPeer};

/// Methods required to persist the chain of block headers.
pub trait HeaderStore {
    /// Load all headers with heights *strictly after* the specified anchor height.
    fn load_after(
        &mut self,
        anchor_height: u32,
    ) -> FutureResult<BTreeMap<u32, Header>, DatabaseError>;

    /// Write an indexed map of block headers to the database, ignoring if they already exist.
    fn write<'a>(
        &'a mut self,
        header_chain: &'a BTreeMap<u32, Header>,
    ) -> FutureResult<'a, (), DatabaseError>;

    /// Write the headers to the database, replacing headers over the specified height.
    fn write_over<'a>(
        &'a mut self,
        header_chain: &'a BTreeMap<u32, Header>,
        height: u32,
    ) -> FutureResult<'a, (), DatabaseError>;

    /// Return the height of a block hash in the database, if it exists.
    fn height_of<'a>(
        &'a mut self,
        hash: &'a BlockHash,
    ) -> FutureResult<'a, Option<u32>, DatabaseError>;

    /// Return the hash at the height in the database, if it exists.
    fn hash_at(&mut self, height: u32) -> FutureResult<Option<BlockHash>, DatabaseError>;
}

/// This is a simple wrapper for the unit type, signifying that no headers will be stored between sessions.
impl HeaderStore for () {
    fn load_after(
        &mut self,
        _anchor_height: u32,
    ) -> FutureResult<BTreeMap<u32, Header>, DatabaseError> {
        async fn do_load_after() -> Result<BTreeMap<u32, Header>, DatabaseError> {
            Ok(BTreeMap::new())
        }
        Box::pin(do_load_after())
    }

    fn write<'a>(
        &'a mut self,
        _header_chain: &'a BTreeMap<u32, Header>,
    ) -> FutureResult<'a, (), DatabaseError> {
        async fn do_write() -> Result<(), DatabaseError> {
            Ok(())
        }
        Box::pin(do_write())
    }

    fn write_over<'a>(
        &'a mut self,
        _header_chain: &'a BTreeMap<u32, Header>,
        _height: u32,
    ) -> FutureResult<'a, (), DatabaseError> {
        async fn do_write_over() -> Result<(), DatabaseError> {
            Ok(())
        }
        Box::pin(do_write_over())
    }

    fn height_of<'a>(
        &'a mut self,
        _block_hash: &'a BlockHash,
    ) -> FutureResult<'a, Option<u32>, DatabaseError> {
        async fn do_height_of() -> Result<Option<u32>, DatabaseError> {
            Ok(None)
        }
        Box::pin(do_height_of())
    }

    fn hash_at(&mut self, _height: u32) -> FutureResult<Option<BlockHash>, DatabaseError> {
        async fn do_hast_at() -> Result<Option<BlockHash>, DatabaseError> {
            Ok(None)
        }
        Box::pin(do_hast_at())
    }
}

/// Methods that define a list of peers on the Bitcoin P2P network.
pub trait PeerStore {
    /// Add a peer to the database, defining if it should be replaced or not.
    fn update(&mut self, peer: PersistedPeer) -> FutureResult<(), DatabaseError>;

    /// Get any peer from the database, selected at random. If no peers exist, an error is thrown.
    fn random(&mut self) -> FutureResult<PersistedPeer, DatabaseError>;

    /// The number of peers in the database that are not marked as banned.
    fn num_unbanned(&mut self) -> FutureResult<u32, DatabaseError>;
}

impl PeerStore for () {
    fn update(&mut self, _peer: PersistedPeer) -> FutureResult<(), DatabaseError> {
        async fn do_update() -> Result<(), DatabaseError> {
            Ok(())
        }
        Box::pin(do_update())
    }

    fn random(&mut self) -> FutureResult<PersistedPeer, DatabaseError> {
        async fn do_random() -> Result<PersistedPeer, DatabaseError> {
            Err(DatabaseError::Load)
        }
        Box::pin(do_random())
    }

    fn num_unbanned(&mut self) -> FutureResult<u32, DatabaseError> {
        async fn do_num_unbanned() -> Result<u32, DatabaseError> {
            Ok(0)
        }
        Box::pin(do_num_unbanned())
    }
}

impl std::fmt::Debug for dyn HeaderStore + Send + Sync + 'static {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Result::Ok(())
    }
}

impl std::fmt::Debug for dyn PeerStore + Send + Sync + 'static {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Result::Ok(())
    }
}
