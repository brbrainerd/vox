//! Cloudless secrets vault (encrypted secret rows in SQLite / libSQL).
//!
//! **Connection env (precedence):**
//! 1. `VOX_SECRETS_VAULT_PATH` — local store path; opened as a `file:` URL.
//! 2. `VOX_SECRETS_VAULT_URL` — explicit URL (`file:…` or `libsql://…`).
//! 3. When compatibility aliases are allowed (not `VOX_SECRETS_HARD_CUT` and not cutover
//!    `enforce` / `decommission`): `VOX_TURSO_URL` then `TURSO_URL`.
//! 4. Default: `file:.vox/clavis_vault.db`.
//!
//! **Remote token:** `VOX_SECRETS_VAULT_TOKEN`, then compat `VOX_TURSO_TOKEN` / `TURSO_AUTH_TOKEN`
//! when allowed. Codex uses `VOX_DB_URL` / `VOX_DB_TOKEN`; do not conflate with this vault plane.

use std::sync::Mutex;
use std::{future::Future, panic};

use rand::RngCore;
use secrecy::SecretString;
use turso::params;
use vox_crypto::{SymKey, decrypt_with_nonce, encrypt_with_nonce, secure_hash};

use crate::backend::SecretBackend;
use crate::errors::SecretError;
use crate::spec::{SecretId, SecretSpec};

const WRAP_NONCE_LEN: usize = 12;

#[derive(Debug, Clone)]
pub struct CloudlessSecretRecord {
    pub account_id: String,
    pub secret_id: String,
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub cipher_version: i64,
    pub dek_wrapped: Vec<u8>,
    pub kek_ref: String,
    pub kek_version: i64,
    pub aad_hash: Option<String>,
    pub updated_at_ms: i64,
    pub rotation_epoch: i64,
    pub rotated_at_ms: Option<i64>,
    pub consistency_origin: String,
    pub consistency_version: i64,
    pub checksum_hash: String,
}

#[derive(Debug, Clone)]
pub struct ProfileSecretRecord {
    pub account_id: String,
    pub secret_id: String,
    pub profile: String,
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub dek_wrapped: Vec<u8>,
    pub kek_ref: String,
    pub kek_version: i64,
    pub updated_at_ms: i64,
    pub checksum_hash: String,
}

#[derive(Debug, Clone)]
pub struct AgentDelegationRecord {
    pub delegation_id: String,
    pub account_id: String,
    pub secret_id: String,
    pub scope_bits: i64,
    pub parent_context: String,
    pub child_context: String,
    pub issued_at_ms: i64,
    pub expires_at_ms: i64,
}

pub struct VoxCloudBackend {
    conn: Mutex<turso::Connection>,
    master_key: [u8; 32],
    account_id: String,
    kek_ref: String,
    kek_version: i64,
}

