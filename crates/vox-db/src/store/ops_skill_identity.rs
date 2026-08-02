//! First-come-first-served skill namespace/identity claims (Task 3.4,
//! harness parity plan). See `skill_identities` table docs in
//! `schema/domains/agents.rs` and `vox_plugin_types::skill_identity` for the
//! namespace format this operates on.

use turso::params;

use crate::store::types::StoreError;

/// Build the friendly "already claimed" conflict error shared by the
/// non-racy read-then-write path and the PK-violation recovery path.
fn conflict_error(identity: &str, existing_owner: &str, requested_owner: &str) -> StoreError {
    StoreError::Db(format!(
        "skill identity '{identity}' is already claimed by owner '{existing_owner}' \
         (requested by '{requested_owner}')"
    ))
}

/// Best-effort classification of a `turso::Error` as a PRIMARY KEY / UNIQUE
/// constraint violation. `turso` doesn't expose a typed SQLite error-code
/// variant we can match on here, so this falls back to a substring check on
/// the stringified error (SQLite's own wording: "UNIQUE constraint failed").
/// A false negative (an unrecognized wording) just means the caller
/// surfaces the raw `turso::Error` instead of the friendly conflict
/// message — degrades to the pre-existing behavior, not a panic.
fn is_unique_violation(err: &turso::Error) -> bool {
    let msg = err.to_string().to_ascii_lowercase();
    msg.contains("unique constraint") || msg.contains("primary key constraint")
}

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
    /// callers) — acceptable for a single-writer local Codex DB. The
    /// `identity` column's PRIMARY KEY constraint is still the real
    /// uniqueness guarantee: if a concurrent writer wins the race between
    /// our `SELECT` and `INSERT`, the `INSERT` fails with a PK violation,
    /// which is caught below and turned into the same friendly conflict
    /// error rather than surfacing a raw `turso`/SQLite error to the
    /// caller.
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
                        return Err(conflict_error(&identity, &existing_owner, &owner));
                    }
                    return Ok(());
                }
                if let Err(e) = conn
                    .execute(
                        "INSERT INTO skill_identities (identity, owner) VALUES (?1, ?2)",
                        params![identity.as_str(), owner.as_str()],
                    )
                    .await
                {
                    if !is_unique_violation(&e) {
                        return Err(StoreError::Turso(e));
                    }
                    // Lost the race: someone else claimed `identity` between
                    // our SELECT and this INSERT. Re-read to report *who*
                    // holds it now, same shape as the non-racy path above.
                    let mut rows = conn
                        .query(
                            "SELECT owner FROM skill_identities WHERE identity = ?1 LIMIT 1",
                            params![identity.as_str()],
                        )
                        .await?;
                    return match rows.next().await? {
                        Some(row) => {
                            let existing_owner: String =
                                row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
                            if existing_owner == owner {
                                Ok(())
                            } else {
                                Err(conflict_error(&identity, &existing_owner, &owner))
                            }
                        }
                        // Row vanished between the failed INSERT and this
                        // re-read (e.g. concurrent delete) — surface the raw
                        // constraint failure rather than guessing an owner.
                        None => Err(StoreError::Turso(e)),
                    };
                }
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

    /// Simulates the genuine race the PK-violation fallback exists for: two
    /// callers both see `identity` as unclaimed (their `SELECT`s race ahead
    /// of either `INSERT`), so one of the two `INSERT`s hits the PRIMARY KEY
    /// constraint. That must surface as the same friendly "already claimed"
    /// conflict, not a raw `turso`/SQLite error.
    #[tokio::test]
    async fn concurrent_claim_race_degrades_to_friendly_conflict() {
        let db = std::sync::Arc::new(test_db().await);
        let db_a = db.clone();
        let db_b = db.clone();
        let (a, b) = tokio::join!(
            db_a.claim_skill_identity("io.github.alice/racey", "alice"),
            db_b.claim_skill_identity("io.github.alice/racey", "bob"),
        );

        // Exactly one of the two distinct-owner claims must win; the loser
        // must be the friendly conflict error, never a raw StoreError::Turso.
        let results = [a, b];
        let oks = results.iter().filter(|r| r.is_ok()).count();
        assert_eq!(oks, 1, "exactly one racing claim should win: {results:?}");
        for r in &results {
            if let Err(e) = r {
                let msg = e.to_string();
                assert!(
                    msg.contains("already claimed"),
                    "race loser must get the friendly conflict message, got: {msg}"
                );
            }
        }
    }
}
