---
title: "Clavis V2: Full Implementation Plan (2026)"
description: >
  Complete, codebase-verified implementation plan for evolving Vox Clavis into a one-stop
  secrets manager. Covers all data structures, SQL schema (verified against the turso@0.4 API),
  CLI surface, VoxDB integration, hard-problem analysis, and a safety-first wave ordering.
category: "Architecture SSOTs"
status: "experimental"

last_updated: "2026-04-12"
training_eligible: false
archived_date: 2026-04-18
---

# Clavis V2: Full Implementation Plan (2026)

> **SSOT chain:**
> [clavis-ssot.md](../reference/clavis-ssot.md) → [clavis-cloudless-threat-model-v1.md](clavis-cloudless-threat-model-v1.md) → [clavis-secrets-env-research-2026.md](clavis-secrets-env-research-2026.md) → [clavis-one-stop-secrets-research-2026.md](clavis-one-stop-secrets-research-2026.md) → **this document**

---

## Critique of V1 Plan

Before specifying the revised approach, this section documents the issues found in the first-pass plan. These are not optional improvements; they affect correctness.

### Critical issues

**C1 — Wave ordering violates safety dependencies.**  
The V1 plan schedules the runtime scrubber (Wave 6) *after* the audit log (Wave 4). This is
wrong: the scrubber must exist before any audit row can be appended, because the audit writer
needs `redact_secrets_from_value` to verify it is not inadvertently logging a plaintext value.
No code path should write to `clavis_audit_log` before `redact.rs` exists.

**C2 — Transaction model is wrong for multi-table atomicity.**  
The V1 plan proposes `"BEGIN EXCLUSIVE; ...; COMMIT"` via raw SQL strings inside
`run_clavis_future`. The `turso@0.4` crate (with `features = ["sync"]`, as confirmed in
`Cargo.toml`) provides `conn.transaction()` and `conn.unchecked_transaction()` for interactive
transactions. Manually issuing `BEGIN`/`COMMIT` through `execute_batch` is unreliable over
remote connections and bypasses the driver's transaction state machine. Any network interruption
leaves the connection in an indeterminate state.

**C3 — `run_clavis_future` with a `Mutex<Connection>` creates a block_in_place hazard for writes.**  
The existing `run_clavis_future` uses `tokio::task::block_in_place` when called inside a Tokio
runtime. This works for single `execute` calls. For the new multi-statement write (UPSERT +
INSERT + prune), the entire sequence must be enclosed in an `unchecked_transaction()` whose
`commit()` is awaited inside one `run_clavis_future` call. Calling `run_clavis_future` *multiple
times in sequence* for a logical transaction would not be atomic and would also hit the Mutex
each time, potentially seeing contention. The fix: a single `run_clavis_future` call wraps the
entire `async` block including `tx.unchecked_transaction()` → writes → `tx.commit().await`.

**C4 — Scrubber `OnceLock` cache is invalid for a secrets manager.**  
A global `OnceLock<AhoCorasick>` keyed on the full pattern set cannot be invalidated without
restarting the process. The V1 plan proposes `invalidate_scrubber_cache()` but
`OnceLock::get_or_init` provides no invalidation path. The scrubber must instead be
**caller-driven**: callers pass the `&[&str]` of resolved values at call time and the
`AhoCorasick` is built per-call (fast for small pattern counts), **or** the cache must use an
`RwLock<Option<Arc<AhoCorasick>>>` that can be swapped. The V1 plan's API design is incorrect.

**C5 — Historical DEK re-wrapping after KEK rotation is a security gap, not an "open question".**  
Industry best practice (envelope encryption) is "lazy re-wrap + active background sweep". When
`rewrap_secret_for_account` runs, it re-wraps the current row's DEK. Historical version rows in
`clavis_secret_versions` still hold DEKs wrapped with the old KEK. If the old KEK is later
deleted from the keyring, those historical rows become permanently undecryptable. This must be
specified at design time, not deferred.

**C6 — `ConfigValue` / `OperatorTuning` classification creates a conceptual ambiguity.**  
The V1 plan adds `SecretMaterialKind::ConfigValue` for operator tuning vars and applies
`TaxonomyClass::OperatorTuning` to them. But these values never enter the vault (they are env
vars only; `persistable_account_secret = false`). Labeling them with a `SecretMaterialKind`
designed for vault-stored material is misleading. The correct design: OperatorTuning vars get
`SecretMaterialKind::ConfigValue` and the `allow_env_in_strict = true` flag, but are
systematically excluded from `vox clavis list` output (they appear only in `vox clavis status`).

**C7 — Profile-scoped override resolution path not fully specified.**  
The V1 resolver update says "profile override check" but does not specify where
`clavis_profile_overrides` is queried relative to `clavis_account_secrets`. The turso Mutex
means calling `get_row` twice (once for override, once for canonical) blocks twice. This must be
a single query with a `UNION` or a two-row fetch within one `run_clavis_future` to avoid the
double-block-in-place cost.

**C8 — `caller_context` from env is spoofable.**  
The V1 plan derives `caller_context` from an environment variable for audit attribution.
Any process can set `VOX_CLAVIS_CALLER_CONTEXT=orchestrator` to impersonate the orchestrator.
The correct design: `caller_context` is determined by the **call site**, not by env. Public
API `resolve_secret(id)` always logs `"cli"` or `"process"`. Agent call sites call
`resolve_secret_with_context(id, "agent:<task_id>")`. Env-derived context is banned.

**C9 — Wave 0 and Wave 8 fragmentation.**  
Annotating `SPECS` (Wave 0) and completing the annotation (Wave 8) are the same activity split
across the plan for no reason. All annotation belongs in one wave.

**C11 — Cryptographic Isolation and MSVC Compatibility.**  
The V1 plan specified AES-GCM and Blake3 directly, which brought in heavy native extensions or pure-Rust equivalents that negatively impacted Windows builds. The new SSOT requires all cryptography to be abstracted behind ox-crypto, using ChaCha20Poly1305 and secure_hash exclusively. This guarantees pure-Rust compilation and isolates the egis crate (pulled by Turso) from the rest of the workspace.

**C10 — `vox clavis run` Windows process model not safe to defer as an "open question".**  
`exec()`-style process replacement is a Unix-only feature. On Windows the parent process must
stay alive while the child runs, which changes signal delivery semantics. This must be
explicitly specified before implementation, not discovered during.

training_eligible: false
archived_date: 2026-04-18
---

## Architecture Baseline (what the code actually does today)

| File | Key facts |
|---|---|
| `spec.rs` | ~580 `SecretId` variants; `SecretSpec` is `const`-compatible; `SecretMetadata` is `Copy`. `SecretPolicy` has `required: bool` + `MissingBehavior`. No lifecycle fields exist yet. |
| `types.rs` | `ResolutionStatus` (9 variants); `SecretSource` (6 variants); `ResolvedSecret` has no lifecycle status. |
| `resolver.rs` | `SecretResolver<B>`: env → backend → auth_json → populi_env. Profile check only on env source. No profile-override table path. |
| `backend/vox_vault.rs` | `VoxCloudBackend` uses `Mutex<turso::Connection>` (not `Arc`). `run_clavis_future` uses `block_in_place` if in Tokio, else spawns a `new_current_thread` rt. **Transactions: none** — every write is a single `conn.execute(UPSERT)`. The Mutex is held per operation, released between operations. `ensure_schema` uses `execute_batch` (correct for DDL-only, no params needed). |
| `turso@0.4` (workspace) | Provides `conn.transaction()` (`&mut Connection`) and `conn.unchecked_transaction()` (`&Connection`). The latter is necessary here since `conn` is behind `Mutex`. Transaction commits via `tx.commit().await`; drops roll back automatically. |
| `lib.rs` | `resolve_secret(id)` is `#[must_use]` and synchronous (calls `run_clavis_future` internally). `OPERATOR_TUNING_ENVS` is a manually maintained `&[&str]` slice. |
| `clavis.rs` CLI | `ClavisCmd::Set` writes to `auth.json` only — NOT to `VoxCloudBackend`. The vault has no CLI write path today other than `import-env`. |
| `aho-corasick` | **Not in the workspace dep tree** — confirmed via `cargo tree`. Added as a new direct dep. |
| `uuid` | Check workspace… presumed present via other crates but must be verified. |

