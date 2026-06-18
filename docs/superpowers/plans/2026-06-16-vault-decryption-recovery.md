# Clavis Vault Decryption Recovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore a working local Clavis vault after `vox secrets import-env` succeeds but `vox secrets backend-status` fails with `decryption failed: aead::Error`, and add diagnostics so future master-key drift is obvious and recoverable without guesswork.

**Architecture:** The Windows absolute-path fix (`file_url_to_local_path` in `vox_vault.rs`) is done — vault **opens**. Decryption fails because the OS keyring master (`vox-secrets-vault` / `master`) used to **unwrap DEKs** no longer matches the key that encrypted the rows in `.vox/clavis_vault.db`. Phase 0 is operator recovery (reset vault + stable keyring + re-import). Phase 1 adds a **vault health probe** (`open → count rows → decrypt-canary`) with actionable remediation text. Phase 2 wires probe output into `backend-status` / `secrets doctor`. Phase 3 adds hermetic tests for import→resolve round-trip. Phase 4 (optional) wires `SonatypeGuideToken` through `auth_registry` so `secrets set` and vault resolution stay aligned.

**Tech Stack:** Rust (`vox-secrets`, `vox-cli`), OS keyring (`keyring` crate), Turso/libSQL local file, ChaCha20-Poly1305 via `vox-crypto`, Windows Credential Manager.

**Prerequisite (already landed):** `crates/vox-secrets/src/backend/vox_vault.rs` — `file_url_to_local_path`, `normalize_turso_local_path`, WAL PRAGMA removed. Verify with `cargo test -p vox-secrets path_url_tests`.

**Related plan:** Build lock hygiene is separate — see `docs/superpowers/plans/2026-06-15-build-binary-lock-resilience.md`.

---

## Background — verified root cause (read before starting)

1. **Master key lives only in OS keyring**, not in the vault file. `derive_master_key()` reads/writes `keyring::Entry::new("vox-secrets-vault", "master")` and hashes the password to 32 bytes (`vox_vault.rs:1234-1254`).
2. **`import-env` writes encrypted rows** via `VoxCloudBackend::new()` → `write_secret_v2` using that master-derived KEK (`kek_ref` default `local-master`, `kek_version` default `1`, `account_id` default `default-account` unless `VOX_ACCOUNT_ID` is set).
3. **`backend-status` reads** by calling `resolve_secret` for every managed spec until one returns `BackendUnavailable`. Decryption error means rows **exist** but `unwrap_dek` / `decrypt_vault` failed — almost always **master key mismatch** (keyring entry regenerated, different Windows user profile, or vault copied from another machine).
4. **`vox secrets set` does NOT write the Clavis vault** — it goes to `~/.vox/auth.json` + per-registry keyring. Only `import-env` / `store_secret` touch the vault. Do not confuse the two stores.
5. **Current operator state (2026-06-16):** path fix verified; `backend-status` reports `decryption failed: aead::Error` with `VOX_SECRETS_VAULT_PATH` pointing at `.vox/clavis_vault.db`.

---

## File Structure

| File | Responsibility |
|------|----------------|
| `crates/vox-secrets/src/backend/vox_vault.rs` | `master_key_fingerprint`, `VaultHealth`, `probe_vault_health`, richer decrypt errors |
| `crates/vox-secrets/src/lib.rs` | Re-export `probe_vault_health`, `VaultHealth` for CLI/GUI |
| `crates/vox-secrets/src/tests.rs` | Hermetic import-env round-trip test |
| `crates/vox-cli/src/commands/secrets.rs` | Richer `backend-status` output |
| `crates/vox-secrets/src/spec/registry/platform.rs` | (Optional) `SonatypeGuideToken` `auth_registry` |
| `docs/src/reference/secrets-ssot.md` | Operator recovery subsection (vault vs auth.json, keyring) |

---

## Task 0: Operator recovery (run once before code changes)

