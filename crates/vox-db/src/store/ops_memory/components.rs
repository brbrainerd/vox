use crate::store::types::StoreError;
use turso::params;

impl crate::VoxDb {
    /// Upsert a row in `components`.
    ///
    /// Called from `vox-db/src/lib.rs` `VoxDb::register_local_project`.
    pub async fn register_component(
        &self,
        name: &str,
        namespace: &str,
        schema_hash: Option<&str>,
        description: Option<&str>,
        version: &str,
    ) -> Result<(), StoreError> {
        let name = name.to_string();
        let namespace = namespace.to_string();
        let schema_hash = schema_hash.map(str::to_string);
        let description = description.map(str::to_string);
        let version = version.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO components (name, namespace, schema_hash, version, description)
                     VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT(name)
                     DO UPDATE SET namespace   = excluded.namespace,
                                   schema_hash = COALESCE(excluded.schema_hash, components.schema_hash),
                                   version     = excluded.version,
                                   description = COALESCE(excluded.description, components.description)",
                    params![
                        name.as_str(),
                        namespace.as_str(),
                        schema_hash.as_deref(),
                        version.as_str(),
                        description.as_deref(),
                    ],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }
}