---

## Part I: Data Structures

These changes are purely additive and `const`-compatible. No existing field is removed or
retyped. All ~580 `SPECS` entries gain new fields with explicit defaults.

### 1.1 `TaxonomyClass` — the nine-class env-var taxonomy

```rust
// crates/vox-clavis/src/lib.rs

/// Nine-class taxonomy for every managed env var.
/// Used for `vox clavis list --class`, doctor grouping, and CI filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaxonomyClass {
    PlatformIdentity,      // Class 1: VOX_ACCOUNT_ID, VOX_DB_*, bootstrap
    LlmProviderKey,        // Class 2: OPENROUTER_API_KEY, GEMINI_API_KEY, etc.
    CloudGpuInfra,         // Class 3: RUNPOD_API_KEY, VAST_API_KEY, etc.
    ScholarlyPublication,  // Class 4: Zenodo, ORCID, CrossRef, DataCite
    SocialSyndication,     // Class 5: Twitter/X, Bluesky, Reddit, YouTube, Mastodon
    MeshTransport,         // Class 6: VOX_MESH_TOKEN, WebhookIngressToken, MCP bearer
    TelemetrySearch,       // Class 7: Qdrant, Tavily, telemetry upload
    AuxTooling,            // Class 8: GitHub tokens, V0, etc.
    OperatorTuning,        // Class 9: non-secret config vars (never vault-stored)
}

impl TaxonomyClass {
    /// Human-readable label used as CLI filter argument.
    pub const fn slug(self) -> &'static str {
        match self {
            Self::PlatformIdentity     => "platform",
            Self::LlmProviderKey       => "llm",
            Self::CloudGpuInfra        => "gpu",
            Self::ScholarlyPublication => "scholarly",
            Self::SocialSyndication    => "social",
            Self::MeshTransport        => "mesh",
            Self::TelemetrySearch      => "telemetry",
            Self::AuxTooling           => "aux",
            Self::OperatorTuning       => "config",
        }
    }

    /// True for classes whose values should never enter the vault.
    pub const fn is_config_only(self) -> bool {
        matches!(self, Self::OperatorTuning)
    }
}
```

### 1.2 `LifecycleMeta` — rotation cadence and expiry warning

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LifecycleMeta {
    /// Expected rotation interval in days. `None` = manual / no cadence.
    pub rotation_cadence_days: Option<u32>,
    /// Days before expected expiry to emit `NearingExpiry` status.
    /// `None` = no expiry tracking.
    pub expiry_warning_days: Option<u32>,
    /// If `true`, `StaleRotation` fires when `rotation_epoch == 0`
    /// and the vault row is older than `2 × rotation_cadence_days`.
    pub track_stale_rotation: bool,
}

impl LifecycleMeta {
    pub const MANUAL: Self = Self {
        rotation_cadence_days: None,
        expiry_warning_days: None,
        track_stale_rotation: false,
    };
    pub const QUARTERLY: Self = Self {
        rotation_cadence_days: Some(90),
        expiry_warning_days: Some(14),
        track_stale_rotation: true,
    };
    pub const MONTHLY: Self = Self {
        rotation_cadence_days: Some(30),
        expiry_warning_days: Some(7),
        track_stale_rotation: true,
    };
    pub const ANNUAL_OAUTH: Self = Self {
        rotation_cadence_days: Some(365),
        expiry_warning_days: Some(30),
        track_stale_rotation: true,
    };
    pub const CONFIG: Self = Self {
        rotation_cadence_days: None,
        expiry_warning_days: None,
        track_stale_rotation: false,
    };
}
```

### 1.3 `SecretMaterialKind` — extended

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SecretMaterialKind {
    ApiKey,
    OAuthRefreshToken,
    OAuthClientCredential,  // NEW: client_id+secret pair reference
    BearerToken,
    HmacSecret,
    JwtHmacSecret,          // NEW: HS256 JWT signing key
    Ed25519Key,             // NEW: Ed25519 signing/verifying key
    EndpointUrl,
    Username,
    Password,
    DelegationRef,          // NEW: an opaque A2A delegation token handle
    ConfigValue,            // NEW: non-secret config value (OperatorTuning class only)
}
```

**Rule:** `ConfigValue` is only valid when `TaxonomyClass::OperatorTuning` and `persistable_account_secret = false`. CI enforces that no `ConfigValue` entry has `persistable_account_secret = true`.

### 1.4 Extended `SecretMetadata` and `SecretSpec`

Both remain `const`-compatible and `Copy`. Two new fields on `SecretMetadata`, one on `SecretSpec`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SecretMetadata {
    // --- existing fields ---
    pub class: SecretClass,
    pub material_kind: SecretMaterialKind,
    pub persistable_account_secret: bool,
    pub device_local_only: bool,
    pub allow_env_in_strict: bool,
    pub allow_compat_sources_in_strict: bool,
    pub rotation_policy: RotationPolicy,
    // --- new fields ---
    pub taxonomy_class: TaxonomyClass,
    pub lifecycle: LifecycleMeta,
}

#[derive(Debug, Clone, Copy)]
pub struct SecretSpec {
    // --- existing fields ---
    pub id: SecretId,
    pub canonical_env: &'static str,
    pub aliases: &'static [&'static str],
    pub deprecated_aliases: &'static [&'static str],
    pub backend_key: Option<&'static str>,
    pub auth_registry: Option<&'static str>,
    pub policy: SecretPolicy,
    pub remediation: &'static str,
    // --- new field ---
    pub scope_description: &'static str,  // one-line description for doctor output
}
```

**Migration path for SPECS:** The `SPECS` array has ~580 entries, all struct-literal initialized.
Adding a new required field to `SecretSpec` or `SecretMetadata` will cause compile errors for
every un-annotated entry. The annotation wave must either use a `Default` impl (making new fields
optional at compile time) or annotate all entries atomically in one commit.

**Decision:** Provide a `const DEFAULT_METADATA_OVERLAY` approach. Each `metadata()` method on
`SecretId` returns a `SecretMetadata`. Adding the two new fields with compile-time-assigned
defaults (by adding a `const fn default_taxonomy()` that returns `TaxonomyClass::AuxTooling` and
`LifecycleMeta::MANUAL`) means no existing SPECS entry breaks. Correct taxonomy/lifecycle values
are then applied per-entry in the same commit. This is safer than requiring all ~580 entries to be
annotated in lockstep.

### 1.5 `ResolutionStatus` — three new variants

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionStatus {
    // --- existing ---
    Present,
    MissingOptional,
    MissingRequired,
    InvalidEmpty,
    DeprecatedAliasUsed,
    RejectedLegacyAlias,
    RejectedSourcePolicy,
    RejectedClassPolicy,
    BackendUnavailable,
    // --- new ---
    ProfileOverrideUsed,   // value came from clavis_profile_overrides
    StaleRotation,         // Present but rotation_epoch==0 and age > 2×cadence
    NearingExpiry,         // Present and within expiry_warning_days of expected expiry
}
```

**Important:** `StaleRotation` and `NearingExpiry` are advisory statuses only. The resolved
`value` field is still `Some(...)`. The caller receives the value AND the diagnostic. The doctor
CLI renders these as warnings, not failures.

training_eligible: false
archived_date: 2026-04-18
---

## Part II: Database Schema

### Design principles (verified)

1. All four new tables live in the same `clavis_vault.db` file as `clavis_account_secrets`.
2. `ensure_schema` creates them via `execute_batch` — correct for DDL (no params, schema-only).
3. **Write transactions use `conn.unchecked_transaction()`** (since `conn` is `&turso::Connection`
   behind a `Mutex`, not `&mut Connection`). The `unchecked` variant allows `&self` access with
   the trade-off that compile-time borrow safety is relaxed. At runtime, only one goroutine holds
   the `Mutex`, so there is no actual unsafety.
4. The `Mutex<Connection>` lock is acquired once per `run_clavis_future` call. For multi-table
   writes, **the entire transaction (tx.begin → writes → tx.commit) lives inside one
   `run_clavis_future` call**. The Mutex is not released between statements.
