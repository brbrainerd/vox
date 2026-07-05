use crate::{VoxDb, store::StoreError, store::types::NodeIdentityRow};
use turso::params;

impl VoxDb {
    /// Upsert a node identity in the foundation schema.
    pub async fn upsert_node_identity(&self, identity: &NodeIdentityRow) -> Result<(), StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        let node_id = identity.node_id.clone();
        let pubkey_hex = identity.pubkey_hex.clone();
        let label = identity.label.clone();
        let account_id = identity.account_id.clone();

        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO node_identities (node_id, pubkey_hex, label, account_id)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(node_id) DO UPDATE SET
                    pubkey_hex = excluded.pubkey_hex,
                    label = excluded.label,
                    account_id = excluded.account_id,
                    last_seen_at = datetime('now')",
                    params![node_id, pubkey_hex, label, account_id],
                )
                .await?;
                Ok(())
            })
            .await
    }

    /// List all trusted nodes from the foundation schema (node_trust_grants joining node_identities).
    pub async fn list_trusted_nodes(
        &self,
        granting_node_id: &str,
    ) -> Result<Vec<NodeIdentityRow>, StoreError> {
        let granting_node_id = granting_node_id.to_string();
        let rows = self.query_all(
            "SELECT i.node_id, i.pubkey_hex, i.label, i.account_id, i.created_at, i.last_seen_at
             FROM node_trust_grants g
             JOIN node_identities i ON g.trusted_node_id = i.node_id
             WHERE g.granting_node_id = ?1",
            (granting_node_id,),
        ).await?;

        rows.into_iter()
            .map(|r| {
                Ok(NodeIdentityRow {
                    node_id: r.get(0).map_err(|e| StoreError::Db(e.to_string()))?,
                    pubkey_hex: r.get(1).map_err(|e| StoreError::Db(e.to_string()))?,
                    label: r.get(2).map_err(|e| StoreError::Db(e.to_string()))?,
                    account_id: r.get(3).map_err(|e| StoreError::Db(e.to_string()))?,
                    created_at: r.get(4).map_err(|e| StoreError::Db(e.to_string()))?,
                    last_seen_at: r.get(5).map_err(|e| StoreError::Db(e.to_string()))?,
                })
            })
            .collect()
    }

    /// Add a trust grant.
    pub async fn add_trust_grant(
        &self,
        granting_node_id: &str,
        trusted_node_id: &str,
    ) -> Result<(), StoreError> {
        let granting_node_id = granting_node_id.to_string();
        let trusted_node_id = trusted_node_id.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();

        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT OR IGNORE INTO node_trust_grants (granting_node_id, trusted_node_id)
                 VALUES (?1, ?2)",
                    params![granting_node_id, trusted_node_id],
                )
                .await?;
                Ok(())
            })
            .await
    }

    /// Check if a node is trusted by a given grantor.
    pub async fn is_node_trusted(
        &self,
        granting_node_id: &str,
        trusted_node_id: &str,
    ) -> Result<bool, StoreError> {
        let mut cursor = self.conn.query(
            "SELECT 1 FROM node_trust_grants WHERE granting_node_id = ?1 AND trusted_node_id = ?2",
            params![granting_node_id, trusted_node_id],
        ).await.map_err(|e| StoreError::Db(e.to_string()))?;

        Ok(cursor
            .next()
            .await
            .map_err(|e| StoreError::Db(e.to_string()))?
            .is_some())
    }

    /// Remove a trust grant.
    pub async fn remove_trust_grant(
        &self,
        granting_node_id: &str,
        trusted_node_id: &str,
    ) -> Result<(), StoreError> {
        let granting_node_id = granting_node_id.to_string();
        let trusted_node_id = trusted_node_id.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();

        breaker.call(|| async move {
            conn.execute(
                "DELETE FROM node_trust_grants WHERE granting_node_id = ?1 AND trusted_node_id = ?2",
                params![granting_node_id, trusted_node_id],
            ).await?;
            Ok(())
        }).await
    }

    /// Record an anomalous event for a node.
    pub async fn record_anomalous_event(
        &self,
        node_id: &str,
        event_type: &str,
        severity: i64,
        payload_json: &str,
    ) -> Result<(), StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        let node_id = node_id.to_string();
        let event_type = event_type.to_string();
        let payload_json = payload_json.to_string();

        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO anomalous_events (node_id, event_type, severity, payload_json)
                 VALUES (?1, ?2, ?3, ?4)",
                    params![node_id, event_type, severity, payload_json],
                )
                .await?;
                Ok(())
            })
            .await
    }

    /// Upsert a linked external-provider identity (e.g. GitHub) for a Vox user in the
    /// LUDUS identity-federation schema (`vox_identities`, schema V19).
    ///
    /// `provider_login` is a mutable display name (shown to the user only); `provider_id`
    /// is expected to be the provider's stable identifier and is used for lookups.
    /// `access_token_ref` is a reference into `vox-secrets`, never the raw token.
    pub async fn upsert_vox_identity(
        &self,
        vox_user_id: &str,
        provider: &str,
        provider_id: &str,
        provider_login: Option<&str>,
        access_token_ref: Option<&str>,
    ) -> Result<(), StoreError> {
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        let vox_user_id = vox_user_id.to_string();
        let provider = provider.to_string();
        let provider_id = provider_id.to_string();
        let provider_login = provider_login.map(|s| s.to_string());
        let access_token_ref = access_token_ref.map(|s| s.to_string());

        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO vox_identities
                        (vox_user_id, provider, provider_id, provider_login, access_token_ref, linked_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, strftime('%s', 'now'))
                     ON CONFLICT(vox_user_id, provider) DO UPDATE SET
                        provider_id = excluded.provider_id,
                        provider_login = excluded.provider_login,
                        access_token_ref = excluded.access_token_ref,
                        linked_at = excluded.linked_at",
                    params![
                        vox_user_id,
                        provider,
                        provider_id,
                        provider_login,
                        access_token_ref
                    ],
                )
                .await?;
                Ok(())
            })
            .await
    }

    /// List all linked external-provider identities for a Vox user.
    ///
    /// Returns `(provider, provider_id, provider_login)` tuples.
    pub async fn get_vox_identities(
        &self,
        vox_user_id: &str,
    ) -> Result<Vec<(String, String, Option<String>)>, StoreError> {
        let vox_user_id = vox_user_id.to_string();
        let rows = self
            .query_all(
                "SELECT provider, provider_id, provider_login
                 FROM vox_identities
                 WHERE vox_user_id = ?1",
                (vox_user_id,),
            )
            .await?;

        rows.into_iter()
            .map(|r| {
                Ok((
                    r.get(0).map_err(|e| StoreError::Db(e.to_string()))?,
                    r.get(1).map_err(|e| StoreError::Db(e.to_string()))?,
                    r.get(2).map_err(|e| StoreError::Db(e.to_string()))?,
                ))
            })
            .collect()
    }

    /// Calculate trust decay and remove grants for nodes that exceed the severity threshold in the last 24 hours.
    pub async fn process_reputation_decay(
        &self,
        grantor_id: &str,
        severity_threshold: i64,
    ) -> Result<usize, StoreError> {
        let sql = "
            WITH BadNodes AS (
                SELECT node_id
                FROM anomalous_events
                WHERE recorded_at >= datetime('now', '-1 day')
                GROUP BY node_id
                HAVING SUM(severity) >= ?2
            )
            DELETE FROM node_trust_grants
            WHERE granting_node_id = ?1
              AND trusted_node_id IN (SELECT node_id FROM BadNodes)
        ";
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        let grantor_id = grantor_id.to_string();

        breaker
            .call(|| async move {
                let affected = conn
                    .execute(sql, params![grantor_id, severity_threshold])
                    .await?;
                Ok(affected as usize)
            })
            .await
    }
}
