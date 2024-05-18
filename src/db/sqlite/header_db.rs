use std::str::FromStr;

use bitcoin::block::{Header, Version};
use bitcoin::constants::genesis_block;
use bitcoin::params::Params;
use bitcoin::{BlockHash, CompactTarget, Network, TxMerkleNode};
use rusqlite::{params, Connection, Result};

use crate::chain::checkpoints::HeaderCheckpoint;

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
    genesis_header: Header,
    network: Network,
    conn: Connection,
    last_checkpoint: HeaderCheckpoint,
}

impl SqliteHeaderDb {
    pub fn new(network: Network, last_checkpoint: HeaderCheckpoint) -> Result<Self> {
        let genesis = match network {
            Network::Bitcoin => panic!("unimplemented"),
            Network::Testnet => genesis_block(Params::new(network)).header,
            Network::Signet => genesis_block(Params::new(network)).header,
            Network::Regtest => panic!("unimplemented"),
            _ => unreachable!(),
        };
        let conn = Connection::open("./data/headers.db")?;
        conn.execute(SCHEMA, [])?;
        Ok(Self {
            genesis_header: genesis,
            network,
            conn,
            last_checkpoint,
        })
    }

    // load all the known headers from storage
    pub fn load(&mut self) -> Result<Vec<Header>> {
        println!("Loading headers from storage");
        let mut headers: Vec<Header> = Vec::with_capacity(200_000);
        let stmt = "SELECT * FROM headers ORDER BY height";
        let mut query = self.conn.prepare(&stmt)?;
        let mut rows = query.query([])?;
        while let Some(row) = rows.next()? {
            let _height: u32 = row.get(0)?;
            let hash: String = row.get(1)?;
            let version: i32 = row.get(2)?;
            let prev_hash: String = row.get(3)?;
            let merkle_root: String = row.get(4)?;
            let time: u32 = row.get(5)?;
            let bits: u32 = row.get(6)?;
            let nonce: u32 = row.get(7)?;

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

            if let Some(header) = headers.last() {
                assert_eq!(
                    header.block_hash(),
                    next_header.prev_blockhash,
                    "db corruption. headers do not link."
                );
                headers.push(next_header);
            } else {
                assert_eq!(
                    next_header.block_hash(),
                    self.genesis_header.block_hash(),
                    "db corruption. genesis mismatch."
                );
                headers.push(next_header);
            }
        }
        Ok(headers)
    }

    pub async fn write(&mut self, header_chain: &Vec<Header>) -> Result<()> {
        let tx = self.conn.transaction()?;
        let count: u64 = tx.query_row("SELECT COUNT(*) FROM headers", [], |row| row.get(0))?;
        let adjusted_count = count.checked_sub(1).unwrap_or(0);
        for (height, header) in header_chain.iter().enumerate() {
            if height.ge(&(adjusted_count as usize)) {
                let hash: String = header.block_hash().to_string();
                let version: i32 = header.version.to_consensus();
                let prev_hash: String = header.prev_blockhash.as_raw_hash().to_string();
                let merkle_root: String = header.merkle_root.to_string();
                let time: u32 = header.time;
                let bits: u32 = header.bits.to_consensus();
                let nonce: u32 = header.nonce;
                // Do not allow rewrites before a checkpoint. if they were written to the db they were correct
                let stmt = if height.le(&self.last_checkpoint.height) {
                    "INSERT OR IGNORE INTO headers (height, block_hash, version, prev_hash, merkle_root, time, bits, nonce) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
                } else {
                    "INSERT OR REPLACE INTO headers (height, block_hash, version, prev_hash, merkle_root, time, bits, nonce) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
                };
                tx.execute(
                    &stmt,
                    params![
                        height as u32,
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
        println!("Wrote headers to db.");
        Ok(())
    }
}