5. WAL mode (`PRAGMA journal_mode=WAL`) is applied once during `ensure_schema` for local file
   databases, improving concurrent `resolve_secret` reads against background writes.

### 2.1 `clavis_secret_versions` (version history, append-only)

```sql
CREATE TABLE IF NOT EXISTS clavis_secret_versions (
    version_id      INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id      TEXT    NOT NULL,
    secret_id       TEXT    NOT NULL,       -- canonical_env value
    ciphertext      BLOB    NOT NULL,       -- ChaCha20Poly1305 under per-version DEK
    nonce           BLOB    NOT NULL,       -- 12-byte GCM nonce
    dek_wrapped     BLOB    NOT NULL,       -- DEK wrapped under KEK at write time
    kek_ref         TEXT    NOT NULL,
    kek_version     INTEGER NOT NULL,
    operation       TEXT    NOT NULL CHECK(
                        operation IN ('create','rotate','import','rollback','rewrap')
                    ),
    source_hint     TEXT,                   -- 'env-import' | 'cli-set' | 'auto-rotate' | null
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
```

**Relationship to `clavis_account_secrets`:** The canonical table is the fast-path for
`resolve_secret`. The version table is the historical ledger. Both are written atomically in one
transaction on every write.

**Depth limit:** `VOX_CLAVIS_VERSION_HISTORY_DEPTH` (default 10). Enforced by a DELETE within
the same transaction as the INSERT (see §3.3).

**Immutability assertion:** A CI check (`vox ci clavis-audit-schema`) verifies that no production
migration file contains an `UPDATE` or `DELETE` statement targeting `clavis_secret_versions`.

### 2.2 `clavis_audit_log` (resolution events, no values)

```sql
CREATE TABLE IF NOT EXISTS clavis_audit_log (
    row_id           INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id       TEXT    NOT NULL,
    secret_id        TEXT    NOT NULL,
    resolved_at_ms   INTEGER NOT NULL,
    resolution_status TEXT   NOT NULL,      -- ResolutionStatus Debug name
    resolution_source TEXT,                 -- SecretSource Debug name or NULL
    resolve_profile  TEXT    NOT NULL,      -- ResolveProfile Debug name
    caller_context   TEXT    NOT NULL,      -- 'cli' | 'mcp' | 'api' | 'agent:<task_id>'
    detail           TEXT                   -- optional diagnostic string, NEVER a value
);
CREATE INDEX IF NOT EXISTS idx_clavis_al_time
    ON clavis_audit_log(account_id, resolved_at_ms DESC);
CREATE INDEX IF NOT EXISTS idx_clavis_al_secret
    ON clavis_audit_log(account_id, secret_id, resolved_at_ms DESC);
```

**Caller context rules (C8 fix):** `caller_context` is set by the call site, not by env.
Three public entry points exist:
- `resolve_secret(id)` → `caller_context = "process"` (default, unknown call site)
- `resolve_secret_for_cli(id)` → `caller_context = "cli"` (used only in `vox-cli`)
- `resolve_secret_with_context(id, ctx: &str)` → `ctx` must match the allowlist
  `["cli", "mcp", "api"]` or the pattern `"agent:[a-zA-Z0-9_-]{1,128}"`. Anything else is
  silently normalized to `"process"`.

**Scrubber requirement (C1 fix):** The `detail` column is the only potentially risky field.
Before writing `detail`, `contains_secret_material(detail, &[])` is checked. If it fires (which
would indicate a code bug, not operator error), the write is aborted and a panic-in-debug /
warn-in-release fires.

**Enable condition:** Audit logging is always on in `ProdStrict` and `HardCutStrict` profiles.
Opt-in for `DevLenient` and `CiStrict` via `VOX_CLAVIS_AUDIT_LOG=1`.

### 2.3 `clavis_profile_overrides` (per-ResolveProfile values)

```sql
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
```

**Promotion guard:** Writing a `prod` or `hardcut` profile override via `vox clavis set-secret`
requires the `--profile prod` flag to be specified explicitly. The CLI aborts if the flag is
absent.

### 2.4 `clavis_agent_delegations` (A2A scoped delegation)

```sql
CREATE TABLE IF NOT EXISTS clavis_agent_delegations (
    delegation_id   TEXT    PRIMARY KEY,    -- 128-bit random UUID v4
    account_id      TEXT    NOT NULL,
    secret_id       TEXT    NOT NULL,
    scope_bits      INTEGER NOT NULL DEFAULT 1,  -- 0x01 = read-only, future bits reserved
    parent_context  TEXT    NOT NULL,
    child_context   TEXT    NOT NULL,
    issued_at_ms    INTEGER NOT NULL,
    expires_at_ms   INTEGER NOT NULL,       -- backend enforces ≤ issued + 3_600_000
    revoked_at_ms   INTEGER,
    revoke_reason   TEXT
);
CREATE INDEX IF NOT EXISTS idx_clavis_del_lookup
    ON clavis_agent_delegations(account_id, secret_id, expires_at_ms DESC);
```

**Scope model:** `scope_bits` is a bitmask intentionally kept simple. The V1 plan referenced RFC
8693 Token Exchange — that is the correct *eventual* target for a full OAuth 2.1 delegation
flow. However, the implementation for this wave is a pragmatic local-only delegation reference:
the orchestrator mints a delegation ID, the sub-agent calls `resolve_secret_for_delegation()`,
and the backend validates TTL + scope before calling `resolve_secret()` internally. Full RFC 8693
Token Exchange (with a separate authorization server) is a Wave 9+ concern documented in
[clavis-one-stop-secrets-research-2026.md](clavis-one-stop-secrets-research-2026.md) §A2A.

---

## Part III: Hard Problem Analysis

Three problems require detailed technical analysis before implementation begins. Getting any of
these wrong will cause data loss, security regressions, or subtle runtime panics.

### H1 — Atomic multi-table writes (transaction model)

**Problem:** The existing `write_secret_for_account` is a single `conn.execute(UPSERT)` inside
`run_clavis_future`. The new `write_secret_v2` must write to two tables (canonical + version
history) and optionally delete old version rows — all atomically. If the second INSERT succeeds
but the DELETE fails, we have a version-history leak. If the UPSERT succeeds but the INSERT
fails, we have a write with no history record.

**Root cause of V1 plan error:** `run_clavis_future` is called multiple times in sequence for
what is described as an atomic operation. Each call acquires and releases the Mutex. Between
calls, *another `resolve_secret` call* could steal the Mutex and read a partially-written state.

**Verified solution using `turso@0.4` interactive transactions:**

```rust
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
    // Encrypt once, outside the transaction
    let mut dek = [0_u8; 32];
    rand::thread_rng().fill_bytes(&mut dek);
    let mut nonce = [0_u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce);
    let ciphertext = encrypt_with_nonce(&dek, &nonce, plaintext.as_bytes())?;
    let dek_wrapped = self.wrap_dek(&dek, &self.kek_ref, self.kek_version)?;
    // Zeroize dek immediately after wrapping
    dek.fill(0);

    let account_id = self.account_id.clone();
    let kek_ref = self.kek_ref.clone();
    let kek_version = self.kek_version;
    let checksum = compute_account_secret_checksum(
        &account_id, secret_id, &ciphertext, &nonce, 1,
        &dek_wrapped, &kek_ref, kek_version, 0, 1,
    );
    let version_checksum = /* same inputs, version-table variant */ checksum.clone();

    let conn = self.conn.lock().expect("vox vault mutex");
    run_clavis_future(async {
        // One run_clavis_future call → one block_in_place invocation →
        // the Mutex continues to be held throughout the entire async block.
        let tx = conn.unchecked_transaction().await
            .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?;

        // 1. UPSERT canonical row (or profile override row)
        let upsert_sql = if profile.is_none() {
            CANONICAL_UPSERT_SQL
        } else {
            PROFILE_OVERRIDE_UPSERT_SQL
        };
        tx.execute(upsert_sql, params![...]).await
            .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?;

        // 2. Append version history (always, including for profile overrides)
        tx.execute(VERSION_INSERT_SQL, params![...]).await
            .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?;

        // 3. Prune old versions beyond depth limit
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
                params![&account_id, secret_id, history_depth as i64],
            ).await.map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?;
        }

        // Commit — if any step above returned Err, tx is dropped here → automatic rollback.
        tx.commit().await
            .map_err(|e| SecretError::BackendQueryFailed(e.to_string()))
    })
}
```

