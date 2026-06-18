# Session Handoff — Vault Decryption Recovery + Branch Context (start here)

> **You do NOT need any prior conversation.** Read this document first, then the implementation plan at [`2026-06-16-vault-decryption-recovery.md`](2026-06-16-vault-decryption-recovery.md). For the mega-branch’s other tracks (pipeline SSOT, skill bundle, GUI), see [Quick navigation](#quick-navigation-other-tracks-on-this-branch) below.

**Branch:** `feat/vault-decryption-recovery`  
**Vault plan tasks 1–6:** **committed** (5 commits listed below)  
**Vault plan Task 0 (operator recovery):** **NOT done** — local machine still fails decrypt  
**Last updated:** 2026-06-16

**Do NOT commit unless the human operator explicitly asks.** (Vault commits on this branch were made during an approved plan execution; do not add more drive-by commits.)

---

## Human intent

1. **Primary goal:** Fix Windows Clavis vault integration so `vox secrets import-env` and `vox secrets backend-status` work with absolute `VOX_SECRETS_VAULT_PATH` and a stable OS keyring master key.
2. **Secondary goal (same session arc):** Dependency/security audit (P1 bumps, Sonatype token, `cargo audit`, MCP batch) — largely verified; see [Dependency audit state](#dependency-audit-state).
3. **Execution style:** TDD + [`subagent-driven-development`](../../.claude/plugins/cache/superpowers-marketplace/superpowers/5.0.7/skills/subagent-driven-development/SKILL.md) / [`dispatching-parallel-agents`](../../.claude/plugins/cache/superpowers-marketplace/superpowers/5.0.7/skills/dispatching-parallel-agents/SKILL.md). Subagents hit **usage limits** late in the session; parent agent finished Tasks 3–6 inline.

**Binding constraints (always):**

- Automation = **VoxScript only** (`vox run scripts/…`); no new `.ps1`/`.sh`/`.py` glue.
- **No `cargo fmt --all`** on Windows — use `vox run scripts/fmt.vox`.
- Secrets via `vox_secrets::resolve_secret(...)` only; never `std::env::var("API_KEY")` in consumers.
- TDD for new `pub fn`; do not hand-regenerate SSOT after merge (fix source; CI bot regens).
- This branch mixes **many unrelated WIP tracks** — prefer **scoped commits/PRs**; see [PR split](#suggested-pr-split).

---

## Vault architecture (must understand)

```
import-env / store_secret
    → VoxCloudBackend::new()
        → open_cloudless_connection()     # Turso local file; VOX_SECRETS_VAULT_PATH
        → derive_master_key()             # OS keyring: service "vox-secrets-vault", user "master"
        → write_secret_v2()             # DEK wrapped with KEK derived from master + kek_ref + kek_version

resolve_secret / backend-status
    → same VoxCloudBackend::new()
    → unwrap_dek + decrypt_vault         # fails with aead::Error if master key ≠ encryption key
```

**Two stores — do not conflate:**

| Command / API | Writes to | Reads via `resolve_secret`? |
|---|---|---|
| `vox secrets set` | `~/.vox/auth.json` + per-registry keyring | Only if spec has `auth_registry: Some(...)` |
| `vox secrets import-env` | `.vox/clavis_vault.db` (Clavis) | Yes (when vault backend active) |
| `vox secrets store_secret` (API) | Clavis vault | Yes |

**SonatypeGuideToken:** `auth_registry: Some("sonatype")` was wired in commit `c188590181` so `secrets set sonatype …` aligns with registry resolution; vault path still requires `import-env` or `store_secret`.

---

## What landed (vault track — committed)

| Commit | Message |
|--------|---------|
| `9af1d0713e` | `feat(secrets): add vault master key fingerprint helper for diagnostics` |
| `cc87a53cc3` | `feat(secrets): add Clavis vault health probe and decrypt remediation hints` |
| `34c475d52c` | `feat(cli): surface Clavis vault health in secrets backend-status` |
| `c188590181` | `test(secrets): import-env round-trip and Sonatype auth_registry` |
| `da9c8afc50` | `docs(secrets): add Clavis vault decryption recovery runbook` |

### Code touchpoints

| File | Responsibility |
|------|----------------|
| `crates/vox-secrets/src/backend/vox_vault.rs` | `file_url_to_local_path`, `probe_vault_health`, `VaultHealth`, `master_key_fingerprint`, decrypt remediation text, WAL PRAGMA removed for Turso 0.6 |
| `crates/vox-secrets/src/lib.rs` | Re-exports `probe_vault_health`, `VaultHealth`, `cloudless_vault_env_diagnostic` |
| `crates/vox-cli/src/commands/secrets.rs` | `BackendStatus` prints vault env + health probe |
| `crates/vox-cli/tests/secrets_backend_status_test.rs` | Integration smoke: stdout contains `vault health:` |
| `crates/vox-secrets/src/tests.rs` | `import_env_round_trips_sonatype_guide_token_via_temp_vault` |
| `crates/vox-secrets/src/spec/registry/platform.rs` | `SonatypeGuideToken` → `auth_registry: Some("sonatype")` |
| `docs/src/reference/secrets-ssot.md` | §Local Clavis vault troubleshooting (Windows) |

### Tests verified in session

```powershell
cargo test -p vox-secrets path_url_tests                    # 9/9 pass
cargo test -p vox-secrets master_key_fingerprint            # 2/2 pass
cargo test -p vox-secrets probe_vault_health                 # 2/2 pass (in vault_health_tests)
cargo test -p vox-secrets import_env_round_trips_sonatype    # 1/1 pass
cargo test -p vox-cli --test secrets_backend_status_test      # run explicitly (see gotchas)
```

**Parallel agent [Vault secrets tests](19e9e6cd-567b-480e-872f-3ba642487051) (2026-06-16, uncommitted):** `cargo test -p vox-secrets` green after removing duplicate `VoxSyndicationTemplateProfileEnabled` spec (`platform.rs`, `ids.rs`) and fixing `redact_replaces_secret_in_json_string` in `semcov_wave45_tests.rs`. Include in next vault-scoped commit if still in working tree.

---

## Current operator machine state (2026-06-16)

**Vault file exists:** `C:\Users\Owner\vox\.vox\clavis_vault.db`  
**Import env file:** `.vox/import-secrets.env` — **does not exist** (operator must create; gitignored)

**Live `vox secrets backend-status` output (after code landed):**

```
secrets backend mode: Auto
vault env: mode=local_file; url_source=default_file; ...
vault health: keyring=false; master_fp=fa9718b5ba353770; rows=1; can_decrypt=false
backend status: unavailable (decryption failed (master key mismatch?): ... Remediation: backup and delete the vault file, ensure OS keyring entry vox-secrets-vault/master is stable, then re-run `vox secrets import-env`.)
```

**Interpretation:**

- Path fix works (no `invalid filename`).
- `keyring=false` — probe could not read a non-empty `vox-secrets-vault`/`master` password (Windows Credential Manager visibility or missing entry).
- `rows=1` — stale ciphertext in vault from a prior import under a different master.
- **Root cause (parallel agent [Vault Task 0 recovery](9d56e125-8657-40a2-852b-93df6bf1fe86), 2026-06-16):** `derive_master_key()` in `vox_vault.rs` generates a **new random bootstrap master on every process** when keyring `get_password` fails — even if `set_password` appeared to succeed. Import encrypts with K₁; the next `backend-status` decrypts with K₂ → `master_fp` rotates each run. Unlike `auth_json.rs`, there is **no file fallback** for vault master persistence.
- **Task 0 cannot complete in agent/sandbox shells** until either (a) interactive user PowerShell with working keyring round-trip, or (b) code adds `.vox` master file fallback + keyring write verify (mirror `write_registry_token` pattern).

---

## Remaining work — vault track

### Task 0: Operator recovery (P0 — human + agent)

From plan [`2026-06-16-vault-decryption-recovery.md`](2026-06-16-vault-decryption-recovery.md) Task 0:

1. Stop stale `vox` processes (unblocks `vox.exe` rebuild on Windows).
2. Backup + delete `.vox/clavis_vault.db`.
3. Create **gitignored** `.vox/import-secrets.env` with managed keys (at minimum `SONATYPE_GUIDE_TOKEN=…`; add others as needed). **Never commit.**
4. `vox secrets import-env --file .vox/import-secrets.env`
5. Optionally set `$env:VOX_SECRETS_VAULT_PATH = 'C:\Users\Owner\vox\.vox\clavis_vault.db'`
6. Confirm `vox secrets backend-status` → `can_decrypt=true`, `backend status: available`
7. Confirm `SONATYPE_GUIDE_TOKEN` resolves (env fallback OK for MCP until vault healthy)

**If keyring stays `false` after import:** investigate Windows Credential Manager — service name `vox-secrets-vault`, user `master`. `derive_master_key()` in `vox_vault.rs:1234` creates the entry on first successful write when keyring works.

### Optional code follow-ups (P1 — from review, not in plan)

| Item | Rationale | File |
|------|-----------|------|
| Canary probes first row only | Delete-vault remediation may miss profile overrides | `vox_vault.rs` `try_decrypt_canary` |
| `keyring=false` vs entry exists | Probe uses `get_password().ok()` — may need Windows-specific diagnostic | `probe_vault_health` |
| Rebuild stale global `vox` | Session saw `vox upgrade` / `cargo install` freshness warnings | Operator shell |
| `secrets doctor` still nags import-env | May not reflect successful import; align doctor with `probe_vault_health` | `secrets.rs` doctor path |

### Verification checklist (end of plan)

- [ ] Task 0 complete on operator machine
- [ ] `cargo test -p vox-secrets path_url_tests probe_vault_health import_env_round_trips`
- [ ] `cargo test -p vox-cli --test secrets_backend_status_test`
- [ ] `vox ci secret-env-guard` + `vox ci secrets-parity` if touching spec/env registry again

---

## Dependency audit state (parallel session arc)

**Done / verified:**

| Check | Result |
|-------|--------|
| `cargo audit` | Exit 0 — **0 vulnerability advisories**; 25 allowed warnings (unmaintained GTK, transitive `lru@0.12.5` via **tantivy@0.22.1**, etc.) |
| `pnpm audit` (vox-gui/ui) | 0 vulnerabilities |
| P1 bumps in tree | `jsonwebtoken=10`, `quick-xml=0.40`, `lru=0.16`, `notify=8`, `wasmtime=45` |
| SonatypeGuideToken spec | Registered in `vox-secrets` spec + env registry |
| Sonatype MCP (HTTP) | Token works when User env set; IDE MCP may need Cursor restart |
| Sonatype at lock versions | jsonwebtoken **10.4.0** clean; lru **0.16.4** clean |

**Remaining (P2 backlog — separate PR from vault):**

- Transitive `lru@0.12.5` — upgrade **tantivy** or wait upstream
- `reqwest` 0.13 wave, `tokio-tungstenite` 0.24→0.29, duplicate-chain hygiene
- Sonatype MCP in Cursor — configure bearer / restart after token in User env
- Handoff reference: [`2026-06-05-dependency-currency-upgrade-handoff.md`](2026-06-05-dependency-currency-upgrade-handoff.md)

---

## Windows / agent gotchas (learned this session)

1. **Cargo file lock storms:** Parallel agents + rust-analyzer spawn dozens of `cargo`/`rustc` jobs. Fix: `taskkill /F /IM cargo.exe /T` and `rustc.exe`, stop `vox.exe`, use `$env:CARGO_BUILD_JOBS='1'`, optional `$env:CARGO_TARGET_DIR='target-ci-parallel'`.
2. **`vox.exe` locked:** `failed to remove file …\target\debug\vox.exe — Access is denied`. Kill all `vox` processes before `cargo build -p vox-cli`.
3. **Subagent usage limits:** Parallel `Task` subagents may fail with “out of usage”; fall back to inline implementation or human retry later.
4. **PowerShell commits:** No bash heredoc; use `git commit -m "message"` directly.
5. **Pre-push hook:** May run line-endings + stale `vox` warnings; use fresh `target\debug\vox.exe` after build.
6. **Do not read** `docs/src/archive/` for planning.

---

## Verification cheat sheet

```powershell
$env:CARGO_BUILD_JOBS = '1'
$cargo = "$env:USERPROFILE\.cargo\bin\cargo.exe"

# Kill locks
Get-Process vox -EA SilentlyContinue | Stop-Process -Force

# Vault tests
& $cargo test -p vox-secrets path_url_tests probe_vault_health import_env_round_trips_sonatype

# CLI integration (explicit test crate)
& $cargo test -p vox-cli --test secrets_backend_status_test -- --nocapture

# Rebuild + manual smoke
& $cargo build -p vox-cli -q
& .\target\debug\vox.exe secrets backend-status

# Operator recovery
& .\target\debug\vox.exe secrets import-env --file .vox\import-secrets.env

# Bootstrap if vox not on PATH
pwsh -File scripts/windows/vox-dev.ps1 secrets backend-status
```

---

## Suggested PR split

This branch has **~299 modified + ~155 untracked** files beyond vault. Strongly recommend:

1. **PR: Vault + secrets only** — the 5 commits above (+ Task 0 verification notes in PR body). Base: `main`.
2. **PR: Pipeline SSOT** — `pipeline_parity.rs`, `canonical-ladder.v1.yaml`, emission profile (see pipeline handoff).
3. **PR: Skill bundle** — `assets/skills/`, catalog, scripts.
4. **PR: GUI / scientia / graphify** — orthogonal surfaces.

Do **not** merge the mega-branch as one PR.

---

## Quick navigation (other tracks on this branch)

| Track | Handoff / plan |
|-------|----------------|
| **MCP / skills parity cleanup** | [`2026-06-16-mcp-skills-cleanup-HANDOFF.md`](2026-06-16-mcp-skills-cleanup-HANDOFF.md) |
| Pipeline / emission ladder | [`2026-06-16-pipeline-ssot-HANDOFF-STATE.md`](2026-06-16-pipeline-ssot-HANDOFF-STATE.md) |
| Universal skill bundle | [`2026-06-16-universal-skill-bundle-cursor-import.md`](2026-06-16-universal-skill-bundle-cursor-import.md) |
| GUI roadmap | [`2026-06-16-gui-roadmap-remaining.md`](2026-06-16-gui-roadmap-remaining.md) |
| Config registry | [`2026-06-15-config-registry-HANDOFF-STATE.md`](2026-06-15-config-registry-HANDOFF-STATE.md) |
| Dependency currency | [`2026-06-05-dependency-currency-upgrade-handoff.md`](2026-06-05-dependency-currency-upgrade-handoff.md) |
| Secrets SSOT (operator) | [`docs/src/reference/secrets-ssot.md`](../../src/reference/secrets-ssot.md) |
| Where code lives | [`docs/src/architecture/where-things-live.md`](../../src/architecture/where-things-live.md) |

---

## Agent execution protocol

1. Read **this doc** (10 min) → open **vault plan** if doing Task 0 or vault fixes only.
2. If assigned a **non-vault track**, open that track’s handoff; do not drive-by edit vault files.
3. Run verification for your scope before claiming done.
4. **Do not commit** until human asks; use conventional commits and atomic chunks when approved.
5. Prefer `vox ci pre-push --complete --since <ref>` when touching 1–3 crates.
6. For new vault behavior: **TDD first** (`test-driven-development` skill).

---

## Definition of done — vault track

- [x] Absolute path opens vault (no `invalid filename`)
- [x] Health probe + CLI diagnostics + remediation text
- [x] Hermetic import-env round-trip test
- [x] Sonatype `auth_registry` wired
- [x] Operator runbook in secrets SSOT
- [ ] **Task 0:** Operator machine `can_decrypt=true` after re-import
- [ ] Optional: PR opened with vault-only commits cherry-picked or branch trimmed

---

## Session transcripts (for archaeology)

- Vault / dependency parallel work (parent): search agent transcripts under `.cursor/projects/c-Users-Owner-vox/agent-transcripts/` for conversations mentioning `vault-decryption-recovery` or `clavis_vault`.
- Implementation plan: [`2026-06-16-vault-decryption-recovery.md`](2026-06-16-vault-decryption-recovery.md)
