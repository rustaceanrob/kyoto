//! Traits and structures that define the data persistence required for a node.
//!
//! All nodes require a [`HeaderStore`](traits::HeaderStore) and a [`PeerStore`](traits::PeerStore). Unless
//! your application dependency tree is particularly strict, SQL-based storage will be sufficient for the majority of
//! applications.

/// Errors a database backend may produce.
pub mod error;
/// Persistence traits defined with SQL Lite to store data between sessions.
#[cfg(feature = "rusqlite")]
pub mod sqlite;