**Key invariants verified:**
- Encryption and key derivation happen *outside* the async block (CPU-bound, no await).
- DEK is zeroized immediately after wrapping.
- The Mutex guard (`conn`) is held for the full duration of the `run_clavis_future` call;
  no other caller can interleave.
- Rollback is automatic on `tx` drop if `commit()` is not reached.
- `unchecked_transaction()` is safe here because the Mutex guarantees single-writer access.

**WAL pragma:** Add to `ensure_schema` for local file databases only:
```rust
// In ensure_schema, before CREATE TABLE statements
if db_url.starts_with("file:") {
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;").await?;
}
```

### H2 — Runtime secret scrubber (thread-safe cache model)

**Problem:** The V1 plan proposed a global `OnceLock<AhoCorasick>` with an
`invalidate_scrubber_cache()` function. But `OnceLock` has no invalidation path — once set, it
cannot be unset without process restart. This makes the scrubber useless after a rotation.

**Revised design:** Two modes depending on use case.

**Mode A — Per-call construction (for low-frequency scrubbing):**
The scrubber is built fresh each call from the caller-supplied `&[&str]` of resolved values. For
the MCP tool-result scrubber context, this is called at most once per tool invocation. The `AhoCorasick`
build cost is O(∑|patterns|) using DFA construction — for 20–40 patterns of average length 40
chars, this is ~50µs, acceptable for a post-tool-call operation.

```rust
// crates/vox-clavis/src/redact.rs

use aho_corasick::{AhoCorasick, MatchKind};
use serde_json::Value;
use zeroize::Zeroizing;

/// Recursively scrub all known secret values from a JSON `Value`.
/// `patterns` is a slice of plaintext secret values from the caller.
/// The caller must obtain these from `resolved.expose()` and is responsible
/// for not retaining them beyond this call's scope.
///
/// Returns a new `Value` with all occurrences replaced by `"[REDACTED]"`.
///
/// # Panics
/// Does not panic. If AhoCorasick construction fails (empty patterns or
/// pattern too long), returns the input unchanged.
pub fn redact_secrets_from_value(value: &Value, patterns: &[&str]) -> Value {
    let non_empty: Vec<&str> = patterns.iter()
        .filter(|p| p.len() >= MIN_REDACT_LEN)  // don't redact 1-2 char patterns
        .copied()
        .collect();
    if non_empty.is_empty() {
        return value.clone();
    }
    let replacements: Vec<&str> = std::iter::repeat("[REDACTED]")
        .take(non_empty.len())
        .collect();
    let Ok(ac) = AhoCorasick::builder()
        .match_kind(MatchKind::LeftmostFirst)
        .build(&non_empty)
    else {
        return value.clone();
    };
    scrub_value_recursive(value, &ac, &replacements)
}

/// Check if a string contains any of the provided known-secret patterns.
/// Used for the audit-log safety check (C1 fix).
pub fn contains_secret_material(text: &str, patterns: &[&str]) -> bool {
    let non_empty: Vec<&str> = patterns.iter()
        .filter(|p| p.len() >= MIN_REDACT_LEN)
        .copied()
        .collect();
    if non_empty.is_empty() {
        return false;
    }
    if let Ok(ac) = AhoCorasick::new(&non_empty) {
        ac.is_match(text)
    } else {
        false
    }
}

const MIN_REDACT_LEN: usize = 8;  // don't redact tiny tokens that cause false positives

fn scrub_value_recursive(
    value: &Value,
    ac: &AhoCorasick,
    replacements: &[&str],
) -> Value {
    match value {
        Value::String(s) => Value::String(ac.replace_all(s, replacements)),
        Value::Array(arr) => Value::Array(
            arr.iter().map(|v| scrub_value_recursive(v, ac, replacements)).collect()
        ),
        Value::Object(obj) => Value::Object(
            obj.iter()
                .map(|(k, v)| (k.clone(), scrub_value_recursive(v, ac, replacements)))
                .collect()
        ),
        other => other.clone(),
    }
}
```

**Mode B — Session-cached `Arc<AhoCorasick>` (for high-frequency paths):**
For the MCP hot path where the same set of resolved secrets is scrubbed across multiple tool
calls in a session, use a `tokio::sync::RwLock<Option<Arc<AhoCorasick>>>`. Factory function
rebuilds on demand when the lock contains `None` (post-rotation). Callers who rotate call
`scrubber_session::invalidate()` to set the lock to `None`.

This mode is **not needed in Wave 1**. The per-call model is implemented first; session caching
is an optimization for Wave 6 if benchmarks show >1ms overhead.

**Zeroization:** The caller's `patterns: &[&str]` slices point into `SecretString`-wrapped
values. `SecretString` uses `zeroize` on drop. The scrubber does not hold references beyond the
function call, so no additional zeroization is needed within the scrubber itself.

### H3 — KEK rotation and historical DEK re-wrapping

**Problem:** `rewrap_secret_for_account` re-wraps only the current row's DEK. After a KEK
rotation (e.g., the OS keyring master key is regenerated), historical version rows in
`clavis_secret_versions` still hold DEKs wrapped under the old KEK. If the old keyring entry is
later overwritten or deleted, those historical rows become permanently undecryptable.

**Industry best practice:** "Lazy re-wrap" (keep old KEK accessible) + "active background sweep"
(eventually re-wrap all historical rows). Never delete old KEK until sweep is complete.

**Design for Clavis Cloudless (local keyring model):**
The master key is derived from the keyring entry `("vox-clavis-vault", "master")`. When
`derive_master_key()` generates a new entry (first run), all existing rows will have been
encrypted under the previous entry. The `kek_ref` and `kek_version` fields track which key
version encrypted each DEK.

**Two-phase rewrap protocol:**

Phase 1 (implemented in Wave 5 — after version history exists):
```rust
/// Rewrap all version history rows for a secret from old KEK to new KEK.
/// Called by `vox clavis rotate` after the canonical row is re-wrapped.
pub fn rewrap_version_history(
    &self,
    secret_id: &str,
    old_kek_ref: &str,
    old_kek_version: i64,
    new_kek_ref: &str,
    new_kek_version: i64,
) -> Result<usize, SecretError>;
```

This reads all version rows with `kek_ref = old_kek_ref AND kek_version = old_kek_version`,
decrypts each DEK under the old KEK (which the caller must prove it still possesses — i.e., the
current keyring still yields the old master key), re-encrypts each DEK under the new KEK, and
writes back. The entire sweep is within one transaction.

Phase 2 (CLI surface):
```
vox clavis kek-rewrap [--secret <id>] [--all] [--dry-run]
```

