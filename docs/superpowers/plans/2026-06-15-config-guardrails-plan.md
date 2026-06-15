# Config Guardrails Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the guardrail layer that must land BEFORE the main "full configurability" plan executes — refactor the three structural defects the adversarial audit found, then add the enforcement (a `vox ci config-hygiene` gate) and the unified registry SSOT that makes config single-homed, searchable, and GUI-generated.

**Architecture:** Three phases. (0) Refactor the defective config so it is correct *before* more config is added: embed shipped contracts at compile time so loaders are never silently inert; delete dead/unwired config; make safety limits safe-by-default. (1) A `vox ci config-hygiene` gate with three machine checks that *prevent* the executor (Sonnet 4.6) from re-introducing the defects. (2) A single config registry (`contracts/config/registry.v1.yaml`) that every operational knob must appear in, enforced by a parity gate, and from which the GUI settings list is generated — so "searchable + visible in GUI" is a property of one SSOT, not 800 scattered env reads.

**Tech Stack:** Rust (`vox-config`, `vox-gamify`, `vox-orchestrator`, `vox-container-types`, `vox-cli`'s `commands/ci`), `serde_yaml`/`serde_yaml_ng`, `include_str!`, the existing `vox ci` gate framework (`crates/vox-cli/src/commands/ci/`), and the GUI `settingsIndex.ts` surface.

**Why this sits ON TOP of the main plan:** The main plan's stated target ("reduce magic values to zero, surface all in the GUI") will, executed mechanically by a less-context-capable model, amplify the audit's own top findings (split-brain, declared-but-unwired). This plan converts the implicit "unless it never needs configuring" clause into *enforced* rules and gives the executor a single registry to write into instead of scattering more `DEFAULT_*` consts. **Do not start the main plan until Phase 1 (the gate) is green** — the gate is the seatbelt.

**Audit evidence this addresses (measured on `origin/main`):** ~801 distinct `VOX_*` env reads, 87 `contracts/*.yaml`, 118 `DEFAULT_*` consts, 29 GUI `constants.ts` exports (fragmentation, not under-configuration); `CostDefenseConfig::from_env_values` has zero non-test callers (dead config); `resolve_economy()` and `CircuitBreakerConfig::from_contract_file(...)` fall back to `Default` in any non-repo-root binary (inert-in-production); `RunOpts::default()` is unbounded (cpus/memory/pids = `None`).

---

## Phase 0 — Refactor the three critical defects

### Task 1: Embed the gamify economy contract at compile time (fix inert-in-production + drift)

`resolve_economy()` resolves the contract via a cwd-relative path and a `CARGO_MANIFEST_DIR`-relative path; in a deployed binary both miss and it silently returns `EconomyConfig::default()`. Refactor so the shipped contract is **embedded at compile time** via `include_str!` — the embedded text becomes the canonical default (so the contract is *always* the live source, and there is one source, not a drift-prone `Default` impl). A runtime override (env path) still layers on top.

**Files:**
- Modify: `crates/vox-gamify/src/economy.rs`
- Test: inline `#[cfg(test)]` in the same file

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod` in `crates/vox-gamify/src/economy.rs`:

```rust
    #[test]
    fn embedded_contract_parses_and_is_canonical_default() {
        // The shipped contract is compiled into the binary, so this never touches
        // the filesystem and can never be silently inert in a deployed binary.
        let embedded = EconomyConfig::embedded();
        // The embedded contract must reproduce the in-code defaults exactly
        // (proves the YAML SSOT and the Default impl have not drifted).
        let def = EconomyConfig::default();
        assert_eq!(embedded.tuning.novelty_factor, def.tuning.novelty_factor);
        assert_eq!(embedded.trust_tier_multipliers, def.trust_tier_multipliers);
        // And it must carry the full reward table (richer than Default's empty map).
        assert!(!embedded.rewards.is_empty(), "embedded contract must carry rewards");
    }

    #[test]
    fn resolve_uses_embedded_when_no_override() {
        // With no env override and no cwd file, resolution must yield the embedded
        // contract (NOT a bare Default with an empty reward table).
        let cfg = resolve_economy_from(None, |_p| false /* pretend no file exists */);
        assert!(!cfg.rewards.is_empty(), "must fall back to embedded, not empty Default");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-gamify economy::tests::embedded_contract_parses_and_is_canonical_default economy::tests::resolve_uses_embedded_when_no_override`
Expected: FAIL — `no function or associated item named embedded` / `resolve_economy_from`.

- [ ] **Step 3: Implement the embedded default + testable resolver**

In `crates/vox-gamify/src/economy.rs`, add near the existing `SHIPPED_CONTRACT_RELPATH` const:

```rust
/// The shipped economy contract, compiled into the binary. Path is relative to
/// THIS source file (`crates/vox-gamify/src/economy.rs`) → repo-root `contracts/`.
/// Compile-time embedding means the contract is ALWAYS the live default — it can
/// never be silently inert in a deployed binary, and there is a single source of
/// truth (no `Default`-vs-contract drift).
const EMBEDDED_ECONOMY_YAML: &str = include_str!("../../../contracts/gamify/economy.v1.yaml");
```

Add these methods in the `impl EconomyConfig` block:

```rust
    /// Parse the compile-time-embedded shipped contract. Panics only if the
    /// committed contract is malformed — which the parity test forbids.
    pub fn embedded() -> Self {
        parse_economy(EMBEDDED_ECONOMY_YAML)
            .expect("embedded economy.v1.yaml must parse (guarded by config-registry-parity)")
    }
```

Refactor `resolve_economy` to delegate to a pure, testable core and use the embedded contract as the floor (replace the body of `resolve_economy`):

```rust
pub fn resolve_economy() -> EconomyConfig {
    resolve_economy_from(
        std::env::var("VOX_GAMIFY_ECONOMY_PATH").ok().map(std::path::PathBuf::from),
        |p| p.exists(),
    )
}

/// Testable core: an explicit override path (if any) wins when it exists and
/// parses; otherwise the COMPILE-TIME-EMBEDDED contract is used (never a bare
/// `Default`). `exists` is injected so tests don't depend on the filesystem.
pub(crate) fn resolve_economy_from(
    override_path: Option<std::path::PathBuf>,
    exists: impl Fn(&std::path::Path) -> bool,
) -> EconomyConfig {
    if let Some(p) = override_path {
        if exists(&p) {
            match load_economy(&p) {
                Ok(cfg) => {
                    tracing::debug!("loaded gamify economy override from {}", p.display());
                    return cfg;
                }
                Err(e) => tracing::warn!(error = %e, path = %p.display(),
                    "gamify economy override failed to parse; using embedded contract"),
            }
        }
    }
    EconomyConfig::embedded()
}
```

Delete the now-unused `CONTRACT_WORKSPACE_RELPATH` cwd-relative const and any cwd-relative candidate logic. Keep `SHIPPED_CONTRACT_RELPATH` only if a test still references it; otherwise delete it too.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vox-gamify economy`
Expected: PASS (all existing economy tests + the 2 new ones). If `include_str!` fails to compile, the relative path is wrong — fix it relative to `economy.rs` (it is three levels up to repo root).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gamify/src/economy.rs
git commit -m "fix(gamify): embed economy contract at compile time so it is never inert in production"
```

---

### Task 2: Embed the circuit-breaker contract at compile time (fix inert-in-production + drift)

`CircuitBreakerConfig::from_contract_file(Path::new("contracts/orchestration/circuit-breaker.v1.yaml"))` is cwd-relative — it loads only when the process runs from the repo root and silently uses `Default` everywhere else. Embed the contract and make it the canonical default.

**Files:**
- Modify: `crates/vox-orchestrator/src/circuit_breaker.rs`
- Modify: `crates/vox-orchestrator/src/orchestrator_policy.rs:162-164` (the production construction site)
- Test: inline `#[cfg(test)]` in `circuit_breaker.rs`

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` in `crates/vox-orchestrator/src/circuit_breaker.rs`:

```rust
    #[test]
    fn embedded_contract_is_canonical_and_matches_default() {
        // Compile-time embed: never reads the filesystem, never silently inert.
        let embedded = CircuitBreakerConfig::embedded();
        let def = CircuitBreakerConfig::default();
        assert_eq!(embedded.no_progress_threshold, def.no_progress_threshold);
        assert_eq!(embedded.tool_thrash_threshold, def.tool_thrash_threshold);
        assert_eq!(embedded.replan_limit, def.replan_limit);
        assert_eq!(embedded.ngram_overlap_threshold, def.ngram_overlap_threshold);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator circuit_breaker::tests::embedded_contract_is_canonical_and_matches_default`
Expected: FAIL — `no function or associated item named embedded`.

- [ ] **Step 3: Implement embedded default + override**

In `crates/vox-orchestrator/src/circuit_breaker.rs`, add the embedded const near the top:

```rust
/// Shipped circuit-breaker contract, compiled into the binary (path relative to
/// THIS file → repo-root `contracts/`). Embedded = always live, never inert.
const EMBEDDED_CIRCUIT_BREAKER_YAML: &str =
    include_str!("../../../contracts/orchestration/circuit-breaker.v1.yaml");
```

Add to the `impl CircuitBreakerConfig` block:

```rust
    /// Parse the compile-time-embedded contract. Panics only if the committed
    /// contract is malformed (guarded by config-registry-parity).
    pub fn embedded() -> Self {
        Self::from_contract_str(EMBEDDED_CIRCUIT_BREAKER_YAML)
            .expect("embedded circuit-breaker.v1.yaml must parse")
    }

    /// Resolve the live config: explicit override file (env) wins when present
    /// and parseable; otherwise the embedded contract. Never silently inert.
    pub fn resolve() -> Self {
        if let Ok(p) = std::env::var("VOX_CIRCUIT_BREAKER_CONTRACT") {
            let path = std::path::PathBuf::from(p);
            if path.exists() {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    if let Ok(cfg) = Self::from_contract_str(&text) {
                        return cfg;
                    }
                    tracing::warn!(path = %path.display(),
                        "circuit-breaker override failed to parse; using embedded contract");
                }
            }
        }
        Self::embedded()
    }
```

In `crates/vox-orchestrator/src/orchestrator_policy.rs:162-164`, replace the cwd-relative `from_contract_file(...)` call with the embedded-backed resolver:

```rust
            circuit_breaker: CircuitBreakerConfig::resolve(),
```

(Keep `from_contract_str` and `from_contract_file` for tests/overrides; they are no longer the production entry point.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vox-orchestrator circuit_breaker`
Expected: PASS (existing loader/drift tests + the new embedded test).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator/src/circuit_breaker.rs crates/vox-orchestrator/src/orchestrator_policy.rs
git commit -m "fix(orchestrator): embed circuit-breaker contract; resolve() is never inert in production"
```

---

### Task 3: Remove the dead cost-defense env resolver (YAGNI) and record the knob in the registry

`CostDefenseConfig::from_env_values` has zero non-test callers — `VOX_COST_DAILY_BUDGET_USD` does nothing. The cost-defense layer has no live consumer, so the resolver is speculative dead config (the exact anti-pattern the audit named). Remove it; the budget knobs will live in the registry (Phase 2) as `declared` entries to be wired when a consumer exists.

**Files:**
- Modify: `crates/vox-scaling-policy/src/cost_defense.rs` (remove `from_env_values` + its `budget_env_tests`)

- [ ] **Step 1: Confirm there is no production caller**

Run: `rg -n "from_env_values" crates --glob '!**/cost_defense.rs'`
Expected: no output (the only references are inside `cost_defense.rs` tests). If a non-test caller IS found, STOP — do not remove; instead this task changes to "wire that caller through `from_env_values`" and skip the deletion. Record which path it was.

- [ ] **Step 2: Remove the dead resolver and its tests**

In `crates/vox-scaling-policy/src/cost_defense.rs`, delete the entire `impl CostDefenseConfig { pub fn from_env_values(...) {...} }` block and the entire `#[cfg(test)] mod budget_env_tests { ... }` block. Leave `CostDefenseConfig`, its fields, and its `Default` impl untouched.

- [ ] **Step 3: Verify the crate still builds and tests pass**

Run: `cargo test -p vox-scaling-policy`
Expected: PASS (the removed resolver and its tests are gone; nothing else referenced them).

- [ ] **Step 4: Commit**

```bash
git add crates/vox-scaling-policy/src/cost_defense.rs
git commit -m "refactor(scaling-policy): remove dead cost-defense env resolver (no consumer; YAGNI)"
```

> The daily/monthly budget knobs are not lost — Task 9 adds them to the registry as `status: declared` entries with `owner_crate: vox-scaling-policy`, so they are tracked in the SSOT and wired the day the cost-defense layer gains a live consumer.

---

### Task 4: Make container resource limits safe-by-default (configurable ≠ unbounded)

`RunOpts::default()` leaves cpus/memory/pids `None` (unbounded). Add a `RunOpts::sandboxed()` constructor with safe default limits and route the untrusted-image run path through it, so the DoS surface is closed by default while remaining overridable.

**Files:**
- Modify: `crates/vox-container-types/src/runtime.rs` (add `sandboxed()` + safe-default consts)
- Modify: `crates/vox-cli/src/commands/container.rs` (the `Run` arm: start from `sandboxed()`, let explicit flags override)
- Test: inline `#[cfg(test)]` in `runtime.rs`

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-container-types/src/runtime.rs`:

```rust
#[cfg(test)]
mod sandboxed_tests {
    use super::*;

    #[test]
    fn sandboxed_sets_safe_limits() {
        let o = RunOpts::sandboxed();
        assert_eq!(o.cpus.as_deref(), Some(DEFAULT_SANDBOX_CPUS));
        assert_eq!(o.memory.as_deref(), Some(DEFAULT_SANDBOX_MEMORY));
        assert_eq!(o.pids_limit, Some(DEFAULT_SANDBOX_PIDS));
        // sandboxed() emits the limit flags by default:
        assert!(!o.resource_args().is_empty());
    }

    #[test]
    fn plain_default_stays_unbounded_for_explicit_opt_out() {
        // Default remains unbounded so trusted/internal callers can opt out
        // deliberately; the SANDBOX path is what untrusted images get.
        assert!(RunOpts::default().resource_args().is_empty());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-container-types sandboxed_tests`
Expected: FAIL — `no function or associated item named sandboxed` / missing consts.

- [ ] **Step 3: Implement safe-default consts + constructor**

In `crates/vox-container-types/src/runtime.rs`, add above `impl RunOpts`:

```rust
/// Safe default CPU quota for untrusted-image runs (`--cpus`).
pub const DEFAULT_SANDBOX_CPUS: &str = "2";
/// Safe default memory cap for untrusted-image runs (`--memory`).
pub const DEFAULT_SANDBOX_MEMORY: &str = "2g";
/// Safe default process cap for untrusted-image runs (`--pids-limit`).
pub const DEFAULT_SANDBOX_PIDS: u32 = 512;
```

Add to the `impl RunOpts` block:

```rust
    /// `RunOpts` with safe resource limits applied — the correct starting point
    /// for running untrusted/external images. Callers may override any field.
    pub fn sandboxed() -> Self {
        Self {
            cpus: Some(DEFAULT_SANDBOX_CPUS.to_string()),
            memory: Some(DEFAULT_SANDBOX_MEMORY.to_string()),
            pids_limit: Some(DEFAULT_SANDBOX_PIDS),
            ..Self::default()
        }
    }
```

- [ ] **Step 4: Route the CLI run path through `sandboxed()`**

In `crates/vox-cli/src/commands/container.rs`, in the `Run` arm where `RunOpts` is constructed, change the base from `RunOpts { ... }`/`RunOpts::default()` to start from `RunOpts::sandboxed()`, then apply the explicit `--cpus/--memory/--pids-limit` flags as overrides only when `Some`:

```rust
            let mut opts = RunOpts::sandboxed();
            opts.image = image;
            opts.ports = ports;
            // ... existing field assignments (env, volumes, name, rm, detach) ...
            if let Some(c) = cpus { opts.cpus = Some(c); }
            if let Some(m) = memory { opts.memory = Some(m); }
            if let Some(p) = pids_limit { opts.pids_limit = Some(p); }
```

(Read the current `Run` arm first and adapt the field names to what is actually there; the key change is the base is `sandboxed()` not unbounded, and flags override.)

- [ ] **Step 5: Run tests + build**

Run: `cargo test -p vox-container-types sandboxed_tests` then `cargo build -p vox-cli`
Expected: both PASS/succeed.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-container-types/src/runtime.rs crates/vox-cli/src/commands/container.rs
git commit -m "feat(container): safe-by-default resource limits via RunOpts::sandboxed() (close DoS-by-default)"
```

---

## Phase 1 — The `vox ci config-hygiene` enforcement gate

This is the seatbelt that stops the main-plan executor from re-introducing the Phase-0 defects. One new gate, three checks. Mirror the existing gate at `crates/vox-cli/src/commands/ci/policy_registry.rs`.

### Task 5: Scaffold `vox ci config-hygiene` with Check A — no cwd-relative contract paths

**Files:**
- Create: `crates/vox-cli/src/commands/ci/config_hygiene.rs`
- Modify: `crates/vox-cli/src/commands/ci/cmd_enums.rs` (add the `ConfigHygiene` variant + dispatch)
- Modify: `crates/vox-cli/src/commands/ci/mod.rs` (declare `pub mod config_hygiene;` + wire dispatch)
- Test: inline `#[cfg(test)]` in `config_hygiene.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/vox-cli/src/commands/ci/config_hygiene.rs`:

```rust
//! `vox ci config-hygiene`: machine guardrails that keep config single-homed,
//! safe-by-default, and never silently inert. Run BEFORE the configurability plan.

/// A single hygiene violation (file:line + message).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub check: &'static str,
    pub file: String,
    pub line: usize,
    pub message: String,
}

/// Check A: forbid cwd-relative `contracts/...` paths passed to file loaders.
/// Such paths are inert in any non-repo-root binary. Use `include_str!`-embedded
/// contracts instead (see Phase 0).
pub fn check_no_cwd_relative_contract_paths(source: &str, file: &str) -> Vec<Violation> {
    let mut hits = Vec::new();
    // A string literal that is a bare relative `contracts/...path.(yaml|yml|toml)`.
    let re = regex::Regex::new(r#""contracts/[^"]+\.(?:ya?ml|toml)""#).unwrap();
    for (i, raw) in source.lines().enumerate() {
        let line = raw.trim_start();
        if line.starts_with("//") || line.starts_with("//!") {
            continue; // doc/comment mention is fine
        }
        if re.is_match(raw) {
            hits.push(Violation {
                check: "no-cwd-relative-contract-path",
                file: file.to_string(),
                line: i + 1,
                message: "cwd-relative \"contracts/...\" path is inert in deployed binaries; \
                          embed the contract with include_str! (see config-guardrails Phase 0)"
                    .to_string(),
            });
        }
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_cwd_relative_contract_literal() {
        let src = r#"let p = Path::new("contracts/orchestration/circuit-breaker.v1.yaml");"#;
        let v = check_no_cwd_relative_contract_paths(src, "x.rs");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].check, "no-cwd-relative-contract-path");
    }

    #[test]
    fn allows_include_str_and_comments() {
        let ok = r#"const E: &str = include_str!("../../../contracts/gamify/economy.v1.yaml");"#;
        assert!(check_no_cwd_relative_contract_paths(ok, "x.rs").is_empty());
        let comment = r#"// loads contracts/gamify/economy.v1.yaml at build time"#;
        assert!(check_no_cwd_relative_contract_paths(comment, "x.rs").is_empty());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli config_hygiene::tests`
Expected: FAIL — `cannot find module config_hygiene` until it's declared in `mod.rs`.

- [ ] **Step 3: Declare the module and a runnable entry point**

In `crates/vox-cli/src/commands/ci/mod.rs`, add `pub mod config_hygiene;`.

Add the gate runner to `config_hygiene.rs` (it walks `crates/**/*.rs`, excluding test files and the gate's own source, and runs every check):

```rust
use std::path::Path;

/// Run all config-hygiene checks across the workspace. Returns Err with a
/// formatted report when any violation is found (exit non-zero).
pub fn run() -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let mut violations = Vec::new();
    collect_rs_files(&root.join("crates"), &mut |path, src| {
        let rel = path.strip_prefix(&root).unwrap_or(path).display().to_string();
        if rel.contains("config_hygiene.rs") {
            return; // don't lint the gate's own regex literals
        }
        violations.extend(check_no_cwd_relative_contract_paths(src, &rel));
        violations.extend(check_protected_modules_have_no_env_reads(src, &rel));
    });
    if violations.is_empty() {
        println!("config-hygiene OK: no violations");
        return Ok(());
    }
    for v in &violations {
        eprintln!("[{}] {}:{} — {}", v.check, v.file, v.line, v.message);
    }
    anyhow::bail!("config-hygiene found {} violation(s)", violations.len())
}

fn collect_rs_files(dir: &Path, f: &mut impl FnMut(&Path, &str)) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            collect_rs_files(&path, f);
        } else if path.extension().is_some_and(|e| e == "rs") {
            // Skip test files: their literals are fixtures, not production config.
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.ends_with("_tests.rs") || name == "tests.rs" {
                continue;
            }
            if let Ok(src) = std::fs::read_to_string(&path) {
                f(&path, &src);
            }
        }
    }
}
```

In `crates/vox-cli/src/commands/ci/cmd_enums.rs`, add the variant next to `PolicyRegistryParity` (~line 143):

```rust
    /// Config hygiene: no cwd-relative contract paths, no env reads in protected
    /// (never-configure) modules. Run before the configurability plan.
    #[command(name = "config-hygiene")]
    ConfigHygiene,
```

Wire its dispatch where `CiCmd` variants are matched to handlers (find the match arm for `PolicyRegistryParity` and add alongside):

```rust
        CiCmd::ConfigHygiene => crate::commands::ci::config_hygiene::run(),
```

(`check_protected_modules_have_no_env_reads` is defined in Task 6 — add it there before this gate references it; if executing Task 5 first, temporarily stub it as `fn check_protected_modules_have_no_env_reads(_: &str, _: &str) -> Vec<Violation> { Vec::new() }` and replace it in Task 6.)

- [ ] **Step 4: Run tests + the gate**

Run: `cargo test -p vox-cli config_hygiene::tests` then `cargo run -p vox-cli -- ci config-hygiene`
Expected: tests PASS; the gate runs (it WILL report the existing cwd-relative path in `orchestrator_policy.rs` until Task 2 lands — if Task 2 is already done, it reports clean).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/ci/config_hygiene.rs crates/vox-cli/src/commands/ci/cmd_enums.rs crates/vox-cli/src/commands/ci/mod.rs
git commit -m "feat(ci): add 'vox ci config-hygiene' gate with Check A (no cwd-relative contract paths)"
```

---

### Task 6: Check B — protected never-configure modules must not read env

The "unless it never needs configuring" clause, enforced structurally: protocol/crypto/grammar/wire-format/calibration code must NOT gain `std::env::var` reads or be turned into config. This is what stops the executor from de-literalizing a crypto nonce size or a grammar constant.

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/config_hygiene.rs` (add Check B + the protected-path list)
- Test: inline `#[cfg(test)]`

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` in `config_hygiene.rs`:

```rust
    #[test]
    fn flags_env_read_in_protected_module() {
        let src = "let n = std::env::var(\"VOX_NONCE_LEN\").unwrap();";
        let v = check_protected_modules_have_no_env_reads(src, "crates/vox-crypto/src/aead.rs");
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].check, "protected-module-no-env");
    }

    #[test]
    fn allows_env_read_in_normal_module() {
        let src = "let n = std::env::var(\"VOX_RAG_CHUNK\").unwrap();";
        assert!(check_protected_modules_have_no_env_reads(src, "crates/vox-search/src/ingest.rs").is_empty());
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli config_hygiene::tests::flags_env_read_in_protected_module`
Expected: FAIL — the function is the Task-5 stub returning empty.

- [ ] **Step 3: Implement Check B (replace the Task-5 stub)**

In `config_hygiene.rs`, add the protected-path SSOT and the real check (replace the stub):

```rust
/// Crates/paths whose constants are protocol-, format-, crypto-, grammar-, or
/// calibration-fixed: configurability is an explicit NON-GOAL. Reading env here
/// is forbidden. This is the structural form of "unless it never needs configuring".
/// Extend deliberately, with a reason in the PR.
pub const PROTECTED_PATH_FRAGMENTS: &[&str] = &[
    "crates/vox-crypto/",
    "crates/vox-wire-format-validator/",
    "crates/vox-grammar-export/",
    "crates/vox-ast/",
    // MENS memory-budget calibration (frozen on-hardware values, not operator knobs):
    "crates/vox-populi/src/mens/tensor/memory_budget.rs",
];

/// Check B: no `std::env::var` reads inside protected never-configure modules.
pub fn check_protected_modules_have_no_env_reads(source: &str, file: &str) -> Vec<Violation> {
    let norm = file.replace('\\', "/");
    if !PROTECTED_PATH_FRAGMENTS.iter().any(|p| norm.contains(p)) {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for (i, raw) in source.lines().enumerate() {
        let line = raw.trim_start();
        if line.starts_with("//") {
            continue;
        }
        if raw.contains("std::env::var") || raw.contains("env::var(") {
            hits.push(Violation {
                check: "protected-module-no-env",
                file: file.to_string(),
                line: i + 1,
                message: "protected never-configure module must not read env; \
                          if this value truly needs configuring, move it out of the protected \
                          path and register it (config-guardrails Phase 2)"
                    .to_string(),
            });
        }
    }
    hits
}
```

- [ ] **Step 4: Run tests + the gate**

Run: `cargo test -p vox-cli config_hygiene::tests` then `cargo run -p vox-cli -- ci config-hygiene`
Expected: tests PASS; gate reports any pre-existing env reads in protected paths (investigate + relocate or whitelist with a reason if found — a genuine finding).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/ci/config_hygiene.rs
git commit -m "feat(ci): config-hygiene Check B — protected never-configure modules forbid env reads"
```

---

### Task 7: Check C — declared-but-unwired config detector

A `resolve_*`/`from_env_*` config helper with no non-test caller is dead config (the cost-defense defect). Add a check that flags any `pub fn resolve_<x>` / `pub fn <x>_from_env` in a non-test module that has no non-test reference anywhere in `crates/`.

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/config_hygiene.rs` (add Check C, run across the whole tree)
- Test: inline `#[cfg(test)]`

- [ ] **Step 1: Write the failing test**

Add to `config_hygiene.rs` tests:

```rust
    #[test]
    fn flags_resolver_with_no_caller() {
        // index maps "symbol" -> count of non-test references across the tree
        let mut refs = std::collections::HashMap::new();
        refs.insert("resolve_orphan".to_string(), 0usize); // defined, never referenced
        refs.insert("resolve_wired".to_string(), 3usize);
        let defined = vec![
            ("resolve_orphan".to_string(), "crates/x/src/a.rs".to_string(), 10usize),
            ("resolve_wired".to_string(), "crates/x/src/b.rs".to_string(), 20usize),
        ];
        let v = check_unwired_config(&defined, &refs);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].message.contains("resolve_orphan"), true);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli config_hygiene::tests::flags_resolver_with_no_caller`
Expected: FAIL — `cannot find function check_unwired_config`.

- [ ] **Step 3: Implement Check C**

Add to `config_hygiene.rs`:

```rust
use std::collections::HashMap;

/// Check C (pure core): given config-resolver symbols and a map of how many
/// NON-test references each has, flag any with zero references (dead config).
/// `defined`: (symbol, file, line) for each `pub fn resolve_*`/`*_from_env`.
pub fn check_unwired_config(
    defined: &[(String, String, usize)],
    ref_counts: &HashMap<String, usize>,
) -> Vec<Violation> {
    defined
        .iter()
        .filter(|(sym, _, _)| ref_counts.get(sym).copied().unwrap_or(0) == 0)
        .map(|(sym, file, line)| Violation {
            check: "declared-but-unwired-config",
            file: file.clone(),
            line: *line,
            message: format!(
                "config resolver `{sym}` has no non-test caller — wire it or delete it (YAGNI)"
            ),
        })
        .collect()
}
```

Wire it into `run()`: collect resolver definitions (regex `pub fn (resolve_[a-z0-9_]+|[a-z0-9_]+_from_env)\b`) and a workspace-wide non-test reference count for each symbol, then call `check_unwired_config`. Add this inside `run()` after the per-file loop, building the two structures from a second pass over the collected files (the per-file closure should also push to `Vec<(symbol,file,line)>` of definitions and increment a `HashMap<String,usize>` for every textual occurrence in a non-test file that is NOT the definition line):

```rust
    // Second pass already accumulated during the walk (see definitions/ref_counts).
    violations.extend(check_unwired_config(&definitions, &ref_counts));
```

(Thread `definitions: Vec<(String,String,usize)>` and `ref_counts: HashMap<String,usize>` through the walk closure: for each non-test file, regex-match the definition pattern to populate `definitions`, and for every line count occurrences of each known resolver symbol — simplest correct approach: first collect all definitions across files, then in a second walk count references. Implement as two `collect_rs_files` passes to keep it simple.)

- [ ] **Step 4: Run tests + the gate**

Run: `cargo test -p vox-cli config_hygiene` then `cargo run -p vox-cli -- ci config-hygiene`
Expected: tests PASS; the gate now also reports any unwired resolver (should be clean after Task 3 removed the cost-defense one).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/ci/config_hygiene.rs
git commit -m "feat(ci): config-hygiene Check C — flag declared-but-unwired config resolvers"
```

> **Wire into pre-push / CI:** add `vox ci config-hygiene` to the workspace's gate list (wherever `vox ci policy-registry-parity` is invoked — check `.github/workflows/` and the pre-push hook). This is the line that makes the guardrail binding for the main-plan executor.

---

## Phase 2 — The unified config registry (searchable + GUI-generated SSOT)

### Task 8: Extend the registry schema with operational metadata

`contracts/config/env-vars.v1.yaml` already lists env vars with `name/owner_crate/kind/description`. Extend the schema so each operational knob also declares its `default`, validation `bound`, config `home`, `gui` surfacing, and lifecycle `status`. This is the single searchable SSOT the GUI generates from.

**Files:**
- Modify: `contracts/config/env-vars.v1.yaml` (bump to a richer schema; add the new fields to a few representative entries as the worked example)
- Create: `docs/src/architecture/config-registry-schema.md` (frontmatter required — `title`/`description`/`category: "Architecture SSOTs"`)

- [ ] **Step 1: Add the schema doc + extend representative entries**

Append to the top of `contracts/config/env-vars.v1.yaml` a schema comment, and extend (for example) the `VOX_WASM_SKILL_FUEL`, `VOX_RAG_CHUNK_CHARS`, and the gamify economy override entries with the new fields:

```yaml
# Registry schema v2 fields (per variable):
#   default:  the in-code default (string form); MUST equal the code constant.
#   bound:    optional {min, max} validation range (numeric knobs).
#   home:     env | vox.toml | voxconfig | contract | gui   (the ONE canonical home)
#   gui:      { surface: true|false, section: "<gui settings section>" }
#   status:   active | declared (declared = registered but not yet wired)
  - name: "VOX_WASM_SKILL_FUEL"
    owner_crate: "vox-plugin-runtime-wasm"
    kind: "u64"
    required: false
    introduced_in: "0.6.0"
    description: "Wasmtime fuel budget for skill execution (instructions)."
    default: "1000000000"
    bound: { min: 1000000, max: 100000000000 }
    home: "env"
    gui: { surface: true, section: "Runtime & Sandbox" }
    status: "active"
```

Write `docs/src/architecture/config-registry-schema.md` documenting the fields (frontmatter first).

- [ ] **Step 2: Validate the YAML parses**

Run: `python -c "import yaml,sys; yaml.safe_load(open('contracts/config/env-vars.v1.yaml',encoding='utf-8')); print('ok')"`
Expected: `ok`.

- [ ] **Step 3: Commit**

```bash
git add contracts/config/env-vars.v1.yaml docs/src/architecture/config-registry-schema.md
git commit -m "feat(config): extend config registry schema with default/bound/home/gui/status"
```

---

### Task 9: `vox ci config-registry-parity` — every operational env knob is registered

Mirror `policy_registry.rs`. The gate asserts: (1) every `VOX_*` env var read in `crates/**/*.rs` (non-test) has a registry entry; (2) every registry entry with `home: env` is actually read in code (no phantom entries). This is what makes config *complete and searchable* — you cannot ship a knob that isn't in the SSOT.

**Files:**
- Create: `crates/vox-cli/src/commands/ci/config_registry.rs`
- Modify: `crates/vox-cli/src/commands/ci/cmd_enums.rs` + `mod.rs` (add `ConfigRegistryParity` variant + dispatch)
- Test: inline `#[cfg(test)]`

- [ ] **Step 1: Write the failing test**

Create `crates/vox-cli/src/commands/ci/config_registry.rs`:

```rust
//! `vox ci config-registry-parity`: every operational VOX_* env knob must be in
//! contracts/config/env-vars.v1.yaml, and every `home: env` entry must be read.

use std::collections::BTreeSet;

/// Pure core: compare env vars referenced in code vs registered names.
/// Returns (unregistered_used, registered_but_unused).
pub fn parity(
    used_in_code: &BTreeSet<String>,
    registered: &BTreeSet<String>,
) -> (Vec<String>, Vec<String>) {
    let unregistered: Vec<String> =
        used_in_code.difference(registered).cloned().collect();
    let unused: Vec<String> =
        registered.difference(used_in_code).cloned().collect();
    (unregistered, unused)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_unregistered_and_phantom() {
        let used: BTreeSet<String> =
            ["VOX_A".into(), "VOX_B".into()].into_iter().collect();
        let reg: BTreeSet<String> =
            ["VOX_A".into(), "VOX_C".into()].into_iter().collect();
        let (unregistered, unused) = parity(&used, &reg);
        assert_eq!(unregistered, vec!["VOX_B".to_string()]); // used, not registered
        assert_eq!(unused, vec!["VOX_C".to_string()]); // registered, not used
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli config_registry::tests`
Expected: FAIL — module not declared in `mod.rs`.

- [ ] **Step 3: Implement the gate runner + wire the command**

Add `pub mod config_registry;` to `crates/vox-cli/src/commands/ci/mod.rs`. Add the `run()` that scans code for `VOX_[A-Z0-9_]+` literals (in non-test `crates/**/*.rs`), loads `registered` names from the YAML (`variables[].name`), calls `parity`, and bails on any unregistered-used (phantom/unused entries are a warning, not a hard fail — they may be `status: declared`):

```rust
pub fn run() -> anyhow::Result<()> {
    let root = std::env::current_dir()?;
    let used = scan_env_var_uses(&root.join("crates"))?;
    let registered = load_registered_names(
        &root.join("contracts/config/env-vars.v1.yaml"),
    )?;
    let (unregistered, unused) = parity(&used, &registered);
    for name in &unused {
        eprintln!("warn: registry entry {name} (home: env) not read in code (phantom or declared)");
    }
    if !unregistered.is_empty() {
        for name in &unregistered {
            eprintln!("error: env var {name} is read in code but NOT in contracts/config/env-vars.v1.yaml");
        }
        anyhow::bail!(
            "config-registry-parity: {} unregistered env var(s) — add them to the registry SSOT",
            unregistered.len()
        );
    }
    println!("config-registry-parity OK: {} env knobs all registered", used.len());
    Ok(())
}
```

Implement `scan_env_var_uses` (regex `VOX_[A-Z0-9_]+` over non-test `.rs`, returning a `BTreeSet<String>`, skipping the registry/gate source files) and `load_registered_names` (parse the YAML, collect `variables[].name`). Add the `ConfigRegistryParity` variant to `cmd_enums.rs` (`#[command(name = "config-registry-parity")]`) and dispatch to `config_registry::run()`.

- [ ] **Step 4: Run tests + the gate**

Run: `cargo test -p vox-cli config_registry` then `cargo run -p vox-cli -- ci config-registry-parity`
Expected: tests PASS; the gate reports the (large) set of currently-unregistered env vars — that is the real backlog the main plan must register. **This number going to zero is the definition of "config is complete and searchable."**

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/ci/config_registry.rs crates/vox-cli/src/commands/ci/cmd_enums.rs crates/vox-cli/src/commands/ci/mod.rs
git commit -m "feat(ci): 'vox ci config-registry-parity' — every operational env knob must be registered"
```

---

### Task 10: Generate the GUI settings surface from the registry (visibility, searchable, one source)

Generate a `generatedSettings.ts` from the registry entries flagged `gui.surface: true`, consumed by the existing `settingsIndex.ts`. The GUI then surfaces exactly the *curated* subset — searchable, organized by `section`, single-sourced — instead of hand-maintained duplicates.

**Files:**
- Create: `crates/vox-cli/src/commands/ci/config_gui_codegen.rs` (the generator + a `--check` parity mode)
- Create (generated): `crates/vox-gui/ui/src/config/generatedSettings.ts`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Settings/settingsIndex.ts` (import + spread the generated entries)
- Modify: `cmd_enums.rs`/`mod.rs` (add `ConfigGuiCodegen { check: bool }`)
- Test: inline `#[cfg(test)]`

- [ ] **Step 1: Write the failing test (pure codegen core)**

Create `crates/vox-cli/src/commands/ci/config_gui_codegen.rs`:

```rust
//! Generate the GUI settings list from the config registry (gui.surface entries).

/// One registry entry projected for the GUI.
#[derive(Debug, Clone)]
pub struct GuiKnob {
    pub name: String,
    pub kind: String,
    pub default: String,
    pub section: String,
    pub description: String,
}

/// Pure: render the generated TypeScript module from the gui-surfaced knobs.
pub fn render_generated_ts(knobs: &[GuiKnob]) -> String {
    let mut out = String::from(
        "// @generated by `vox ci config-gui-codegen` from contracts/config/env-vars.v1.yaml — DO NOT EDIT.\n\
         export interface GeneratedSetting { name: string; kind: string; default: string; section: string; description: string; }\n\
         export const GENERATED_SETTINGS: GeneratedSetting[] = [\n",
    );
    for k in knobs {
        out.push_str(&format!(
            "  {{ name: {:?}, kind: {:?}, default: {:?}, section: {:?}, description: {:?} }},\n",
            k.name, k.kind, k.default, k.section, k.description
        ));
    }
    out.push_str("];\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_stable_sorted_ts() {
        let knobs = vec![GuiKnob {
            name: "VOX_WASM_SKILL_FUEL".into(), kind: "u64".into(),
            default: "1000000000".into(), section: "Runtime & Sandbox".into(),
            description: "Wasmtime fuel budget.".into(),
        }];
        let ts = render_generated_ts(&knobs);
        assert!(ts.contains("@generated"));
        assert!(ts.contains("\"VOX_WASM_SKILL_FUEL\""));
        assert!(ts.trim_end().ends_with("];"));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli config_gui_codegen::tests`
Expected: FAIL — module not declared.

- [ ] **Step 3: Implement generator + `--check` mode + wire the command**

Add `pub mod config_gui_codegen;` to `mod.rs`. Implement `run(check: bool)`: load the registry, project entries with `gui.surface == true` (sorted by `section` then `name` for stable output) into `Vec<GuiKnob>`, `render_generated_ts`, then either write `crates/vox-gui/ui/src/config/generatedSettings.ts` (generate mode) or compare against the on-disk file and bail if it differs (`--check`, for CI drift). Add the `ConfigGuiCodegen { #[arg(long)] check: bool }` variant + dispatch. In `settingsIndex.ts`, `import { GENERATED_SETTINGS } from '../../../config/generatedSettings';` and spread/merge those into the settings list (read the file first to match its existing list shape).

- [ ] **Step 4: Generate + verify drift gate**

Run: `cargo run -p vox-cli -- ci config-gui-codegen` (writes the file), then `cargo run -p vox-cli -- ci config-gui-codegen --check`
Expected: first writes `generatedSettings.ts`; second prints OK (no drift). `cargo test -p vox-cli config_gui_codegen` PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/ci/config_gui_codegen.rs crates/vox-gui/ui/src/config/generatedSettings.ts crates/vox-gui/ui/src/components/surfaces/Settings/settingsIndex.ts crates/vox-cli/src/commands/ci/cmd_enums.rs crates/vox-cli/src/commands/ci/mod.rs
git commit -m "feat(config): generate GUI settings from the registry SSOT (searchable, single-sourced)"
```

> **Wire `--check` into CI** alongside `config-registry-parity` so the generated GUI file can never drift from the registry.

---

## Handoff contract for the main "full configurability" plan (Sonnet 4.6)

After this guardrail plan is merged, the main plan MUST obey these rules — now machine-enforced:

1. **Never** add a cwd-relative `"contracts/..."` path (Check A). Embed with `include_str!` or read via an env-overridable resolver that falls back to the embedded contract.
2. **Never** read env or de-literalize inside `PROTECTED_PATH_FRAGMENTS` (Check B). That is the "unless it never needs configuring" set — protocol/crypto/grammar/wire-format/calibration stay const.
3. **Never** add a config resolver without a non-test caller (Check C). Wire it or don't add it.
4. **Every** new operational env knob MUST be added to `contracts/config/env-vars.v1.yaml` with `default/bound/home/status` (Check `config-registry-parity`), and if operator-facing, `gui.surface: true` with a `section` (it then appears in the GUI automatically via codegen).
5. **Safety limits** (sandbox, fuel, budgets, breaker thresholds) MUST have a safe default and a `bound`; "configurable" never means "unbounded/disableable by default."
6. The metric of done is **not** "zero literals." It is: `config-registry-parity` shows zero unregistered knobs, `config-hygiene` is green, and the GUI surface is the generated curated subset. Magic values that fall in the protected set stay const **by design**.

---

## Self-Review

- **Spec coverage:** Refactor (not exclude) the 3 defects → Tasks 1–4. Arch-check/lint enforcement → Tasks 5–7 (the `config-hygiene` gate). The registry → Tasks 8–9; GUI visibility/searchability generated from it → Task 10. The "exclude as necessary" reframed as the structural protected-set (Check B) + the handoff contract.
- **Placeholder scan:** Tasks 1–9 carry complete code. Task 7's `run()` integration and Task 10's `run(check)` describe the wiring around fully-given pure cores (`check_unwired_config`, `render_generated_ts`) — the non-trivial logic is fully coded; the file-walk plumbing reuses the `collect_rs_files` helper given in Task 5. Task 4 Step 4 and Task 10 Step 3 say "read the current file first and adapt field/list shape" because those touch hand-written surfaces whose exact current shape must be matched — that is a correctness instruction, not a placeholder.
- **Type consistency:** `Violation` (Task 5) is reused by Checks B/C; `check_protected_modules_have_no_env_reads` is stubbed in Task 5 and implemented in Task 6 with the same signature; `parity`, `GuiKnob`, `render_generated_ts` signatures are consistent between definition and use; `RunOpts::sandboxed()` + the `DEFAULT_SANDBOX_*` consts are defined and consumed in Task 4.
