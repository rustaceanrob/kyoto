use bitcoin::consensus::{deserialize, serialize};
use bitcoin::p2p::ServiceFlags;
use bitcoin::Network;
use rusqlite::params;
use rusqlite::{Connection, Result};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::db::error::{SqlInitializationError, SqlPeerStoreError};
use crate::db::traits::PeerStore;
use crate::db::{PeerStatus, PersistedPeer};
use crate::prelude::FutureResult;

use super::{DATA_DIR, DEFAULT_CWD};

const FILE_NAME: &str = "peers.db";
// Labels for the schema table
const SCHEMA_TABLE_NAME: &str = "peer_schema_versions";
const SCHEMA_COLUMN: &str = "schema_key";
const VERSION_COLUMN: &str = "version";
const SCHEMA_KEY: &str = "current_version";
// Update this in the case of schema changes
const SCHEMA_VERSION: u8 = 0;
// Always execute this query and adjust the schema with migrations
const INITIAL_PEER_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS peers (
    ip_addr BLOB PRIMARY KEY,
    port INTEGER NOT NULL,
    service_flags BLOB NOT NULL,
    tried BOOLEAN NOT NULL,
    banned BOOLEAN NOT NULL
)";

/// Structure to create a SQL Lite backend to store peers.
#[derive(Debug)]
pub struct SqlitePeerDb {
    conn: Arc<Mutex<Connection>>,
}

impl SqlitePeerDb {
    /// Create a new peer storage with an optional directory path. If no path is provided,
    /// the file will be stored in a `data` subdirectory where the program is ran.
    pub fn new(network: Network, path: Option<PathBuf>) -> Result<Self, SqlInitializationError> {
        // Open a connection
        let mut path = path.unwrap_or_else(|| PathBuf::from(DEFAULT_CWD));
        path.push(DATA_DIR);
        path.push(network.to_string());
        if !path.exists() {
            fs::create_dir_all(&path)?
        }
        let conn = Connection::open(path.join(FILE_NAME))?;
        // Create the schema version
        let schema_table_query = format!("CREATE TABLE IF NOT EXISTS {SCHEMA_TABLE_NAME} ({SCHEMA_COLUMN} TEXT PRIMARY KEY, {VERSION_COLUMN} INTEGER NOT NULL)");
        // Update the schema version
        conn.execute(&schema_table_query, [])?;
        let schema_init_version = format!(
            "INSERT OR REPLACE INTO {SCHEMA_TABLE_NAME} ({SCHEMA_COLUMN}, {VERSION_COLUMN}) VALUES (?1, ?2)");
        conn.execute(&schema_init_version, params![SCHEMA_KEY, SCHEMA_VERSION])?;
        // Build the table if it doesn't exist
        conn.execute(INITIAL_PEER_SCHEMA, [])?;
        // Migrate to any new schema versions
        Self::migrate(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    // This function currently does nothing, but if new columns are required this may be used to alter the tables
    // without breaking older tables.
    fn migrate(conn: &Connection) -> Result<(), SqlInitializationError> {
        let version_query =
            format!("SELECT {VERSION_COLUMN} FROM {SCHEMA_TABLE_NAME} WHERE {SCHEMA_COLUMN} = ?1");
        let _current_version: u8 =
            conn.query_row(&version_query, [SCHEMA_KEY], |row| row.get(0))?;
        // Match on the version and migrate to new schemas in the future
        Ok(())
    }

    async fn update(&mut self, peer: PersistedPeer) -> Result<(), SqlPeerStoreError> {
        let lock = self.conn.lock().await;
        let stmt = match peer.status {
            PeerStatus::New => "INSERT OR IGNORE INTO peers (ip_addr, port, service_flags, tried, banned) VALUES (?1, ?2, ?3, ?4, ?5)",
            _ => "INSERT OR REPLACE INTO peers (ip_addr, port, service_flags, tried, banned) VALUES (?1, ?2, ?3, ?4, ?5)",
        };
        let (tried, banned) = match peer.status {
            PeerStatus::New => (false, false),
            PeerStatus::Tried => (true, false),
            PeerStatus::Ban => (true, true),
        };
        let address_blob = serialize(&peer.addr);
        let service_blob = peer.services.to_u64().to_le_bytes();
        lock.execute(
            stmt,
            params![address_blob, peer.port, service_blob, tried, banned,],
        )?;
        Ok(())
    }

    async fn random(&mut self) -> Result<PersistedPeer, SqlPeerStoreError> {
        let lock = self.conn.lock().await;
        let mut stmt =
            lock.prepare("SELECT * FROM peers WHERE banned = false ORDER BY RANDOM() LIMIT 1")?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let ip_addr: Vec<u8> = row.get(0)?;
            let port: u16 = row.get(1)?;
            let service_blob: [u8; 8] = row.get(2)?;
            let service_flags = u64::from_le_bytes(service_blob);
            let tried: bool = row.get(3)?;
            let status = if tried {
                PeerStatus::Tried
            } else {
                PeerStatus::New
            };
            let ip = deserialize(&ip_addr)?;
            let services: ServiceFlags = ServiceFlags::from(service_flags);
            Ok(PersistedPeer::new(ip, port, services, status))
        } else {
            Err(SqlPeerStoreError::Empty)
        }
    }

    async fn num_unbanned(&mut self) -> Result<u32, SqlPeerStoreError> {
        let lock = self.conn.lock().await;
        let mut stmt = lock.prepare("SELECT COUNT(*) FROM peers WHERE banned = false")?;
        let count: u32 = stmt.query_row([], |row| row.get(0))?;
        Ok(count)
    }
}

impl PeerStore for SqlitePeerDb {
    type Error = SqlPeerStoreError;
    fn update(&mut self, peer: PersistedPeer) -> FutureResult<(), Self::Error> {
        Box::pin(self.update(peer))
    }