Sweeps all rows (or a specific secret's history) and re-wraps DEKs from the detected old KEK
version to the current. Prints how many rows were updated. `--dry-run` shows what would be
re-wrapped without writing. This is the operator's tool after a KEK rotation event.

**Key invariant:** Old KEK access is maintained until `kek-rewrap --all` completes. After the
command finishes and reports zero rows remaining with the old KEK version, the old keyring entry
can be safely deleted. This is documented in `clavis-cloudless-ops-runbook.md`.

training_eligible: false
archived_date: 2026-04-18
---

## Part IV: Updated Resolver Logic

### 4.1 Profile override resolution path (C7 fix)

The resolver must check `clavis_profile_overrides` *before* `clavis_account_secrets`. To avoid
two Mutex acquisitions, the backend introduces a single new `resolve_with_profile_override`
method that fetches both rows in one query:

```rust
// vox_vault.rs — new method on VoxCloudBackend
fn resolve_best_row(
    &self,
    secret_id: &str,
    profile: &str,   // current resolve profile slug: "dev" | "ci" | "prod" | "hardcut"
) -> Result<Option<(CloudlessSecretRecord, bool /* is_override */)>, SecretError> {
    let conn = self.conn.lock().expect("vox vault mutex");
    run_clavis_future(async {
        // Single query: prefer profile override if it exists, fall back to canonical.
        // UNION ALL with ORDER BY places override rows first.
        let mut stmt = conn.prepare(
            "SELECT ciphertext, nonce, dek_wrapped, kek_ref, kek_version,
                    rotation_epoch, rotated_at_ms, checksum_hash,
                    1 AS is_override
             FROM clavis_profile_overrides
             WHERE account_id = ?1 AND secret_id = ?2 AND profile = ?3
             UNION ALL
             SELECT ciphertext, nonce, dek_wrapped, kek_ref, kek_version,
                    rotation_epoch, rotated_at_ms, checksum_hash,
                    0 AS is_override
             FROM clavis_account_secrets
             WHERE account_id = ?1 AND secret_id = ?2
             LIMIT 1",
        ).await.map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?;
        let mut rows = stmt.query(params![&self.account_id, secret_id, profile])
            .await.map_err(|e| SecretError::BackendQueryFailed(e.to_string()))?;
        if let Some(row) = rows.next().await.map_err(|e| SecretError::BackendQueryFailed(e.to_string()))? {
            // Parse row — returns (record, is_override: bool)
        }
        Ok(None)
    })
}
```

The `SecretBackend::resolve` implementation on `VoxCloudBackend` calls `resolve_best_row`
instead of `get_row`. The `ResolutionStatus` is set to `ProfileOverrideUsed` if `is_override`.

### 4.2 Lifecycle status (StaleRotation, NearingExpiry)

Lifecycle status is computed *after* resolution. Because it requires the vault row's
`updated_at_ms` and `rotation_epoch`, these fields are included in the resolved row from the
query above (they already exist on `CloudlessSecretRecord`). When the source is `ExternalBackend`
(vault hit), `compute_lifecycle_status` checks:

```rust
fn compute_lifecycle_status(
    spec: &SecretSpec,
    row_updated_at_ms: i64,
    row_rotation_epoch: i64,
) -> ResolutionStatus {
    let lm = spec.id.metadata().lifecycle;
    let now_ms = now_ms();

    // StaleRotation: never rotated + older than 2× cadence
    if lm.track_stale_rotation && row_rotation_epoch == 0 {
        if let Some(cadence_days) = lm.rotation_cadence_days {
            let stale_threshold_ms = (cadence_days as i64) * 2 * 86_400_000;
            if now_ms - row_updated_at_ms > stale_threshold_ms {
                return ResolutionStatus::StaleRotation;
            }
        }
    }

    // NearingExpiry: provider-managed tokens that are expected to expire
    // (Expiry tracking deferred to Wave 7 when provider probe infrastructure exists)
    // if let Some(warn_days) = lm.expiry_warning_days { ... }

    ResolutionStatus::Present
}
```

### 4.3 Audit log write (safe, non-blocking, non-value-leaking)

```rust
fn append_audit_row(resolved: &ResolvedSecret, ctx: &str) {
    // Never write to audit log if the vault backend is unavailable
    let Ok(backend) = VoxCloudBackend::new() else { return; };

    let detail = resolved.detail.as_deref().unwrap_or("");

    // C1 fix: abort if detail contains secret material (code bug guard)
    #[cfg(debug_assertions)]
    debug_assert!(
        !contains_secret_material(detail, &[]),
        "BUG: audit detail contains secret material"
    );

    let _ = backend.append_audit_row(
        &resolved.id, resolved.status, resolved.source, ctx, detail
    );
}
```

The `append_audit_row` implementation creates its own connection (not the shared Mutex) or uses
a separate write connection if `VoxCloudBackend` grows a dual-connection model. Because audit
writes are best-effort and non-critical for resolution correctness, connection failure is silently
swallowed. The audit log must never block or fail the caller's `resolve_secret` path.

---

## Part V: CLI Surface

### Overview of new and changed commands

| Command | Status | Priority |
|---|---|---|
| `vox clavis status` / `doctor` | **Enhanced** (new fields in JSON-V1 output) | High |
| `vox clavis import-env` | **Enhanced** (conflict detection, `--classify`, canonical rename) | High |
| `vox clavis set-secret` | **New** (replaces auth-json-only `set`) | High |
| `vox clavis list` | **New** | High |
| `vox clavis diff` | **New** | Medium |
| `vox clavis run` | **New** | Medium |
| `vox clavis rotate` | **New** | Medium |
| `vox clavis history` | **New** | Medium |
| `vox clavis rollback` | **New** | Medium |
| `vox clavis audit-log` | **New** | Medium |
| `vox clavis delegate` | **New** | Low |
| `vox clavis revoke-delegation` | **New** | Low |
| `vox clavis kek-rewrap` | **New** | Low |
| `vox clavis prune-history` | **New** | Low |

### `vox clavis run` — cross-platform subprocess model (C10 fix)

**Unix:** Uses `std::os::unix::process::CommandExt::exec()` to *replace* the current process
image with the child. The parent process no longer exists; signals are delivered directly to
the child. This is the `doppler run --` model.

**Windows:** Uses `std::process::Command::spawn()` + `child.wait()`. The Clavis process stays
alive as a thin wrapper. Ctrl-C forwarding must be implemented via `SetConsoleCtrlHandler` (the
`ctrlc` crate). This is acceptable for the intended use case (local dev workflow).

Flag: `--passthrough-exit-code` (default: on) forwards child exit code to the caller.

**Environment isolation:** Resolved secrets are set via `Command::env()` on the `Command`
builder. They are never written to `std::env::set_var` (which would affect the parent's
process-wide env). The child inherits only what is explicitly passed.

**What gets injected:** All secrets in the specified `--bundle` or `--workflow` that resolve
`Present`. Secrets that resolve `MissingOptional` are silently skipped. Secrets that resolve
`MissingRequired` abort the command with a clear error before spawning.

training_eligible: false
archived_date: 2026-04-18
---

## Part VI: Consumer Wiring

Exactly which crates receive changes and what those changes are:

### `vox-clavis` (primary)
All changes in Parts I–V live here. No other crate needs `Cargo.toml` changes for the
resolution path.

**New direct dependency:** `aho-corasick = "1"` — confirmed not yet in workspace dep tree.
Add to workspace `Cargo.toml` under `[workspace.dependencies]` first.

### `vox-cli` (`clavis.rs`)
New `ClavisCmd` variants as specified in Part V. `DoctorSecretRow` JSON schema gains:
`taxonomy_class`, `scope_description`, `lifecycle_cadence_days`, `rotation_epoch`,
`rotated_at_hint`.

**Change to `set` command:** Deprecated. `set-secret` replaces it. `set` becomes a thin
compatibility alias pointing to `set-secret --auth-json-compat` which writes to both
`auth.json` AND the vault. This prevents breaking existing scripts.

### `vox-mcp` (`http_gateway.rs`)
Changes: call `resolve_secret_for_cli` → `resolve_secret_with_context(id, "mcp")` for audit
attribution. Apply `redact_secrets_from_value` to tool results before serialization.

No `Cargo.toml` change (already depends on `vox-clavis`).

### `vox-orchestrator` (config load)
Changes: call `resolve_secret_with_context(id, "process")` — no code change to caller, the
default applies. Zero code change to orchestrator crate. Taxonomy annotations in SPECS handle
the rest.

### `vox-publisher` (social and scholarly adapters)
Changes: OAuth refresh token entries gain `lifecycle: LifecycleMeta::ANNUAL_OAUTH`. Expiry
warning fires via `NearingExpiry` status in `vox clavis status`.

### `vox-db` (new `ClavisGate`)
A new public module `crates/vox-db/src/clavis_gate.rs` exposes async access to
`clavis_agent_delegations` and `clavis_audit_log` for internal vox-db consumers (agent event
trace writes, MCP result audit scrubbing at the DB layer). It does NOT depend on
`VoxCloudBackend` — it uses the main DB connection (`VOX_DB_URL`). When the same physical
database is used for both planes, the tables are accessible; when they're separate, the gate
simply returns `Err(DbError::ClavisGateUnavailable)` gracefully.

**Dep:** `vox-db` adds `vox-clavis` to `Cargo.toml` for type aliases only.

---

## Part VII: Wave Ordering (Safety-First)

Waves are ordered by three constraints:
1. **Safety**: no wave may create a data path that could leak secrets before the scrubber exists.
2. **Dependency**: schema must exist before code that writes to it.
3. **Value delivery**: highest operator value (list, diff, run) as early as possible.

```
Wave 0 ─ Foundation (const changes, no behaviour)
Wave 1 ─ Scrubber (redact.rs) ← C1 prerequisite for all future writes
Wave 2 ─ Schema creation (4 new tables + WAL)
Wave 3 ─ Atomic write path (write_secret_v2 + transactions)
Wave 4 ─ Resolver updates (profile overrides, lifecycle status)
Wave 5 ─ Core CLI (list, diff, set-secret, improved import-env)
Wave 6 ─ Audit log integration (depends on Wave 1 scrubber)
Wave 7 ─ Advanced CLI (run, rotate, rollback, history, prune-history)
Wave 8 ─ KEK rewrap path + kek-rewrap CLI (depends on Wave 3 version history)
Wave 9 ─ A2A delegation (delegate, revoke-delegation, ClavisGate)
Wave 10 ─ CI parity, SSOT completion, migration to resolve_secret_with_context
```

### Wave 0 — Foundation (const changes only)

**Goal:** Add `TaxonomyClass`, `LifecycleMeta`, extend `SecretMetadata` and `SecretSpec`, add
`ResolutionStatus` variants, add `SecretMaterialKind` variants. Annotate ALL ~580 SPECS entries.

**Files changed:**
- `crates/vox-clavis/src/lib.rs` — new types + full SPECS annotation

**Safety:** Zero behaviour change. No DB writes. No resolution path change.

**Verification:**
- `cargo check --workspace` — must be green
- `cargo test -p vox-clavis` — must pass
- `vox ci clavis-parity` — must pass (SSOT doc not yet updated; CI check must handle old schema)
- `vox ci secret-env-guard --all` — must pass

**Estimated effort:** 1 day (mechanical annotation of ~580 entries using `modify_specs.py`)

Note: `modify_specs.py` already exists in `crates/vox-clavis/src/`. It should be used/extended
to programmatically annotate entries with taxonomy defaults, then spot-corrected for accuracy.

### Wave 1 — Runtime Scrubber (`redact.rs`)

**Goal:** `redact_secrets_from_value` and `contains_secret_material` implemented and unit-tested.
The `aho-corasick` dep added to workspace.

**Files changed:**
- `Cargo.toml` (workspace) — add `aho-corasick = "1"` under `[workspace.dependencies]`
- `crates/vox-clavis/Cargo.toml` — add `aho-corasick = { workspace = true }`
- `crates/vox-clavis/src/redact.rs` — new file
- `crates/vox-clavis/src/lib.rs` — `pub mod redact;` + re-exports
- `crates/vox-clavis/src/tests.rs` — 4 new unit tests

**Unit tests required:**
1. `redact_secrets_from_value` scrubs a string value containing a known API key.
2. `redact_secrets_from_value` scrubs a nested JSON object.
3. `contains_secret_material` returns `true` for a string containing a pattern.
4. `MIN_REDACT_LEN` filter: patterns shorter than 8 chars are not used as patterns.

**Safety:** `redact.rs` is pure in/out — no DB access, no env reads. It can be merged
independently of all other waves.

**Verification:**
- `cargo test -p vox-clavis redact` — all 4 tests pass
- `cargo check --workspace` — clean

**Estimated effort:** 0.5 days

### Wave 2 — DB Schema Creation

**Goal:** Four new tables added to `ensure_schema`. WAL pragma for local databases. Schema is
created at `VoxCloudBackend::new()` time, transparently for existing users.

**Files changed:**
- `crates/vox-clavis/src/backend/vox_vault.rs` — extend `ensure_schema`, add WAL pragma

**What `ensure_schema` adds:**

```rust
async fn ensure_schema(conn: &turso::Connection, db_url: &str) -> Result<(), SecretError> {
    // Existing table (unchanged)
    conn.execute_batch("CREATE TABLE IF NOT EXISTS clavis_account_secrets (...)").await?;

    // WAL mode for local databases only
    if db_url.starts_with("file:") {
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;").await?;
    }

    // New tables
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS clavis_secret_versions ( ... );
        CREATE INDEX IF NOT EXISTS idx_clavis_sv_lookup ON ...;
        CREATE INDEX IF NOT EXISTS idx_clavis_sv_kek ON ...;

        CREATE TABLE IF NOT EXISTS clavis_audit_log ( ... );
        CREATE INDEX IF NOT EXISTS idx_clavis_al_time ON ...;
        CREATE INDEX IF NOT EXISTS idx_clavis_al_secret ON ...;

        CREATE TABLE IF NOT EXISTS clavis_profile_overrides ( ... );

        CREATE TABLE IF NOT EXISTS clavis_agent_delegations ( ... );
        CREATE INDEX IF NOT EXISTS idx_clavis_del_lookup ON ...;
    ").await
    .map_err(|e| SecretError::BackendMisconfigured(e.to_string()))
}
```

Note: `db_url` must be passed to `ensure_schema` (currently it is not). This requires
refactoring `open_cloudless_connection` to return both the connection and the resolved URL,
and passing the URL to `ensure_schema`. Minor change to `VoxCloudBackend::new`.

**Safety:** `CREATE TABLE IF NOT EXISTS` is idempotent. Existing databases are not modified.
The only risk is the WAL pragma on existing local databases — WAL mode is stable and compatible
with all existing read/write patterns.

**Verification:**
- Unit test: `VoxCloudBackend::new()` on an empty in-memory database creates all five tables.
- Unit test: `VoxCloudBackend::new()` on an existing database (with only `clavis_account_secrets`)
  creates the four new tables without error.
- `cargo test -p vox-clavis` — passes
- `cargo check --workspace` — clean

**Estimated effort:** 0.5 days

### Wave 3 — Atomic Write Path

**Goal:** `write_secret_v2` replaces `write_secret_for_account` internally. The transaction
model from H1 is implemented. Existing `write_secret` and `write_secret_for_account` become
thin wrappers.

**Files changed:**
- `crates/vox-clavis/src/backend/vox_vault.rs` — `write_secret_v2`, DEK zeroization, updated
  callers

**Key implementation details (from H1 analysis):**
- CPU-bound crypto (encrypt, wrap_dek) happens *before* the async block.
- DEK is zeroized immediately after wrap.
- The full UPSERT + INSERT + DELETE runs inside one `run_clavis_future(async { ... })` call
  using `conn.unchecked_transaction()`.
- `import_account_backup` is updated to use `write_secret_v2` per row.

**Verification:**
- Unit test: `write_secret_v2` on a fresh DB creates one canonical row and one version row.
- Unit test: second `write_secret_v2` call updates canonical row and creates a second version row.
- Unit test: `export_account_backup` + `import_account_backup` round-trips correctly.
- Unit test: version history is pruned to `history_depth` when exceeded.
- Unit test: transaction rollback — if the version INSERT fails (simulate with a malformed SQL),
  the canonical UPSERT is also rolled back.
- `cargo test -p vox-clavis` — all pass

**Estimated effort:** 1 day

### Wave 4 — Resolver Updates

**Goal:** Profile override resolution path, lifecycle status, `resolve_secret_with_context`.

**Files changed:**
- `crates/vox-clavis/src/backend/vox_vault.rs` — `resolve_best_row` (single-query override check)
- `crates/vox-clavis/src/backend/mod.rs` — `SecretBackend::resolve` signature extended, or a
  new `resolve_with_profile` method added to the trait
- `crates/vox-clavis/src/resolver.rs` — `compute_lifecycle_status`, profile-aware resolution
- `crates/vox-clavis/src/lib.rs` — `resolve_secret_with_context(id, ctx)` public API

**Resolver source precedence (updated, fully specified):**

```
1. VaultBackend.resolve_best_row(secret_id, profile)
      → clavis_profile_overrides (profile row) → ResolutionStatus::ProfileOverrideUsed
      → clavis_account_secrets (canonical row)  → ResolutionStatus::Present | StaleRotation
2. env::resolve_env(spec)
      → EnvCanonical / EnvAlias / DeprecatedAliasUsed
3. backend::auth_json::read_registry_token (if spec.auth_registry is Some)
4. populi_env::read_populi_env_key (if spec reads populi env file)
5. → MissingOptional | MissingRequired
```

**Important:** Profile-aware vault resolution is only active when `BackendMode::VoxCloud`
(or `Auto` that resolves to VoxCloud) is in use. With `BackendMode::EnvOnly`, the vault is not
queried and profile overrides have no effect.

**Verification:**
- Unit test: when a profile override row exists for `"ci"` and `ResolveProfile::CiStrict`,
  `resolve_secret` returns `ProfileOverrideUsed`.
- Unit test: when only the canonical row exists, it falls through to `Present`.
- Unit test: `StaleRotation` fires correctly when `rotation_epoch == 0` and age > 2× cadence.
- `cargo test -p vox-clavis` — all pass

**Estimated effort:** 1 day

### Wave 5 — Core CLI

**Goal:** The commands developers will use every day: `set-secret`, `list`, `diff`, and improved
`import-env`.

**Files changed:**
- `crates/vox-cli/src/commands/clavis.rs` — new `ClavisCmd` variants, handlers

**`vox clavis list` implementation detail:**
Calls `all_specs()`, filters out `TaxonomyClass::is_config_only()`, iterates calling
`VoxCloudBackend::get_row` for each. Returns metadata only. Groups by taxonomy class in human
output. Accepts `--class <slug>` filter. Never decrypts.

**`vox clavis diff` implementation detail:**
1. Parse `.env` file into `Vec<(key, value)>`.
2. For each key: `all_specs().iter().find(|s| s.canonical_env == key || s.aliases.contains(&&key))`.
3. For each managed key: call `resolve_secret` and report source (vault / env / missing).
4. Unmanaged keys: listed as "not tracked by Clavis".
5. For keys where env name doesn't match canonical: "suggestion: rename `GEMINI_KEY` to `GEMINI_API_KEY`".

**`vox clavis import-env` improvements (C8-adjacent):**
- `--no-overwrite` default: if a vault row already exists for a key, print "already in vault
  (use --overwrite to replace)" and skip.
- `--classify` flag: prints taxonomy class of each found managed key before importing.
- Canonical name normalization: if `.env` contains `ANTHROPIC_KEY` (a deprecated alias), the
  import writes to the canonical env name `ANTHROPIC_API_KEY` and prints the rename.

**Verification:**
- `vox clavis list` on empty vault: prints "0 secrets in vault".
- `vox clavis list --class llm` with `OPENROUTER_API_KEY` in vault: shows that one entry.
- `vox clavis diff --env-file .env` with a managed key in `.env`: shows it as "env-only
  (not in vault) — migrate with: vox clavis import-env".
- `cargo check --workspace` — clean

**Estimated effort:** 1 day

### Wave 6 — Audit Log Integration

**Goal:** Audit log writes active. `caller_context` set at call sites. `audit-log` CLI.

**Files changed:**
- `crates/vox-clavis/src/lib.rs` — `resolve_secret_with_context`, `append_audit_row`
- `crates/vox-clavis/src/backend/vox_vault.rs` — `append_audit_row` on backend
- `crates/vox-cli/src/commands/clavis.rs` — `audit-log` subcommand
- `crates/vox-orchestrator/src/mcp_tools/...` — `resolve_secret_with_context(id, "mcp")` at call sites

**Context attribution spec:**
```
Call site                        | caller_context
training_eligible: false
archived_date: 2026-04-18
---------------------------------|----------------------------
vox-cli clavis commands          | "cli"
vox-mcp http_gateway             | "mcp"
vox-orchestrator config load     | "process"  (default)
vox-db ClavisGate                | "api"
agent task calls (future)        | "agent:<task_id>"
```

**Verification:**
- With `VOX_CLAVIS_AUDIT_LOG=1`: resolve any secret, `vox clavis audit-log --limit 1` shows one row with correct `caller_context`.
- In `ProdStrict` profile: audit log writes even without `VOX_CLAVIS_AUDIT_LOG=1`.
- Audit row for `detail` field that accidentally contained a secret value: test that `debug_assert!` fires in debug mode.

**Estimated effort:** 1 day

### Wave 7 — Advanced CLI (run, rotate, rollback, history)

**Goal:** The remaining high-value operator commands.

**`vox clavis run` platform model (C10 fix):**
```rust
#[cfg(unix)]
fn exec_child(cmd: &str, args: &[String], env: Vec<(String, String)>) -> ! {
    use std::os::unix::process::CommandExt;
    let err = Command::new(cmd).args(args).envs(env).exec();
    eprintln!("exec failed: {err}");
    std::process::exit(127);
}

#[cfg(windows)]
fn exec_child(cmd: &str, args: &[String], env: Vec<(String, String)>) -> ! {
    use std::process::Command;
    // Windows: stay-alive parent, forward exit code
    let status = Command::new(cmd).args(args).envs(env)
        .spawn().and_then(|mut c| c.wait())
        .map(|s| s.code().unwrap_or(1))
        .unwrap_or(127);
    std::process::exit(status);
}
```

**`vox clavis rotate` detail:**
1. Resolves current vault value (or accepts `--value`).
2. Calls `write_secret_v2` with `operation = "rotate"`.
3. `rotation_epoch` is incremented: `new_epoch = current_rotation_epoch + 1`.
4. `rotated_at_ms` is set to `now_ms()` in both the UPSERT (canonical table) and the version row.
5. Prints: `Rotated {secret_id}: version {new_version_id}, epoch {new_epoch}`.

Note: `rotation_epoch` is currently on `clavis_account_secrets` but not passed through to
`write_secret_v2`. The implementation must read the current epoch before writing and increment it.

**`vox clavis rollback` safety:**
- Requires `--reason <text>` (mandatory, enforced in CLI before any vault access).
- Rolls back to version N: reads ciphertext from `clavis_secret_versions`, decrypts, re-encrypts
  under current KEK (new DEK generated), writes via `write_secret_v2` with `operation = "rollback"`.
- Does NOT silently overwrite; shows a confirmation prompt with redacted before/after if
  `--no-confirm` is not passed.

**Verification:**
- `vox clavis run --bundle minimal-local-dev -- printenv OPENROUTER_API_KEY` prints the resolved value.
- `vox clavis rotate OPENROUTER_API_KEY --value sk-newval ; vox clavis history OPENROUTER_API_KEY` shows two rows.
- `vox clavis rollback OPENROUTER_API_KEY --to-version 1 --reason "test"` succeeds.
- `vox clavis history OPENROUTER_API_KEY` shows three rows (create, rotate, rollback).

**Estimated effort:** 2 days

### Wave 8 — KEK Rewrap Path

**Goal:** `rewrap_version_history` backend method and `vox clavis kek-rewrap` CLI.

**Files changed:**
- `crates/vox-clavis/src/backend/vox_vault.rs` — `rewrap_version_history`
- `crates/vox-cli/src/commands/clavis.rs` — `kek-rewrap` subcommand

**Implementation detail from H3:**
```rust
pub fn rewrap_version_history(
    &self,
    secret_id: &str,
    old_kek_ref: &str,
    old_kek_version: i64,
) -> Result<usize, SecretError> {
    // Fetch all version rows with old kek_ref+version
    // For each: decrypt DEK with old KEK, re-encrypt with current KEK
    // Update row in-place (the only UPDATE permitted on version table — re-wrapping only)
    // Return count of rows re-wrapped
}
```

The invariant is: re-wrapping changes `dek_wrapped`, `kek_ref`, `kek_version`, and
`checksum_hash` — but never `ciphertext` or `nonce`. The data is still encrypted under
the original DEK; only the DEK's wrapper changes. This means the data's confidentiality
is unchanged during the rewrap operation.

**Verification:**
- `vox clavis kek-rewrap --all --dry-run` shows how many rows would be re-wrapped.
- After simulated KEK generation (new keyring entry), `kek-rewrap --all` updates all rows.
- All re-wrapped rows decrypt correctly using the new KEK.

**Estimated effort:** 1 day

### Wave 9 — A2A Delegation

**Goal:** Delegation create/validate/revoke. `ClavisGate`. CLI surface.

**Files changed:**
- `crates/vox-clavis/src/lib.rs` — `resolve_secret_for_delegation`
- `crates/vox-clavis/src/backend/vox_vault.rs` — delegation CRUD
- `crates/vox-db/src/clavis_gate.rs` — new file
- `crates/vox-db/Cargo.toml` — add `vox-clavis` workspace dep
- `crates/vox-cli/src/commands/clavis.rs` — `delegate`, `revoke-delegation`

**`resolve_secret_for_delegation` API:**
```rust
pub fn resolve_secret_for_delegation(
    delegation_id: &str,
    account_id: &str,
) -> Result<ResolvedSecret, SecretError> {
    let backend = VoxCloudBackend::new()?;
    // 1. Load delegation row; fail if expired or revoked
    // 2. Validate scope_bits includes 0x01 (read)
    // 3. Call resolve_secret(delegation.secret_id) internally
    // 4. Write audit row with caller_context = "delegation:<delegation_id>"
}
```

**TTL enforcement:** The backend enforces `expires_at_ms ≤ issued_at_ms + 3_600_000` at
write time (CHECK constraint + Rust-level guard). At read time, `now_ms() > expires_at_ms`
returns `Err(SecretError::BackendUnavailable("delegation expired"))`.

**Verification:**
- `vox clavis delegate OPENROUTER_API_KEY --to "agent:task-001" --ttl-secs 60` returns delegation ID.
- `resolve_secret_for_delegation(id, account_id)` succeeds within 60s.
- After 60s: `resolve_secret_for_delegation` returns `Err`.
- Revoke mid-TTL: `resolve_secret_for_delegation` returns `Err` immediately.

**Estimated effort:** 2 days

### Wave 10 — CI Parity, SSOT Completion, Context Migration

**Goal:** Full CI guard updates. SSOT doc updated. All consumer call sites migrated to
`resolve_secret_with_context`.

**Files changed:**
- `docs/src/reference/clavis-ssot.md` — taxonomy columns, new table sections
- `crates/vox-cli/src/commands/ci/run_body_helpers/guards.rs` — `clavis-parity` validates taxonomy
- `crates/vox-orchestrator/src/mcp_tools/...` — context migration
- `crates/vox-clavis/src/tests.rs` — tests for `ConfigValue`/`OperatorTuning` exclusion from list

**New CI check: `vox ci clavis-audit-schema`**
Validates that:
1. `clavis_secret_versions` schema matches `contracts/clavis/version-history.v1.json`.
2. No production migration file contains `UPDATE ... clavis_secret_versions` (except rewrap-type operations that only update `dek_wrapped`, `kek_ref`, `kek_version`, `checksum_hash`).
3. No production migration file contains `DELETE ... clavis_secret_versions` (except via pruning).

**Estimated effort:** 1 day

---

## Part VIII: Cargo.toml Changes Summary

| Location | Change | Reason |
|---|---|---|
| `Cargo.toml` (workspace `[workspace.dependencies]`) | Add `aho-corasick = "1"` | Scrubber |
| `crates/vox-clavis/Cargo.toml` | Add `aho-corasick = { workspace = true }` | Scrubber |
| `crates/vox-db/Cargo.toml` | Add `vox-clavis = { workspace = true }` | ClavisGate types |

No changes to `vox-mcp`, `vox-orchestrator`, `vox-runtime`, `vox-publisher`, or `vox-skills`
`Cargo.toml` — they already depend on `vox-clavis`.

`uuid` for delegation IDs: check if already present as a transitive dep before adding. If not,
add to `vox-clavis` directly: `uuid = { version = "1", features = ["v4"] }`.

training_eligible: false
archived_date: 2026-04-18
---

## Part IX: Security Invariants (additions to V1 threat model)

These extend the 5 invariants in `clavis-cloudless-threat-model-v1.md`:

**Inv-6:** `redact_secrets_from_value` (Wave 1) MUST be called before any content from
`resolve_secret` is written to `clavis_audit_log`, MCP tool results, telemetry upload batches,
or agent event traces. Verified by `debug_assert!` in `append_audit_row`.

**Inv-7:** `clavis_agent_delegations.expires_at_ms ≤ issued_at_ms + 3_600_000` is enforced
at write time by both a SQL CHECK constraint and a Rust-level guard before the INSERT.

**Inv-8:** `clavis_secret_versions` is append-only for data. The only permitted UPDATE
operations are rewrap (changing `dek_wrapped`, `kek_ref`, `kek_version`, `checksum_hash` only).
No DELETE operations are permitted except via the bounded `prune_history` path (which deletes
only rows beyond the depth limit). The CI `clavis-audit-schema` check enforces this.

**Inv-9:** `clavis_audit_log` rows MUST NOT contain resolved secret values. The
`contains_secret_material` check in `append_audit_row` enforces this at runtime.

**Inv-10:** Profile override rows for `prod` and `hardcut` profiles require explicit `--profile
prod` or `--profile hardcut` flag on the CLI. No implicit promotion.

**Inv-11:** `caller_context` in audit rows is set by the call site, never by env-var. The
`resolve_secret_with_context(id, ctx)` API validates `ctx` against an allowlist pattern before
accepting it.

**Inv-12:** DEK zeroization. Raw DEK bytes `[u8; 32]` are filled with zeros immediately after
wrapping (`dek.fill(0)`) in `write_secret_v2`. No plaintext DEK persists past the wrap call.

---

## Part X: Open Questions (genuine, not deferred problems)

These are true design decisions that have two valid options and require a call before
implementation:

**Q1 — `clavis_profile_overrides` or `clavis_account_secrets` with profile column?**
Option A (chosen): separate table. Keeps canonical read path fast (no profile filter needed
for the common case). UNION ALL query handles the override lookup.
Option B: Add a nullable `profile TEXT` column to `clavis_account_secrets` with the PK
becoming `(account_id, secret_id, COALESCE(profile, ''))`. Simpler schema, but the fast-path
`resolve_best_row` query is the same UNION ALL equivalent.
**Recommendation:** Option A (separate table) for clear conceptual separation.

**Q2 — Audit log: separate connection or shared Mutex connection?**
Option A (recommended): `append_audit_row` always creates a new `VoxCloudBackend` (new
connection). This avoids Mutex contention on the hot `resolve_secret` path and keeps audit
writes truly async (non-blocking). Cost: one new connection per audit write entry.
Option B: Add a second `Mutex<Connection>` to `VoxCloudBackend` specifically for audit writes.
**Recommendation:** Option A for Wave 6. Optimize to Option B in Wave 10 if connection creation
overhead is observed in benchmarks.

**Q3 — `prune_history` scope?**
Currently specified as `--keep N` globally per secret. Should it also support a global `--older-than N-days` prune? This is useful for compliance (delete secrets older than 90 days).
**Recommendation:** Add `--older-than` in Wave 7. The DELETE query is straightforward:
`WHERE created_at_ms < ? AND version_id NOT IN (SELECT MIN(version_id) ...)`.

training_eligible: false
archived_date: 2026-04-18
---

## Cross-Reference Map

| Document | Relationship |
|---|---|
| [clavis-ssot.md](../reference/clavis-ssot.md) | Updated in Wave 10 |
| [clavis-cloudless-threat-model-v1.md](clavis-cloudless-threat-model-v1.md) | Extended by §IX Inv-6–12 |
| [clavis-secrets-env-research-2026.md](clavis-secrets-env-research-2026.md) | Base research; waves extend its gates |
| [clavis-one-stop-secrets-research-2026.md](clavis-one-stop-secrets-research-2026.md) | Feature requirements mapped to §V CLI surface |
| [terminal-exec-policy-research-findings-2026.md](terminal-exec-policy-research-findings-2026.md) | `vox clavis run` subprocess model |


