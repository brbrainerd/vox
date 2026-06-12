//! Single source of truth (SSOT) for the SCIENTIA human-review flow.
//!
//! This module owns the DB + secrets I/O that backs the discovery-review
//! lifecycle so that BOTH the CLI (`vox scientia publication-*`) and the GUI
//! (DiscoveryReview surface) call ONE shared implementation:
//!
//!   - [`record_claim_review`] — persist a human review decision, content-bound
//!     to the publication's current manifest digest;
//!   - [`approval_for`] — bridge the DB review ledger to a minted
//!     [`crate::review::ApprovalToken`];
//!   - [`nanopub_build`] — build a spec-compliant, RSA-signed, OFFLINE-validated
//!     nanopublication for an approved claim and persist it locally;
//!   - [`resolve_or_create_identity`] — get-or-create the per-user RSA + ORCID
//!     signing identity (vault-backed);
//!   - [`publication_session_id`] — derive the stable `session_id` bucket for a
//!     publication's extracted claims.
//!
//! It does NOT publish anything to the network (no `publish`, no test server).

use vox_db::VoxDb;
use vox_db::store::{NanopubRow, UserIdentityRow};
use vox_secrets::SecretId;

use crate::nanopub::spec::{NanopubProfile, SignedNanopubDoc, gen_keys};

/// Derive a stable `session_id` from a publication id (FNV-1a). `scientia_claims`
/// is keyed by `session_id`; a publication's extracted claims share this bucket.
pub fn publication_session_id(publication_id: &str) -> i64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in publication_id.as_bytes() {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash as i64
}

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
    if existing.is_some()
        && let Some(priv_b64) = vox_secrets::resolve_secret(key_id).expose()
    {
        return Ok(NanopubProfile {
            orcid: chosen_orcid,
            name: user_id.to_string(),
            rsa_private_key_b64: priv_b64.to_string().into(),
        });
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
        rsa_private_key_b64: priv_b64.into(),
    })
}