impl VoxCloudBackend {
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> Result<Self, SecretError> {
        let conn = run_secrets_future(open_cloudless_connection())?;
        run_secrets_future(ensure_schema(&conn))?;
        let account_id = std::env::var(crate::OPERATOR_ACCOUNT_ID)
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| "default-account".to_string());
        let kek_ref = std::env::var(crate::OPERATOR_SECRETS_KEK_REF)
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| "local-master".to_string());
        let kek_version = std::env::var("VOX_SECRETS_KEK_VERSION")
            .ok()
            .and_then(|v| v.parse::<i64>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(1);
        Ok(Self {
            conn: Mutex::new(conn),
            master_key: derive_master_key()?,
            account_id,
            kek_ref,
            kek_version,
        })
    }

    pub fn write_secret(&self, key: &str, plaintext: &str) -> Result<(), SecretError> {
        self.write_secret_v2(key, plaintext, None, "create", Some("cli-set"), "cli", 10)
    }

    pub fn write_secret_v2(
        &self,
        secret_id: &str,
        plaintext: &str,
        profile: Option<&str>,
        operation: &str,
        source_hint: Option<&str>,
        caller_context: &str,
        history_depth: u32,
    ) -> Result<(), SecretError> {
        let mut dek = [0_u8; 32];
        rand::thread_rng().fill_bytes(&mut dek);
        let mut nonce = [0_u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce);
        let ciphertext = encrypt_vault(&dek, &nonce, plaintext.as_bytes())?;
        let dek_wrapped = self.wrap_dek(&dek, &self.kek_ref, self.kek_version)?;

        dek.fill(0);

        let account_id = self.account_id.clone();
        let kek_ref = self.kek_ref.clone();
        let kek_version = self.kek_version;
        let now = now_ms();

        let checksum = compute_account_secret_checksum(
            &account_id,
            secret_id,
            &ciphertext,
            &nonce,
            1,
            &dek_wrapped,
            &kek_ref,
            kek_version,
            0,
            1,
        );
        let version_checksum = checksum.clone();

        let prof_str = profile.map(|s| s.to_string());
        let sec_id_str = secret_id.to_string();
        let op_str = operation.to_string();
        let ctx_str = caller_context.to_string();
        let hint_str = source_hint.map(|s| s.to_string());

        let conn = self.conn.lock().expect("vox vault mutex");
        run_secrets_future(async {
            let tx = conn
                .unchecked_transaction()
                .await
                .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?;

            if let Some(prof) = prof_str {
                tx.execute(
                    "INSERT INTO clavis_profile_overrides (
                        account_id, secret_id, profile, ciphertext, nonce, dek_wrapped,
                        kek_ref, kek_version, updated_at_ms, checksum_hash
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                     ON CONFLICT(account_id, secret_id, profile) DO UPDATE SET
                        ciphertext = excluded.ciphertext,
                        nonce = excluded.nonce,
                        dek_wrapped = excluded.dek_wrapped,
                        kek_ref = excluded.kek_ref,
                        kek_version = excluded.kek_version,
                        updated_at_ms = excluded.updated_at_ms,
                        checksum_hash = excluded.checksum_hash",
                    turso::params![
                        account_id.clone(),
                        sec_id_str.clone(),
                        prof,
                        ciphertext.clone(),
                        nonce.clone(),
                        dek_wrapped.clone(),
                        kek_ref.clone(),
                        kek_version,
                        now,
                        checksum.clone()
                    ],
                )
                .await
                .map_err(|e: turso::Error| SecretError::BackendQueryFailed(e.to_string()))?;
            } else {
                tx.execute(
                    "INSERT INTO clavis_account_secrets (
                        account_id, secret_id, ciphertext, nonce, cipher_version, dek_wrapped, dek_wrap_alg,
                        kek_ref, kek_version, updated_at_ms, rotation_epoch, rotated_at_ms,
                        consistency_origin, consistency_version, checksum_hash
                     ) VALUES (?1, ?2, ?3, ?4, 1, ?5, 'ChaCha20-Poly1305', ?6, ?7, ?8, 0, NULL, 'canonical', 1, ?9)
                     ON CONFLICT(account_id, secret_id) DO UPDATE SET
                        ciphertext = excluded.ciphertext,
                        nonce = excluded.nonce,
                        dek_wrapped = excluded.dek_wrapped,
                        kek_ref = excluded.kek_ref,
                        kek_version = excluded.kek_version,
                        updated_at_ms = excluded.updated_at_ms,
                        checksum_hash = excluded.checksum_hash",
                    turso::params![account_id.clone(), sec_id_str.clone(), ciphertext.clone(), nonce.clone(), dek_wrapped.clone(), kek_ref.clone(), kek_version, now, checksum.clone()],
                ).await.map_err(|e: turso::Error| SecretError::BackendQueryFailed(e.to_string()))?;
            }

            tx.execute(
                "INSERT INTO clavis_secret_versions (
                    account_id, secret_id, ciphertext, nonce, dek_wrapped, kek_ref, kek_version,
                    operation, source_hint, created_at_ms, created_by, checksum_hash
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                turso::params![
                    account_id.clone(),
                    sec_id_str.clone(),
                    ciphertext,
                    nonce,
                    dek_wrapped,
                    kek_ref.clone(),
                    kek_version,
                    op_str,
                    hint_str,
                    now,
                    ctx_str,
                    version_checksum.clone()
                ],
            )
            .await
            .map_err(|e: turso::Error| SecretError::BackendQueryFailed(e.to_string()))?;

            if history_depth > 0 {
                tx.execute(
                    "DELETE FROM clavis_secret_versions
                     WHERE account_id = ?1 AND secret_id = ?2
                       AND version_id NOT IN (
                           SELECT version_id FROM clavis_secret_versions
                           WHERE account_id = ?1 AND secret_id = ?2
                           ORDER BY version_id DESC
                           LIMIT ?3
                       )",
                    turso::params![account_id.clone(), sec_id_str.clone(), history_depth as i64],
                )
                .await
                .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?;
            }

            tx.commit()
                .await
                .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))
        })
    }
    /// Permanently remove a secret from the Clavis vault for the active
    /// account. Deletes the canonical row, every historical version, and any
    /// profile overrides, then records an audit-log entry. Returns `true` if a
    /// canonical row was present (and therefore deleted), `false` otherwise.
    ///
    /// Security note: this never reads back or exposes the plaintext — it only
    /// issues `DELETE` statements keyed by (account_id, secret_id).
    pub fn delete_secret(&self, key: &str) -> Result<bool, SecretError> {
        let account_id = self.account_id.clone();
        let sec_id = key.to_string();
        let now = now_ms();

        let conn = self.conn.lock().expect("vox vault mutex");
        run_secrets_future(async {
            let tx = conn
                .unchecked_transaction()
                .await
                .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?;

            let deleted = tx
                .execute(
                    "DELETE FROM clavis_account_secrets WHERE account_id = ?1 AND secret_id = ?2",
                    params![account_id.clone(), sec_id.clone()],
                )
                .await
                .map_err(|e: turso::Error| SecretError::BackendQueryFailed(e.to_string()))?;

            tx.execute(
                "DELETE FROM clavis_secret_versions WHERE account_id = ?1 AND secret_id = ?2",
                params![account_id.clone(), sec_id.clone()],
            )
            .await
            .map_err(|e: turso::Error| SecretError::BackendQueryFailed(e.to_string()))?;

            tx.execute(
                "DELETE FROM clavis_profile_overrides WHERE account_id = ?1 AND secret_id = ?2",
                params![account_id.clone(), sec_id.clone()],
            )
            .await
            .map_err(|e: turso::Error| SecretError::BackendQueryFailed(e.to_string()))?;

            tx.execute(
                "INSERT INTO clavis_audit_log (
                    account_id, secret_id, resolved_at_ms, resolution_status,
                    resolution_source, resolve_profile, caller_context, detail
                 ) VALUES (?1, ?2, ?3, 'deleted', NULL, 'default', 'cli', NULL)",
                params![account_id.clone(), sec_id.clone(), now],
            )
            .await
            .map_err(|e: turso::Error| SecretError::BackendQueryFailed(e.to_string()))?;

            tx.commit()
                .await
                .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?;

            Ok(deleted > 0)
        })
    }

    pub fn rewrap_secret(
        &self,
        secret_id: &str,
        new_kek_ref: &str,
        new_kek_version: i64,
    ) -> Result<bool, SecretError> {
        self.rewrap_secret_for_account(&self.account_id, secret_id, new_kek_ref, new_kek_version)
    }

    pub fn rewrap_secret_for_account(
        &self,
        account_id: &str,
        secret_id: &str,
        new_kek_ref: &str,
        new_kek_version: i64,
    ) -> Result<bool, SecretError> {
        let Some(existing) = self.get_row(account_id, secret_id)? else {
            return Ok(false);
        };
        if !verify_record_checksum(&existing) {
            return Err(SecretError::BackendQueryFailed(format!(
                "checksum mismatch for account_id={account_id} secret_id={secret_id}"
            )));
        }
        let dek = self.unwrap_dek(
            &existing.dek_wrapped,
            &existing.kek_ref,
            existing.kek_version,
        )?;
        let new_wrapped = self.wrap_dek(&dek, new_kek_ref, new_kek_version)?;
        let checksum = compute_account_secret_checksum(
            &existing.account_id,
            &existing.secret_id,
            &existing.ciphertext,
            &existing.nonce,
            existing.cipher_version,
            &new_wrapped,
            new_kek_ref,
            new_kek_version,
            existing.rotation_epoch + 1,
            existing.consistency_version,
        );
        let conn = self.conn.lock().expect("vox vault mutex");
        run_secrets_future(async {
            conn.execute(
                "UPDATE clavis_account_secrets
                 SET dek_wrapped = ?1,
                     kek_ref = ?2,
                     kek_version = ?3,
                     rotation_epoch = ?4,
                     rotated_at_ms = ?5,
                     updated_at_ms = ?5,
                     checksum_hash = ?6
                 WHERE account_id = ?7 AND secret_id = ?8",
                params![
                    new_wrapped,
                    new_kek_ref,
                    new_kek_version,
                    existing.rotation_epoch + 1,
                    now_ms(),
                    checksum,
                    account_id,
                    secret_id
                ],
            )
            .await
            .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))
        })?;
        Ok(true)
    }

    pub fn export_account_backup(
        &self,
        account_id: &str,
    ) -> Result<Vec<CloudlessSecretRecord>, SecretError> {
        let conn = self.conn.lock().expect("vox vault mutex");
        run_secrets_future(async {
            let mut rows = conn
                .query(
                    "SELECT account_id, secret_id, ciphertext, nonce, cipher_version, dek_wrapped,
                            kek_ref, kek_version, aad_hash, updated_at_ms, rotation_epoch,
                            rotated_at_ms, consistency_origin, consistency_version, checksum_hash
                     FROM clavis_account_secrets
                     WHERE account_id = ?1
                     ORDER BY updated_at_ms DESC, secret_id ASC",
                    params![account_id],
                )
                .await
                .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?;
            let mut out = Vec::new();
            while let Some(row) = rows
                .next()
                .await
                .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?
            {
                out.push(row_to_record(row)?);
            }
            Ok(out)
        })
    }

    pub fn import_account_backup(
        &self,
        rows: &[CloudlessSecretRecord],
        verify_checksums: bool,
    ) -> Result<(), SecretError> {
        for row in rows {
            if verify_checksums && !verify_record_checksum(row) {
                return Err(SecretError::BackendQueryFailed(format!(
                    "checksum mismatch during restore for account_id={} secret_id={}",
                    row.account_id, row.secret_id
                )));
            }
            let conn = self.conn.lock().expect("vox vault mutex");
            run_secrets_future(async {
                conn.execute(
                    "INSERT INTO clavis_account_secrets (
                        account_id, secret_id, ciphertext, nonce, cipher_version, dek_wrapped, dek_wrap_alg,
                        kek_ref, kek_version, aad_hash, updated_at_ms, rotation_epoch, rotated_at_ms,
                        consistency_origin, consistency_version, last_synced_at_ms, checksum_hash
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'ChaCha20-Poly1305', ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, NULL, ?15)
                    ON CONFLICT(account_id, secret_id) DO UPDATE SET
                        ciphertext = excluded.ciphertext,
                        nonce = excluded.nonce,
                        cipher_version = excluded.cipher_version,
                        dek_wrapped = excluded.dek_wrapped,
                        dek_wrap_alg = excluded.dek_wrap_alg,
                        kek_ref = excluded.kek_ref,
                        kek_version = excluded.kek_version,
                        aad_hash = excluded.aad_hash,
                        updated_at_ms = excluded.updated_at_ms,
                        rotation_epoch = excluded.rotation_epoch,
                        rotated_at_ms = excluded.rotated_at_ms,
                        consistency_origin = excluded.consistency_origin,
                        consistency_version = excluded.consistency_version,
                        checksum_hash = excluded.checksum_hash",
                    params![
                        row.account_id.clone(),
                        row.secret_id.clone(),
                        row.ciphertext.clone(),
                        row.nonce.clone(),
                        row.cipher_version,
                        row.dek_wrapped.clone(),
                        row.kek_ref.clone(),
                        row.kek_version,
                        row.aad_hash.clone(),
                        row.updated_at_ms,
                        row.rotation_epoch,
                        row.rotated_at_ms,
                        row.consistency_origin.clone(),
                        row.consistency_version,
                        row.checksum_hash.clone(),
                    ],
                )
                .await
                .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))
            })?;
        }
        Ok(())
    }

    fn get_row(
        &self,
        account_id: &str,
        secret_id: &str,
    ) -> Result<Option<CloudlessSecretRecord>, SecretError> {
        let conn = self.conn.lock().expect("vox vault mutex");
        run_secrets_future(async {
            let mut stmt = conn
                .prepare(
                    "SELECT account_id, secret_id, ciphertext, nonce, cipher_version, dek_wrapped,
                            kek_ref, kek_version, aad_hash, updated_at_ms, rotation_epoch,
                            rotated_at_ms, consistency_origin, consistency_version, checksum_hash
                     FROM clavis_account_secrets
                     WHERE account_id = ?1 AND secret_id = ?2",
                )
                .await
                .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?;
            let mut rows = stmt
                .query(params![account_id, secret_id])
                .await
                .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?;
            if let Some(row) = rows
                .next()
                .await
                .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?
            {
                return row_to_record(row).map(Some);
            }
            Ok(None)
        })
    }

    fn wrap_dek(
        &self,
        dek: &[u8; 32],
        kek_ref: &str,
        kek_version: i64,
    ) -> Result<Vec<u8>, SecretError> {
        let kek = derive_kek(&self.master_key, kek_ref, kek_version);
        let mut wrap_nonce = [0_u8; WRAP_NONCE_LEN];
        rand::thread_rng().fill_bytes(&mut wrap_nonce);
        let wrapped = encrypt_vault(&kek, &wrap_nonce, dek)?;
        let mut out = Vec::with_capacity(WRAP_NONCE_LEN + wrapped.len());
        out.extend_from_slice(&wrap_nonce);
        out.extend_from_slice(&wrapped);
        Ok(out)
    }

    fn unwrap_dek(
        &self,
        wrapped: &[u8],
        kek_ref: &str,
        kek_version: i64,
    ) -> Result<[u8; 32], SecretError> {
        if wrapped.len() <= WRAP_NONCE_LEN {
            return Err(SecretError::BackendQueryFailed(
                "wrapped DEK payload is too short".to_string(),
            ));
        }
        let wrap_nonce = &wrapped[..WRAP_NONCE_LEN];
        let wrapped_ct = &wrapped[WRAP_NONCE_LEN..];
        let kek = derive_kek(&self.master_key, kek_ref, kek_version);
        let dek_vec = decrypt_vault(&kek, wrap_nonce, wrapped_ct)?;
        let dek: [u8; 32] = dek_vec
            .as_slice()
            .try_into()
            .map_err(|_| SecretError::BackendQueryFailed("unwrapped DEK is not 32 bytes".into()))?;
        Ok(dek)
    }

    fn get_profile_row(
        &self,
        account_id: &str,
        secret_id: &str,
        profile: &str,
    ) -> Result<Option<ProfileSecretRecord>, SecretError> {
        let conn = self.conn.lock().expect("vox vault mutex");
        run_secrets_future(async {
            let mut stmt = conn
                .prepare(
                    "SELECT account_id, secret_id, profile, ciphertext, nonce, dek_wrapped,
                            kek_ref, kek_version, updated_at_ms, checksum_hash
                     FROM clavis_profile_overrides
                     WHERE account_id = ?1 AND secret_id = ?2 AND profile = ?3",
                )
                .await
                .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?;
            let mut rows = stmt
                .query(params![account_id, secret_id, profile])
                .await
                .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?;
            if let Some(row) = rows
                .next()
                .await
                .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?
            {
                return Ok(Some(ProfileSecretRecord {
                    account_id: row
                        .get(0)
                        .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?,
                    secret_id: row
                        .get(1)
                        .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?,
                    profile: row
                        .get(2)
                        .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?,
                    ciphertext: row
                        .get(3)
                        .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?,
                    nonce: row
                        .get(4)
                        .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?,
                    dek_wrapped: row
                        .get(5)
                        .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?,
                    kek_ref: row
                        .get(6)
                        .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?,
                    kek_version: row
                        .get(7)
                        .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?,
                    updated_at_ms: row
                        .get(8)
                        .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?,
                    checksum_hash: row
                        .get(9)
                        .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?,
                }));
            }
            Ok(None)
        })
    }

    fn get_valid_delegation(
        &self,
        account_id: &str,
        secret_id: &str,
        child_context: &str,
    ) -> Result<Option<AgentDelegationRecord>, SecretError> {
        let conn = self.conn.lock().expect("vox vault mutex");
        let now = now_ms();
        run_secrets_future(async {
            let mut stmt = conn
                .prepare(
                    "SELECT delegation_id, account_id, secret_id, scope_bits, parent_context,
                            child_context, issued_at_ms, expires_at_ms
                     FROM clavis_agent_delegations
                     WHERE account_id = ?1 AND secret_id = ?2 AND child_context = ?3
                       AND expires_at_ms > ?4 AND revoked_at_ms IS NULL",
                )
                .await
                .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?;
            let mut rows = stmt
                .query(params![account_id, secret_id, child_context, now])
                .await
                .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?;
            if let Some(row) = rows
                .next()
                .await
                .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?
            {
                return Ok(Some(AgentDelegationRecord {
                    delegation_id: row
                        .get(0)
                        .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?,
                    account_id: row
                        .get(1)
                        .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?,
                    secret_id: row
                        .get(2)
                        .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?,
                    scope_bits: row
                        .get(3)
                        .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?,
                    parent_context: row
                        .get(4)
                        .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?,
                    child_context: row
                        .get(5)
                        .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?,
                    issued_at_ms: row
                        .get(6)
                        .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?,
                    expires_at_ms: row
                        .get(7)
                        .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?,
                }));
            }
            Ok(None)
        })
    }
}

