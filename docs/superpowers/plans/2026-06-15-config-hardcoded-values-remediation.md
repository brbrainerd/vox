# Config & Hard-Coded-Values Remediation — Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire the 8 highest-severity hard-coded values surfaced by the config audit into real, testable configuration seams — closing the "declared-but-unwired", "missing safety limit", and "baked-in pin" gaps.

**Architecture:** Each fix extracts a small **pure, deterministic resolver function** (takes an explicit override value, returns the resolved value), unit-tests that resolver, then calls it at the existing site. This keeps every change TDD-friendly without touching global process state in tests, and gives each knob a single SSOT. Defaults are preserved exactly, so behavior is unchanged until an operator opts in.

**Tech Stack:** Rust (workspace crates), `reqwest` per-request timeouts, `vox_config::timeouts` SSOT, env-var overrides, `cargo test -p <crate>`.

**Source of truth:** [`graphify-out/config-audit-graph/FINDINGS_INDEX.md`](../../../graphify-out/config-audit-graph/FINDINGS_INDEX.md) (135 findings) and the per-group docs in [`graphify-out/config-audit/`](../../../graphify-out/config-audit/). This plan covers the **8 high-severity** rows; medium/low batches are listed under *Follow-On Plans*.

**Out of scope (owned by another plan):** the GUI/LLM **settings-registry** split-brain (vox-config accessors ↔ operator_registry ↔ vox-gui FIELDS) is owned by the *Enforceable LLM/AI settings SSOT* plan (`project_llm_ai_settings_ssot_enforce_2026`). Do not touch the GUI settings registry here.

**Convention for every task:** the resolver reads an env override **passed in by the caller** (`Option<&str>`), so tests never mutate global env. Call sites do `resolver(std::env::var("VOX_…").ok().as_deref())`.

---

## Phase 1 — Wire declared-but-unwired config (audit pattern #2)

### Task 1: Apply the configured LLM request timeout (HC-G04-09)

`LlmConfig.timeout_ms` exists and the cost-defense cascade sets it to `Some(30_000)`, but `chat.rs`, `embed.rs`, and `stream.rs` build the request and call `.send()` without ever applying it. The shared `vox_http_client::client()` sets only `connect_timeout` (15s) — once connected, a hung endpoint stalls forever. Apply a per-request `reqwest` timeout on the two **unary** calls (chat, embed). **Streaming is deliberately excluded** — `reqwest`'s `.timeout()` bounds the whole request including the body, which would sever long SSE streams; a first-byte deadline for streams is a follow-up (see Follow-On Plans).

**Files:**
- Create: `crates/vox-actor-runtime/src/llm/timeout.rs`
- Modify: `crates/vox-actor-runtime/src/llm/mod.rs` (add `mod timeout;`)
- Modify: `crates/vox-actor-runtime/src/llm/chat.rs:72` (the `let mut req = client.post(&base_url).json(&req_body);` builder)
- Modify: `crates/vox-actor-runtime/src/llm/embed.rs:82` (the `let mut req = client.post(&base_url).json(&req_body);` builder)
- Test: inline `#[cfg(test)]` in `crates/vox-actor-runtime/src/llm/timeout.rs`

- [ ] **Step 1: Write the failing test**

Create `crates/vox-actor-runtime/src/llm/timeout.rs`:

```rust
//! SSOT resolver for the per-request LLM HTTP timeout.
//!
//! Precedence: explicit `LlmConfig.timeout_ms` → shared `vox_config::timeouts::HTTP_REQUEST`.
//! Applied to unary chat/embed calls only (streaming is excluded — a whole-request
//! deadline would cut off long SSE streams).

use std::time::Duration;

use super::types::LlmConfig;

/// Resolve the request timeout for a unary LLM call.
pub(crate) fn request_timeout(config: &LlmConfig) -> Duration {
    match config.timeout_ms {
        Some(ms) => Duration::from_millis(ms),
        None => vox_config::timeouts::HTTP_REQUEST,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(timeout_ms: Option<u64>) -> LlmConfig {
        let mut c = LlmConfig::openrouter("test-model");
        c.timeout_ms = timeout_ms;
        c
    }

    #[test]
    fn explicit_timeout_is_used() {
        assert_eq!(request_timeout(&cfg(Some(5_000))), Duration::from_millis(5_000));
    }

    #[test]
    fn falls_back_to_ssot_default_when_unset() {
        assert_eq!(request_timeout(&cfg(None)), vox_config::timeouts::HTTP_REQUEST);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-actor-runtime llm::timeout`
Expected: FAIL — `error[E0583]: file not found for module` / `cannot find module timeout` until `mod timeout;` is added; if the module is added but `request_timeout` is wrong, the asserts fail.

- [ ] **Step 3: Wire the module and apply it at the two unary call sites**

