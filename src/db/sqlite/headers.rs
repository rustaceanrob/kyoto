use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use bitcoin::block::{Header, Version};
use bitcoin::{BlockHash, CompactTarget, Network, TxMerkleNode};
use rusqlite::{params, Connection, Result};
use tokio::sync::Mutex;

use crate::db::error::{SqlError, SqlInitializationError};
use crate::db::traits::HeaderStore;
use crate::prelude::FutureResult;

// Labels for the schema table
const SCHEMA_TABLE_NAME: &str = "header_schema_versions";
const SCHEMA_COLUMN: &str = "schema_key";
const VERSION_COLUMN: &str = "version";
const SCHEMA_KEY: &str = "current_version";
// Update this in the case of schema changes
const SCHEMA_VERSION: u8 = 0;
// Always execute this query and adjust the schema with migrations
const INITIAL_HEADER_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS headers (
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
    conn: Arc<Mutex<Connection>>,
}

impl SqliteHeaderDb {
    /// Create a new [`SqliteHeaderDb`] with an optional file path. If no path is provided,
    /// the file will be stored in a `data` subdirectory where the program is ran.
    pub fn new(network: Network, path: Option<PathBuf>) -> Result<Self, SqlInitializationError> {
        let mut path = path.unwrap_or_else(|| PathBuf::from("."));
        path.push("data");
        path.push(network.to_string());
        if !path.exists() {
            fs::create_dir_all(&path)?;
        }
        let conn = Connection::open(path.join("headers.db"))?;
        // Create the schema version
        let schema_table_query = format!(
            "CREATE TABLE IF NOT EXISTS {SCHEMA_TABLE_NAME} ({SCHEMA_COLUMN} TEXT PRIMARY KEY, {VERSION_COLUMN} INTEGER NOT NULL)");
        // Update the schema version
        conn.execute(&schema_table_query, [])?;
        let schema_init_version = format!(
            "INSERT OR REPLACE INTO {SCHEMA_TABLE_NAME} ({SCHEMA_COLUMN}, {VERSION_COLUMN}) VALUES (?1, ?2)");
        conn.execute(&schema_init_version, params![SCHEMA_KEY, SCHEMA_VERSION])?;
        // Build the table if it doesn't exist
        conn.execute(INITIAL_HEADER_SCHEMA, [])?;
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

    async fn load_after(&mut self, anchor_height: u32) -> Result<BTreeMap<u32, Header>, SqlError> {
        let mut headers = BTreeMap::<u32, Header>::new();
        let stmt = "SELECT * FROM headers ORDER BY height";
        let write_lock = self.conn.lock().await;
        let mut query = write_lock.prepare(stmt)?;
        let mut rows = query.query([])?;
        while let Some(row) = rows.next()? {
            let height: u32 = row.get(0)?;
            // The anchor height should not be included in the chain, as the anchor is non-inclusive
            if height.le(&anchor_height) {
                continue;
            }
            let hash: String = row.get(1)?;
            let version: i32 = row.get(2)?;
            let prev_hash: String = row.get(3)?;
            let merkle_root: String = row.get(4)?;
            let time: u32 = row.get(5)?;
            let bits: u32 = row.get(6)?;
            let nonce: u32 = row.get(7)?;

            let next_header = Header {
                version: Version::from_consensus(version),
                prev_blockhash: BlockHash::from_str(&prev_hash)
                    .map_err(|_| SqlError::StringConversion)?,
                merkle_root: TxMerkleNode::from_str(&merkle_root)
                    .map_err(|_| SqlError::StringConversion)?,
                time,
                bits: CompactTarget::from_consensus(bits),
                nonce,
            };
            if BlockHash::from_str(&hash)
                .map_err(|_| SqlError::StringConversion)?
                .ne(&next_header.block_hash())
            {
                return Err(SqlError::Corruption);
            }
            if let Some(header) = headers.values().last() {
                if header.block_hash().ne(&next_header.prev_blockhash) {
                    return Err(SqlError::Corruption);
                }
            }
            headers.insert(height, next_header);
        }
        Ok(headers)
    }

    async fn write<'a>(&mut self, header_chain: &'a BTreeMap<u32, Header>) -> Result<(), SqlError> {
        let mut write_lock = self.conn.lock().await;
        let tx = write_lock.transaction()?;
        let best_height: Option<u32> =
            tx.query_row("SELECT MAX(height) FROM headers", [], |row| row.get(0))?;
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
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    async fn write_over<'a>(
        &mut self,
        header_chain: &'a BTreeMap<u32, Header>,
        height: u32,
    ) -> Result<(), SqlError> {
        let mut write_lock = self.conn.lock().await;
        let tx = write_lock.transaction()?;
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
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    async fn height_of<'a>(&mut self, block_hash: &'a BlockHash) -> Result<Option<u32>, SqlError> {
        let write_lock = self.conn.lock().await;
        let stmt = "SELECT height FROM headers WHERE block_hash = ?1";
        let row: Option<u32> =
            write_lock.query_row(stmt, params![block_hash.to_string()], |row| row.get(0))?;
        Ok(row)
    }

