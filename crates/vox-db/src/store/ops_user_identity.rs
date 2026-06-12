//! Store ops for `user_identities` and `scientia_nanopubs` (design §4.1).
//!
//! `user_identities` binds a `user_id` to its nanopublication signing material:
//! an ORCID, the base64 PKCS#8 public key (nanopub crate format), and a
//! `SecretId` canonical env reference that holds the private key. `scientia_nanopubs`
//! is the local/staged ledger of signed claim artifacts, one row per Trusty URI.

use crate::VoxDb;
use crate::store::types::StoreError;
use serde::{Deserialize, Serialize};
use turso::params;

/// One row of `user_identities`. Field order matches the DDL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserIdentityRow {
    pub user_id: String,
    pub orcid_id: Option<String>,
    /// base64 PKCS#8 public key (nanopub crate format).
    pub nanopub_pubkey_b64: Option<String>,
    /// `SecretId` canonical env that holds the private key.
    pub nanopub_key_ref: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

/// One row of `scientia_nanopubs`. Field order matches the DDL (the
/// autoincrement `id` is not surfaced; rows are keyed by `trusty_uri`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NanopubRow {
    pub trusty_uri: String,
    pub claim_id: i64,
    pub publication_id: Option<String>,
    pub user_id: String,
    pub orcid_id: Option<String>,
    pub trig: String,
    /// `true` once the reference validator passes.
    pub validated_offline: bool,
    /// `local|test_server|published` (only `local` used in this phase).
    pub published_state: String,
    pub created_at_ms: i64,
}

impl VoxDb {
    /// Insert or replace the identity binding for `row.user_id` (PK upsert).
    ///
    /// `nanopub_key_ref` must be non-empty: it names the `SecretId` that holds
    /// the signing key, so an empty ref would silently orphan the identity from
    /// its key material. Enforced in Rust because Turso/libSQL does not support
    /// SQL `CHECK` constraints.
    pub async fn upsert_user_identity(&self, row: &UserIdentityRow) -> Result<(), StoreError> {
        if row.nanopub_key_ref.trim().is_empty() {
            return Err(StoreError::Db(
                "user_identities.nanopub_key_ref must be a non-empty SecretId reference"
                    .to_string(),
            ));
        }
        self.conn
            .execute(
                "INSERT INTO user_identities(\
                    user_id, orcid_id, nanopub_pubkey_b64, nanopub_key_ref, \
                    created_at_ms, updated_at_ms\
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
                 ON CONFLICT(user_id) DO UPDATE SET \
                    orcid_id = excluded.orcid_id, \
                    nanopub_pubkey_b64 = excluded.nanopub_pubkey_b64, \
                    nanopub_key_ref = excluded.nanopub_key_ref, \
                    updated_at_ms = excluded.updated_at_ms",
                params![
                    row.user_id.clone(),
                    row.orcid_id.clone(),
                    row.nanopub_pubkey_b64.clone(),
                    row.nanopub_key_ref.clone(),
                    row.created_at_ms,
                    row.updated_at_ms,
                ],
            )
            .await
            .map_err(StoreError::Turso)?;
        Ok(())
    }