In `crates/vox-actor-runtime/src/llm/mod.rs`, add alongside the other `mod` lines:

```rust
mod timeout;
```

In `crates/vox-actor-runtime/src/llm/chat.rs`, change the builder at line 72 from:

```rust
            let mut req = client.post(&base_url).json(&req_body);
```

to:

```rust
            let mut req = client
                .post(&base_url)
                .json(&req_body)
                .timeout(super::timeout::request_timeout(config));
```

In `crates/vox-actor-runtime/src/llm/embed.rs`, change the builder at line 82 from:

```rust
            let mut req = client.post(&base_url).json(&req_body);
```

to:

```rust
            let mut req = client
                .post(&base_url)
                .json(&req_body)
                .timeout(super::timeout::request_timeout(config));
```

Leave `stream.rs` unchanged. Add a one-line comment above the `stream.rs` send (`crates/vox-actor-runtime/src/llm/stream.rs:73`):

```rust
    // NOTE: no whole-request .timeout() here — it would sever long SSE streams.
    // First-byte deadline for streams is a follow-up (config-audit HC-G04-09).
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vox-actor-runtime llm::timeout`
Expected: PASS (2 tests). Then `cargo build -p vox-actor-runtime` succeeds (confirms `.timeout()` typechecks on the `RequestBuilder`).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-actor-runtime/src/llm/timeout.rs crates/vox-actor-runtime/src/llm/mod.rs crates/vox-actor-runtime/src/llm/chat.rs crates/vox-actor-runtime/src/llm/embed.rs crates/vox-actor-runtime/src/llm/stream.rs
git commit -m "fix(llm): apply configured timeout_ms to unary chat/embed requests (HC-G04-09)"
```

---

### Task 2: Load circuit-breaker thresholds from contract, not just Default (HC-G02-08)

`CircuitBreakerConfig`'s doc says "Thresholds loaded from contract YAML" and references `contracts/orchestration/circuit-breaker.v1.yaml`, but every construction is `CircuitBreakerConfig::default()` — the contract is never read. Add a `from_contract_str` pure parser (deterministic, testable) plus a thin `from_contract_file` wrapper, and route production construction through it with a Default fallback.

**Files:**
- Modify: `crates/vox-orchestrator/src/circuit_breaker.rs` (add loader + tests after the `Default` impl, ~line 92)
- Test: inline `#[cfg(test)]` in the same file

- [ ] **Step 1: Write the failing test**

Add to the existing `#[cfg(test)] mod tests` block in `crates/vox-orchestrator/src/circuit_breaker.rs`:

```rust
    #[test]
    fn from_contract_str_overrides_defaults() {
        let yaml = r#"
no_progress_threshold: 7
same_error_threshold: 9
"#;
        let cfg = CircuitBreakerConfig::from_contract_str(yaml).expect("parse");
        // Overridden:
        assert_eq!(cfg.no_progress_threshold, 7);
        assert_eq!(cfg.same_error_threshold, 9);
        // Unspecified keys fall back to Default:
        assert_eq!(cfg.tool_thrash_threshold, 15);
        assert_eq!(cfg.replan_limit, 3);
    }

    #[test]
    fn from_contract_str_empty_is_all_defaults() {
        let cfg = CircuitBreakerConfig::from_contract_str("").expect("parse");
        assert_eq!(cfg.no_progress_threshold, 3);
        assert_eq!(cfg.ngram_overlap_threshold, 0.85);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator circuit_breaker::tests::from_contract`
Expected: FAIL — `no function or associated item named from_contract_str found`.

- [ ] **Step 3: Implement the loader**

Add to `crates/vox-orchestrator/src/circuit_breaker.rs`, immediately after the `impl Default for CircuitBreakerConfig { … }` block (after line 92). This uses a partial-overlay struct so unspecified keys keep their `Default` values:

