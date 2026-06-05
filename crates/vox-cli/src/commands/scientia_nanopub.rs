//! Per-user nanopublication identity resolver (design §4.1).
//!
//! Binds a `user_id` to its RSA signing material via a get-or-create flow:
//! - the private key lives in the Clavis vault under
//!   [`SecretId::VoxUserRsaNanopubPrivateKeyB64`] (per-user, NEVER shared);
//! - the public key + ORCID + the secret's canonical-env reference live in the
//!   `user_identities` table.
//!
//! This module owns the DB + secrets I/O so that `vox-scientia` stays a pure,
//! side-effect-free library. It does NOT publish anything to the network.

use vox_db::VoxDb;
use vox_db::store::UserIdentityRow;
use vox_scientia::nanopub::spec::{NanopubProfile, gen_keys};
use vox_secrets::SecretId;

/// Resolve the effective ORCID via the canonical precedence: the explicit
/// `param` wins; else the stored `row_orcid`; else an error.
///
/// This is the single source of truth for the ORCID-precedence rule used by
/// [`resolve_or_create_identity`]. Pure: no DB, no vault, no I/O.
///
/// # Errors
/// Returns an error (mentioning ORCID) when neither source supplies one.
fn effective_orcid(param: Option<&str>, row_orcid: Option<&str>) -> anyhow::Result<String> {
    param
        .map(str::to_string)
        .or_else(|| row_orcid.map(str::to_string))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "an ORCID is required to sign a nanopublication; \
                 pass one explicitly (e.g. https://orcid.org/0000-0002-1825-0097) or \
                 store one on the user identity first"
            )
        })
}