**No files changed.** Confirms the vault can be made healthy on this machine.

- [ ] **Step 1: Ensure no stale `vox` holds the binary**

Run (PowerShell, repo root):

```powershell
Get-Process -Name vox -ErrorAction SilentlyContinue | Stop-Process -Force
```

Expected: no output (or process list then cleared).

- [ ] **Step 2: Confirm keyring entry exists**

```powershell
& target\debug\vox.exe secrets backend-status 2>&1
```

Note whether mode is `Auto` and error is `decryption failed` (not `invalid filename`).

- [ ] **Step 3: Backup and reset the local vault file**

```powershell
$vault = "C:\Users\Owner\vox\.vox\clavis_vault.db"
if (Test-Path $vault) { Copy-Item $vault "$vault.bak-$(Get-Date -Format yyyyMMdd-HHmmss)" }
Remove-Item $vault -Force -ErrorAction SilentlyContinue
```

Expected: backup file created; vault removed.

- [ ] **Step 4: Re-import secrets from a `.env` fragment**

Create a git-ignored file (never commit):

```text
# .vox/import-secrets.env (example — use your real keys)
SONATYPE_GUIDE_TOKEN=sgt_...
```

Run:

```powershell
& target\debug\vox.exe secrets import-env --file .vox\import-secrets.env
```

Expected: `Import complete: N managed secrets injected into vault`.

- [ ] **Step 5: Verify backend**

```powershell
$env:VOX_SECRETS_VAULT_PATH = $vault
& target\debug\vox.exe secrets backend-status
```

Expected (after Phase 1 lands): `backend status: available or env-only fallback`.  
Expected (before Phase 1): same, without decryption error.

- [ ] **Step 6: Verify resolution**

```powershell
& target\debug\vox.exe secrets get sonatype  # if wired; else env fallback:
# Confirm SONATYPE_GUIDE_TOKEN resolves — never print raw token in logs
```

---

### Task 1: Master key fingerprint helper

**Files:**
- Modify: `crates/vox-secrets/src/backend/vox_vault.rs` (after `derive_master_key`)
- Test: same file, `path_url_tests` module or new `vault_health_tests` module

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-secrets/src/backend/vox_vault.rs` in a `#[cfg(test)] mod vault_health_tests`:

```rust
#[test]
fn master_key_fingerprint_is_stable_for_same_input() {
    let key = [7_u8; 32];
    let a = super::master_key_fingerprint(&key);
    let b = super::master_key_fingerprint(&key);
    assert_eq!(a, b);
    assert_eq!(a.len(), 16, "fingerprint is 16 hex chars (64-bit prefix)");
}

#[test]
fn master_key_fingerprint_differs_for_different_keys() {
    let a = super::master_key_fingerprint(&[1_u8; 32]);
    let b = super::master_key_fingerprint(&[2_u8; 32]);
    assert_ne!(a, b);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-secrets master_key_fingerprint_is_stable_for_same_input -- --nocapture`

Expected: FAIL — `master_key_fingerprint` not found

- [ ] **Step 3: Implement minimal helper**

In `vox_vault.rs` (pub(crate) or pub for lib re-export):

```rust
/// Non-secret diagnostic: first 8 bytes of `secure_hash(b"vox-vault-master-fp:" + key)` as hex.
/// Used to detect keyring master drift without exposing key material.
pub(crate) fn master_key_fingerprint(master_key: &[u8; 32]) -> String {
    let mut data = b"vox-vault-master-fp:".to_vec();
    data.extend_from_slice(master_key);
    let hash = secure_hash(&data);
    hash.iter()
        .take(8)
        .map(|b| format!("{b:02x}"))
        .collect()
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p vox-secrets master_key_fingerprint -- --nocapture`

Expected: PASS (2 tests)

- [ ] **Step 5: Commit**

