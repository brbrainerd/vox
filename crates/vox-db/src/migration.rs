use vox_pm::store::StoreError;

/// Declarative schema migration entry.
#[derive(Debug, Clone)]
pub struct Migration {
    pub version: i64,
    pub name: String,
    pub up_sql: String,
}

impl Migration {
    pub fn new(version: i64, name: impl Into<String>, up_sql: impl Into<String>) -> Self {
        Self {
            version,
            name: name.into(),
            up_sql: up_sql.into(),
        }
    }
}

/// Validate migration ordering and uniqueness.
pub fn validate_migrations(migrations: &[Migration]) -> Result<(), StoreError> {
    let mut seen = std::collections::BTreeSet::new();
    let mut last = 0i64;
    for migration in migrations {
        if migration.version <= 0 {
            return Err(StoreError::NotFound(
                "migration version must be > 0".to_string(),
            ));
        }
        if migration.version <= last {
            return Err(StoreError::NotFound(
                "migrations must be sorted by increasing version".to_string(),
            ));
        }
        if !seen.insert(migration.version) {
            return Err(StoreError::NotFound(format!(
                "duplicate migration version {}",
                migration.version
            )));
        }
        last = migration.version;
    }
    Ok(())
}

/// Returns the canonical set of built-in schema migrations defined in vox-pm.
///
/// These correspond 1:1 with the `MIGRATIONS` constant in `vox_pm::schema`.
pub fn builtin_migrations() -> Vec<Migration> {
    vox_pm::schema::MIGRATIONS
        .iter()
        .map(|&(version, sql)| Migration::new(version, format!("v{version}"), sql))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_sorted_unique() {
        let migrations = vec![
            Migration::new(1, "one", "CREATE TABLE a(id INTEGER);"),
            Migration::new(2, "two", "CREATE TABLE b(id INTEGER);"),
        ];
        assert!(validate_migrations(&migrations).is_ok());
    }
}
