//! SQLite pragmas applied on every [`crate::VoxDb`] open path.
//!
//! Implemented in [`VoxDb::apply_pragmas`](crate::VoxDb::apply_pragmas) using
//! [`turso::Connection::pragma_update`] because `PRAGMA name = value` returns a result row and
//! must not be sent through [`turso::Connection::execute_batch`] (which uses `execute` only).
//!
//! Applied settings:
//! - `journal_mode=WAL` (or `mvcc` if `VOX_DB_MVCC=1`)
//! - `busy_timeout=5000`
//! - `synchronous=NORMAL`
//! - `foreign_keys=ON`
//! - `cache_size=-65536`
//!
//! Notes:
//! - `temp_store` and `mmap_size` are intentionally NOT applied because libSQL/Turso
//!   does not consistently support them across local/remote transports.