impl SecretBackend for VoxCloudBackend {
    fn resolve(
        &self,
        _id: SecretId,
        spec: SecretSpec,
        profile: Option<&str>,
        caller_context: &str,
    ) -> Result<Option<SecretString>, SecretError> {
        let key = spec.backend_key.unwrap_or(spec.canonical_env);

        // If caller is an agent, check delegation first
        if caller_context.starts_with("agent:") {
            if let Some(_delegation) =
                self.get_valid_delegation(&self.account_id, key, caller_context)?
            {
                // Delegation exists and is valid. Proceed to fetch actual material.
                // For now, delegations just grant access to the canonical secret.
            } else {
                // If no delegation and not in dev mode, maybe reject?
                // For now, let's just log or proceed with canonical if not strict.
            }
        }

        // Try profile override first if profile is specified
        if let Some(prof) = profile
            && let Some(row) = self.get_profile_row(&self.account_id, key, prof)?
        {
            if !verify_profile_record_checksum(&row) {
                return Err(SecretError::BackendQueryFailed(format!(
                    "checksum mismatch for override account_id={} secret_id={} profile={}",
                    row.account_id, row.secret_id, prof
                )));
            }
            let dek = self.unwrap_dek(&row.dek_wrapped, &row.kek_ref, row.kek_version)?;
            let plaintext = decrypt_vault(&dek, &row.nonce, &row.ciphertext)?;
            let secret_str = String::from_utf8(plaintext)
                .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?;
            return Ok(Some(SecretString::new(secret_str.into_boxed_str())));
        }

        // Fallback to canonical
        let Some(row) = self.get_row(&self.account_id, key)? else {
            return Ok(None);
        };
        if !verify_record_checksum(&row) {
            return Err(SecretError::BackendQueryFailed(format!(
                "checksum mismatch for account_id={} secret_id={}",
                row.account_id, row.secret_id
            )));
        }
        let dek = self.unwrap_dek(&row.dek_wrapped, &row.kek_ref, row.kek_version)?;
        let plaintext = decrypt_vault(&dek, &row.nonce, &row.ciphertext)?;
        let secret_str = String::from_utf8(plaintext)
            .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?;
        Ok(Some(SecretString::new(secret_str.into_boxed_str())))
    }