/// P1 — Build a spec-compliant, RSA-signed, OFFLINE-validated nanopublication for
/// a single extracted claim, then persist it locally. This function performs NO
/// network publishing of any kind (no `publish`, no test server).
///
/// Steps:
/// 1. Load the claim row + its latest verdict from the SCIENTIA claim ledger
///    (errors helpfully if absent — run `publication-extract-claims` first).
/// 2. Resolve (or create) the per-user RSA + ORCID signing identity.
/// 3. Assemble the enriched assertion Turtle and RSA-sign it (build + sign).
/// 4. VALIDATE the signed TriG OFFLINE (trusty hash + signature). Fails hard if
///    invalid — nothing is persisted on a validation failure.
/// 5. Persist a `scientia_nanopubs` row with `published_state="local"` and
///    `validated_offline=true`.
///
/// Returns the [`SignedNanopubDoc`] (Trusty URI + signed TriG). The caller prints
/// a short human line (the Trusty URI) — this function deliberately writes NOTHING
/// to stdout, but note that [`crate::nanopub::spec::validate_offline`]
/// internally calls the upstream `nanopub` crate's `check()`, which prints a
/// `✅ ... is valid` line to stdout on success. A strict `--json` mode for this
/// command therefore needs upstream stdout suppression and is deferred.
///
/// # Errors
/// Returns an error if the claim/verdict is missing, if no ORCID is available, if
/// signing fails, if offline validation fails, or if any DB op fails.
pub async fn nanopub_build(
    db: &VoxDb,
    publication_id: &str,
    claim_id: i64,
    orcid: Option<&str>,
    token: &crate::review::ApprovalToken,
) -> anyhow::Result<SignedNanopubDoc> {
    // 0. SECURITY GATE (P2 Task 3): refuse to build/sign/persist anything unless
    // an approval token is content-bound to the publication's CURRENT manifest.
    // This runs BEFORE any signing or persistence, so a stale/mismatched approval
    // leaves nothing behind. The token itself is unforgeable: it can only be
    // minted from an "approved" `ReviewDecisionRow` (see `crate::review`).
    // Cheapest/most-specific identity check first: is this token even for the
    // requested publication? Guards against cross-publication replay when two
    // publications share a claim hash AND content digest.
    if token.publication_id() != publication_id {
        anyhow::bail!(
            "approval token publication mismatch (token approves publication `{}`, build requested `{publication_id}`)",
            token.publication_id()
        );
    }
    // Then: is this token even for the requested claim?
    if token.claim_id() != claim_id {
        anyhow::bail!(
            "approval token claim mismatch (token approves claim {}, build requested {claim_id})",
            token.claim_id()
        );
    }

    // Then the DB round-trip for the freshness (content-binding) check.
    let manifest = db
        .get_publication_manifest(publication_id)
        .await
        .map_err(|e| anyhow::anyhow!("fetch publication manifest: {e}"))?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no publication manifest for `{publication_id}`; \
                 cannot verify the approval is bound to current content"
            )
        })?;

    if token.bound_digest() != manifest.content_sha3_256 {
        anyhow::bail!(
            "approval is stale: the publication content changed since it was approved \
             (approved digest {}, current {}); re-review the claim before nanopublishing",
            token.bound_digest(),
            manifest.content_sha3_256
        );
    }

    // 1. Load the claim row joined to its latest verdict. We reuse the existing
    // `list_publication_claims` op (keyed by the publication's derived session id)
    // and select the matching `claim_id`, rather than adding a narrow getter.
    let session_id = publication_session_id(publication_id);
    let claims = db
        .list_publication_claims(session_id)
        .await
        .map_err(|e| anyhow::anyhow!("list publication claims: {e}"))?;
    let claim = claims
        .into_iter()
        .find(|c| c.claim_id == claim_id)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "claim {claim_id} not found for publication `{publication_id}`; \
                 run `vox scientia publication-extract-claims --publication-id {publication_id}` first"
            )
        })?;

    // A confidence is required: it maps to `scientia:confidence` in the assertion.
    // Absent → the claim has no verdict yet.
    let confidence = claim.confidence.ok_or_else(|| {
        anyhow::anyhow!(
            "claim {claim_id} has no recorded verdict/confidence; \
             run `vox scientia publication-extract-claims --publication-id {publication_id}` first"
        )
    })?;

    // 2. Resolve (or create) the per-user RSA + ORCID signing identity.
    let user = vox_config::paths::local_user_id();
    let profile = resolve_or_create_identity(db, &user, orcid).await?;

    // 3. Assemble the enriched assertion. Mapping (design §; P1 scope):
    //   - tuple: None — the atomic claim tuple (variable_a/relation/variable_b)
    //     is NOT persisted in `scientia_claims` (only `text`); future enhancement.
    //   - verifiability: "numeric" when the claim is numeric, else "semantic".
    //   - confidence: the latest verdict's confidence (required, checked above).
    //   - novelty: "insufficient_evidence" for P1 — novelty-bundle wiring is a
    //     later phase, so no novelty verdict is available here.
    //   - prior_art_uris: empty for P1 (sourced from the novelty bundle later).
    let verifiability = if claim.is_numeric {
        "numeric"
    } else {
        "semantic"
    };
    let assertion = crate::nanopub::spec::assertion_ttl_for_claim(
        &claim.text,
        None, // tuple not persisted in scientia_claims; future enhancement.
        verifiability,
        confidence,
        "insufficient_evidence",
        &[],
    );

    // 4. Build + RSA-sign. Stamp the claim's creation time (its provenance moment).
    let signed = crate::nanopub::spec::build_and_sign(
        &assertion,
        &profile.orcid,
        claim.created_at_ms / 1000,
        &profile,
    )
    .map_err(|e| anyhow::anyhow!("build/sign nanopub: {e}"))?;

    // 5. OFFLINE validation gate. Fail hard if invalid — persist NOTHING.
    // NOTE: the upstream `check()` inside `validate_offline` prints a `✅ ... is
    // valid` line to stdout on success; a strict `--json` mode needs upstream
    // stdout suppression (no clean suppression crate is a workspace dep today).
    crate::nanopub::spec::validate_offline(&signed.trig)
        .map_err(|e| anyhow::anyhow!("offline validation failed for signed nanopub: {e}"))?;

    // 6. Persist the local, offline-validated artifact.
    let row = NanopubRow {
        trusty_uri: signed.trusty_uri.clone(),
        claim_id,
        publication_id: Some(publication_id.to_string()),
        user_id: user,
        orcid_id: Some(profile.orcid.clone()),
        trig: signed.trig.clone(),
        validated_offline: true,
        published_state: "local".to_string(),
        created_at_ms: chrono::Utc::now().timestamp_millis(),
    };
    db.insert_scientia_nanopub(&row)
        .await
        .map_err(|e| anyhow::anyhow!("persist scientia_nanopubs row: {e}"))?;

    Ok(signed)
}

