use std::{
    collections::BTreeMap,
    net::{IpAddr, Ipv4Addr},
};

use async_trait::async_trait;
use bitcoin::{block::Header, p2p::ServiceFlags, BlockHash};

use crate::prelude::default_port_from_network;

use super::{error::DatabaseError, PersistedPeer};

#[async_trait]
pub(crate) trait HeaderStore {
    async fn load(&mut self, anchor_height: u32) -> Result<BTreeMap<u32, Header>, DatabaseError>;

    async fn write<'a>(
        &mut self,
        header_chain: &'a BTreeMap<u32, Header>,
    ) -> Result<(), DatabaseError>;

    async fn write_over<'a>(
        &mut self,
        header_chain: &'a BTreeMap<u32, Header>,
        height: u32,
    ) -> Result<(), DatabaseError>;

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

#[async_trait]
pub(crate) trait PeerStore {
    async fn update(&mut self, peer: PersistedPeer) -> Result<(), DatabaseError>;

    async fn random(&mut self) -> Result<PersistedPeer, DatabaseError>;

    async fn num_unbanned(&mut self) -> Result<u32, DatabaseError>;
}

#[async_trait]
impl PeerStore for () {
    async fn update(&mut self, _peer: PersistedPeer) -> Result<(), DatabaseError> {
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