```bash
git add crates/vox-secrets/src/backend/vox_vault.rs
git commit -m "feat(secrets): add vault master key fingerprint helper for diagnostics"
```

---

### Task 2: Vault health probe

**Files:**
- Modify: `crates/vox-secrets/src/backend/vox_vault.rs`
- Modify: `crates/vox-secrets/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Add to `vault_health_tests`:

```rust
#[test]
#[allow(unsafe_code)]
fn probe_vault_health_reports_empty_vault_as_ok() {
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _g = ENV_LOCK.lock().expect("env lock");
    let tmp = tempfile::tempdir().expect("tempdir");
    let db_path = tmp.path().join("health_empty.db");
    unsafe {
        std::env::set_var("VOX_SECRETS_VAULT_PATH", &db_path);
        std::env::set_var("VOX_ACCOUNT_ID", "health-test-account");
    }
    let health = match super::VoxCloudBackend::new().and_then(|b| super::probe_vault_health(&b)) {
        Ok(h) => h,
        Err(e) => {
            let msg = e.to_string().to_lowercase();
            if msg.contains("keyring") || msg.contains("misconfigured") {
                return; // sandbox skip
            }
            panic!("unexpected init failure: {e}");
        }
    };
    unsafe {
        std::env::remove_var("VOX_SECRETS_VAULT_PATH");
        std::env::remove_var("VOX_ACCOUNT_ID");
    }
    assert!(health.can_decrypt, "empty vault should probe OK");
    assert_eq!(health.row_count, 0);
}

#[test]
#[allow(unsafe_code)]
fn probe_vault_health_fails_after_simulated_master_drift() {
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _g = ENV_LOCK.lock().expect("env lock");
    let tmp = tempfile::tempdir().expect("tempdir");
    let db_path = tmp.path().join("health_drift.db");
    unsafe {
        std::env::set_var("VOX_SECRETS_VAULT_PATH", &db_path);
        std::env::set_var("VOX_ACCOUNT_ID", "drift-test-account");
    }
    let backend = match super::VoxCloudBackend::new() {
        Ok(b) => b,
        Err(_) => {
            unsafe {
                std::env::remove_var("VOX_SECRETS_VAULT_PATH");
                std::env::remove_var("VOX_ACCOUNT_ID");
            }
            return;
        }
    };
    backend
        .write_secret("PROBE_CANARY", "canary-value-0123456789")
        .expect("write canary");
    // Simulate drift: corrupt master key in-memory (cannot rotate OS keyring in unit test)
    let mut drifted = backend;
    drifted.master_key = [0_u8; 32];
    let health = super::probe_vault_health(&drifted).expect("probe returns struct");
    unsafe {
        std::env::remove_var("VOX_SECRETS_VAULT_PATH");
        std::env::remove_var("VOX_ACCOUNT_ID");
    }
    assert!(!health.can_decrypt);
    assert!(health.decrypt_error.is_some());
    assert!(health.row_count >= 1);
}
```

Note: `master_key` is private — either add `#[cfg(test)] pub(crate) fn set_master_key_for_test` or make probe accept `&VoxCloudBackend` and use a test-only drift via separate backend instance. **Preferred:** add `#[cfg(test)] fn with_master_key_for_test(self, key: [u8; 32]) -> Self` on `VoxCloudBackend`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-secrets probe_vault_health -- --nocapture`

Expected: FAIL — `probe_vault_health` / `VaultHealth` not defined

- [ ] **Step 3: Add types and probe**