```rust
impl CircuitBreakerConfig {
    /// Parse thresholds from contract YAML text, overlaying onto `Default`.
    /// Unspecified keys retain their default. SSOT: `contracts/orchestration/circuit-breaker.v1.yaml`.
    pub fn from_contract_str(yaml: &str) -> Result<Self, serde_yaml::Error> {
        #[derive(serde::Deserialize, Default)]
        struct Overlay {
            no_progress_threshold: Option<u32>,
            same_error_threshold: Option<u32>,
            tool_thrash_threshold: Option<u32>,
            ngram_overlap_threshold: Option<f64>,
            semantic_drift_sigma: Option<f64>,
            caution_no_progress: Option<u32>,
            caution_same_error: Option<u32>,
            caution_tool_thrash: Option<u32>,
            warning_no_progress: Option<u32>,
            warning_same_error: Option<u32>,
            warning_tool_thrash: Option<u32>,
            replan_limit: Option<u32>,
        }
        let o: Overlay = if yaml.trim().is_empty() {
            Overlay::default()
        } else {
            serde_yaml::from_str(yaml)?
        };
        let mut c = Self::default();
        if let Some(v) = o.no_progress_threshold { c.no_progress_threshold = v; }
        if let Some(v) = o.same_error_threshold { c.same_error_threshold = v; }
        if let Some(v) = o.tool_thrash_threshold { c.tool_thrash_threshold = v; }
        if let Some(v) = o.ngram_overlap_threshold { c.ngram_overlap_threshold = v; }
        if let Some(v) = o.semantic_drift_sigma { c.semantic_drift_sigma = v; }
        if let Some(v) = o.caution_no_progress { c.caution_no_progress = v; }
        if let Some(v) = o.caution_same_error { c.caution_same_error = v; }
        if let Some(v) = o.caution_tool_thrash { c.caution_tool_thrash = v; }
        if let Some(v) = o.warning_no_progress { c.warning_no_progress = v; }
        if let Some(v) = o.warning_same_error { c.warning_same_error = v; }
        if let Some(v) = o.warning_tool_thrash { c.warning_tool_thrash = v; }
        if let Some(v) = o.replan_limit { c.replan_limit = v; }
        Ok(c)
    }

    /// Load thresholds from the contract file if it exists; otherwise `Default`.
    pub fn from_contract_file(path: &std::path::Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(text) => Self::from_contract_str(&text).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }
}
```

Confirm `serde_yaml` is a dependency of `vox-orchestrator`. Check with `cargo tree -p vox-orchestrator -i serde_yaml`. If absent, add to `crates/vox-orchestrator/Cargo.toml` under `[dependencies]`: `serde_yaml = { workspace = true }` (the workspace already pins it — verify with `grep serde_yaml Cargo.toml`).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vox-orchestrator circuit_breaker`
Expected: PASS (existing trip tests + the 2 new loader tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator/src/circuit_breaker.rs crates/vox-orchestrator/Cargo.toml
git commit -m "feat(orchestrator): load circuit-breaker thresholds from contract overlay (HC-G02-08)"
```

> **Wiring note for the executor:** the production `CircuitBreaker` construction site (the non-test caller in `orchestrator_policy.rs`) should be switched from `CircuitBreakerConfig::default()` to `CircuitBreakerConfig::from_contract_file(Path::new("contracts/orchestration/circuit-breaker.v1.yaml"))`. Locate it with `rg "CircuitBreaker::new" crates/vox-orchestrator/src --glob '!*test*'` and update that single call in the same commit. If no non-test caller exists yet (breaker not yet wired into the live loop), record that in the commit message and stop — do not fabricate a call site.

---

### Task 3: Resolve cost-defense budgets from env (HC-G01-01)

`CostDefenseConfig::default()` hard-codes `daily_budget_usd: 25.0` and `monthly_budget_usd: 500.0` (a split-brain risk — these reappear elsewhere). Add a deterministic resolver that overlays env overrides onto the defaults.

**Files:**
- Modify: `crates/vox-scaling-policy/src/cost_defense.rs` (add `from_env_values` after the `Default` impl, ~line 63)
- Test: inline `#[cfg(test)]` in the same file

- [ ] **Step 1: Write the failing test**

Add a `#[cfg(test)] mod tests` block (or extend the existing one) in `crates/vox-scaling-policy/src/cost_defense.rs`:

```rust
#[cfg(test)]
mod budget_env_tests {
    use super::*;

    #[test]
    fn env_overrides_apply() {
        let c = CostDefenseConfig::from_env_values(Some("10.0"), Some("200.0"));
        assert_eq!(c.daily_budget_usd, 10.0);
        assert_eq!(c.monthly_budget_usd, 200.0);
    }

    #[test]
    fn missing_env_keeps_defaults() {
        let c = CostDefenseConfig::from_env_values(None, None);
        assert_eq!(c.daily_budget_usd, 25.0);
        assert_eq!(c.monthly_budget_usd, 500.0);
    }

    #[test]
    fn unparseable_env_keeps_default() {
        let c = CostDefenseConfig::from_env_values(Some("not-a-number"), None);
        assert_eq!(c.daily_budget_usd, 25.0);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-scaling-policy budget_env_tests`
Expected: FAIL — `no function or associated item named from_env_values`.

- [ ] **Step 3: Implement the resolver**

Add after the `impl Default for CostDefenseConfig { … }` block in `crates/vox-scaling-policy/src/cost_defense.rs`:

