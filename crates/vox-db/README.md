# vox-db

High-level database facade for the Vox ecosystem. Wraps `vox-pm::CodeStore` with connection management, retry logic, and transaction support.

## Connection Modes

| Mode | Feature Flag | Use Case |
|------|-------------|----------|
| Remote (Turso) | (default) | Production — cloud-hosted |
| Local Turso | `local` | Development — file-based |
| In-Memory | `local` | Testing — ephemeral |
| Embedded Replica | `replication` | Hybrid — local + cloud sync |

## Key APIs

| Method | Description |
|--------|-------------|
| `VoxDb::connect(config)` | Connect with automatic retry (3× w/ backoff) |
| `VoxDb::store()` | Access the underlying `CodeStore` |
| `VoxDb::sync()` | Sync embedded replica with remote |
| `VoxDb::schema_version()` | Get current schema version |
| `VoxDb::transaction(f)` | Execute within BEGIN/COMMIT/ROLLBACK |

## Usage

```rust
use vox_db::{VoxDb, DbConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db = VoxDb::connect(DbConfig::Remote {
        url: "turso://my-db.turso.io".to_string(),
        token: "my-token".to_string(),
    }).await?;

    let hash = db.store().store("fn", b"fn hello(): ret 42").await?;
    println!("Stored: {hash}");
    Ok(())
}
```
