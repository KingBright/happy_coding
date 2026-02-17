//! Storage layer
//!
//! Uses SQLite (embedded) instead of PostgreSQL for simplicity.
//! Uses DashMap (in-memory) instead of Redis for caching.

pub mod db;
pub mod memory;

pub use db::Database;
pub use memory::MemoryCache;
