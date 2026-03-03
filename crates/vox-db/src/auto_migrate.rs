//! Auto-migration engine for VoxDB.
//!
//! Introspects the live SQLite schema from the database, compares it against the
//! desired schema derived from `@table` declarations, and applies non-destructive
//! migrations (ADD COLUMN, CREATE TABLE, CREATE INDEX).
//!
//! Destructive operations (DROP TABLE, DROP COLUMN) are never performed automatically
//! — they are reported as pending manual migrations.

use turso::Connection;

/// A column definition as read from `PRAGMA table_info(...)`.
#[derive(Debug, Clone)]
pub struct LiveColumn {
    pub name: String,
    pub col_type: String,
    pub not_null: bool,
    pub default_value: Option<String>,
    pub is_pk: bool,
}

/// A table definition as introspected from the live database.
#[derive(Debug, Clone)]
pub struct LiveTable {
    pub name: String,
    pub columns: Vec<LiveColumn>,
}

use vox_ast::decl::{CollectionDecl, IndexDecl, TableDecl};

/// A single migration action to be applied.
#[derive(Debug, Clone)]
pub enum MigrationAction {
    /// Create a new table with all its columns.
    CreateTable { sql: String },
    /// Add a column to an existing table.
    AddColumn {
        table: String,
        column: String,
        col_type: String,
    },
    /// Create an index.
    CreateIndex { sql: String },
    /// Create a collection document table.
    CreateCollection { sql: String },
    /// ⚠️ A column was removed from the declaration but still exists in the DB.
    /// Not applied automatically.
    ManualDropColumn { table: String, column: String },
    /// ⚠️ A table was removed from declarations but still exists in the DB.
    /// Not applied automatically.
    ManualDropTable { table: String },
}

impl MigrationAction {
    /// Returns `true` if this action is safe to apply automatically.
    pub fn is_auto_safe(&self) -> bool {
        matches!(
            self,
            MigrationAction::CreateTable { .. }
                | MigrationAction::AddColumn { .. }
                | MigrationAction::CreateIndex { .. }
                | MigrationAction::CreateCollection { .. }
        )
    }

    /// Generate SQL for this action. Returns `None` for manual-only actions.
    pub fn to_sql(&self) -> Option<String> {
        match self {
            MigrationAction::CreateTable { sql } => Some(sql.clone()),
            MigrationAction::AddColumn {
                table,
                column,
                col_type,
            } => Some(format!(
                "ALTER TABLE {table} ADD COLUMN {column} {col_type}"
            )),
            MigrationAction::CreateIndex { sql } => Some(sql.clone()),
            MigrationAction::CreateCollection { sql } => Some(sql.clone()),
            MigrationAction::ManualDropColumn { .. } | MigrationAction::ManualDropTable { .. } => {
                None
            }
        }
    }

    /// Human-readable description of this action.
    pub fn describe(&self) -> String {
        match self {
            MigrationAction::CreateTable { .. } => "create new table".to_string(),
            MigrationAction::AddColumn {
                table,
                column,
                col_type,
            } => format!("add column `{column}` ({col_type}) to `{table}`"),
            MigrationAction::CreateIndex { .. } => "create index".to_string(),
            MigrationAction::CreateCollection { .. } => "create collection table".to_string(),
            MigrationAction::ManualDropColumn { table, column } => {
                format!(
                    "⚠️  column `{column}` in `{table}` no longer declared (manual review needed)"
                )
            }
            MigrationAction::ManualDropTable { table } => {
                format!("⚠️  table `{table}` no longer declared (manual review needed)")
            }
        }
    }
}

/// Result of a migration plan.
#[derive(Debug)]
pub struct MigrationPlan {
    pub actions: Vec<MigrationAction>,
}

impl MigrationPlan {
    /// Returns `true` if no changes are needed.
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    /// Actions that are safe to apply automatically.
    pub fn auto_actions(&self) -> Vec<&MigrationAction> {
        self.actions.iter().filter(|a| a.is_auto_safe()).collect()
    }

    /// Actions that require manual intervention.
    pub fn manual_actions(&self) -> Vec<&MigrationAction> {
        self.actions.iter().filter(|a| !a.is_auto_safe()).collect()
    }

    /// Generate a human-readable summary of the migration plan.
    pub fn describe(&self) -> String {
        if self.is_empty() {
            return "Schema is up to date.".to_string();
        }
        let mut lines = Vec::new();
        for action in &self.actions {
            let prefix = if action.is_auto_safe() {
                "  ✓"
            } else {
                "  ⚠"
            };
            lines.push(format!("{prefix} {}", action.describe()));
        }
        lines.join("\n")
    }
}

/// The auto-migration engine.
pub struct AutoMigrator<'a> {
    conn: &'a Connection,
}

use crate::schema_digest::{SchemaDigest, IndexKind};