    async fn hash_at(&mut self, height: u32) -> Result<Option<BlockHash>, SqlError> {
        let write_lock = self.conn.lock().await;
        let stmt = "SELECT block_hash FROM headers WHERE height = ?1";
        let row: Option<String> = write_lock.query_row(stmt, params![height], |row| row.get(0))?;
        match row {
            Some(row) => match BlockHash::from_str(&row) {
                Ok(hash) => Ok(Some(hash)),
                Err(_) => Err(SqlError::StringConversion),
            },
            None => Ok(None),
        }
    }
}

impl HeaderStore for SqliteHeaderDb {
    type Error = SqlError;
    fn load_after(
        &mut self,
        anchor_height: u32,
    ) -> FutureResult<BTreeMap<u32, Header>, Self::Error> {
        Box::pin(self.load_after(anchor_height))
    }

    fn write<'a>(
        &'a mut self,
        header_chain: &'a BTreeMap<u32, Header>,
    ) -> FutureResult<'a, (), Self::Error> {
        Box::pin(self.write(header_chain))
    }

    fn write_over<'a>(
        &'a mut self,
        header_chain: &'a BTreeMap<u32, Header>,
        height: u32,
    ) -> FutureResult<'a, (), Self::Error> {
        Box::pin(self.write_over(header_chain, height))
    }

    fn height_of<'a>(
        &'a mut self,
        hash: &'a BlockHash,
    ) -> FutureResult<'a, Option<u32>, Self::Error> {
        Box::pin(self.height_of(hash))
    }

    fn hash_at(&mut self, height: u32) -> FutureResult<Option<BlockHash>, Self::Error> {
        Box::pin(self.hash_at(height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitcoin::consensus::deserialize;

    #[tokio::test]
    async fn test_sql_header_store_normal_use() {
        let binding = tempfile::tempdir().unwrap();
        let path = binding.path();
        let mut db = SqliteHeaderDb::new(Network::Regtest, Some(path.into())).unwrap();
        let block_8: Header = deserialize(&hex::decode("0000002016fe292517eecbbd63227d126a6b1db30ebc5262c61f8f3a4a529206388fc262dfd043cef8454f71f30b5bbb9eb1a4c9aea87390f429721e435cf3f8aa6e2a9171375166ffff7f2000000000").unwrap()).unwrap();
        let block_9: Header = deserialize(&hex::decode("000000205708a90197d93475975545816b2229401ccff7567cb23900f14f2bd46732c605fd8de19615a1d687e89db365503cdf58cb649b8e935a1d3518fa79b0d408704e71375166ffff7f2000000000").unwrap()).unwrap();
        let block_10: Header = deserialize(&hex::decode("000000201d062f2162835787db536c55317e08df17c58078c7610328bdced198574093790c9f554a7780a6043a19619d2a4697364bb62abf6336c0568c31f1eedca3c3e171375166ffff7f2000000000").unwrap()).unwrap();
        let mut map = BTreeMap::new();
        map.insert(8, block_8);
        map.insert(9, block_9);
        map.insert(10, block_10);
        let block_hash_8 = block_8.block_hash();
        let block_hash_9 = block_9.block_hash();
        let w = db.write(&map).await;
        assert!(w.is_ok());
        let get_hash_9 = db.hash_at(9).await.unwrap().unwrap();
        assert_eq!(get_hash_9, block_hash_9);
        let get_height_8 = db.height_of(&block_hash_8).await.unwrap().unwrap();
        assert_eq!(get_height_8, 8);
        let load = db.load_after(7).await.unwrap();
        assert_eq!(map, load);
        drop(db);
        binding.close().unwrap();
    }

    #[tokio::test]
    async fn test_sql_header_loads_with_fork() {
        let binding = tempfile::tempdir().unwrap();
        let path = binding.path();
        let mut db = SqliteHeaderDb::new(Network::Regtest, Some(path.into())).unwrap();
        let block_8: Header = deserialize(&hex::decode("0000002016fe292517eecbbd63227d126a6b1db30ebc5262c61f8f3a4a529206388fc262dfd043cef8454f71f30b5bbb9eb1a4c9aea87390f429721e435cf3f8aa6e2a9171375166ffff7f2000000000").unwrap()).unwrap();
        let block_9: Header = deserialize(&hex::decode("000000205708a90197d93475975545816b2229401ccff7567cb23900f14f2bd46732c605fd8de19615a1d687e89db365503cdf58cb649b8e935a1d3518fa79b0d408704e71375166ffff7f2000000000").unwrap()).unwrap();
        let block_10: Header = deserialize(&hex::decode("000000201d062f2162835787db536c55317e08df17c58078c7610328bdced198574093790c9f554a7780a6043a19619d2a4697364bb62abf6336c0568c31f1eedca3c3e171375166ffff7f2000000000").unwrap()).unwrap();
        let mut map = BTreeMap::new();
        map.insert(8, block_8);
        map.insert(9, block_9);
        map.insert(10, block_10);
        let w = db.write(&map).await;
        assert!(w.is_ok());
        let new_block_10: Header = deserialize(&hex::decode("000000201d062f2162835787db536c55317e08df17c58078c7610328bdced198574093792151c0e9ce4e4c789ca98427d7740cc7acf30d2ca0c08baef266bf152289d814567e5e66ffff7f2001000000").unwrap()).unwrap();
        let block_11: Header = deserialize(&hex::decode("00000020efcf8b12221fccc735b9b0b657ce15b31b9c50aff530ce96a5b4cfe02d8c0068496c1b8a89cf5dec22e46c35ea1035f80f5b666a1b3aa7f3d6f0880d0061adcc567e5e66ffff7f2001000000").unwrap()).unwrap();
        let mut map = BTreeMap::new();
        map.insert(10, new_block_10);
        map.insert(11, block_11);
        let w = db.write_over(&map, 10).await;
        assert!(w.is_ok());
        let block_hash_11 = block_11.block_hash();
        let block_hash_10 = new_block_10.block_hash();
        let w = db.write(&map).await;
        assert!(w.is_ok());
        let get_hash_10 = db.hash_at(10).await.unwrap().unwrap();
        assert_eq!(get_hash_10, block_hash_10);
        let get_height_11 = db.height_of(&block_hash_11).await.unwrap().unwrap();
        assert_eq!(get_height_11, 11);
        let mut map = BTreeMap::new();
        map.insert(8, block_8);
        map.insert(9, block_9);
        map.insert(10, new_block_10);
        map.insert(11, block_11);
        let load = db.load_after(7).await.unwrap();
        assert_eq!(map, load);
        drop(db);
        binding.close().unwrap();
    }
}
