use async_trait::async_trait;
use bitcoin::block::Header;

use super::error::HeaderDatabaseError;

#[async_trait]
pub(crate) trait HeaderStore {
    async fn load(&mut self) -> Result<Vec<Header>, HeaderDatabaseError>;
    async fn write(&mut self, header_chain: &Vec<Header>) -> Result<(), HeaderDatabaseError>;
}

#[async_trait]
impl HeaderStore for () {
    async fn load(&mut self) -> Result<Vec<Header>, HeaderDatabaseError> {
        Ok(Vec::new())
    }
    async fn write(&mut self, _header_chain: &Vec<Header>) -> Result<(), HeaderDatabaseError> {
        Ok(())
    }
}