    /// Fetch the identity binding for `user_id`, if present.
    pub async fn get_user_identity(
        &self,
        user_id: &str,
    ) -> Result<Option<UserIdentityRow>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT user_id, orcid_id, nanopub_pubkey_b64, nanopub_key_ref, \
                        created_at_ms, updated_at_ms \
                 FROM user_identities WHERE user_id = ?1",
                params![user_id.to_string()],
            )
            .await
            .map_err(StoreError::Turso)?;
        if let Some(row) = rows.next().await.map_err(StoreError::Turso)? {
            Ok(Some(UserIdentityRow {
                user_id: row.get(0).map_err(StoreError::Turso)?,
                orcid_id: row.get(1).map_err(StoreError::Turso)?,
                nanopub_pubkey_b64: row.get(2).map_err(StoreError::Turso)?,
                nanopub_key_ref: row.get(3).map_err(StoreError::Turso)?,
                created_at_ms: row.get(4).map_err(StoreError::Turso)?,
                updated_at_ms: row.get(5).map_err(StoreError::Turso)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Insert a signed nanopublication artifact. `trusty_uri` is UNIQUE.
    pub async fn insert_scientia_nanopub(&self, row: &NanopubRow) -> Result<(), StoreError> {
        self.conn
            .execute(
                "INSERT INTO scientia_nanopubs(\
                    trusty_uri, claim_id, publication_id, user_id, orcid_id, \
                    trig, validated_offline, published_state, created_at_ms\
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    row.trusty_uri.clone(),
                    row.claim_id,
                    row.publication_id.clone(),
                    row.user_id.clone(),
                    row.orcid_id.clone(),
                    row.trig.clone(),
                    i64::from(row.validated_offline),
                    row.published_state.clone(),
                    row.created_at_ms,
                ],
            )
            .await
            .map_err(StoreError::Turso)?;
        Ok(())
    }

    /// Fetch a nanopublication artifact by its Trusty URI, if present.
    pub async fn get_nanopub_by_trusty_uri(
        &self,
        trusty_uri: &str,
    ) -> Result<Option<NanopubRow>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT trusty_uri, claim_id, publication_id, user_id, orcid_id, \
                        trig, validated_offline, published_state, created_at_ms \
                 FROM scientia_nanopubs WHERE trusty_uri = ?1",
                params![trusty_uri.to_string()],
            )
            .await
            .map_err(StoreError::Turso)?;
        if let Some(row) = rows.next().await.map_err(StoreError::Turso)? {
            let validated_offline: i64 = row.get(6).map_err(StoreError::Turso)?;
            Ok(Some(NanopubRow {
                trusty_uri: row.get(0).map_err(StoreError::Turso)?,
                claim_id: row.get(1).map_err(StoreError::Turso)?,
                publication_id: row.get(2).map_err(StoreError::Turso)?,
                user_id: row.get(3).map_err(StoreError::Turso)?,
                orcid_id: row.get(4).map_err(StoreError::Turso)?,
                trig: row.get(5).map_err(StoreError::Turso)?,
                validated_offline: validated_offline != 0,
                published_state: row.get(7).map_err(StoreError::Turso)?,
                created_at_ms: row.get(8).map_err(StoreError::Turso)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Update the `published_state` (and optionally `published_uri`) for a
    /// persisted nanopublication artifact, keyed by its Trusty URI.
    ///
    /// The `published_state` column encodes the lifecycle: `local` →
    /// `test_server` → (future: `published`). The Trusty URI doubles as the
    /// canonical published identifier for test-server nanopubs, so
    /// `published_uri` is stored in the existing `trusty_uri` column (no schema
    /// change needed). Pass the returned URI from the test-server publish call
    /// as `published_uri`; if the server returns a different URI, this op
    /// stores it for traceability.
    pub async fn update_nanopub_published_state(
        &self,
        trusty_uri: &str,
        state: &str,
        published_uri: &str,
    ) -> Result<(), StoreError> {
        // Store the published_uri back into `trusty_uri` only when it differs
        // (the test server always echoes the same Trusty URI, but we handle the
        // general case). In practice this is a state-column update; the PK
        // (`trusty_uri`) is unchanged unless the server assigned a new one.
        self.conn
            .execute(
                "UPDATE scientia_nanopubs \
                 SET published_state = ?1 \
                 WHERE trusty_uri = ?2",
                params![state.to_string(), trusty_uri.to_string()],
            )
            .await
            .map_err(StoreError::Turso)?;
        // When the server returned a distinct URI (edge case), insert a
        // lightweight cross-reference row so the caller can look up by the
        // server-assigned URI. If identical we skip to avoid a duplicate-PK
        // error on the UNIQUE constraint.
        if published_uri != trusty_uri {
            // Best-effort: if a row for `published_uri` already exists (e.g.
            // idempotent retry), ignore the conflict.
            self.conn
                .execute(
                    "INSERT OR IGNORE INTO scientia_nanopubs(\
                        trusty_uri, claim_id, publication_id, user_id, orcid_id, \
                        trig, validated_offline, published_state, created_at_ms\
                     ) \
                     SELECT ?1, claim_id, publication_id, user_id, orcid_id, \
                            trig, validated_offline, ?2, created_at_ms \
                     FROM scientia_nanopubs WHERE trusty_uri = ?3",
                    params![
                        published_uri.to_string(),
                        state.to_string(),
                        trusty_uri.to_string()
                    ],
                )
                .await
                .map_err(StoreError::Turso)?;
        }
        Ok(())
    }

    /// Fetch the most-recently-created `scientia_nanopubs` row for a claim
    /// that has `published_state = 'local'`.
    ///
    /// Returns `None` when no local row exists. When multiple local builds exist
    /// for the same claim (e.g. re-builds), returns the one with the highest
    /// `created_at_ms`.
    pub async fn get_local_nanopub_for_claim(
        &self,
        claim_id: i64,
    ) -> Result<Option<NanopubRow>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT trusty_uri, claim_id, publication_id, user_id, orcid_id, \
                        trig, validated_offline, published_state, created_at_ms \
                 FROM scientia_nanopubs \
                 WHERE claim_id = ?1 AND published_state = 'local' \
                 ORDER BY created_at_ms DESC \
                 LIMIT 1",
                params![claim_id],
            )
            .await
            .map_err(StoreError::Turso)?;
        if let Some(row) = rows.next().await.map_err(StoreError::Turso)? {
            let validated_offline: i64 = row.get(6).map_err(StoreError::Turso)?;
            Ok(Some(NanopubRow {
                trusty_uri: row.get(0).map_err(StoreError::Turso)?,
                claim_id: row.get(1).map_err(StoreError::Turso)?,
                publication_id: row.get(2).map_err(StoreError::Turso)?,
                user_id: row.get(3).map_err(StoreError::Turso)?,
                orcid_id: row.get(4).map_err(StoreError::Turso)?,
                trig: row.get(5).map_err(StoreError::Turso)?,
                validated_offline: validated_offline != 0,
                published_state: row.get(7).map_err(StoreError::Turso)?,
                created_at_ms: row.get(8).map_err(StoreError::Turso)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Count the locally-persisted nanopublication artifacts for a claim.
    ///
    /// Used to prove the human-gate refused emission (a refused stale/unapproved
    /// build must persist nothing → count stays 0). A typed op rather than a raw
    /// `query_all` so callers stay within the Codex query surface.
    pub async fn count_scientia_nanopubs_for_claim(
        &self,
        claim_id: i64,
    ) -> Result<i64, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT COUNT(*) FROM scientia_nanopubs WHERE claim_id = ?1",
                params![claim_id],
            )
            .await
            .map_err(StoreError::Turso)?;
        let row = rows
            .next()
            .await
            .map_err(StoreError::Turso)?
            .ok_or_else(|| StoreError::Db("COUNT(*) returned no row".to_string()))?;
        row.get(0).map_err(StoreError::Turso)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DbConfig, VoxDb};

    #[tokio::test]
    async fn upsert_then_get_user_identity_round_trips() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        let row = UserIdentityRow {
            user_id: "local-user".into(),
            orcid_id: Some("https://orcid.org/0000-0002-1825-0097".into()),
            nanopub_pubkey_b64: Some("MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8_fake_b64".into()),
            nanopub_key_ref: "VOX_USER_RSA_NANOPUB_PRIVATE_KEY_B64".into(),
            created_at_ms: 1_747_000_000_000,
            updated_at_ms: 1_747_000_000_000,
        };
        db.upsert_user_identity(&row).await.expect("upsert");

        let got = db
            .get_user_identity("local-user")
            .await
            .expect("get")
            .expect("row present");
        assert_eq!(got, row);
    }

    #[tokio::test]
    async fn upsert_user_identity_rejects_empty_key_ref() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        let row = UserIdentityRow {
            user_id: "local-user".into(),
            orcid_id: None,
            nanopub_pubkey_b64: None,
            nanopub_key_ref: "   ".into(),
            created_at_ms: 1,
            updated_at_ms: 1,
        };
        let err = db
            .upsert_user_identity(&row)
            .await
            .expect_err("empty nanopub_key_ref must error");
        assert!(
            err.to_string().contains("nanopub_key_ref"),
            "error should mention nanopub_key_ref, got: {err}"
        );
        // And nothing was persisted.
        let got = db.get_user_identity("local-user").await.expect("get");
        assert!(got.is_none(), "rejected upsert must not persist a row");
    }

    #[tokio::test]
    async fn count_scientia_nanopubs_for_claim_counts_persisted_rows() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");
        // Empty case.
        assert_eq!(
            db.count_scientia_nanopubs_for_claim(7)
                .await
                .expect("count"),
            0,
            "no rows → 0"
        );
        // Insert two rows for claim 7 and one for claim 8.
        let mk = |trusty: &str, claim_id: i64| NanopubRow {
            trusty_uri: trusty.into(),
            claim_id,
            publication_id: Some("pub-x".into()),
            user_id: "u".into(),
            orcid_id: None,
            trig: "@prefix : <x> .".into(),
            validated_offline: true,
            published_state: "local".into(),
            created_at_ms: 1,
        };
        db.insert_scientia_nanopub(&mk("RA_a", 7))
            .await
            .expect("insert a");
        db.insert_scientia_nanopub(&mk("RA_b", 7))
            .await
            .expect("insert b");
        db.insert_scientia_nanopub(&mk("RA_c", 8))
            .await
            .expect("insert c");
        assert_eq!(
            db.count_scientia_nanopubs_for_claim(7)
                .await
                .expect("count"),
            2,
            "two rows for claim 7"
        );
        assert_eq!(
            db.count_scientia_nanopubs_for_claim(8)
                .await
                .expect("count"),
            1,
            "one row for claim 8"
        );
    }
}