    fn random(&mut self) -> FutureResult<PersistedPeer, Self::Error> {
        Box::pin(self.random())
    }

    fn num_unbanned(&mut self) -> FutureResult<u32, Self::Error> {
        Box::pin(self.num_unbanned())
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use bitcoin::p2p::{address::AddrV2, ServiceFlags};

    use super::*;

    #[tokio::test]
    async fn test_sql_peer_store() {
        let binding = tempfile::tempdir().unwrap();
        let path = binding.path();
        let mut peer_store =
            SqlitePeerDb::new(bitcoin::Network::Testnet, Some(path.into())).unwrap();
        let ip_1 = Ipv4Addr::new(1, 1, 1, 1);
        let ip_2 = Ipv4Addr::new(2, 2, 2, 2);
        let tor = AddrV2::TorV2([8; 10]);
        let peer_1 = PersistedPeer::new(AddrV2::Ipv4(ip_1), 0, ServiceFlags::NONE, PeerStatus::New);
        let peer_2 = PersistedPeer::new(AddrV2::Ipv4(ip_2), 0, ServiceFlags::NONE, PeerStatus::New);
        let peer_3 = PersistedPeer::new(tor, 0, ServiceFlags::NONE, PeerStatus::New);
        let try_peer_2 =
            PersistedPeer::new(AddrV2::Ipv4(ip_2), 0, ServiceFlags::NONE, PeerStatus::Tried);
        let ban_peer_1 =
            PersistedPeer::new(AddrV2::Ipv4(ip_1), 0, ServiceFlags::NONE, PeerStatus::Ban);
        peer_store.update(peer_1).await.unwrap();
        assert_eq!(peer_store.num_unbanned().await.unwrap(), 1);
        peer_store.update(peer_2).await.unwrap();
        assert_eq!(peer_store.num_unbanned().await.unwrap(), 2);
        peer_store.update(peer_3).await.unwrap();
        assert_eq!(peer_store.num_unbanned().await.unwrap(), 3);
        peer_store.update(try_peer_2).await.unwrap();
        assert_eq!(peer_store.num_unbanned().await.unwrap(), 3);
        peer_store.update(ban_peer_1.clone()).await.unwrap();
        assert_eq!(peer_store.num_unbanned().await.unwrap(), 2);
        let random = peer_store.random().await.unwrap();
        assert_ne!(ban_peer_1.addr, random.addr);
        let random = peer_store.random().await.unwrap();
        assert_ne!(ban_peer_1.addr, random.addr);
        let _ = peer_store.random().await.unwrap();
        let _ = peer_store.random().await.unwrap();
        let _ = peer_store.random().await.unwrap();
        let random = peer_store.random().await.unwrap();
        assert_ne!(ban_peer_1.addr, random.addr);
        drop(peer_store);
        binding.close().unwrap();
    }
}
