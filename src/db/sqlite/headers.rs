use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use bitcoin::block::{Header, Version};
use bitcoin::{BlockHash, CompactTarget, Network, TxMerkleNode};
use rusqlite::{params, Connection, Result};
use tokio::sync::Mutex;

use crate::db::error::DatabaseError;
use crate::db::traits::HeaderStore;

const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS headers (
    height INTEGER PRIMARY KEY,
    block_hash TEXT NOT NULL,
    version INTEGER NOT NULL,
    prev_hash TEXT NOT NULL,
    merkle_root TEXT NOT NULL,
    time INTEGER NOT NULL,
    bits INTEGER NOT NULL,
    nonce INTEGER NOT NULL
) STRICT";

/// Header storage implementation with SQL Lite.
#[derive(Debug)]
pub struct SqliteHeaderDb {
    network: Network,
    conn: Arc<Mutex<Connection>>,
}

impl SqliteHeaderDb {
    /// Create a new [`SqliteHeaderDb`] with an optional file path. If no path is provided,
    /// the file will be stored in a `data` subdirectory where the program is ran.
    pub fn new(network: Network, path: Option<PathBuf>) -> Result<Self, DatabaseError> {
        let mut path = path.unwrap_or_else(|| PathBuf::from("."));
        path.push("data");
        path.push(network.to_string());
        if !path.exists() {
            fs::create_dir_all(&path).unwrap();
        }
        let conn = Connection::open(path.join("headers.db")).map_err(|_| DatabaseError::Open)?;
        conn.execute(SCHEMA, []).map_err(|_| DatabaseError::Load)?;
        Ok(Self {
            network,
            conn: Arc::new(Mutex::new(conn)),
        })
    }
}

#[async_trait]
impl HeaderStore for SqliteHeaderDb {
    async fn load(&mut self, anchor_height: u32) -> Result<BTreeMap<u32, Header>, DatabaseError> {
        let mut headers = BTreeMap::<u32, Header>::new();
        let stmt = "SELECT * FROM headers ORDER BY height";
        let write_lock = self.conn.lock().await;
        let mut query = write_lock.prepare(stmt).map_err(|_| DatabaseError::Load)?;
        let mut rows = query.query([]).map_err(|_| DatabaseError::Load)?;
        while let Some(row) = rows.next().map_err(|_| DatabaseError::Load)? {
            let height: u32 = row.get(0).map_err(|_| DatabaseError::Load)?;
            // The anchor height should not be included in the chain, as the anchor is non-inclusive
            if height.le(&anchor_height) {
                continue;
            }
            let hash: String = row.get(1).map_err(|_| DatabaseError::Load)?;
            let version: i32 = row.get(2).map_err(|_| DatabaseError::Load)?;
            let prev_hash: String = row.get(3).map_err(|_| DatabaseError::Load)?;
            let merkle_root: String = row.get(4).map_err(|_| DatabaseError::Load)?;
            let time: u32 = row.get(5).map_err(|_| DatabaseError::Load)?;
            let bits: u32 = row.get(6).map_err(|_| DatabaseError::Load)?;
            let nonce: u32 = row.get(7).map_err(|_| DatabaseError::Load)?;

            let next_header = Header {
                version: Version::from_consensus(version),
                prev_blockhash: BlockHash::from_str(&prev_hash).unwrap(),
                merkle_root: TxMerkleNode::from_str(&merkle_root).unwrap(),
                time,
                bits: CompactTarget::from_consensus(bits),
                nonce,
            };
            if BlockHash::from_str(&hash)
                .unwrap()
                .ne(&next_header.block_hash())
            {
                return Err(DatabaseError::Corruption);
            }
            if let Some(header) = headers.values().last() {
                if header.block_hash().ne(&next_header.prev_blockhash) {
                    return Err(DatabaseError::Corruption);
                }
            }
            headers.insert(height, next_header);
        }
        Ok(headers)
    }

    async fn write<'a>(
        &mut self,
        header_chain: &'a BTreeMap<u32, Header>,
    ) -> Result<(), DatabaseError> {
        let mut write_lock = self.conn.lock().await;
        let tx = write_lock.transaction().map_err(|_| DatabaseError::Load)?;
        let best_height: Option<u32> = tx
            .query_row("SELECT MAX(height) FROM headers", [], |row| row.get(0))
            .map_err(|_| DatabaseError::Write)?;
        for (height, header) in header_chain {
            if height.ge(&(best_height.unwrap_or(0))) {
                let hash: String = header.block_hash().to_string();
                let version: i32 = header.version.to_consensus();
                let prev_hash: String = header.prev_blockhash.as_raw_hash().to_string();
                let merkle_root: String = header.merkle_root.to_string();
                let time: u32 = header.time;
                let bits: u32 = header.bits.to_consensus();
                let nonce: u32 = header.nonce;
                let stmt = "INSERT OR REPLACE INTO headers (height, block_hash, version, prev_hash, merkle_root, time, bits, nonce) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)";
                tx.execute(
                    stmt,
                    params![
                        height,
                        hash,
                        version,
                        prev_hash,
                        merkle_root,
                        time,
                        bits,
                        nonce
                    ],
                )
                .map_err(|_| DatabaseError::Write)?;
            }
        }
        tx.commit().map_err(|_| DatabaseError::Write)?;
        Ok(())
    }

    async fn write_over<'a>(
        &mut self,
        header_chain: &'a BTreeMap<u32, Header>,
        height: u32,
    ) -> Result<(), DatabaseError> {
        let mut write_lock = self.conn.lock().await;
        let tx = write_lock.transaction().map_err(|_| DatabaseError::Write)?;
        for (new_height, header) in header_chain {
            if new_height.ge(&height) {
                let hash: String = header.block_hash().to_string();
                let version: i32 = header.version.to_consensus();
                let prev_hash: String = header.prev_blockhash.as_raw_hash().to_string();
                let merkle_root: String = header.merkle_root.to_string();
                let time: u32 = header.time;
                let bits: u32 = header.bits.to_consensus();
                let nonce: u32 = header.nonce;
                let stmt = "INSERT OR REPLACE INTO headers (height, block_hash, version, prev_hash, merkle_root, time, bits, nonce) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)";
                tx.execute(
                    stmt,
                    params![
                        new_height,
                        hash,
                        version,
                        prev_hash,
                        merkle_root,
                        time,
                        bits,
                        nonce
                    ],
                )
                .map_err(|_| DatabaseError::Write)?;
            }
        }
        tx.commit().map_err(|_| DatabaseError::Write)?;
        Ok(())
    }

    async fn height_of<'a>(
        &mut self,
        block_hash: &'a BlockHash,
    ) -> Result<Option<u32>, DatabaseError> {
        let write_lock = self.conn.lock().await;
        let stmt = "SELECT height FROM headers WHERE block_hash = ?1";
        let row: Option<u32> = write_lock
            .query_row(stmt, params![block_hash.to_string()], |row| row.get(0))
            .map_err(|_| DatabaseError::Load)?;
        Ok(row)
    }

    async fn hash_at(&mut self, height: u32) -> Result<Option<BlockHash>, DatabaseError> {
        let write_lock = self.conn.lock().await;
        let stmt = "SELECT block_hash FROM headers WHERE height = ?1";
        let row: Option<String> = write_lock
            .query_row(stmt, params![height], |row| row.get(0))
            .map_err(|_| DatabaseError::Load)?;
        match row {
            Some(row) => match BlockHash::from_str(&row) {
                Ok(hash) => Ok(Some(hash)),
                Err(_) => Err(DatabaseError::Deserialization),
            },
            None => Ok(None),
        }
    }
}