In `vox_vault.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultHealth {
    pub vault_path_display: String,
    pub keyring_entry_present: bool,
    pub master_fingerprint: String,
    pub account_id: String,
    pub kek_ref: String,
    pub kek_version: i64,
    pub row_count: u64,
    pub can_decrypt: bool,
    pub decrypt_error: Option<String>,
}

pub fn probe_vault_health(backend: &VoxCloudBackend) -> Result<VaultHealth, SecretError> {
    let vault_path_display = cloudless_vault_env_diagnostic();
    let keyring_entry_present = keyring::Entry::new("vox-secrets-vault", "master")
        .ok()
        .and_then(|e| e.get_password().ok())
        .is_some_and(|p| !p.is_empty());
    let master_fingerprint = master_key_fingerprint(&backend.master_key);
    let row_count = backend.count_account_secrets()?; // add simple COUNT(*) helper
    let (can_decrypt, decrypt_error) = match backend.try_decrypt_canary() {
        Ok(()) => (true, None),
        Err(e) => (false, Some(e.to_string())),
    };
    Ok(VaultHealth {
        vault_path_display,
        keyring_entry_present,
        master_fingerprint,
        account_id: backend.account_id.clone(),
        kek_ref: backend.kek_ref.clone(),
        kek_version: backend.kek_version,
        row_count,
        can_decrypt,
        decrypt_error,
    })
}
```

Implement helpers on `VoxCloudBackend`:
- `count_account_secrets()` — `SELECT COUNT(*) FROM clavis_secrets WHERE account_id = ?1`
- `try_decrypt_canary()` — if `row_count == 0`, return `Ok(())`; else read/decrypt first row (or dedicated `__vault_probe__` key if present)

Improve `decrypt_vault` error mapping:

```rust
.map_err(|e| {
    let msg = format!("{e}");
    if msg.contains("aead") {
        SecretError::BackendQueryFailed(format!(
            "decryption failed (master key mismatch?): {msg}. \
             Remediation: backup and delete the vault file, ensure OS keyring entry \
             vox-secrets-vault/master is stable, then re-run `vox secrets import-env`."
        ))
    } else {
        SecretError::BackendQueryFailed(format!("decryption failed: {msg}"))
    }
})
```

- [ ] **Step 4: Re-export from lib.rs**

```rust
pub use backend::vox_vault::{probe_vault_health, VaultHealth};
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p vox-secrets probe_vault_health master_key_fingerprint -- --nocapture`

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/vox-secrets/src/backend/vox_vault.rs crates/vox-secrets/src/lib.rs
git commit -m "feat(secrets): add Clavis vault health probe and decrypt remediation hints"
```

---

### Task 3: CLI `backend-status` uses health probe

**Files:**
- Modify: `crates/vox-cli/src/commands/secrets.rs:212-229`

- [ ] **Step 1: Write failing CLI test**

Add to `crates/vox-cli/tests/` (or extend existing secrets test if present):

```rust
#[test]
fn backend_status_prints_vault_health_lines() {
    // Smoke: command exits 0 and includes "secrets backend mode"
    // Full probe assertions require keyring — keep minimal
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_vox"))
        .args(["secrets", "backend-status"])
        .output()
        .expect("spawn vox");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("secrets backend mode"));
    // After implementation:
    assert!(stdout.contains("vault health:") || stdout.contains("backend status:"));
}
```

- [ ] **Step 2: Run test — expect fail on missing `vault health:`**

Run: `cargo test -p vox-cli backend_status_prints_vault_health -- --nocapture`

- [ ] **Step 3: Update `BackendStatus` handler**

Replace early-return loop with probe-first logic:

```rust
SecretsCmd::BackendStatus => {
    let mode = vox_secrets::BackendMode::from_env();
    println!("secrets backend mode: {mode:?}");
    println!("vault env: {}", vox_secrets::cloudless_vault_env_diagnostic());
    match vox_secrets::backend::vox_vault::VoxCloudBackend::new() {
        Ok(backend) => match vox_secrets::probe_vault_health(&backend) {
            Ok(h) => {
                println!(
                    "vault health: keyring={}; master_fp={}; rows={}; can_decrypt={}",
                    h.keyring_entry_present, h.master_fingerprint, h.row_count, h.can_decrypt
                );
                if !h.can_decrypt {
                    if let Some(err) = h.decrypt_error {
                        println!("backend status: unavailable ({err})");
                        return Ok(());
                    }
                }
                println!("backend status: available");
            }
            Err(e) => println!("vault health probe failed: {e}"),
        },
        Err(e) => println!("vault backend init failed: {e}"),
    }
    Ok(())
}
```

Export `cloudless_vault_env_diagnostic` from `vox_secrets` if not already public.

- [ ] **Step 4: Run test + manual check**

Run: `cargo test -p vox-cli backend_status_prints_vault_health -- --nocapture`

Manual:

```powershell
$env:VOX_SECRETS_VAULT_PATH='C:/Users/Owner/vox/.vox/clavis_vault.db'
cargo build -p vox-cli -q
target\debug\vox.exe secrets backend-status
```

Expected: `vault health: ... can_decrypt=true` after Task 0 recovery.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/secrets.rs crates/vox-cli/tests/
git commit -m "feat(cli): surface Clavis vault health in secrets backend-status"
```

