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

use crate::chain::checkpoints::HeaderCheckpoint;
use crate::db::error::HeaderDatabaseError;
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

#[derive(Debug)]
pub(crate) struct SqliteHeaderDb {
    network: Network,
    conn: Arc<Mutex<Connection>>,
    anchor_height: u32,
    anchor_hash: BlockHash,
}

impl SqliteHeaderDb {
    pub fn new(
        network: Network,
        anchor_checkpoint: HeaderCheckpoint,
        path: Option<PathBuf>,
    ) -> Result<Self, HeaderDatabaseError> {
        let mut path = path.unwrap_or_else(|| PathBuf::from("."));
        path.push("data");
        path.push(network.to_string());
        if !path.exists() {
            fs::create_dir_all(&path).unwrap();
        }
        let conn = Connection::open(path.join("headers.db"))
            .map_err(|_| HeaderDatabaseError::LoadError)?;
        conn.execute(SCHEMA, [])
            .map_err(|_| HeaderDatabaseError::LoadError)?;
        Ok(Self {
            network,
            conn: Arc::new(Mutex::new(conn)),
            anchor_height: anchor_checkpoint.height,
            anchor_hash: anchor_checkpoint.hash,
        })
    }
}

#[async_trait]
impl HeaderStore for SqliteHeaderDb {
    // load all the known headers from storage
    async fn load(&mut self) -> Result<BTreeMap<u32, Header>, HeaderDatabaseError> {
        let mut headers = BTreeMap::<u32, Header>::new();
        let stmt = "SELECT * FROM headers ORDER BY height";
        let write_lock = self.conn.lock().await;
        let mut query = write_lock
            .prepare(stmt)
            .map_err(|_| HeaderDatabaseError::LoadError)?;
        let mut rows = query
            .query([])
            .map_err(|_| HeaderDatabaseError::LoadError)?;
        while let Some(row) = rows.next().map_err(|_| HeaderDatabaseError::LoadError)? {
            let height: u32 = row.get(0).map_err(|_| HeaderDatabaseError::LoadError)?;
            // The anchor height should not be included in the chain, as the anchor is non-inclusive
            if height.le(&self.anchor_height) {
                continue;
            }
            let hash: String = row.get(1).map_err(|_| HeaderDatabaseError::LoadError)?;
            let version: i32 = row.get(2).map_err(|_| HeaderDatabaseError::LoadError)?;
            let prev_hash: String = row.get(3).map_err(|_| HeaderDatabaseError::LoadError)?;
            let merkle_root: String = row.get(4).map_err(|_| HeaderDatabaseError::LoadError)?;
            let time: u32 = row.get(5).map_err(|_| HeaderDatabaseError::LoadError)?;
            let bits: u32 = row.get(6).map_err(|_| HeaderDatabaseError::LoadError)?;
            let nonce: u32 = row.get(7).map_err(|_| HeaderDatabaseError::LoadError)?;

            let next_header = Header {
                version: Version::from_consensus(version),
                prev_blockhash: BlockHash::from_str(&prev_hash).unwrap(),
                merkle_root: TxMerkleNode::from_str(&merkle_root).unwrap(),
                time,
                bits: CompactTarget::from_consensus(bits),
                nonce,
            };

            assert_eq!(
                BlockHash::from_str(&hash).unwrap(),
                next_header.block_hash(),
                "db corruption. incorrect header hash."
            );

            if let Some(header) = headers.values().last() {
                assert_eq!(
                    header.block_hash(),
                    next_header.prev_blockhash,
                    "db corruption. headers do not link."
                );
            }
            headers.insert(height, next_header);
        }
        Ok(headers)
    }

    async fn write<'a>(
        &mut self,
        header_chain: &'a BTreeMap<u32, Header>,
    ) -> Result<(), HeaderDatabaseError> {
        let mut write_lock = self.conn.lock().await;
        let tx = write_lock
            .transaction()
            .map_err(|_| HeaderDatabaseError::WriteError)?;
        let best_height: Option<u32> = tx
            .query_row("SELECT MAX(height) FROM headers", [], |row| row.get(0))
            .map_err(|_| HeaderDatabaseError::WriteError)?;
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
                .map_err(|_| HeaderDatabaseError::WriteError)?;
            }
        }
        tx.commit().map_err(|_| HeaderDatabaseError::WriteError)?;
        Ok(())
    }

    async fn write_over<'a>(
        &mut self,
        header_chain: &'a BTreeMap<u32, Header>,
        height: u32,
    ) -> Result<(), HeaderDatabaseError> {
        let mut write_lock = self.conn.lock().await;
        let tx = write_lock
            .transaction()
            .map_err(|_| HeaderDatabaseError::WriteError)?;
        for (h, header) in header_chain {
            if h.ge(&height) {
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
                .map_err(|_| HeaderDatabaseError::WriteError)?;
            }
        }
        tx.commit().map_err(|_| HeaderDatabaseError::WriteError)?;
        Ok(())
    }
}
