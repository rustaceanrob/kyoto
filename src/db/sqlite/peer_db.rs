use bitcoin::p2p::Address;
use bitcoin::Network;
use rusqlite::params;
use rusqlite::{Connection, Result};
use std::net::IpAddr;

const MAXIMUM_TABLE_SIZE: i64 = 256;

const NEW_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS peers (
    ip_addr TEXT PRIMARY KEY,
    port INTEGER NOT NULL,
    last_seen INTEGER
)";

const TRIED_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS peers (
    ip_addr TEXT PRIMARY KEY,
    services INTEGER NOT NULL,
    port INTEGER NOT NULL
)";

const CPF_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS cpfpeers (
    ip_addr TEXT PRIMARY KEY,
    port INTEGER NOT NULL
)";

const BAN_SCHEMA: &str = "CREATE TABLE IF NOT EXISTS banned (
    ip_addr TEXT PRIMARY KEY,
    banned_until INTEGER
)";

pub(crate) struct SqlitePeerDb {
    default_port: u16,
    conn_new: Connection,
    conn_tried: Connection,
    conn_evict: Connection,
    conn_cpf: Connection,
    conn_ban: Connection,
}

impl SqlitePeerDb {
    pub fn new(network: Network) -> Result<Self> {
        let default_port = match network {
            Network::Bitcoin => 8333,
            Network::Testnet => 18333,
            Network::Signet => 38333,
            Network::Regtest => panic!("unimplemented"),
            _ => unreachable!(),
        };
        let conn_new = Connection::open("./data/peers_new.db")?;
        let conn_tried = Connection::open("./data/peers_tried.db")?;
        let conn_evict = Connection::open("./data/peers_evict.db")?;
        let conn_cpf = Connection::open("./data/peers_cpf.db")?;
        let conn_ban = Connection::open("./data/peers_ban.db")?;
        conn_new.execute(NEW_SCHEMA, [])?;
        conn_evict.execute(NEW_SCHEMA, [])?;
        conn_tried.execute(TRIED_SCHEMA, [])?;
        conn_cpf.execute(CPF_SCHEMA, [])?;
        conn_ban.execute(BAN_SCHEMA, [])?;
        Ok(Self {
            default_port,
            conn_new,
            conn_tried,
            conn_evict,
            conn_cpf,
            conn_ban,
        })
    }

    pub async fn add_new(
        &mut self,
        ip: IpAddr,
        port: Option<u16>,
        last_seen: Option<u32>,
    ) -> Result<()> {
        println!("Adding new peer {}", ip.to_string());
        let new_count: i64 = self
            .conn_new
            .query_row("SELECT COUNT(*) FROM peers", [], |row| row.get(0))?;
        let peer_banned = self.is_banned(&ip).await?;

        if new_count < MAXIMUM_TABLE_SIZE && !peer_banned {
            self.conn_new.execute(
                "INSERT OR IGNORE INTO peers (ip_addr, port, last_seen) VALUES (?1, ?2, ?3)",
                params![
                    ip.to_string(),
                    port.unwrap_or(self.default_port),
                    last_seen.unwrap_or(0),
                ],
            )?;
        } else if !peer_banned {
            self.evict_random_peer().await?;
            self.conn_new.execute(
                "INSERT OR IGNORE INTO peers (ip_addr, port, last_seen) VALUES (?1, ?2, ?3)",
                params![
                    ip.to_string(),
                    port.unwrap_or(self.default_port),
                    last_seen.unwrap_or(0),
                ],
            )?;
        }
        Ok(())
    }

    pub async fn add_cpf_peers(&mut self, peers: Vec<Address>) -> Result<()> {
        let tx = self.conn_cpf.transaction()?;
        for peer in peers {
            let ip = peer
                .socket_addr()
                .expect("peers should have been screened")
                .ip()
                .to_string();
            println!("Adding peer to CP filter peers: {}", ip);
            tx.execute(
                "INSERT OR IGNORE INTO cpfpeers (ip_addr, port) VALUES (?1, ?2)",
                params![ip, peer.port],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    async fn is_banned(&self, ip_addr: &IpAddr) -> Result<bool> {
        let mut stmt = self
            .conn_ban
            .prepare("SELECT COUNT(*) FROM banned WHERE ip_addr = ?1")?;
        let count: i64 = stmt.query_row(params![ip_addr.to_string()], |row| row.get(0))?;
        Ok(count > 0)
    }

    async fn evict_random_peer(&mut self) -> Result<()> {
        let mut stmt = self
            .conn_new
            .prepare("SELECT ip_addr FROM peers ORDER BY RANDOM() LIMIT 1")?;
        let ip_addr: String = stmt.query_row([], |row| row.get(0))?;

        self.conn_evict.execute(
            "INSERT OR IGNORE INTO peers (ip_addr, port, last_seen) SELECT ip_addr, port, last_seen FROM peers WHERE ip_addr = ?1",
            params![ip_addr],
        )?;

        self.conn_new
            .execute("DELETE FROM peers WHERE ip_addr = ?1", params![ip_addr])?;

        Ok(())
    }

    pub async fn get_random_new(&mut self) -> Result<Option<(IpAddr, u16)>> {
        println!("Fetching random peer");
        if let Ok(Some(peer)) = self.get_random_from_evict().await {
            println!("Got peer from eviction table");
            return Ok(Some(peer));
        }
        println!("Could not find a peer from eviction table. Trying a new peer.");
        let mut stmt = self
            .conn_new
            .prepare("SELECT ip_addr, port FROM peers ORDER BY RANDOM() LIMIT 1")?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let ip_addr: String = row.get(0)?;
            let port: u16 = row.get(1)?;
            let ip = ip_addr
                .parse::<IpAddr>()
                .map_err(|_| rusqlite::Error::InvalidQuery)?;
            Ok(Some((ip, port)))
        } else {
            Ok(None)
        }
    }

    async fn get_random_from_evict(&mut self) -> Result<Option<(IpAddr, u16)>> {
        let mut stmt = self
            .conn_evict
            .prepare("SELECT ip_addr, port FROM peers ORDER BY RANDOM() LIMIT 1")?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let ip_addr: String = row.get(0)?;
            let port: u16 = row.get(1)?;
            self.conn_evict
                .execute("DELETE FROM peers WHERE ip_addr = ?1", &[&ip_addr])?;
            let ip = ip_addr
                .parse::<IpAddr>()
                .map_err(|_| rusqlite::Error::InvalidQuery)?;
            Ok(Some((ip, port)))
        } else {
            Ok(None)
        }
    }

    pub async fn get_random_cpf_peer(&mut self) -> Result<Option<(IpAddr, u16)>> {
        let mut stmt = self
            .conn_cpf
            .prepare("SELECT ip_addr, port FROM cpfpeers ORDER BY RANDOM() LIMIT 1")?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let ip_addr: String = row.get(0)?;
            let port: u16 = row.get(1)?;
            let ip = ip_addr
                .parse::<IpAddr>()
                .map_err(|_| rusqlite::Error::InvalidQuery)?;
            Ok(Some((ip, port)))
        } else {
            Ok(None)
        }
    }
}
