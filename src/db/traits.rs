use std::collections::BTreeMap;

use async_trait::async_trait;
use bitcoin::{block::Header, BlockHash};

use super::error::HeaderDatabaseError;

#[async_trait]
pub(crate) trait HeaderStore {
    async fn load(
        &mut self,
        anchor_height: u32,
    ) -> Result<BTreeMap<u32, Header>, HeaderDatabaseError>;

    async fn write<'a>(
        &mut self,
        header_chain: &'a BTreeMap<u32, Header>,
    ) -> Result<(), HeaderDatabaseError>;

    async fn write_over<'a>(
        &mut self,
        header_chain: &'a BTreeMap<u32, Header>,
        height: u32,
    ) -> Result<(), HeaderDatabaseError>;

    async fn height_of<'a>(
        &mut self,
        hash: &'a BlockHash,
    ) -> Result<Option<u32>, HeaderDatabaseError>;
}

// Do nothing
#[async_trait]
impl HeaderStore for () {
    async fn load(
        &mut self,
        _anchor_height: u32,
    ) -> Result<BTreeMap<u32, Header>, HeaderDatabaseError> {
        Ok(BTreeMap::new())
    }

    async fn write<'a>(
        &mut self,
        _header_chain: &'a BTreeMap<u32, Header>,
    ) -> Result<(), HeaderDatabaseError> {
        Ok(())
    }

    async fn write_over<'a>(
        &mut self,
        _header_chain: &'a BTreeMap<u32, Header>,
        _height: u32,
    ) -> Result<(), HeaderDatabaseError> {
        Ok(())
    }

    async fn height_of<'a>(
        &mut self,
        _block_hash: &'a BlockHash,
    ) -> Result<Option<u32>, HeaderDatabaseError> {
        Ok(None)
    }
}

impl std::fmt::Debug for dyn HeaderStore + Send + Sync + 'static {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Result::Ok(())
    }
}