/// P2 Task 4 — Record a human review decision for ONE extracted claim.
///
/// Fetches the publication's CURRENT content manifest and binds the decision to
/// its `content_sha3_256` digest. A later content edit therefore invalidates a
/// prior approval (the digest will no longer match the manifest's current value),
/// and `publication-nanopub-build` will refuse to emit until the claim is
/// re-reviewed.
///
/// # Errors
/// Returns an error if no manifest exists for `publication_id`, if
/// `db.record_review_decision` rejects the decision string, or if any DB op
/// fails.
pub async fn record_claim_review(
    db: &VoxDb,
    publication_id: &str,
    claim_id: i64,
    decision: &str,
    reason: Option<String>,
) -> anyhow::Result<vox_db::store::ReviewDecisionRow> {
    let manifest = db
        .get_publication_manifest(publication_id)
        .await
        .map_err(|e| anyhow::anyhow!("fetch publication manifest: {e}"))?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no publication manifest for `{publication_id}`; \
                 prepare the publication before reviewing its claims"
            )
        })?;

    let bound_digest = manifest.content_sha3_256;
    let actor = vox_config::paths::local_user_id();
    let now_ms = chrono::Utc::now().timestamp_millis();

    // TODO(P3): populate model_fingerprints_json from the artifact's AI-disclosure
    // metadata once that surface is wired (P2 leaves it None per spec).
    let row = vox_db::store::ReviewDecisionRow {
        claim_id,
        publication_id: publication_id.to_string(),
        bound_digest,
        decision: decision.into(),
        actor,
        reason,
        model_fingerprints_json: None,
        decided_at_ms: now_ms,
    };

    db.record_review_decision(&row)
        .await
        .map_err(|e| anyhow::anyhow!("record review decision: {e}"))?;

    Ok(row)
}

