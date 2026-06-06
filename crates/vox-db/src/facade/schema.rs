use crate::{
    AutoMigrator, StoreError, auto_migrate, collection, paths, schema_digest::SchemaDigest,
};

impl crate::VoxDb {
    /// Apply a [`SchemaDigest`]-driven plan: create missing tables/columns/indexes, never drop.
    pub async fn sync_schema_from_digest(&self, digest: &SchemaDigest) -> Result<(), StoreError> {
        let migrator = AutoMigrator::new(&self.conn);
        migrator.sync_from_digest(digest).await?;
        Ok(())
    }

    /// Return the platform-specific data directory (if resolvable).
    pub fn data_dir() -> Option<std::path::PathBuf> {
        paths::data_dir()
    }

    // ── Collection & Schema Methods ─────────────────────

    /// Get a handle to a schemaless document collection.
    ///
    /// The collection stores JSON documents in a SQLite table with `json_extract`
    /// based querying. Call `ensure_table()` on the returned handle to create the
    /// backing table if it doesn't exist.
    pub fn collection(&self, name: impl Into<String>) -> collection::Collection {
        collection::Collection::new(name, self.conn.clone(), self.breaker.clone())
    }

    /// Create an auto-migrator for schema synchronization.
    ///
    /// Use this to introspect the live database schema and diff it against your
    /// desired `@table` declarations, then apply non-destructive migrations.
    pub fn auto_migrator(&self) -> auto_migrate::AutoMigrator<'_> {
        auto_migrate::AutoMigrator::new(&self.conn)
    }

    /// Automatically sync the database schema derived from AST declarations.
    pub async fn sync_schema_ast(
        &self,
        tables: &[&vox_ast::decl::TableDecl],
        collections: &[&vox_ast::decl::CollectionDecl],
        indexes: &[&vox_ast::decl::IndexDecl],
    ) -> Result<auto_migrate::MigrationPlan, StoreError> {
        let plan = self
            .auto_migrator()
            .sync_schema(tables, collections, indexes)
            .await?;
        Ok(plan)
    }
}