---

### Task 4: Import-env round-trip integration test

**Files:**
- Modify: `crates/vox-secrets/src/tests.rs`

- [ ] **Step 1: Write failing test**

```rust
#[test]
#[allow(unsafe_code)]
fn import_env_round_trips_sonatype_guide_token_via_temp_vault() {
    let _g = ENV_LOCK.lock().expect("env lock");
    let tmp_dir = tempfile::tempdir().expect("tempdir");
    let db_path = tmp_dir.path().join("import_env_vault.db");
    let env_file = tmp_dir.path().join("import.env");
    std::fs::write(&env_file, "SONATYPE_GUIDE_TOKEN=sgt_test_import_roundtrip_0123456789\n")
        .expect("write env file");

    let prev_path = std::env::var("VOX_SECRETS_VAULT_PATH").ok();
    let prev_account = std::env::var("VOX_ACCOUNT_ID").ok();
    let prev_cutover = std::env::var("VOX_SECRETS_CUTOVER_PHASE").ok();
    unsafe {
        std::env::set_var("VOX_SECRETS_VAULT_PATH", &db_path);
        std::env::set_var("VOX_ACCOUNT_ID", "import-env-test-account");
        std::env::set_var("VOX_SECRETS_CUTOVER_PHASE", "decommission");
        std::env::remove_var("SONATYPE_GUIDE_TOKEN");
    }

    let result = match crate::import_env_from_path(&env_file, true) {
        Ok(r) => r,
        Err(e) => {
            let msg = e.to_string().to_lowercase();
            if msg.contains("keyring") || msg.contains("misconfigured") {
                // restore env and skip
                restore_import_env_test_env(prev_path, prev_account, prev_cutover);
                return;
            }
            panic!("import failed: {e}");
        }
    };
    assert_eq!(result.count(), 1);
    assert_eq!(result.entries[0].canonical_env, "SONATYPE_GUIDE_TOKEN");

    let resolved = crate::resolve_secret(SecretId::SonatypeGuideToken);
    restore_import_env_test_env(prev_path, prev_account, prev_cutover);

    let exposed = resolved.expose().map(|s| s.to_string());
    assert_eq!(
        exposed.as_deref(),
        Some("sgt_test_import_roundtrip_0123456789"),
        "resolve after import-env should read vault, got status {:?}",
        resolved.status
    );
}
```

Add `restore_import_env_test_env` helper in the same module to avoid duplication.

- [ ] **Step 2: Run test — expect FAIL until import+resolve path works**

Run: `cargo test -p vox-secrets import_env_round_trips_sonatype -- --nocapture`

- [ ] **Step 3: Fix any resolution gaps** (only if test fails for code reasons, not keyring skip)

Common fixes:
- Ensure `VOX_SECRETS_CUTOVER_PHASE=decommission` forces vox_cloud backend (already used in sibling tests)
- Ensure `SecretId::SonatypeGuideToken` spec `backend_key` / `canonical_env` is `SONATYPE_GUIDE_TOKEN`