/// P2 — Bridge the DB review ledger to a minted [`ApprovalToken`].
///
/// This is the only sanctioned way for the CLI to obtain the token that
/// [`nanopub_build`] now requires. It is a thin, testable boundary so the
/// "not approved → refused" path can be exercised without the live-DB CLI arm.
///
/// 1. Look up the LATEST review decision for `claim_id`. Absent → error telling
///    the operator to run `publication-claim-review` first.
/// 2. Mint a token via [`crate::review::mint_from_decision`], which
///    returns `Some` ONLY when the latest decision is `"approved"`. Any other
///    status (rejected/deferred/edited) yields `None` → error.
///
/// [`ApprovalToken`]: crate::review::ApprovalToken
///
/// # Errors
/// Returns an error when no decision exists, when the latest decision is not
/// `"approved"`, or when the underlying DB op fails.
pub async fn approval_for(
    db: &VoxDb,
    publication_id: &str,
    claim_id: i64,
) -> anyhow::Result<crate::review::ApprovalToken> {
    let decision = db
        .latest_decision_for_claim(claim_id, publication_id)
        .await
        .map_err(|e| anyhow::anyhow!("fetch latest review decision: {e}"))?;
    let row = decision.ok_or_else(|| {
        anyhow::anyhow!(
            "claim {claim_id} has no review decision; \
             run `vox scientia publication-claim-review \
             --publication-id {publication_id} --claim-id {claim_id} \
             --decision approve` first"
        )
    })?;
    crate::review::mint_from_decision(&row).ok_or_else(|| {
        anyhow::anyhow!(
            "claim {claim_id} is not approved for nanopublication \
             (latest decision: {}); \
             run `vox scientia publication-claim-review \
             --publication-id {publication_id} --claim-id {claim_id} \
             --decision approve` first",
            row.decision
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use vox_db::{DbConfig, VoxDb};

    /// Guard: the real signing logic lives here, so this file must carry NO
    /// production-network publishing symbols (no network-publish toggle, no
    /// test-server toggle). Needles are assembled from fragments at runtime so
    /// this file cannot trip its own assertion. Mirrors the CLI guard in
    /// `crates/vox-cli/src/commands/scientia_nanopub.rs`.
    #[test]
    fn no_network_publish_symbol_in_review_flow() {
        let src = include_str!("review_flow.rs");
        let publish = format!("{}{}", "publish_to_", "network");
        let test_server = format!("{}{}", "use_test_", "server");
        assert!(!src.to_lowercase().contains(&publish));
        assert!(!src.contains(&test_server));
    }

    #[test]
    fn session_id_is_stable_and_distinct() {
        let a = publication_session_id("pub-aaa");
        assert_eq!(a, publication_session_id("pub-aaa"));
        assert_ne!(a, publication_session_id("pub-bbb"));
    }

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

    /// True only when an error indicates the sandbox vault/keyring is unavailable
    /// (so the test may skip cleanly). Any other error is a real regression and
    /// must fail CI rather than false-pass.
    fn is_sandbox_vault_unavailable(err: &anyhow::Error) -> bool {
        let msg = format!("{err:#}").to_lowercase();
        [
            "vault",
            "keyring",
            "invalid filename",
            "i/o error",
            "unavailable",
            "backend misconfigured",
            "active tokio runtime",
        ]
        .iter()
        .any(|needle| msg.contains(needle))
    }

    // A multi-threaded runtime is REQUIRED: the vox-secrets vault backend bridges
    // its async ops via `tokio::task::block_in_place`, which panics on the default
    // current-thread runtime. Without this flavor the create path always errors and
    // the test silently takes its skip branch (assertions never run).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[allow(unsafe_code)]
    // ENV_LOCK guards process-global env-var mutation; it must span the awaits to
    // serialize concurrent tests. The std Mutex<()> is never contended across a real
    // async boundary (each test runs its critical section to completion), so holding
    // it across .await is intentional and deadlock-free here.
    #[allow(clippy::await_holding_lock)]
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
                if !is_sandbox_vault_unavailable(&e) {
                    panic!("unexpected failure: {e:#}");
                }
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
                secrecy::ExposeSecret::expose_secret(&id1.rsa_private_key_b64),
                secrecy::ExposeSecret::expose_secret(&id2.rsa_private_key_b64),
                "the private key must be reused across calls, not regenerated"
            );
            assert!(
                !secrecy::ExposeSecret::expose_secret(&id1.rsa_private_key_b64).is_empty(),
                "the reused private key must be non-empty"
            );
            assert_eq!(
                id2.orcid, ORCID,
                "the ORCID supplied on the first call must persist and be reused when omitted"
            );
            assert_eq!(id2.name, USER, "the profile name must be the user_id");
        }
    }

    /// Behavior (TDD, hermetic): seed one claim + verdict via the real store ops,
    /// build a local nanopub, and assert the Trusty URI + persisted local row.
    ///
    /// Same multi-thread + temp-vault contract as the resolver test above: the
    /// vault backend bridges async via `block_in_place` (panics on a
    /// current-thread runtime), so a `multi_thread` flavor is REQUIRED. If the
    /// vault can't init in this sandbox, the create path errors and the test
    /// takes a documented skip branch — the guard test runs regardless.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[allow(unsafe_code)]
    // See note on `reuses_key_and_persists_orcid_across_calls`: ENV_LOCK must span the
    // awaits to serialize process-global env-var mutation; holding the std Mutex across
    // .await is intentional and deadlock-free in this single-critical-section test.
    #[allow(clippy::await_holding_lock)]
    async fn nanopub_build_persists_local_offline_validated_artifact() {
        let _g = ENV_LOCK.lock().expect("env lock");

        let tmp_dir = tempfile::tempdir().expect("tempdir");
        let db_path = tmp_dir.path().join("nanopub_build_vault.db");

        let prev_path = std::env::var("VOX_SECRETS_VAULT_PATH").ok();
        let prev_account = std::env::var("VOX_ACCOUNT_ID").ok();
        let prev_cutover = std::env::var("VOX_SECRETS_CUTOVER_PHASE").ok();
        unsafe {
            std::env::set_var("VOX_SECRETS_VAULT_PATH", &db_path);
            std::env::set_var("VOX_ACCOUNT_ID", "nanopub-build-test-account");
            std::env::set_var("VOX_SECRETS_CUTOVER_PHASE", "decommission");
        }

        let db = VoxDb::connect(DbConfig::Memory)
            .await
            .expect("in-memory db connect");

        const PUB_ID: &str = "pub-nanopub-build-test";
        const CLAIM_ID: u64 = 42;
        const ORCID: &str = "https://orcid.org/0000-0002-1825-0097";

        // Seed one claim + a latest verdict via the REAL store ops, keyed by the
        // same session id the build path derives from the publication id.
        let session_id = publication_session_id(PUB_ID);
        db.store_claim(
            session_id,
            CLAIM_ID,
            "mosquitoes transmit malaria",
            false,
            false,
            false,
        )
        .await
        .expect("seed claim");
        db.store_claim_verdict(CLAIM_ID, "Supported", 0.91, "mock")
            .await
            .expect("seed verdict");

        // P2 Task 3: the build path now requires a content-bound approval. Seed a
        // manifest with a known digest, record an APPROVED decision bound to that
        // exact digest, and mint the token via the DB→token bridge.
        const APPROVED_DIGEST: &str = "digest-approved-v1";
        db.upsert_publication_manifest(vox_db::store::types::PublicationManifestParams {
            publication_id: PUB_ID,
            content_type: "scientia",
            source_ref: None,
            title: "nanopub build test",
            author: "tester",
            abstract_text: None,
            body_markdown: "body",
            citations_json: None,
            metadata_json: None,
            revision_history_json: None,
            content_sha3_256: APPROVED_DIGEST,
            state: "approved",
        })
        .await
        .expect("seed manifest");
        db.record_review_decision(&vox_db::store::ReviewDecisionRow {
            claim_id: CLAIM_ID as i64,
            publication_id: PUB_ID.into(),
            bound_digest: APPROVED_DIGEST.into(),
            decision: "approved".into(),
            actor: "tester".into(),
            reason: None,
            model_fingerprints_json: None,
            decided_at_ms: 1,
        })
        .await
        .expect("seed approved decision");

        let token = approval_for(&db, PUB_ID, CLAIM_ID as i64)
            .await
            .expect("approved → token");
        let built = nanopub_build(&db, PUB_ID, CLAIM_ID as i64, Some(ORCID), &token).await;

        let outcome = match built {
            Ok(signed) => {
                let row = db
                    .get_nanopub_by_trusty_uri(&signed.trusty_uri)
                    .await
                    .expect("query persisted nanopub row");
                Some((signed, row))
            }
            Err(e) => {
                if !is_sandbox_vault_unavailable(&e) {
                    panic!("unexpected failure: {e:#}");
                }
                eprintln!(
                    "SKIP nanopub_build_persists_local_offline_validated_artifact: \
                     vault unavailable in sandbox ({e})"
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

        if let Some((signed, row)) = outcome {
            assert!(
                signed.trusty_uri.contains("RA"),
                "trusty URI must carry the RA artifact code, got: {}",
                signed.trusty_uri
            );
            let row = row.expect("a persisted scientia_nanopubs row must exist");
            assert_eq!(
                row.published_state, "local",
                "the persisted artifact must be in the `local` state (no publish)"
            );
            assert!(
                row.validated_offline,
                "the persisted artifact must be marked offline-validated"
            );
            assert_eq!(row.claim_id, CLAIM_ID as i64);
            assert_eq!(row.publication_id.as_deref(), Some(PUB_ID));
        }
    }

    /// Pure DB gate (no vault): with NO review decision on record, `approval_for`
    /// must refuse and point the operator at the review command. A caller that
    /// cannot get a token cannot even call `nanopub_build`.
    #[tokio::test]
    async fn approval_for_errors_when_no_decision() {
        let db = VoxDb::connect(DbConfig::Memory)
            .await
            .expect("in-memory db connect");
        let err = approval_for(&db, "pub-x", 999)
            .await
            .expect_err("no decision must refuse");
        let msg = err.to_string();
        assert!(
            msg.contains("no review decision"),
            "error must explain there is no decision, got: {msg}"
        );
        assert!(
            msg.contains("publication-claim-review"),
            "error must point at the review command, got: {msg}"
        );
    }

    /// Pure DB gate (no vault): when the LATEST decision is `rejected` (recorded
    /// after an earlier `approved`), the rejection wins by `decided_at_ms` and
    /// `mint_from_decision` returns `None`, so `approval_for` must refuse.
    #[tokio::test]
    async fn approval_for_errors_when_latest_is_rejected() {
        let db = VoxDb::connect(DbConfig::Memory)
            .await
            .expect("in-memory db connect");

        const CLAIM: i64 = 7;
        db.record_review_decision(&vox_db::store::ReviewDecisionRow {
            claim_id: CLAIM,
            publication_id: "pub-x".into(),
            bound_digest: "digest-v1".into(),
            decision: "approved".into(),
            actor: "alice".into(),
            reason: None,
            model_fingerprints_json: None,
            decided_at_ms: 1,
        })
        .await
        .expect("seed approved");
        db.record_review_decision(&vox_db::store::ReviewDecisionRow {
            claim_id: CLAIM,
            publication_id: "pub-x".into(),
            bound_digest: "digest-v1".into(),
            decision: "rejected".into(),
            actor: "bob".into(),
            reason: Some("on reflection, not novel".into()),
            model_fingerprints_json: None,
            decided_at_ms: 2, // later → wins
        })
        .await
        .expect("seed later rejection");

        let err = approval_for(&db, "pub-x", CLAIM)
            .await
            .expect_err("latest rejected must refuse");
        let msg = err.to_string();
        assert!(
            msg.contains("not approved"),
            "error must say the claim is not approved, got: {msg}"
        );
        assert!(
            msg.contains("rejected"),
            "error must surface the latest decision value, got: {msg}"
        );
    }

    /// P2 Task 4 — Pure DB gate (no vault): `record_claim_review` fetches the
    /// current publication manifest's `content_sha3_256` as the bound_digest,
    /// writes the row, and returns it. The `latest_decision_for_claim` op must
    /// then surface the same row (end-to-end round-trip in the DB layer).
    #[tokio::test]
    async fn record_claim_review_persists_digest_bound_decision() {
        let db = VoxDb::connect(DbConfig::Memory)
            .await
            .expect("in-memory db connect");

        const PUB_ID: &str = "pub-rev";
        const CLAIM: i64 = 7;
        const DIGEST: &str = "digest-rev-1";

        db.upsert_publication_manifest(vox_db::store::types::PublicationManifestParams {
            publication_id: PUB_ID,
            content_type: "scientia",
            source_ref: None,
            title: "review test pub",
            author: "tester",
            abstract_text: None,
            body_markdown: "body text",
            citations_json: None,
            metadata_json: None,
            revision_history_json: None,
            content_sha3_256: DIGEST,
            state: "draft",
        })
        .await
        .expect("seed manifest");

        let row = record_claim_review(&db, PUB_ID, CLAIM, "approved", Some("looks solid".into()))
            .await
            .expect("record_claim_review must succeed");

        assert_eq!(
            row.bound_digest, DIGEST,
            "bound_digest must equal the manifest's content_sha3_256"
        );
        assert_eq!(row.decision, "approved", "decision must be persisted");
        assert!(
            !row.actor.is_empty(),
            "actor must be non-empty (local_user_id)"
        );
        assert_eq!(
            row.reason.as_deref(),
            Some("looks solid"),
            "reason must be stored"
        );
        assert_eq!(row.claim_id, CLAIM, "claim_id must be preserved");

        // Round-trip: the DB must surface the same row via `latest_decision_for_claim`.
        let fetched = db
            .latest_decision_for_claim(CLAIM, PUB_ID)
            .await
            .expect("query latest decision")
            .expect("a decision must exist after record_claim_review");
        assert_eq!(fetched.bound_digest, DIGEST);
        assert_eq!(fetched.decision, "approved");
    }

    /// P2 Task 4 — `record_claim_review` must fail with a descriptive error
    /// when no manifest exists for the given publication_id.
    #[tokio::test]
    async fn record_claim_review_errors_without_manifest() {
        let db = VoxDb::connect(DbConfig::Memory)
            .await
            .expect("in-memory db connect");

        let err = record_claim_review(&db, "no-such-pub", 1, "approved", None)
            .await
            .expect_err("missing manifest must error");
        let msg = err.to_string();
        assert!(
            msg.contains("no publication manifest"),
            "error must mention 'no publication manifest', got: {msg}"
        );
    }

    /// P2 Task 4 — Integration compose: after `record_claim_review(..., "approved", None)`,
    /// `approval_for` must succeed and the returned token's `bound_digest()` must
    /// equal the manifest's digest. Proves Task 4 → Task 3 composition.
    #[tokio::test]
    async fn approval_for_succeeds_after_record_claim_review() {
        let db = VoxDb::connect(DbConfig::Memory)
            .await
            .expect("in-memory db connect");

        const PUB_ID: &str = "pub-compose-test";
        const CLAIM: i64 = 99;
        const DIGEST: &str = "digest-compose-v1";

        db.upsert_publication_manifest(vox_db::store::types::PublicationManifestParams {
            publication_id: PUB_ID,
            content_type: "scientia",
            source_ref: None,
            title: "compose test",
            author: "tester",
            abstract_text: None,
            body_markdown: "body",
            citations_json: None,
            metadata_json: None,
            revision_history_json: None,
            content_sha3_256: DIGEST,
            state: "draft",
        })
        .await
        .expect("seed manifest");

        record_claim_review(&db, PUB_ID, CLAIM, "approved", None)
            .await
            .expect("record_claim_review must succeed");

        let token = approval_for(&db, PUB_ID, CLAIM)
            .await
            .expect("approval_for must succeed after an approved decision");

        assert_eq!(
            token.bound_digest(),
            DIGEST,
            "token bound_digest must equal the manifest's content_sha3_256"
        );
        assert_eq!(token.claim_id(), CLAIM, "token claim_id must match");
    }

    /// Freshness gate (no vault needed — the digest check fires BEFORE any
    /// signing/persisting): the manifest's CURRENT digest differs from the
    /// approved (now-stale) digest, simulating an edit after approval. The build
    /// must refuse and persist NOTHING.
    #[tokio::test]
    async fn nanopub_build_refuses_stale_approval_persists_nothing() {
        let db = VoxDb::connect(DbConfig::Memory)
            .await
            .expect("in-memory db connect");

        const PUB_ID: &str = "pub-stale-approval-test";
        const CLAIM: i64 = 314;

        // Current content digest (post-edit).
        db.upsert_publication_manifest(vox_db::store::types::PublicationManifestParams {
            publication_id: PUB_ID,
            content_type: "scientia",
            source_ref: None,
            title: "stale approval test",
            author: "tester",
            abstract_text: None,
            body_markdown: "edited body",
            citations_json: None,
            metadata_json: None,
            revision_history_json: None,
            content_sha3_256: "digest-CURRENT",
            state: "approved",
        })
        .await
        .expect("seed manifest");

        // The human approved the OLD digest — the content has since changed.
        db.record_review_decision(&vox_db::store::ReviewDecisionRow {
            claim_id: CLAIM,
            publication_id: PUB_ID.into(),
            bound_digest: "digest-OLD".into(),
            decision: "approved".into(),
            actor: "tester".into(),
            reason: None,
            model_fingerprints_json: None,
            decided_at_ms: 1,
        })
        .await
        .expect("seed stale approval");

        let token = approval_for(&db, PUB_ID, CLAIM)
            .await
            .expect("approved (stale) → token still mints");

        // `SignedNanopubDoc` is not `Debug`, so match rather than `expect_err`.
        let err = match nanopub_build(
            &db,
            PUB_ID,
            CLAIM,
            Some("https://orcid.org/0000-0002-1825-0097"),
            &token,
        )
        .await
        {
            Ok(_) => panic!("stale approval must be refused, but build succeeded"),
            Err(e) => e,
        };
        let msg = err.to_string();
        assert!(
            msg.contains("stale") || msg.contains("changed"),
            "error must explain the content changed since approval, got: {msg}"
        );

        // Prove NO `scientia_nanopubs` row was persisted for this claim — the
        // gate fired before any persistence. Uses a typed count op (not raw
        // query_all) so the test stays within the Codex query surface.
        let count = db
            .count_scientia_nanopubs_for_claim(CLAIM)
            .await
            .expect("count nanopub rows");
        assert_eq!(count, 0, "a refused stale approval must persist nothing");
    }

    /// Cross-publication replay gate (no vault — the publication-identity check
    /// fires BEFORE any signing/persisting): a token minted for "pub-A" must not
    /// build "pub-B", EVEN when the two publications share the same content
    /// digest (so the freshness/digest check alone would pass). The build must
    /// refuse with a "publication mismatch" and persist NOTHING.
    #[tokio::test]
    async fn nanopub_build_refuses_cross_publication_token_persists_nothing() {
        let db = VoxDb::connect(DbConfig::Memory)
            .await
            .expect("in-memory db connect");

        const CLAIM: i64 = 271;
        const SHARED_DIGEST: &str = "dig";
        const ORCID: &str = "https://orcid.org/0000-0002-1825-0097";

        // pub-A: manifest + an approved decision bound to the shared digest.
        db.upsert_publication_manifest(vox_db::store::types::PublicationManifestParams {
            publication_id: "pub-A",
            content_type: "scientia",
            source_ref: None,
            title: "pub A",
            author: "tester",
            abstract_text: None,
            body_markdown: "body",
            citations_json: None,
            metadata_json: None,
            revision_history_json: None,
            content_sha3_256: SHARED_DIGEST,
            state: "approved",
        })
        .await
        .expect("seed manifest A");
        db.record_review_decision(&vox_db::store::ReviewDecisionRow {
            claim_id: CLAIM,
            publication_id: "pub-A".into(),
            bound_digest: SHARED_DIGEST.into(),
            decision: "approved".into(),
            actor: "tester".into(),
            reason: None,
            model_fingerprints_json: None,
            decided_at_ms: 1,
        })
        .await
        .expect("seed approved decision for pub-A");

        // Token is scoped to pub-A.
        let token = approval_for(&db, "pub-A", CLAIM)
            .await
            .expect("approved (pub-A) → token");

        // pub-B shares the SAME content digest, so the freshness check would pass.
        db.upsert_publication_manifest(vox_db::store::types::PublicationManifestParams {
            publication_id: "pub-B",
            content_type: "scientia",
            source_ref: None,
            title: "pub B",
            author: "tester",
            abstract_text: None,
            body_markdown: "body",
            citations_json: None,
            metadata_json: None,
            revision_history_json: None,
            content_sha3_256: SHARED_DIGEST,
            state: "approved",
        })
        .await
        .expect("seed manifest B");

        // `SignedNanopubDoc` is not `Debug`, so match rather than `expect_err`.
        let err = match nanopub_build(&db, "pub-B", CLAIM, Some(ORCID), &token).await {
            Ok(_) => panic!("cross-publication token must be refused, but build succeeded"),
            Err(e) => e,
        };
        let msg = err.to_string();
        assert!(
            msg.contains("publication mismatch"),
            "error must explain the publication mismatch, got: {msg}"
        );

        // The gate fired before any persistence — nothing for this claim.
        let count = db
            .count_scientia_nanopubs_for_claim(CLAIM)
            .await
            .expect("count nanopub rows");
        assert_eq!(
            count, 0,
            "a refused cross-publication token must persist nothing"
        );
    }
}
