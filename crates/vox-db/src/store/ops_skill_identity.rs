//! First-come-first-served skill namespace/identity claims (Task 3.4,
//! harness parity plan). See `skill_identities` table docs in
//! `schema/domains/agents.rs` and `vox_plugin_types::skill_identity` for the
//! namespace format this operates on.

use turso::params;

use crate::store::types::StoreError;

impl crate::VoxDb {
    /// Claim `identity` for `owner`, first-come-first-served.
    ///
    /// * No existing row for `identity` — claims it for `owner`, returns `Ok(())`.
    /// * Existing row with the same `owner` — idempotent no-op (e.g. re-publishing
    ///   a new version of the same skill), returns `Ok(())`.
    /// * Existing row with a *different* `owner` — anti-squatting conflict,
    ///   returns `Err(StoreError::Db(..))` describing the conflict. Callers
    ///   (e.g. `SkillRegistry::install_bundle`) must surface this as a real
    ///   rejection, not silently overwrite.
    ///
    /// Not transactionally atomic against a concurrent claim of the same
    /// identity between the read and the write (same read-then-write
    /// pattern used elsewhere in this module, e.g. `get_skill_reliability`
    /// callers) — acceptable for a single-writer local Codex DB; a
    /// multi-writer deployment would need `INSERT ... ON CONFLICT DO
    /// NOTHING` plus a follow-up read, which turso's SQLite dialect
    /// supports if this becomes a real race in practice.
    pub async fn claim_skill_identity(
        &self,
        identity: &str,
        owner: &str,
    ) -> Result<(), StoreError> {
        let identity = identity.to_string();
        let owner = owner.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT owner FROM skill_identities WHERE identity = ?1 LIMIT 1",
                        params![identity.as_str()],
                    )
                    .await?;
                if let Some(row) = rows.next().await? {
                    let existing_owner: String =
                        row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
                    if existing_owner != owner {
                        return Err(StoreError::Db(format!(
                            "skill identity '{identity}' is already claimed by owner '{existing_owner}' \
                             (requested by '{owner}')"
                        )));
                    }
                    return Ok(());
                }
                conn.execute(
                    "INSERT INTO skill_identities (identity, owner) VALUES (?1, ?2)",
                    params![identity.as_str(), owner.as_str()],
                )
                .await?;
                Ok::<(), StoreError>(())
            })
            .await
    }

    /// Read the current owner of `identity`, or `None` if unclaimed.
    pub async fn get_skill_identity_owner(
        &self,
        identity: &str,
    ) -> Result<Option<String>, StoreError> {
        let identity = identity.to_string();
        let breaker = self.breaker.clone();
        let conn = self.conn.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT owner FROM skill_identities WHERE identity = ?1 LIMIT 1",
                        params![identity.as_str()],
                    )
                    .await?;
                match rows.next().await? {
                    Some(row) => {
                        let owner: String =
                            row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
                        Ok(Some(owner))
                    }
                    None => Ok(None),
                }
            })
            .await
    }
}

#[cfg(test)]
mod tests {
    use crate::{DbConfig, VoxDb};

    async fn test_db() -> VoxDb {
        VoxDb::connect(DbConfig::Memory)
            .await
            .expect("open in-memory db")
    }

    #[tokio::test]
    async fn first_claim_succeeds() {
        let db = test_db().await;
        db.claim_skill_identity("local/my-skill", "local")
            .await
            .expect("first claim should succeed");
        assert_eq!(
            db.get_skill_identity_owner("local/my-skill").await.unwrap(),
            Some("local".to_string())
        );
    }

    #[tokio::test]
    async fn same_owner_reclaim_is_idempotent() {
        let db = test_db().await;
        db.claim_skill_identity("io.github.alice/foo", "alice")
            .await
            .unwrap();
        db.claim_skill_identity("io.github.alice/foo", "alice")
            .await
            .expect("re-claim by same owner must not error");
    }

    #[tokio::test]
    async fn distinct_owner_reclaim_is_rejected() {
        let db = test_db().await;
        db.claim_skill_identity("io.github.alice/foo", "alice")
            .await
            .unwrap();
        let err = db
            .claim_skill_identity("io.github.alice/foo", "mallory")
            .await
            .expect_err("squat attempt must be rejected");
        let msg = err.to_string();
        assert!(msg.contains("already claimed"), "unexpected message: {msg}");
    }
}