impl<'a> AutoMigrator<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Introspect the live database to get all user tables.
    pub async fn introspect_tables(&self) -> Result<Vec<LiveTable>, turso::Error> {
        // Get all table names (excluding sqlite internal tables)
        let mut rows = self
            .conn
            .query(
                "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '_vox_%' AND name != 'schema_version'",
                (),
            )
            .await?;

        let mut tables = Vec::new();
        let mut table_names = Vec::new();
        while let Some(row) = rows.next().await? {
            let name: String = row.get::<String>(0)?;
            table_names.push(name);
        }

        for name in table_names {
            let sql = format!("PRAGMA table_info('{name}')");
            let mut col_rows = self.conn.query(&sql, ()).await?;
            let mut columns = Vec::new();

            while let Some(row) = col_rows.next().await? {
                let col_name: String = row.get::<String>(1)?;
                let col_type: String = row.get::<String>(2)?;
                let not_null: i64 = row.get::<i64>(3)?;
                let default_val: Option<String> = row.get::<Option<String>>(4)?;
                let is_pk: i64 = row.get::<i64>(5)?;

                columns.push(LiveColumn {
                    name: col_name,
                    col_type,
                    not_null: not_null != 0,
                    default_value: default_val,
                    is_pk: is_pk != 0,
                });
            }

            tables.push(LiveTable { name, columns });
        }

        Ok(tables)
    }

    /// Compute the migration plan by diffing live schema against desired tables.
    pub async fn plan(
        &self,
        tables: &[&TableDecl],
        collections: &[&CollectionDecl],
        indexes: &[&IndexDecl],
    ) -> Result<MigrationPlan, turso::Error> {
        let live_tables = self.introspect_tables().await?;
        let live_names: std::collections::HashSet<&str> =
            live_tables.iter().map(|t| t.name.as_str()).collect();

        self.plan_internal(&live_tables, &live_names, tables, collections, indexes)
    }

    pub async fn plan_from_digest(
        &self,
        digest: &SchemaDigest,
    ) -> Result<MigrationPlan, turso::Error> {
        let live_tables = self.introspect_tables().await?;
        let mut actions: Vec<MigrationAction> = Vec::new();

        let live_names: std::collections::HashSet<&str> =
            live_tables.iter().map(|t| t.name.as_str()).collect();

        // 1. Tables to create
        for dt in &digest.tables {
            let snaked_name = crate::ddl::to_snake_case(&dt.name);
            if !live_names.contains(snaked_name.as_str()) {
                // We need table_to_ddl but SchemaDigest doesn't have TableDecl
                // Let's add ddl_from_info to ddl.rs
                let sql = crate::ddl::table_info_to_ddl(dt);
                actions.push(MigrationAction::CreateTable { sql });
            }
        }

        // 2. Collections to create
        for dc in &digest.collections {
            let snaked_name = crate::ddl::to_snake_case(&dc.name);
            if !live_names.contains(snaked_name.as_str()) {
                let sql = crate::ddl::collection_info_to_ddl(dc);
                actions.push(MigrationAction::CreateCollection { sql });
            }
        }

        // 3. Columns to add
        for dt in &digest.tables {
            let snaked_name = crate::ddl::to_snake_case(&dt.name);
            if let Some(lt) = live_tables.iter().find(|t| t.name == snaked_name) {
                let live_cols: std::collections::HashSet<&str> =
                    lt.columns.iter().map(|c| c.name.as_str()).collect();

                for f in &dt.fields {
                    if !live_cols.contains(f.name.as_str()) {
                        actions.push(MigrationAction::AddColumn {
                            table: snaked_name.clone(),
                            column: f.name.clone(),
                            col_type: crate::ddl::vox_type_to_sqlite_type(&f.type_str).to_string(),
                        });
                    }
                }
            }
        }

        // 4. Indexes to create
        for idx in &digest.indexes {
            // Check if index exists in live DB
            // (introspection for indexes is TBD, for now IF NOT EXISTS handles it)
            let sql = match idx.kind {
                IndexKind::Standard => crate::ddl::index_info_to_ddl(idx),
                _ => continue, // Vector/Search indexes handled separately
            };
            actions.push(MigrationAction::CreateIndex { sql });
        }

        Ok(MigrationPlan { actions })
    }

    fn plan_internal(
        &self,
        live_tables: &[LiveTable],
        live_names: &std::collections::HashSet<&str>,
        tables: &[&TableDecl],
        collections: &[&CollectionDecl],
        indexes: &[&IndexDecl],
    ) -> Result<MigrationPlan, turso::Error> {
        let mut actions = Vec::new();

        // 1. Tables to create
        for dt in tables {
            let snaked_name = crate::ddl::to_snake_case(&dt.name);
            if !live_names.contains(snaked_name.as_str()) {
                let sql = crate::ddl::table_to_ddl(dt);
                actions.push(MigrationAction::CreateTable { sql });
            }
        }

        // 2. Collections to create
        for dc in collections {
            let snaked_name = crate::ddl::to_snake_case(&dc.name);
            if !live_names.contains(snaked_name.as_str()) {
                let sql = crate::ddl::collection_to_ddl(dc);
                actions.push(MigrationAction::CreateCollection { sql });
            }
        }

        // 3. Columns to add (in desired table but not in live table)
        for dt in tables {
            let snaked_name = crate::ddl::to_snake_case(&dt.name);
            if let Some(lt) = live_tables.iter().find(|t| t.name == snaked_name) {
                let live_cols: std::collections::HashSet<&str> =
                    lt.columns.iter().map(|c| c.name.as_str()).collect();

                for f in &dt.fields {
                    if !live_cols.contains(f.name.as_str()) {
                        actions.push(MigrationAction::AddColumn {
                            table: snaked_name.clone(),
                            column: f.name.clone(),
                            col_type: crate::ddl::type_to_sqlite_type(&f.type_ann).to_string(),
                        });
                    }
                }

                // Columns removed from declaration — flag as manual
                let desired_cols: std::collections::HashSet<&str> =
                    dt.fields.iter().map(|c| c.name.as_str()).collect();
                for lc in &lt.columns {
                    if lc.name != "_id"
                        && lc.name != "_creationTime"
                        && !desired_cols.contains(lc.name.as_str())
                    {
                        actions.push(MigrationAction::ManualDropColumn {
                            table: snaked_name.clone(),
                            column: lc.name.clone(),
                        });
                    }
                }
            }
        }

        // 4. Tables in live but not in desired — flag as manual
        let desired_table_names: std::collections::HashSet<String> = tables
            .iter()
            .map(|t| crate::ddl::to_snake_case(&t.name))
            .collect();
        let desired_coll_names: std::collections::HashSet<String> = collections
            .iter()
            .map(|c| crate::ddl::to_snake_case(&c.name))
            .collect();

        for lt in live_tables.iter() {
            if !desired_table_names.contains(&lt.name) && !desired_coll_names.contains(&lt.name) {
                // Only flag user-defined tables, not internal ones
                if !lt.name.starts_with('_') {
                    actions.push(MigrationAction::ManualDropTable {
                        table: lt.name.clone(),
                    });
                }
            }
        }

        // 5. Build indexes
        // For simplicity right now, if the table/collection exists, we create the index.
        // IF NOT EXISTS will prevent errors if it's already there.
        // We could introspect sqlite_master for indexes to be perfect.
        for idx in indexes {
            let table_exists =
                desired_table_names.contains(&crate::ddl::to_snake_case(&idx.table_name));
            let sql = if table_exists {
                crate::ddl::index_to_ddl(idx)
            } else {
                crate::ddl::collection_index_to_ddl(idx)
            };
            actions.push(MigrationAction::CreateIndex { sql });
        }

        Ok(MigrationPlan { actions })
    }

    /// Apply all safe migration actions. Returns the number of actions applied.
    pub async fn apply(&self, plan: &MigrationPlan) -> Result<usize, turso::Error> {
        let mut applied = 0;
        for action in plan.auto_actions() {
            if let Some(sql) = action.to_sql() {
                self.conn.execute(&sql, ()).await?;
                applied += 1;
            }
        }
        Ok(applied)
    }

    /// Plan and apply auto-safe migrations in one call.
    pub async fn sync_schema(
        &self,
        tables: &[&TableDecl],
        collections: &[&CollectionDecl],
        indexes: &[&IndexDecl],
    ) -> Result<MigrationPlan, turso::Error> {
        let plan = self.plan(tables, collections, indexes).await?;
        self.apply(&plan).await?;
        Ok(plan)
    }

    pub async fn sync_from_digest(
        &self,
        digest: &SchemaDigest,
    ) -> Result<MigrationPlan, turso::Error> {
        let plan = self.plan_from_digest(digest).await?;
        self.apply(&plan).await?;
        Ok(plan)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_action_describe() {
        let action = MigrationAction::AddColumn {
            table: "users".into(),
            column: "email".into(),
            col_type: "TEXT".into(),
        };
        assert_eq!(action.describe(), "add column `email` (TEXT) to `users`");
        assert!(action.is_auto_safe());
    }

    #[test]
    fn test_migration_action_sql() {
        let action = MigrationAction::AddColumn {
            table: "users".into(),
            column: "email".into(),
            col_type: "TEXT".into(),
        };
        assert_eq!(
            action.to_sql().unwrap(),
            "ALTER TABLE users ADD COLUMN email TEXT"
        );
    }

    #[test]
    fn test_manual_action_not_auto_safe() {
        let action = MigrationAction::ManualDropColumn {
            table: "users".into(),
            column: "old_field".into(),
        };
        assert!(!action.is_auto_safe());
        assert!(action.to_sql().is_none());
    }

    #[test]
    fn test_empty_plan() {
        let plan = MigrationPlan { actions: vec![] };
        assert!(plan.is_empty());
        assert_eq!(plan.describe(), "Schema is up to date.");
    }
}