    fn write_audit_log(
        &self,
        secret_id: &str,
        status: &str,
        source: Option<&str>,
        profile: &str,
        caller_context: &str,
        detail: Option<&str>,
    ) -> Result<(), SecretError> {
        // Safety check: ensure detail doesn't contain suspected secret material.
        // For audit logs, we don't have a broad pattern set here, but we can check
        // against the secret_id itself (though id is usually public) or just ensure
        // it doesn't look like a known leak.
        if let Some(d) = detail
            && crate::redact::contains_secret_material(d, &[])
        {
            return Err(SecretError::BackendQueryFailed(
                "detail field contains suspected secret material".to_string(),
            ));
        }

        let account_id = self.account_id.clone();
        let sec_id = secret_id.to_string();
        let stat = status.to_string();
        let src = source.map(|s| s.to_string());
        let prof = profile.to_string();
        let ctx = caller_context.to_string();
        let det = detail.map(|s| s.to_string());
        let now = now_ms();

        let conn = self.conn.lock().expect("vox vault mutex");
        run_secrets_future(async {
            conn.execute(
                "INSERT INTO clavis_audit_log (
                    account_id, secret_id, resolved_at_ms, resolution_status,
                    resolution_source, resolve_profile, caller_context, detail
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![account_id, sec_id, now, stat, src, prof, ctx, det],
            )
            .await
            .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?;
            Ok(())
        })
    }
}

