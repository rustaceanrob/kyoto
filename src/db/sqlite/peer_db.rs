use bitcoin::p2p::Address;
use bitcoin::Network;
use rusqlite::params;
use rusqlite::{Connection, Result};
use std::fs;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

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
    conn_new: Arc<Mutex<Connection>>,
    conn_tried: Arc<Mutex<Connection>>,
    conn_evict: Arc<Mutex<Connection>>,
    conn_cpf: Arc<Mutex<Connection>>,
    conn_ban: Arc<Mutex<Connection>>,
}

impl SqlitePeerDb {
    pub fn new(network: Network, path: Option<PathBuf>) -> Result<Self> {
        let default_port = match network {
            Network::Bitcoin => 8333,
            Network::Testnet => 18333,
            Network::Signet => 38333,
            Network::Regtest => panic!("unimplemented"),
            _ => unreachable!(),
        };
        let mut path = path.unwrap_or_else(|| PathBuf::from("."));
        path.push("data");
        path.push(network.to_string());
        if !path.exists() {
            fs::create_dir_all(&path).unwrap();
        }
        let conn_new = Connection::open(path.join("peers_new.db"))?;
        let conn_tried = Connection::open(path.join("peers_tried.db"))?;
        let conn_evict = Connection::open(path.join("peers_evict.db"))?;
        let conn_cpf = Connection::open(path.join("peers_cpf.db"))?;
        let conn_ban = Connection::open(path.join("peers_ban.db"))?;
        conn_new.execute(NEW_SCHEMA, [])?;
        conn_evict.execute(NEW_SCHEMA, [])?;
        conn_tried.execute(TRIED_SCHEMA, [])?;
        conn_cpf.execute(CPF_SCHEMA, [])?;
        conn_ban.execute(BAN_SCHEMA, [])?;
        Ok(Self {
            default_port,
            conn_new: Arc::new(Mutex::new(conn_new)),
            conn_tried: Arc::new(Mutex::new(conn_tried)),
            conn_evict: Arc::new(Mutex::new(conn_evict)),
            conn_cpf: Arc::new(Mutex::new(conn_cpf)),
            conn_ban: Arc::new(Mutex::new(conn_ban)),
        })
    }

    pub async fn add_new(
        &mut self,
        ip: IpAddr,
        port: Option<u16>,
        last_seen: Option<u32>,
    ) -> Result<()> {
        let lock = self.conn_new.lock().await;
        let new_count: i64 = lock.query_row("SELECT COUNT(*) FROM peers", [], |row| row.get(0))?;
        let peer_banned = self.is_banned(&ip).await?;

        if new_count < MAXIMUM_TABLE_SIZE && !peer_banned {
            lock.execute(
                "INSERT OR IGNORE INTO peers (ip_addr, port, last_seen) VALUES (?1, ?2, ?3)",
                params![
                    ip.to_string(),
                    port.unwrap_or(self.default_port),
                    last_seen.unwrap_or(0),
                ],
            )?;
        } else if !peer_banned {
            self.evict_random_peer().await?;
            lock.execute(
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
        let mut lock = self.conn_cpf.lock().await;
        let tx = lock.transaction()?;
        for peer in peers {
            let ip = peer
                .socket_addr()
                .expect("peers should have been screened")
                .ip()
                .to_string();
            tx.execute(
                "INSERT OR IGNORE INTO cpfpeers (ip_addr, port) VALUES (?1, ?2)",
                params![ip, peer.port],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    async fn is_banned(&self, ip_addr: &IpAddr) -> Result<bool> {
        let lock = self.conn_ban.lock().await;
        let mut stmt = lock.prepare("SELECT COUNT(*) FROM banned WHERE ip_addr = ?1")?;
        let count: i64 = stmt.query_row(params![ip_addr.to_string()], |row| row.get(0))?;
        Ok(count > 0)
    }

    async fn evict_random_peer(&self) -> Result<()> {
        let lock = self.conn_new.lock().await;
        let mut stmt = lock.prepare("SELECT ip_addr FROM peers ORDER BY RANDOM() LIMIT 1")?;
        let ip_addr: String = stmt.query_row([], |row| row.get(0))?;

        lock.execute(
            "INSERT OR IGNORE INTO peers (ip_addr, port, last_seen) SELECT ip_addr, port, last_seen FROM peers WHERE ip_addr = ?1",
            params![ip_addr],
        )?;

        lock.execute("DELETE FROM peers WHERE ip_addr = ?1", params![ip_addr])?;

        Ok(())
    }

    pub async fn get_random_new(&mut self) -> Result<Option<(IpAddr, u16)>> {
        if let Ok(Some(peer)) = self.get_random_from_evict().await {
            return Ok(Some(peer));
        }
        let lock = self.conn_new.lock().await;
        let mut stmt = lock.prepare("SELECT ip_addr, port FROM peers ORDER BY RANDOM() LIMIT 1")?;
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
        let lock = self.conn_evict.lock().await;
        let mut stmt = lock.prepare("SELECT ip_addr, port FROM peers ORDER BY RANDOM() LIMIT 1")?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            let ip_addr: String = row.get(0)?;
            let port: u16 = row.get(1)?;
            lock.execute("DELETE FROM peers WHERE ip_addr = ?1", &[&ip_addr])?;
            let ip = ip_addr
                .parse::<IpAddr>()
                .map_err(|_| rusqlite::Error::InvalidQuery)?;
            Ok(Some((ip, port)))
        } else {
            Ok(None)
        }
    }

    pub async fn get_random_cpf_peer(&mut self) -> Result<Option<(IpAddr, u16)>> {
        let lock = self.conn_cpf.lock().await;
        let mut stmt =
            lock.prepare("SELECT ip_addr, port FROM cpfpeers ORDER BY RANDOM() LIMIT 1")?;
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
