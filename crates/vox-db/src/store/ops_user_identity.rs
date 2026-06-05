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
    pub async fn upsert_user_identity(&self, row: &UserIdentityRow) -> Result<(), StoreError> {
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
}