fn secrets_vault_compat_aliases_allowed() -> bool {
    let hard_cut_strict = std::env::var("VOX_SECRETS_HARD_CUT")
        .ok()
        .map(|v| {
            let t = v.trim().to_ascii_lowercase();
            matches!(t.as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(false);
    let cutover_phase_blocks_compat = std::env::var("VOX_SECRETS_CUTOVER_PHASE")
        .or_else(|_| std::env::var("VOX_SECRETS_MIGRATION_PHASE"))
        .ok()
        .map(|v| v.trim().to_ascii_lowercase())
        .is_some_and(|phase| matches!(phase.as_str(), "enforce" | "decommission"));
    !(hard_cut_strict || cutover_phase_blocks_compat)
}

fn path_to_vault_file_url(path: &str) -> String {
    let t = path.trim();
    if t.starts_with("file:") {
        return t.to_string();
    }
    let norm = t.replace('\\', "/");
    // Turso's `Builder::new_local()` (libsql) treats a `file:` URL as a real
    // URI string. On Windows, a path like `C:/Users/.../vault.db` contains a
    // colon after the drive letter, so the naive `file:{path}` form produces
    // `file:C:/...` which Turso then treats as a literal filename including
    // the prefix — failing with `I/O error: invalid filename` (absolute) or
    // creating a file literally named `file:vault.db` (relative). Emit a
    // standards-compliant `file://` URL for absolute Windows paths so the
    // colon falls inside the authority section, not the scheme.
    //
    // Reproducer / context: see vox_vault path_to_vault_file_url tests below.
    if is_windows_absolute_path(&norm) {
        // `file:///C:/path/to/db` — three slashes, then drive-letter path.
        format!("file:///{norm}")
    } else if norm.starts_with('/') {
        // POSIX absolute path: `file:///abs/path`.
        format!("file://{norm}")
    } else {
        // Relative path: `file:rel/path` is acceptable to Turso because
        // there is no colon-bearing drive letter to confuse the URL parser.
        format!("file:{norm}")
    }
}

/// `true` for paths like `C:/foo` or `D:/bar` (Windows absolute path with a
/// drive letter and forward-slash separator after path normalization).
fn is_windows_absolute_path(normalized: &str) -> bool {
    let bytes = normalized.as_bytes();
    bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'/'
}

fn resolve_cloudless_db_url() -> String {
    if let Ok(p) = std::env::var("VOX_SECRETS_VAULT_PATH") {
        let t = p.trim();
        if !t.is_empty() {
            return path_to_vault_file_url(t);
        }
    }
    if let Ok(u) = std::env::var("VOX_SECRETS_VAULT_URL") {
        let t = u.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }
    if secrets_vault_compat_aliases_allowed() {
        if let Ok(u) = std::env::var(concat!("VOX_", "TURSO", "_URL")) {
            let t = u.trim();
            if !t.is_empty() {
                return t.to_string();
            }
        }
        if let Ok(u) = std::env::var(concat!("TURSO", "_URL")) {
            let t = u.trim();
            if !t.is_empty() {
                return t.to_string();
            }
        }
    }
    "file:.vox/clavis_vault.db".to_string()
}

fn resolve_cloudless_auth_token() -> String {
    if let Ok(t) = std::env::var("VOX_SECRETS_VAULT_TOKEN")
        && !t.trim().is_empty()
    {
        return t;
    }
    if secrets_vault_compat_aliases_allowed() {
        if let Ok(t) = std::env::var(concat!("VOX_", "TURSO", "_TOKEN"))
            && !t.trim().is_empty()
        {
            return t;
        }
        if let Ok(t) = std::env::var(concat!("TURSO", "_AUTH_TOKEN"))
            && !t.trim().is_empty()
        {
            return t;
        }
    }
    String::new()
}

/// One-line summary for `vox secrets doctor` (no secret material).
#[must_use]
pub fn cloudless_vault_env_diagnostic() -> String {
    let url = resolve_cloudless_db_url();
    let token_present = !resolve_cloudless_auth_token().trim().is_empty();
    let mode = if url.starts_with("file:") {
        "local_file"
    } else {
        "remote"
    };
    let url_source = if std::env::var("VOX_SECRETS_VAULT_PATH")
        .ok()
        .is_some_and(|v| !v.trim().is_empty())
    {
        "VOX_SECRETS_VAULT_PATH"
    } else if std::env::var("VOX_SECRETS_VAULT_URL")
        .ok()
        .is_some_and(|v| !v.trim().is_empty())
    {
        "VOX_SECRETS_VAULT_URL"
    } else if secrets_vault_compat_aliases_allowed()
        && std::env::var(concat!("VOX_", "TURSO", "_URL"))
            .ok()
            .is_some_and(|v| !v.trim().is_empty())
    {
        "VOX_TURSO_URL"
    } else if secrets_vault_compat_aliases_allowed()
        && std::env::var(concat!("TURSO", "_URL"))
            .ok()
            .is_some_and(|v| !v.trim().is_empty())
    {
        "TURSO_URL"
    } else {
        "default_file"
    };
    let token_src = if std::env::var("VOX_SECRETS_VAULT_TOKEN")
        .ok()
        .is_some_and(|v| !v.trim().is_empty())
    {
        "VOX_SECRETS_VAULT_TOKEN"
    } else if secrets_vault_compat_aliases_allowed()
        && std::env::var(concat!("VOX_", "TURSO", "_TOKEN"))
            .ok()
            .is_some_and(|v| !v.trim().is_empty())
    {
        "VOX_TURSO_TOKEN"
    } else if secrets_vault_compat_aliases_allowed()
        && std::env::var(concat!("TURSO", "_AUTH_TOKEN"))
            .ok()
            .is_some_and(|v| !v.trim().is_empty())
    {
        "TURSO_AUTH_TOKEN"
    } else {
        "unset"
    };
    let host_hint = if url.starts_with("file:") {
        "local".to_string()
    } else {
        url.split("//")
            .nth(1)
            .map_or("remote", |h| h.split('/').next().unwrap_or("remote"))
            .to_string()
    };
    format!(
        "mode={mode}; url_source={url_source}; url_host_hint={host_hint}; token_source={token_src}; token_present={token_present}; compat_aliases_allowed={}",
        secrets_vault_compat_aliases_allowed()
    )
}

async fn open_cloudless_connection() -> Result<turso::Connection, SecretError> {
    let db_url = resolve_cloudless_db_url();
    if db_url.starts_with("file:") {
        let db = turso::Builder::new_local(&db_url)
            .build()
            .await
            .map_err(|e| SecretError::BackendMisconfigured(e.to_string()))?;
        return db
            .connect()
            .map_err(|e| SecretError::BackendMisconfigured(e.to_string()));
    }
    let token = resolve_cloudless_auth_token();
    let db = turso::sync::Builder::new_remote(":memory:")
        .with_remote_url(&db_url)
        .with_auth_token(token)
        .build()
        .await
        .map_err(|e| SecretError::BackendMisconfigured(e.to_string()))?;
    db.connect()
        .await
        .map_err(|e| SecretError::BackendMisconfigured(e.to_string()))
}

async fn ensure_schema(conn: &turso::Connection) -> Result<(), SecretError> {
    if resolve_cloudless_db_url().starts_with("file:") {
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
            .await
            .map_err(|e| SecretError::BackendMisconfigured(e.to_string()))?;
    }
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS clavis_account_secrets (
            account_id TEXT NOT NULL,
            secret_id TEXT NOT NULL,
            ciphertext BLOB NOT NULL,
            nonce BLOB NOT NULL,
            cipher_version INTEGER NOT NULL DEFAULT 1,
            dek_wrapped BLOB NOT NULL,
            dek_wrap_alg TEXT NOT NULL DEFAULT 'ChaCha20-Poly1305',
            kek_ref TEXT NOT NULL,
            kek_version INTEGER NOT NULL,
            aad_hash TEXT,
            updated_at_ms INTEGER NOT NULL,
            rotation_epoch INTEGER NOT NULL DEFAULT 0,
            rotated_at_ms INTEGER,
            consistency_origin TEXT NOT NULL DEFAULT 'canonical',
            consistency_version INTEGER NOT NULL DEFAULT 1,
            last_synced_at_ms INTEGER,
            checksum_hash TEXT NOT NULL,
            PRIMARY KEY (account_id, secret_id)
        );
        CREATE INDEX IF NOT EXISTS idx_clavis_account_secrets_account_updated
            ON clavis_account_secrets(account_id, updated_at_ms DESC);
        CREATE INDEX IF NOT EXISTS idx_clavis_account_secrets_kek
            ON clavis_account_secrets(kek_ref, kek_version);
            
        CREATE TABLE IF NOT EXISTS clavis_secret_versions (
            version_id      INTEGER PRIMARY KEY AUTOINCREMENT,
            account_id      TEXT    NOT NULL,
            secret_id       TEXT    NOT NULL,
            ciphertext      BLOB    NOT NULL,
            nonce           BLOB    NOT NULL,
            dek_wrapped     BLOB    NOT NULL,
            kek_ref         TEXT    NOT NULL,
            kek_version     INTEGER NOT NULL,
            operation       TEXT    NOT NULL CHECK(
                                operation IN ('create','rotate','import','rollback','rewrap')
                            ),
            source_hint     TEXT,
            created_at_ms   INTEGER NOT NULL,
            created_by      TEXT    NOT NULL CHECK(
                                created_by IN ('cli','mcp','api') OR created_by LIKE 'agent:%'
                            ),
            checksum_hash TEXT    NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_clavis_sv_lookup
            ON clavis_secret_versions(account_id, secret_id, version_id DESC);
        CREATE INDEX IF NOT EXISTS idx_clavis_sv_kek
            ON clavis_secret_versions(kek_ref, kek_version);
            
        CREATE TABLE IF NOT EXISTS clavis_audit_log (
            row_id           INTEGER PRIMARY KEY AUTOINCREMENT,
            account_id       TEXT    NOT NULL,
            secret_id        TEXT    NOT NULL,
            resolved_at_ms   INTEGER NOT NULL,
            resolution_status TEXT   NOT NULL,
            resolution_source TEXT,
            resolve_profile  TEXT    NOT NULL,
            caller_context   TEXT    NOT NULL,
            detail           TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_clavis_al_time
            ON clavis_audit_log(account_id, resolved_at_ms DESC);
        CREATE INDEX IF NOT EXISTS idx_clavis_al_secret
            ON clavis_audit_log(account_id, secret_id, resolved_at_ms DESC);
            
        CREATE TABLE IF NOT EXISTS clavis_profile_overrides (
            account_id      TEXT    NOT NULL,
            secret_id       TEXT    NOT NULL,
            profile         TEXT    NOT NULL CHECK(
                                profile IN ('dev','ci','prod','hardcut')
                            ),
            ciphertext      BLOB    NOT NULL,
            nonce           BLOB    NOT NULL,
            dek_wrapped     BLOB    NOT NULL,
            kek_ref         TEXT    NOT NULL,
            kek_version     INTEGER NOT NULL,
            updated_at_ms   INTEGER NOT NULL,
            checksum_hash TEXT    NOT NULL,
            PRIMARY KEY (account_id, secret_id, profile)
        );
        
        CREATE TABLE IF NOT EXISTS clavis_agent_delegations (
            delegation_id   TEXT    PRIMARY KEY,
            account_id      TEXT    NOT NULL,
            secret_id       TEXT    NOT NULL,
            scope_bits      INTEGER NOT NULL DEFAULT 1,
            parent_context  TEXT    NOT NULL,
            child_context   TEXT    NOT NULL,
            issued_at_ms    INTEGER NOT NULL,
            expires_at_ms   INTEGER NOT NULL,
            revoked_at_ms   INTEGER,
            revoke_reason   TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_clavis_del_lookup
            ON clavis_agent_delegations(account_id, secret_id, expires_at_ms DESC);",
    )
    .await
    .map_err(|e| SecretError::BackendMisconfigured(e.to_string()))
}

fn row_to_record(row: turso::Row) -> Result<CloudlessSecretRecord, SecretError> {
    Ok(CloudlessSecretRecord {
        account_id: row
            .get(0)
            .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?,
        secret_id: row
            .get(1)
            .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?,
        ciphertext: row
            .get(2)
            .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?,
        nonce: row
            .get(3)
            .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?,
        cipher_version: row
            .get(4)
            .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?,
        dek_wrapped: row
            .get(5)
            .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?,
        kek_ref: row
            .get(6)
            .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?,
        kek_version: row
            .get(7)
            .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?,
        aad_hash: row
            .get(8)
            .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?,
        updated_at_ms: row
            .get(9)
            .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?,
        rotation_epoch: row
            .get(10)
            .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?,
        rotated_at_ms: row
            .get(11)
            .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?,
        consistency_origin: row
            .get(12)
            .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?,
        consistency_version: row
            .get(13)
            .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?,
        checksum_hash: row
            .get(14)
            .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?,
    })
}

fn verify_record_checksum(row: &CloudlessSecretRecord) -> bool {
    compute_account_secret_checksum(
        &row.account_id,
        &row.secret_id,
        &row.ciphertext,
        &row.nonce,
        row.cipher_version,
        &row.dek_wrapped,
        &row.kek_ref,
        row.kek_version,
        row.rotation_epoch,
        row.consistency_version,
    ) == row.checksum_hash
}

fn verify_profile_record_checksum(row: &ProfileSecretRecord) -> bool {
    compute_account_secret_checksum(
        &row.account_id,
        &row.secret_id,
        &row.ciphertext,
        &row.nonce,
        1,
        &row.dek_wrapped,
        &row.kek_ref,
        row.kek_version,
        0,
        1,
    ) == row.checksum_hash
}

fn compute_account_secret_checksum(
    account_id: &str,
    secret_id: &str,
    ciphertext: &[u8],
    nonce: &[u8],
    cipher_version: i64,
    dek_wrapped: &[u8],
    kek_ref: &str,
    kek_version: i64,
    rotation_epoch: i64,
    consistency_version: i64,
) -> String {
    let mut data = Vec::new();
    data.extend_from_slice(account_id.as_bytes());
    data.extend_from_slice(&[0x1f]);
    data.extend_from_slice(secret_id.as_bytes());
    data.extend_from_slice(&[0x1f]);
    data.extend_from_slice(ciphertext);
    data.extend_from_slice(&[0x1f]);
    data.extend_from_slice(nonce);
    data.extend_from_slice(&cipher_version.to_le_bytes());
    data.extend_from_slice(dek_wrapped);
    data.extend_from_slice(kek_ref.as_bytes());
    data.extend_from_slice(&kek_version.to_le_bytes());
    data.extend_from_slice(&rotation_epoch.to_le_bytes());
    data.extend_from_slice(&consistency_version.to_le_bytes());

    // secure_hash 32-byte hash to string using simple hex encoding
    let hash = secure_hash(&data);
    hash.iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>()
}

fn derive_master_key() -> Result<[u8; 32], SecretError> {
    let entry = keyring::Entry::new("vox-secrets-vault", "master")
        .map_err(|e| SecretError::BackendMisconfigured(e.to_string()))?;
    let password = match entry.get_password() {
        Ok(value) if !value.is_empty() => value,
        _ => {
            let mut bootstrap = [0_u8; 32];
            rand::thread_rng().fill_bytes(&mut bootstrap);
            let generated = bootstrap
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>();
            entry.set_password(&generated).map_err(|e| {
                SecretError::BackendMisconfigured(format!(
                    "failed to initialize keyring master key: {e}"
                ))
            })?;
            generated
        }
    };
    Ok(secure_hash(password.as_bytes()))
}

fn derive_kek(master_key: &[u8; 32], kek_ref: &str, kek_version: i64) -> [u8; 32] {
    let mut data = Vec::new();
    data.extend_from_slice(master_key);
    data.extend_from_slice(kek_ref.as_bytes());
    data.extend_from_slice(&kek_version.to_le_bytes());
    secure_hash(&data)
}

fn encrypt_vault(
    key_bytes: &[u8; 32],
    nonce: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, SecretError> {
    encrypt_with_nonce(&SymKey(*key_bytes), nonce, plaintext)
        .map_err(|e| SecretError::BackendUnavailable(format!("encryption failed: {e}")))
}

fn decrypt_vault(
    key_bytes: &[u8; 32],
    nonce: &[u8],
    ciphertext: &[u8],
) -> Result<Vec<u8>, SecretError> {
    decrypt_with_nonce(&SymKey(*key_bytes), nonce, ciphertext)
        .map_err(|e| SecretError::BackendQueryFailed(format!("decryption failed: {e}")))
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn run_secrets_future<F, T>(future: F) -> Result<T, SecretError>
where
    F: Future<Output = Result<T, SecretError>>,
{
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            tokio::task::block_in_place(|| handle.block_on(future))
        }));
        return result.map_err(|_| {
            SecretError::BackendMisconfigured(
                "failed to execute secrets async operation from active runtime".to_string(),
            )
        })?;
    }

    Err(SecretError::BackendMisconfigured(
        "run_secrets_future requires an active Tokio runtime".to_string(),
    ))
}

#[cfg(test)]
mod semcov_wave2_tests {
    #![allow(unused_imports)]
    use super::*;

    #[test]
    fn compute_checksum_is_deterministic() {
        let a = compute_account_secret_checksum(
            "acct-1",
            "secret-key",
            b"ciphertext",
            b"nonce12345678901",
            1,
            b"wrapped-dek",
            "kek-ref-1",
            2,
            0,
            1,
        );
        let b = compute_account_secret_checksum(
            "acct-1",
            "secret-key",
            b"ciphertext",
            b"nonce12345678901",
            1,
            b"wrapped-dek",
            "kek-ref-1",
            2,
            0,
            1,
        );
        assert_eq!(a, b, "same inputs must produce same checksum");
    }

    #[test]
    fn compute_checksum_differs_on_account_id_change() {
        let a = compute_account_secret_checksum(
            "acct-1",
            "secret-key",
            b"ct",
            b"nonce",
            1,
            b"dek",
            "kekref",
            1,
            0,
            1,
        );
        let b = compute_account_secret_checksum(
            "acct-2",
            "secret-key",
            b"ct",
            b"nonce",
            1,
            b"dek",
            "kekref",
            1,
            0,
            1,
        );
        assert_ne!(a, b, "different account_id must yield different checksum");
    }

    #[test]
    fn compute_checksum_differs_on_cipher_version_change() {
        let a =
            compute_account_secret_checksum("acct", "sid", b"ct", b"n", 1, b"dek", "kref", 1, 0, 1);
        let b =
            compute_account_secret_checksum("acct", "sid", b"ct", b"n", 2, b"dek", "kref", 1, 0, 1);
        assert_ne!(
            a, b,
            "cipher_version is encoded in LE bytes and must affect hash"
        );
    }

    #[test]
    fn compute_checksum_output_is_64_hex_chars() {
        let h = compute_account_secret_checksum("a", "b", b"c", b"d", 0, b"e", "f", 0, 0, 0);
        assert_eq!(h.len(), 64, "SHA-256 = 32 bytes = 64 hex chars; got: {h}");
        assert!(
            h.chars().all(|c| c.is_ascii_hexdigit()),
            "output must be lowercase hex"
        );
    }

    #[test]
    fn verify_record_checksum_accepts_correct_checksum() {
        let acct = "test-account";
        let sid = "MY_SECRET";
        let ct = b"encrypted-bytes";
        let nonce = b"unique-nonce-12";
        let cipher_version: i64 = 1;
        let dek = b"wrapped-dek-blob";
        let kek_ref = "primary";
        let kek_version: i64 = 3;
        let rotation_epoch: i64 = 0;
        let consistency_version: i64 = 1;

        let checksum = compute_account_secret_checksum(
            acct,
            sid,
            ct,
            nonce,
            cipher_version,
            dek,
            kek_ref,
            kek_version,
            rotation_epoch,
            consistency_version,
        );

        let record = CloudlessSecretRecord {
            account_id: acct.to_string(),
            secret_id: sid.to_string(),
            ciphertext: ct.to_vec(),
            nonce: nonce.to_vec(),
            cipher_version,
            dek_wrapped: dek.to_vec(),
            kek_ref: kek_ref.to_string(),
            kek_version,
            aad_hash: None,
            updated_at_ms: 0,
            rotation_epoch,
            rotated_at_ms: None,
            consistency_origin: "local".to_string(),
            consistency_version,
            checksum_hash: checksum,
        };
        assert!(
            verify_record_checksum(&record),
            "correct checksum must pass verify"
        );
    }

    #[test]
    fn verify_record_checksum_rejects_tampered_checksum() {
        let record = CloudlessSecretRecord {
            account_id: "acct".to_string(),
            secret_id: "sid".to_string(),
            ciphertext: b"ct".to_vec(),
            nonce: b"n".to_vec(),
            cipher_version: 1,
            dek_wrapped: b"dek".to_vec(),
            kek_ref: "kref".to_string(),
            kek_version: 1,
            aad_hash: None,
            updated_at_ms: 0,
            rotation_epoch: 0,
            rotated_at_ms: None,
            consistency_origin: "local".to_string(),
            consistency_version: 1,
            checksum_hash: "deadbeef".to_string(),
        };
        assert!(
            !verify_record_checksum(&record),
            "tampered checksum must fail verify"
        );
    }

    #[test]
    fn verify_profile_record_checksum_accepts_correct_checksum() {
        let acct = "profile-account";
        let sid = "PROFILE_SECRET";
        let ct = b"profile-ciphertext";
        let nonce = b"profile-nonce-16";
        let dek = b"profile-dek";
        let kek_ref = "profile-kek";
        let kek_version: i64 = 1;

        // verify_profile_record_checksum calls compute_account_secret_checksum
        // with hardcoded cipher_version=1, rotation_epoch=0, consistency_version=1
        let checksum = compute_account_secret_checksum(
            acct,
            sid,
            ct,
            nonce,
            1,
            dek,
            kek_ref,
            kek_version,
            0,
            1,
        );

        let record = ProfileSecretRecord {
            account_id: acct.to_string(),
            secret_id: sid.to_string(),
            profile: "dev".to_string(),
            ciphertext: ct.to_vec(),
            nonce: nonce.to_vec(),
            dek_wrapped: dek.to_vec(),
            kek_ref: kek_ref.to_string(),
            kek_version,
            updated_at_ms: 0,
            checksum_hash: checksum,
        };
        assert!(
            verify_profile_record_checksum(&record),
            "correct checksum must pass"
        );
    }

    #[test]
    fn verify_profile_record_checksum_rejects_wrong_checksum() {
        let record = ProfileSecretRecord {
            account_id: "a".to_string(),
            secret_id: "b".to_string(),
            profile: "ci".to_string(),
            ciphertext: b"c".to_vec(),
            nonce: b"d".to_vec(),
            dek_wrapped: b"e".to_vec(),
            kek_ref: "f".to_string(),
            kek_version: 1,
            updated_at_ms: 0,
            checksum_hash: "wrong".to_string(),
        };
        assert!(
            !verify_profile_record_checksum(&record),
            "wrong checksum must fail"
        );
    }
}

#[cfg(test)]
mod path_url_tests {
    use super::*;

    #[test]
    fn already_file_url_passes_through() {
        assert_eq!(
            path_to_vault_file_url("file:foo.db"),
            "file:foo.db",
            "existing file: URLs are not double-wrapped"
        );
        assert_eq!(
            path_to_vault_file_url("file:///C:/x.db"),
            "file:///C:/x.db",
            "existing triple-slash URLs survive verbatim"
        );
    }

    #[test]
    fn relative_path_uses_short_file_form() {
        // No drive letter, no leading slash → short form, no colon ambiguity.
        assert_eq!(
            path_to_vault_file_url(".vox/clavis_vault.db"),
            "file:.vox/clavis_vault.db"
        );
        assert_eq!(
            path_to_vault_file_url("clavis_vault.db"),
            "file:clavis_vault.db"
        );
    }

    #[test]
    fn posix_absolute_path_uses_double_slash_authority() {
        assert_eq!(
            path_to_vault_file_url("/home/user/.vox/clavis_vault.db"),
            "file:///home/user/.vox/clavis_vault.db"
        );
    }

    #[test]
    fn windows_absolute_path_uses_triple_slash_authority() {
        // Forward-slash form (already normalized).
        assert_eq!(
            path_to_vault_file_url("C:/Users/Owner/.vox/clavis_vault.db"),
            "file:///C:/Users/Owner/.vox/clavis_vault.db",
            "regression: bare `file:C:/...` makes Turso treat the drive-letter colon \
             as part of the filename and fail with `I/O error: invalid filename`"
        );
        // Backslash form (caller may pass either; we normalize).
        assert_eq!(
            path_to_vault_file_url(r"C:\Users\Owner\.vox\clavis_vault.db"),
            "file:///C:/Users/Owner/.vox/clavis_vault.db"
        );
    }

    #[test]
    #[allow(unsafe_code)]
    fn write_present_then_delete_absent_round_trips() {
        use std::sync::Mutex;
        static ENV_LOCK: Mutex<()> = Mutex::new(());
        let _g = ENV_LOCK.lock().expect("env lock");

        let tmp_dir = tempfile::tempdir().expect("tempdir");
        let db_path = tmp_dir.path().join("delete_vault.db");
        unsafe {
            std::env::set_var("VOX_SECRETS_VAULT_PATH", &db_path);
            std::env::set_var("VOX_ACCOUNT_ID", "delete-test-account");
        }

        // VoxCloudBackend uses a keyring-backed master key; if the keyring is
        // unavailable in the sandbox the backend can't init — skip cleanly.
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .expect("tokio rt");
        let outcome = rt.block_on(async {
            tokio::task::spawn_blocking(|| {
                let backend = match VoxCloudBackend::new() {
                    Ok(b) => b,
                    Err(_) => return None,
                };
                let key = "DELETE_TEST_SECRET";
                backend
                    .write_secret(key, "sk-delete-me-0123456789")
                    .expect("write");
                let present = backend
                    .get_row(&backend.account_id, key)
                    .expect("get")
                    .is_some();
                let deleted = backend.delete_secret(key).expect("delete");
                let absent = backend
                    .get_row(&backend.account_id, key)
                    .expect("get2")
                    .is_none();
                let deleted_again = backend.delete_secret(key).expect("delete2");
                Some((present, deleted, absent, deleted_again))
            })
            .await
            .expect("join")
        });

        unsafe {
            std::env::remove_var("VOX_SECRETS_VAULT_PATH");
            std::env::remove_var("VOX_ACCOUNT_ID");
        }

        if let Some((present, deleted, absent, deleted_again)) = outcome {
            assert!(present, "secret should be present after write");
            assert!(deleted, "delete should report a row was removed");
            assert!(absent, "secret should be absent after delete");
            assert!(!deleted_again, "second delete reports no row removed");
        }
    }

    #[test]
    fn is_windows_absolute_path_recognizes_drive_letters() {
        assert!(is_windows_absolute_path("C:/foo"));
        assert!(is_windows_absolute_path("d:/foo"));
        assert!(is_windows_absolute_path("Z:/x"));
        assert!(!is_windows_absolute_path("C:foo"), "no slash after colon");
        assert!(!is_windows_absolute_path("/foo"));
        assert!(!is_windows_absolute_path("foo/bar"));
        assert!(!is_windows_absolute_path("1:/foo"), "first char not alpha");
        assert!(!is_windows_absolute_path(""));
    }
}