```rust
impl CostDefenseConfig {
    /// Overlay daily/monthly USD budget ceilings from raw env strings onto `Default`.
    /// Unparseable or absent values keep the default. Callers pass
    /// `std::env::var("VOX_COST_DAILY_BUDGET_USD").ok().as_deref()` etc.
    pub fn from_env_values(daily: Option<&str>, monthly: Option<&str>) -> Self {
        let mut c = Self::default();
        if let Some(v) = daily.and_then(|s| s.trim().parse::<f64>().ok()) {
            c.daily_budget_usd = v;
        }
        if let Some(v) = monthly.and_then(|s| s.trim().parse::<f64>().ok()) {
            c.monthly_budget_usd = v;
        }
        c
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vox-scaling-policy cost_defense`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-scaling-policy/src/cost_defense.rs
git commit -m "feat(scaling-policy): resolve cost-defense budgets from env (HC-G01-01)"
```

> **Wiring note:** switch the production construction of `CostDefenseConfig` from `::default()` to `::from_env_values(std::env::var("VOX_COST_DAILY_BUDGET_USD").ok().as_deref(), std::env::var("VOX_COST_MONTHLY_BUDGET_USD").ok().as_deref())`. Find it with `rg "CostDefenseConfig::default\(\)" crates --glob '!*test*'` and update the single non-test caller in this commit.

---

## Phase 2 — Missing safety limits (high-severity)

### Task 4: Emit container resource limits (HC-G07-05)

`RunOpts` has no cpu/mem/pids/timeout fields, so `docker run`/`podman run` are emitted **unbounded** — an untrusted image can exhaust the host. Add fields to `RunOpts` (defaulting to `None`, preserving behavior) plus a **pure** `resource_args()` builder, unit-test it, then call it from both `docker.rs` sites.

**Files:**
- Modify: `crates/vox-container-types/src/runtime.rs` (add fields to `RunOpts`, its `Default`, and a `resource_args` method)
- Modify: `crates/vox-container/src/docker.rs:97` (the `run` method, before `cmd.arg(&opts.image);`)
- Modify: `crates/vox-plugin-runtime-container/src/docker.rs` (the duplicate `run` method — same edit)
- Test: inline `#[cfg(test)]` in `crates/vox-container-types/src/runtime.rs`

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-container-types/src/runtime.rs`:

```rust
#[cfg(test)]
mod resource_args_tests {
    use super::*;

    #[test]
    fn no_limits_emits_nothing() {
        let opts = RunOpts::default();
        assert!(opts.resource_args().is_empty());
    }