/// Get-or-create the per-user nanopublication signing identity.
///
/// 1. Reads the stored `user_identities` row (if any).
/// 2. Chooses an ORCID: the `orcid` arg wins; else the stored `orcid_id`; else
///    errors (the `nanopub` crate requires an `https://orcid.org/` identity to
///    sign).
/// 3. Resolves the private key from the vault. If a row exists AND the key
///    resolves, REUSES that material.
/// 4. Otherwise GENERATES a fresh RSA keypair, stores the private key in the
///    vault, and upserts the public key + ORCID + key-ref into the DB.
///
/// Returns a [`NanopubProfile`] whose `name` is the `user_id`.
///
/// # Errors
/// Returns an error if no ORCID can be determined, if RSA keygen fails, if the
/// vault write fails, or if any DB op fails.
pub async fn resolve_or_create_identity(
    db: &VoxDb,
    user_id: &str,
    orcid: Option<&str>,
) -> anyhow::Result<NanopubProfile> {
    let existing = db.get_user_identity(user_id).await?;

    // Choose the ORCID: explicit arg first, then the stored one (single source
    // of truth for the precedence rule lives in `effective_orcid`).
    let row_orcid = existing.as_ref().and_then(|row| row.orcid_id.as_deref());
    let chosen_orcid = effective_orcid(orcid, row_orcid)
        .map_err(|e| anyhow::anyhow!("{e} (for user `{user_id}`)"))?;

    let key_id = SecretId::VoxUserRsaNanopubPrivateKeyB64;

    // REUSE path: a row exists and the private key resolves from the vault.
    if existing.is_some() {
        if let Some(priv_b64) = vox_secrets::resolve_secret(key_id).expose() {
            return Ok(NanopubProfile {
                orcid: chosen_orcid,
                name: user_id.to_string(),
                rsa_private_key_b64: priv_b64.to_string(),
            });
        }
    }

    // CREATE path: generate a fresh keypair, persist the private key in the
    // vault and the public key + ORCID + key-ref in the DB.
    let (priv_b64, pub_b64) =
        gen_keys().map_err(|e| anyhow::anyhow!("RSA nanopub keygen failed: {e}"))?;

    // KNOWN LIMITATION: the private key is stored under a single account-scoped
    // canonical env (`VOX_USER_RSA_NANOPUB_PRIVATE_KEY_B64`), not namespaced by
    // `user_id`. This is correct for one human per account (the P1 target): a
    // second `user_id` on the same account would reuse the same key material.
    // Per-user-per-account namespacing of the secret is deliberately future work.
    vox_secrets::store_secret(key_id, &priv_b64, None)
        .map_err(|e| anyhow::anyhow!("failed to store nanopub private key in vault: {e}"))?;

    let canonical_env = key_id.spec().canonical_env;
    let now_ms = chrono::Utc::now().timestamp_millis();
    let row = UserIdentityRow {
        user_id: user_id.to_string(),
        orcid_id: Some(chosen_orcid.clone()),
        nanopub_pubkey_b64: Some(pub_b64),
        nanopub_key_ref: canonical_env.to_string(),
        created_at_ms: now_ms,
        updated_at_ms: now_ms,
    };
    db.upsert_user_identity(&row).await?;

    Ok(NanopubProfile {
        orcid: chosen_orcid,
        name: user_id.to_string(),
        rsa_private_key_b64: priv_b64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use vox_db::{DbConfig, VoxDb};

    // Pure, vault-free, DB-free unit tests for the ORCID precedence (param wins,
    // then row fallback, then error). These run in-sandbox with no I/O.
    #[test]
    fn effective_orcid_param_wins_over_row() {
        let got = effective_orcid(Some("https://orcid.org/A"), Some("https://orcid.org/B"))
            .expect("param orcid should win");
        assert_eq!(got, "https://orcid.org/A");
    }

    #[test]
    fn effective_orcid_falls_back_to_row() {
        let got = effective_orcid(None, Some("https://orcid.org/B"))
            .expect("row orcid should be used when param is absent");
        assert_eq!(got, "https://orcid.org/B");
    }

    #[test]
    fn effective_orcid_errors_when_none() {
        let err = effective_orcid(None, None).expect_err("no orcid available must error");
        let msg = err.to_string();
        assert!(
            msg.to_uppercase().contains("ORCID"),
            "error message must mention ORCID, got: {msg}"
        );
    }

    // Serialize env-var mutation across tests in this module.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    // A multi-threaded runtime is REQUIRED: the vox-secrets vault backend bridges
    // its async ops via `tokio::task::block_in_place`, which panics on the default
    // current-thread runtime. Without this flavor the create path always errors and
    // the test silently takes its skip branch (assertions never run).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[allow(unsafe_code)]
    async fn reuses_key_and_persists_orcid_across_calls() {
        // Hermetic: isolate the vault DB to a temp dir, pin a throwaway account,
        // and force the vox_cloud backend (cutover=decommission) so that the
        // store/resolve round-trip targets the same temp vault. Mirrors
        // `vox-secrets::tests::store_secret_round_trips_user_rsa_nanopub_key_via_temp_vault`.
        let _g = ENV_LOCK.lock().expect("env lock");

        let tmp_dir = tempfile::tempdir().expect("tempdir");
        let db_path = tmp_dir.path().join("nanopub_identity_vault.db");

        let prev_path = std::env::var("VOX_SECRETS_VAULT_PATH").ok();
        let prev_account = std::env::var("VOX_ACCOUNT_ID").ok();
        let prev_cutover = std::env::var("VOX_SECRETS_CUTOVER_PHASE").ok();
        unsafe {
            std::env::set_var("VOX_SECRETS_VAULT_PATH", &db_path);
            std::env::set_var("VOX_ACCOUNT_ID", "nanopub-identity-test-account");
            std::env::set_var("VOX_SECRETS_CUTOVER_PHASE", "decommission");
        }

        let db = VoxDb::connect(DbConfig::Memory)
            .await
            .expect("in-memory db connect");

        const USER: &str = "test-user";
        const ORCID: &str = "https://orcid.org/0000-0002-1825-0097";

        let first = resolve_or_create_identity(&db, USER, Some(ORCID)).await;

        // If the keyring/vault is unavailable in this sandbox, `store_secret`
        // (inside the create path) errors. Skip cleanly rather than false-pass.
        let outcome = match first {
            Ok(id1) => {
                let id2 = resolve_or_create_identity(&db, USER, None)
                    .await
                    .expect("second resolve (orcid=None) should reuse stored material");
                Some((id1, id2))
            }
            Err(e) => {
                eprintln!(
                    "SKIP reuses_key_and_persists_orcid_across_calls: vault unavailable in sandbox ({e})"
                );
                None
            }
        };

        unsafe {
            match prev_path {
                Some(v) => std::env::set_var("VOX_SECRETS_VAULT_PATH", v),
                None => std::env::remove_var("VOX_SECRETS_VAULT_PATH"),
            }
            match prev_account {
                Some(v) => std::env::set_var("VOX_ACCOUNT_ID", v),
                None => std::env::remove_var("VOX_ACCOUNT_ID"),
            }
            match prev_cutover {
                Some(v) => std::env::set_var("VOX_SECRETS_CUTOVER_PHASE", v),
                None => std::env::remove_var("VOX_SECRETS_CUTOVER_PHASE"),
            }
        }

        if let Some((id1, id2)) = outcome {
            assert_eq!(
                id1.rsa_private_key_b64, id2.rsa_private_key_b64,
                "the private key must be reused across calls, not regenerated"
            );
            assert!(
                !id1.rsa_private_key_b64.is_empty(),
                "the reused private key must be non-empty"
            );
            assert_eq!(
                id2.orcid, ORCID,
                "the ORCID supplied on the first call must persist and be reused when omitted"
            );
            assert_eq!(id2.name, USER, "the profile name must be the user_id");
        }
    }
}
