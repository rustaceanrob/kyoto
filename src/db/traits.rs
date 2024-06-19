use std::{
    collections::BTreeMap,
    net::{IpAddr, Ipv4Addr},
};

use async_trait::async_trait;
use bitcoin::{block::Header, p2p::ServiceFlags, BlockHash};

use crate::prelude::default_port_from_network;

use super::{error::DatabaseError, PersistedPeer};

/// Methods required to persist the chain of block headers.
#[async_trait]
pub trait HeaderStore {
    /// Load all headers with heights *strictly after* the specified anchor height.
    async fn load(&mut self, anchor_height: u32) -> Result<BTreeMap<u32, Header>, DatabaseError>;

    /// Write an indexed map of block headers to the database, ignoring if they already exist.
    async fn write<'a>(
        &mut self,
        header_chain: &'a BTreeMap<u32, Header>,
    ) -> Result<(), DatabaseError>;

    /// Write the headers to the database, replacing headers over the specified height.
    async fn write_over<'a>(
        &mut self,
        header_chain: &'a BTreeMap<u32, Header>,
        height: u32,
    ) -> Result<(), DatabaseError>;

    /// Return the height of a block hash in the database, if it exists.
    async fn height_of<'a>(&mut self, hash: &'a BlockHash) -> Result<Option<u32>, DatabaseError>;
}

// Do nothing
#[async_trait]
impl HeaderStore for () {
    async fn load(&mut self, _anchor_height: u32) -> Result<BTreeMap<u32, Header>, DatabaseError> {
        Ok(BTreeMap::new())
    }

    async fn write<'a>(
        &mut self,
        _header_chain: &'a BTreeMap<u32, Header>,
    ) -> Result<(), DatabaseError> {
        Ok(())
    }

    async fn write_over<'a>(
        &mut self,
        _header_chain: &'a BTreeMap<u32, Header>,
        _height: u32,
    ) -> Result<(), DatabaseError> {
        Ok(())
    }

    async fn height_of<'a>(
        &mut self,
        _block_hash: &'a BlockHash,
    ) -> Result<Option<u32>, DatabaseError> {
        Ok(None)
    }
}

/// Methods that define a list of peers on the Bitcoin P2P network.
#[async_trait]
pub trait PeerStore {
    /// Add a peer to the database, defining if it should be replaced or not.
    async fn update(&mut self, peer: PersistedPeer, replace: bool) -> Result<(), DatabaseError>;

    /// Get any peer from the database, selected at random.
    async fn random(&mut self) -> Result<PersistedPeer, DatabaseError>;

    /// The number of peers in the database that are not marked as banned.
    async fn num_unbanned(&mut self) -> Result<u32, DatabaseError>;
}

#[async_trait]
impl PeerStore for () {
    async fn update(&mut self, _peer: PersistedPeer, _replace: bool) -> Result<(), DatabaseError> {
        Ok(())
    }

    async fn random(&mut self) -> Result<PersistedPeer, DatabaseError> {
        let addr = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let port = default_port_from_network(&bitcoin::Network::Regtest);
        Ok(PersistedPeer::new(
            addr,
            port,
            ServiceFlags::COMPACT_FILTERS,
            true,
            false,
        ))
    }

    async fn num_unbanned(&mut self) -> Result<u32, DatabaseError> {
        Ok(1)
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