    #[test]
    fn limits_emit_flags_in_order() {
        let opts = RunOpts {
            cpus: Some("1.5".into()),
            memory: Some("512m".into()),
            pids_limit: Some(128),
            ..RunOpts::default()
        };
        assert_eq!(
            opts.resource_args(),
            vec!["--cpus", "1.5", "--memory", "512m", "--pids-limit", "128"]
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-container-types resource_args`
Expected: FAIL — `struct RunOpts has no field named cpus` and `no method named resource_args`.

- [ ] **Step 3: Add the fields and the pure builder**

In `crates/vox-container-types/src/runtime.rs`, add three fields to `RunOpts` (after `pub rm: bool,`):

```rust
    /// `--cpus` quota (e.g. `"1.5"`). `None` = unlimited.
    pub cpus: Option<String>,
    /// `--memory` cap (e.g. `"512m"`). `None` = unlimited.
    pub memory: Option<String>,
    /// `--pids-limit` cap. `None` = unlimited.
    pub pids_limit: Option<u32>,
```

Add them to the `Default` impl (after `rm: true,`):

```rust
            cpus: None,
            memory: None,
            pids_limit: None,
```

Add the pure builder as an `impl RunOpts` block after the `Default` impl:

```rust
impl RunOpts {
    /// CLI resource-limit flags for `docker run` / `podman run`, in a stable order.
    /// Empty when no limits are set (behavior-preserving).
    pub fn resource_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        if let Some(c) = &self.cpus {
            args.push("--cpus".to_string());
            args.push(c.clone());
        }
        if let Some(m) = &self.memory {
            args.push("--memory".to_string());
            args.push(m.clone());
        }
        if let Some(p) = self.pids_limit {
            args.push("--pids-limit".to_string());
            args.push(p.to_string());
        }
        args
    }
}
```

- [ ] **Step 4: Run the unit test to verify it passes**

Run: `cargo test -p vox-container-types resource_args`
Expected: PASS (2 tests).

- [ ] **Step 5: Emit the flags at both docker.rs sites**

In `crates/vox-container/src/docker.rs`, in `run()`, immediately before `cmd.arg(&opts.image);` (line 98):

```rust
        for arg in opts.resource_args() {
            cmd.arg(arg);
        }
```

Apply the identical insertion in `crates/vox-plugin-runtime-container/src/docker.rs` (the duplicated `run()` — confirm with `rg -n "cmd.arg\(&opts.image\)" crates/vox-plugin-runtime-container/src/docker.rs`).

- [ ] **Step 6: Verify both crates build**

Run: `cargo build -p vox-container -p vox-plugin-runtime-container`
Expected: success (the new fields are `..Default::default()`-compatible for all existing `RunOpts` constructors).

- [ ] **Step 7: Commit**

```bash
git add crates/vox-container-types/src/runtime.rs crates/vox-container/src/docker.rs crates/vox-plugin-runtime-container/src/docker.rs
git commit -m "feat(container): emit cpu/memory/pids resource limits on container run (HC-G07-05)"
```

> **Follow-up (note in commit, not implemented here):** the two `docker.rs` files are byte-identical duplicates — dedup into one shared impl is tracked in the Follow-On Plans. Also: a `--stop-timeout`/run-timeout knob and sane *default* limits (rather than `None`) belong in the deploy-codegen caller, which is a separate task so defaults don't silently change existing deploys.

---

### Task 5: Resolve WASM skill fuel from env (HC-G07-01)

`WasmRuntime::new()` hard-codes `WasmHost::with_fuel(1_000_000_000)`. Add a pure resolver reading an env override with that value as the SSOT default const, and call it from `new()`. (This crate sits below `vox-config`, so the knob is an env var, not a `VoxConfig` field.)

**Files:**
- Modify: `crates/vox-plugin-runtime-wasm/src/runtime.rs` (const + resolver + `new()`; ~lines 26-32)
- Test: inline `#[cfg(test)]` in the same file

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-plugin-runtime-wasm/src/runtime.rs`:

```rust
#[cfg(test)]
mod fuel_tests {
    use super::*;

    #[test]
    fn default_fuel_when_unset() {
        assert_eq!(resolve_fuel(None), DEFAULT_WASM_SKILL_FUEL);
    }

    #[test]
    fn env_override_parsed() {
        assert_eq!(resolve_fuel(Some("250000000")), 250_000_000);
    }

    #[test]
    fn unparseable_env_keeps_default() {
        assert_eq!(resolve_fuel(Some("lots")), DEFAULT_WASM_SKILL_FUEL);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-plugin-runtime-wasm fuel_tests`
Expected: FAIL — `cannot find value DEFAULT_WASM_SKILL_FUEL` / `cannot find function resolve_fuel`.

- [ ] **Step 3: Implement const + resolver, call from `new()`**

In `crates/vox-plugin-runtime-wasm/src/runtime.rs`, add above `pub struct WasmRuntime` (after the `use` lines):

```rust
/// Default WASM skill fuel: ~1B instructions (~seconds of compute). SSOT for the
/// default; override at runtime with `VOX_WASM_SKILL_FUEL`.
pub const DEFAULT_WASM_SKILL_FUEL: u64 = 1_000_000_000;

/// Resolve the fuel budget from a raw env override, falling back to the default.
pub fn resolve_fuel(raw: Option<&str>) -> u64 {
    raw.and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_WASM_SKILL_FUEL)
}
```

Change `WasmRuntime::new()` (lines 28-31) from:

```rust
    pub fn new() -> Result<Self> {
        // Default fuel: 1 billion instructions (~seconds of compute on modern hardware).
        let host = WasmHost::with_fuel(1_000_000_000)?;
        Ok(Self { host })
    }
```

to:

```rust
    pub fn new() -> Result<Self> {
        let fuel = resolve_fuel(std::env::var("VOX_WASM_SKILL_FUEL").ok().as_deref());
        let host = WasmHost::with_fuel(fuel)?;
        Ok(Self { host })
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vox-plugin-runtime-wasm fuel_tests`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-plugin-runtime-wasm/src/runtime.rs
git commit -m "feat(wasm-runtime): resolve skill fuel from VOX_WASM_SKILL_FUEL env (HC-G07-01)"
```

---

## Phase 3 — De-literalize pins & baked-in defaults (audit pattern #3)

### Task 6: Resolve MENS default model id from env (HC-G08-01)

`DEFAULT_MODEL_ID` is a hard `const` (`Qwen/Qwen2.5-Coder-7B-Instruct`) consumed at 5 train/cloud call sites, and it contradicts the "4B" narrative in the budgeting module. Keep the const as the SSOT default, add a `default_model_id()` resolver reading `VOX_MENS_DEFAULT_MODEL`, and route call sites through it.

**Files:**
- Modify: `crates/vox-populi/src/mens/mod.rs:42` (add resolver beside the const)
- Modify call sites: `crates/vox-ml-cli/src/schola/train/run_train.rs:101`, `crates/vox-ml-cli/src/commands/ai/train.rs:80`, `crates/vox-ml-cli/src/mens/populi/train_arm.rs:76`, `crates/vox-populi/src/cloud/resolver.rs:420`, `crates/vox-populi/src/cloud/mod.rs:319`
- Test: inline `#[cfg(test)]` in `crates/vox-populi/src/mens/mod.rs`

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-populi/src/mens/mod.rs`:

```rust
#[cfg(test)]
mod default_model_tests {
    use super::*;

    #[test]
    fn falls_back_to_const() {
        assert_eq!(resolve_default_model_id(None), DEFAULT_MODEL_ID);
    }

    #[test]
    fn env_override_wins() {
        assert_eq!(resolve_default_model_id(Some("org/My-Model")), "org/My-Model");
    }

    #[test]
    fn blank_env_falls_back() {
        assert_eq!(resolve_default_model_id(Some("   ")), DEFAULT_MODEL_ID);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-populi default_model_tests`
Expected: FAIL — `cannot find function resolve_default_model_id`.

- [ ] **Step 3: Implement the resolver**

Add immediately after the `DEFAULT_MODEL_ID` const (line 42) in `crates/vox-populi/src/mens/mod.rs`:

```rust
/// Resolve the default training/inference base model id from a raw env override,
/// falling back to [`DEFAULT_MODEL_ID`]. Blank/whitespace overrides fall back.
pub fn resolve_default_model_id(raw: Option<&str>) -> String {
    match raw.map(str::trim) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => DEFAULT_MODEL_ID.to_string(),
    }
}

/// Convenience: resolve from the `VOX_MENS_DEFAULT_MODEL` process env.
pub fn default_model_id() -> String {
    resolve_default_model_id(std::env::var("VOX_MENS_DEFAULT_MODEL").ok().as_deref())
}
```

- [ ] **Step 4: Run unit tests to verify they pass**

Run: `cargo test -p vox-populi default_model_tests`
Expected: PASS (3 tests).

- [ ] **Step 5: Route the 5 call sites through `default_model_id()`**

At each listed location, replace the bare `DEFAULT_MODEL_ID` (or `DEFAULT_MODEL_ID.to_string()`) usage with a call to the resolver. First confirm each site:

Run: `rg -n "DEFAULT_MODEL_ID" crates/vox-ml-cli/src crates/vox-populi/src`

For each non-definition hit, replace `DEFAULT_MODEL_ID.to_string()` → `vox_populi::mens::default_model_id()` (or `crate::mens::default_model_id()` inside `vox-populi`), and a bare `DEFAULT_MODEL_ID` used where a `&str`/`String` default is needed → the same call. Do **not** change the `pub const` definition itself.

- [ ] **Step 6: Verify both crates build and the doc drift is corrected**

Run: `cargo build -p vox-populi -p vox-ml-cli`
Then fix the stale doc comment: in `crates/vox-populi/src/mens/mod.rs`, the comment block above the const still describes "Qwen3.5-4B" as the *previous* default — leave that history line, but add one line noting the value is now env-overridable via `VOX_MENS_DEFAULT_MODEL`.
Expected: success.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-populi/src/mens/mod.rs crates/vox-ml-cli/src crates/vox-populi/src/cloud
git commit -m "feat(mens): resolve default model id from VOX_MENS_DEFAULT_MODEL (HC-G08-01)"
```

---

### Task 7: Single SSOT const for the `@vox/runtime` npm version pin (HC-G06-03)

`scaffold.rs` hard-codes `"0.6.0"` twice in a generated `package.json`, duplicated across codegen crates. Introduce one const and reference it.

**Files:**
- Modify: `crates/vox-rn-codegen/src/scaffold.rs:216-217` (and add the const near the top of the file)
- Test: inline `#[cfg(test)]` in `crates/vox-rn-codegen/src/scaffold.rs`

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-rn-codegen/src/scaffold.rs`:

```rust
#[cfg(test)]
mod runtime_pin_tests {
    use super::*;

    #[test]
    fn runtime_version_is_semver_triple() {
        let parts: Vec<&str> = VOX_RUNTIME_NPM_VERSION.split('.').collect();
        assert_eq!(parts.len(), 3, "expected MAJOR.MINOR.PATCH");
        assert!(parts.iter().all(|p| p.parse::<u32>().is_ok()));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-rn-codegen runtime_pin_tests`
Expected: FAIL — `cannot find value VOX_RUNTIME_NPM_VERSION`.

- [ ] **Step 3: Add the const and reference it in the template**

Near the top of `crates/vox-rn-codegen/src/scaffold.rs` (after the `use` lines), add:

```rust
/// SSOT for the generated `@vox/runtime-*` npm dependency pin emitted into
/// scaffolded React-Native projects. Bump in lockstep with the published runtime.
pub const VOX_RUNTIME_NPM_VERSION: &str = "0.6.0";
```

The pin lives inside a `format!`/`r#"…"#` template (lines 216-217). The literal version string must become an interpolated field. Replace:

```rust
    "@vox/runtime-types": "0.6.0",
    "@vox/runtime-rn": "0.6.0"{router_dep}
```

with (matching the existing `{router_dep}` interpolation style — add a `rt = VOX_RUNTIME_NPM_VERSION` named arg to the enclosing `format!`):

```rust
    "@vox/runtime-types": "{rt}",
    "@vox/runtime-rn": "{rt}"{router_dep}
```

Locate the enclosing `format!(` call that owns this `r#"…"#` block and add `rt = VOX_RUNTIME_NPM_VERSION,` to its argument list. Confirm the template uses `{{`/`}}` escaping for literal JSON braces (it does — see lines 207-223), so only the version fields change.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vox-rn-codegen`
Expected: PASS — the pin test plus any existing scaffold golden tests (the rendered output is byte-identical, so goldens stay green).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-rn-codegen/src/scaffold.rs
git commit -m "refactor(rn-codegen): single SSOT const for @vox/runtime npm pin (HC-G06-03)"
```

> **Follow-up (note only):** the same pin appears in other codegen/package crates per the audit cross-reference — promoting `VOX_RUNTIME_NPM_VERSION` to a shared location (e.g. `vox-build-meta`) and having all emitters reference it is tracked in the Follow-On Plans.

---

### Task 8: Make the RAG markdown chunk size configurable (HC-G11-02)

`chunk_markdown_sections` hard-codes a `4096`-char split with zero overlap. Parameterize the threshold (keep `4096` as the default const) so it can be tuned without code edits.

**Files:**
- Modify: `crates/vox-search/src/ingest.rs:75-100`
- Test: inline `#[cfg(test)]` in `crates/vox-search/src/ingest.rs`

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-search/src/ingest.rs`:

```rust
#[cfg(test)]
mod chunk_tests {
    use super::*;

    #[test]
    fn small_threshold_splits_more() {
        let text = "para one is fairly long here\nand continues across lines\nmore body text follows\n";
        let big = chunk_markdown_sections_with_size(text, 4096);
        let small = chunk_markdown_sections_with_size(text, 20);
        assert!(small.len() >= big.len());
        assert!(!small.is_empty());
    }

    #[test]
    fn default_wrapper_uses_default_const() {
        let text = "# Heading\nbody\n";
        assert_eq!(
            chunk_markdown_sections(text),
            chunk_markdown_sections_with_size(text, DEFAULT_RAG_CHUNK_CHARS),
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-search chunk_tests`
Expected: FAIL — `cannot find function chunk_markdown_sections_with_size` / `cannot find value DEFAULT_RAG_CHUNK_CHARS`.

- [ ] **Step 3: Parameterize the function**

In `crates/vox-search/src/ingest.rs`, add above `fn chunk_markdown_sections`:

```rust
/// Default RAG markdown chunk size in characters. Override at the call boundary
/// (e.g. via `VOX_RAG_CHUNK_CHARS`) by passing `chunk_markdown_sections_with_size`.
pub const DEFAULT_RAG_CHUNK_CHARS: usize = 4096;
```

Replace the existing `fn chunk_markdown_sections(text: &str) -> Vec<String> { … }` (lines 75-100) with a thin default wrapper plus the parameterized core, changing only the `> 4096` literal to the `max_chars` parameter:

```rust
fn chunk_markdown_sections(text: &str) -> Vec<String> {
    chunk_markdown_sections_with_size(text, DEFAULT_RAG_CHUNK_CHARS)
}

fn chunk_markdown_sections_with_size(text: &str, max_chars: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut cur = String::new();
    for line in text.lines() {
        if line.starts_with("# ") || line.starts_with("## ") {
            if !cur.trim().is_empty() {
                chunks.push(cur.trim().to_string());
            }
            cur = format!("{line}\n");
        } else {
            cur.push_str(line);
            cur.push('\n');
            if cur.len() > max_chars {
                chunks.push(cur.trim().to_string());
                cur.clear();
            }
        }
    }
    if !cur.trim().is_empty() {
        chunks.push(cur.trim().to_string());
    }
    if chunks.is_empty() && !text.trim().is_empty() {
        chunks.push(text.trim().to_string());
    }
    chunks
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vox-search chunk_tests`
Expected: PASS (2 tests). Existing callers of `chunk_markdown_sections` are unchanged.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-search/src/ingest.rs
git commit -m "refactor(search): parameterize RAG markdown chunk size (HC-G11-02)"
```

> **Wiring note (note only):** reading `VOX_RAG_CHUNK_CHARS` at the ingest entry point and passing it through is a one-line follow-up; the audit also flags that doc chunking is triplicated across three sites (ingest.rs, persisted-doc RAG, research orchestrator) — unifying them is tracked in the Follow-On Plans.

---

## Final verification (after all 8 tasks)

- [ ] **Run the full touched-crate test + clippy sweep**

```bash
cargo test -p vox-actor-runtime -p vox-orchestrator -p vox-scaling-policy -p vox-container-types -p vox-container -p vox-plugin-runtime-container -p vox-plugin-runtime-wasm -p vox-populi -p vox-ml-cli -p vox-rn-codegen -p vox-search
cargo clippy -p vox-actor-runtime -p vox-orchestrator -p vox-scaling-policy -p vox-container-types -p vox-plugin-runtime-wasm -p vox-populi -p vox-rn-codegen -p vox-search -- -D warnings
```
Expected: all green. (Per project policy, run `cargo clippy -p <crate>` on touched crates before any admin-merge; do **not** `cargo fmt --all` — format touched crates with `cargo fmt -p <crate>`.)

- [ ] **Format touched crates**

```bash
cargo fmt -p vox-actor-runtime -p vox-orchestrator -p vox-scaling-policy -p vox-container-types -p vox-plugin-runtime-wasm -p vox-populi -p vox-rn-codegen -p vox-search
```

---

## Follow-On Plans (the other 127 findings — not in this plan)

Each is its own plan/PR so each lands working, testable software. Finding IDs reference [`FINDINGS_INDEX.md`](../../../graphify-out/config-audit-graph/FINDINGS_INDEX.md).

1. **GUI / LLM settings-registry split-brain** — **already owned** by `project_llm_ai_settings_ssot_enforce_2026` (vox-config accessors ↔ operator_registry ↔ vox-gui FIELDS, dual-egress seal, `vox://llm-config-changed`). Covers HC-G05-02/03/04 and the LLM-settings backend-gaps. **Do not duplicate.**
2. **Escaped-`vox_config::timeouts` SSOT batch** — values that hand-roll a duration instead of using the existing SSOT they already import: HC-G10-03 (browser CDP 90s), HC-G12-09 (workflow populi 30s), G09 jj OP_TIMEOUT 120s, and the AIMD triplet HC-G04-01..05. One PR routing each through `vox_config::timeouts`.
3. **Container hardening v2** — dedup the two identical `docker.rs`, add `--stop-timeout`/run-timeout, and set safe *default* limits in the deploy-codegen caller (depends on Task 4).
4. **GUI inline-literal cleanup** — HC-G05-05..09 (poll intervals, fetch caps, debounce) routed through `config/constants.ts`; bridges to plan #1 but is GUI-local. Coordinate ordering with plan #1.
5. **Webhook / plugin endpoints** — HC-G10-01 (`0.0.0.0:9080`), HC-G10-02 (channel cap 256), HC-G10-04 (retry/backoff) → env/config.
6. **CLI default literals** — HC-G03-01..17 (server port 3000, inference `:7863`, OpenClaw ports, upgrade repo slug, voxup Rust pin drift) → clap defaults / env, de-duplicating the repeated URL/port literals.
7. **Gamify economy SSOT** — HC-G12-05 (~120-entry reward table + grind/streak/trust constants) into the existing DB-override mechanism; the clearest "needs an economy SSOT" gap.
8. **Long-tail medium/low batch** — remaining ~40 low-severity literals grouped by crate, mechanical de-literalization, lowest priority.

**Suggested sequencing:** Phase 1-3 of *this* plan first (highest blast-radius safety + cost correctness), then plan #1 (settings registry, already specced), then #2 (SSOT convergence), then the rest opportunistically.

---

## Self-Review

- **Spec coverage:** All 8 high-severity findings (HC-G01-01, G02-08, G04-09, G06-03, G07-01, G07-05, G08-01, G11-02) each map to a task (3,2,1,7,5,4,6,8 respectively). The 127 medium/low are enumerated as scoped follow-on plans; the GUI settings-registry overlap is explicitly delegated to the existing plan to avoid duplication.
- **Placeholder scan:** No TBD/TODO/"handle edge cases" — every code step shows complete code; the three "wiring notes" point at a single concrete call site found via an exact `rg` command, with an explicit instruction to stop rather than fabricate if the site doesn't exist.
- **Type consistency:** Resolver naming is consistent (`resolve_*`/`from_*`/`*_with_size`); `RunOpts.resource_args()` is defined in Task 4 Step 3 and consumed in Step 5 with the same signature; `request_timeout(config)` defined and consumed in Task 1; `DEFAULT_WASM_SKILL_FUEL`/`resolve_fuel`, `DEFAULT_RAG_CHUNK_CHARS`/`chunk_markdown_sections_with_size`, `VOX_RUNTIME_NPM_VERSION`, `resolve_default_model_id`/`default_model_id` all defined before use.
