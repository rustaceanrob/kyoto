use async_trait::async_trait;
use bitcoin::p2p::ServiceFlags;
use bitcoin::Network;
use rusqlite::params;
use rusqlite::{Connection, Result};
use std::fs;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::db::error::DatabaseError;
use crate::db::traits::PeerStore;
use crate::db::PersistedPeer;

const PEER_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS peers (
    ip_addr TEXT PRIMARY KEY,
    port INTEGER NOT NULL,
    service_flags INTEGER NOT NULL,
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
    pub fn new(network: Network, path: Option<PathBuf>) -> Result<Self, DatabaseError> {
        let mut path = path.unwrap_or_else(|| PathBuf::from("."));
        path.push("data");
        path.push(network.to_string());
        if !path.exists() {
            fs::create_dir_all(&path).unwrap();
        }
        let conn = Connection::open(path.join("peers.db")).map_err(|_| DatabaseError::Open)?;
        conn.execute(PEER_SCHEMA, [])
            .map_err(|_| DatabaseError::Write)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }
}

#[async_trait]
impl PeerStore for SqlitePeerDb {
    async fn update(&mut self, peer: PersistedPeer, replace: bool) -> Result<(), DatabaseError> {
        let lock = self.conn.lock().await;
        let stmt = if !replace {
            "INSERT OR IGNORE INTO peers (ip_addr, port, service_flags, tried, banned) VALUES (?1, ?2, ?3, ?4, ?5)"
        } else {
            "INSERT OR REPLACE INTO peers (ip_addr, port, service_flags, tried, banned) VALUES (?1, ?2, ?3, ?4, ?5)"
        };
        lock.execute(
            stmt,
            params![
                peer.addr.to_string(),
                peer.port,
                peer.services.to_u64(),
                peer.tried,
                peer.banned,
            ],
        )
        .map_err(|_| DatabaseError::Write)?;
        Ok(())
    }

    async fn random(&mut self) -> Result<PersistedPeer, DatabaseError> {
        let lock = self.conn.lock().await;
        let mut stmt = lock
            .prepare("SELECT * FROM peers WHERE banned = false ORDER BY RANDOM() LIMIT 1")
            .map_err(|_| DatabaseError::Write)?;
        let mut rows = stmt.query([]).map_err(|_| DatabaseError::Load)?;
        if let Some(row) = rows.next().map_err(|_| DatabaseError::Load)? {
            let ip_addr: String = row.get(0).map_err(|_| DatabaseError::Load)?;
            let port: u16 = row.get(1).map_err(|_| DatabaseError::Load)?;
            let service_flags: u64 = row.get(2).map_err(|_| DatabaseError::Load)?;
            let tried: bool = row.get(3).map_err(|_| DatabaseError::Load)?;
            let banned: bool = row.get(4).map_err(|_| DatabaseError::Load)?;
            let ip = ip_addr
                .parse::<IpAddr>()
                .map_err(|_| DatabaseError::Deserialization)?;
            let services: ServiceFlags = ServiceFlags::from(service_flags);
            return Ok(PersistedPeer::new(ip, port, services, tried, banned));
        } else {
            return Err(DatabaseError::Load);
        }
    }

    async fn num_unbanned(&mut self) -> Result<u32, DatabaseError> {
        let lock = self.conn.lock().await;
        let mut stmt = lock
            .prepare("SELECT COUNT(*) FROM peers WHERE banned = false")
            .map_err(|_| DatabaseError::Load)?;
        let count: u32 = stmt
            .query_row([], |row| row.get(0))
            .map_err(|_| DatabaseError::Deserialization)?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use bitcoin::p2p::ServiceFlags;

    use super::*;

    #[tokio::test]
    #[ignore = "no tmp file"]
    async fn test_sql_peer_store() {
        let mut peer_store = SqlitePeerDb::new(bitcoin::Network::Testnet, None).unwrap();
        let ip_1 = IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1));
        let ip_2 = IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2));
        let ip_3 = IpAddr::V4(Ipv4Addr::new(3, 3, 3, 3));
        let peer_1 = PersistedPeer::new(ip_1, 0, ServiceFlags::NONE, false, false);
        let peer_2 = PersistedPeer::new(ip_2, 0, ServiceFlags::NONE, false, false);
        let peer_3 = PersistedPeer::new(ip_3, 0, ServiceFlags::NONE, false, false);
        let try_peer_2 = PersistedPeer::new(ip_2, 0, ServiceFlags::NONE, true, false);
        let ban_peer_1 = PersistedPeer::new(ip_1, 0, ServiceFlags::NONE, false, true);
        peer_store.update(peer_1, false).await.unwrap();
        assert_eq!(peer_store.num_unbanned().await.unwrap(), 1);
        peer_store.update(peer_2, false).await.unwrap();
        assert_eq!(peer_store.num_unbanned().await.unwrap(), 2);
        peer_store.update(peer_3, false).await.unwrap();
        assert_eq!(peer_store.num_unbanned().await.unwrap(), 3);
        peer_store.update(try_peer_2, true).await.unwrap();
        assert_eq!(peer_store.num_unbanned().await.unwrap(), 3);
        peer_store.update(ban_peer_1.clone(), true).await.unwrap();
        assert_eq!(peer_store.num_unbanned().await.unwrap(), 2);
        let random = peer_store.random().await.unwrap();
        assert_ne!(ban_peer_1.addr, random.addr);
        let random = peer_store.random().await.unwrap();
        assert_ne!(ban_peer_1.addr, random.addr);
        let random = peer_store.random().await.unwrap();
        assert_ne!(ban_peer_1.addr, random.addr);
    }
}