- [ ] **Step 4: Run test — PASS or clean skip**

- [ ] **Step 5: Commit**

```bash
git add crates/vox-secrets/src/tests.rs
git commit -m "test(secrets): import-env round-trip for SonatypeGuideToken via temp vault"
```

---

### Task 5: Document operator recovery in secrets SSOT

**Files:**
- Modify: `docs/src/reference/secrets-ssot.md` (add subsection under Resolution Precedence)

- [ ] **Step 1: Add subsection**

```markdown
### Local Clavis vault troubleshooting (Windows)

| Symptom | Likely cause | Recovery |
| --- | --- | --- |
| `invalid filename` on `backend-status` | Stale `vox.exe` or `file:` URL passed to Turso | Rebuild `vox-cli`; ensure `VOX_SECRETS_VAULT_PATH` is a filesystem path env var (not a `file:` URL) |
| `decryption failed: aead::Error` | OS keyring master drift vs vault ciphertext | Backup → delete `.vox/clavis_vault.db` → `vox secrets import-env --file <env>` |
| `secrets set` wrote auth.json but vault empty | `set` targets auth store, not Clavis | Use `import-env` for vault writes |
| `secrets get` misses vault token | Spec has `auth_registry: None` | Use env var, vault via `resolve_secret`, or wire `auth_registry` |

Keyring entry: service `vox-secrets-vault`, user `master`. **Never** commit vault files or `.env` imports.
```

- [ ] **Step 2: Lint doc**

Run: `cargo run -p vox-doc-pipeline -- --lint-only --paths docs/src/reference/secrets-ssot.md`

Expected: PASS (frontmatter already present)

- [ ] **Step 3: Commit**

```bash
git add docs/src/reference/secrets-ssot.md
git commit -m "docs(secrets): add Clavis vault decryption recovery runbook"
```

---

### Task 6 (Optional): Wire `SonatypeGuideToken` auth_registry

**Files:**
- Modify: `crates/vox-secrets/src/spec/registry/platform.rs`
- Modify: `crates/vox-secrets/src/tests.rs`

- [ ] **Step 1: Write failing test** — `secrets set` registry name resolves via `get_registry_token` pattern

Follow `OpenRouterApiKey` / `auth_registry: Some("openrouter")` precedent in `registry/llm.rs`.

```rust
// In platform.rs SonatypeGuideToken entry:
auth_registry: Some("sonatype"),
```

- [ ] **Step 2: Implement + run `cargo test -p vox-secrets`**

- [ ] **Step 3: Commit**

```bash
git commit -m "feat(secrets): wire SonatypeGuideToken auth_registry for secrets set parity"
```

---

## Verification checklist (end of plan)

- [ ] `cargo test -p vox-secrets path_url_tests probe_vault_health import_env_round_trips -- --nocapture`
- [ ] `cargo test -p vox-cli backend_status_prints_vault_health -- --nocapture`
- [ ] `cargo build -p vox-cli` with no `vox.exe` lock (stop stale processes first)
- [ ] `vox secrets backend-status` → `can_decrypt=true` after Task 0 recovery
- [ ] `vox ci secret-env-guard` and `vox ci secrets-parity` (if spec/env registry touched)

---

## Self-review (completed)

| Check | Result |
|-------|--------|
| Spec coverage: path fix | Prerequisite noted; Task 0 verifies |
| Spec coverage: decryption recovery | Tasks 0, 2, 5 |
| Spec coverage: diagnostics | Tasks 1–3 |
| Spec coverage: import-env test | Task 4 |
| Spec coverage: Sonatype auth_registry | Task 6 (optional) |
| Placeholder scan | No TBD/TODO steps |
| Type/name consistency | `VaultHealth`, `probe_vault_health`, `master_key_fingerprint` used consistently |
