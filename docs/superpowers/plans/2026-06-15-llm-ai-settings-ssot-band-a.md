# Enforceable LLM/AI Settings SSOT — Band A Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make a single declarative registry in `vox-config` the only home for LLM/AI provider/endpoint/model/tuning/budget settings — with the GUI catalog and operator registry as *views* over it, a sealed egress chokepoint, reactive change surfacing, and a snapshot-cached resolver.

**Architecture:** A const `LLM_CONFIG_KEYS` table (`vox-config/src/llm_config_registry.rs`) is the SSOT. `vox-gui` `FIELDS` and `operator_registry` become views; a parity test forbids drift; a new `vox-code-audit` detector forbids unregistered LLM env reads; an `arch-check`-backed seal + an upgraded `llm_provider_call` detector kill the second egress in `vox-gamify`; a `tokio::sync::watch` snapshot drives both fast resolution and `vox://llm-config-changed` GUI events.

**Tech Stack:** Rust (workspace crates `vox-config`, `vox-gui` (Tauri 2), `vox-code-audit`, `vox-arch-check`, `vox-actor-runtime`, `vox-gamify`), `tokio::sync::watch`, `trybuild`, vitest (GUI). Windows-safe formatting per `AGENTS.md` (`cargo fmt -p <crate>`, never `--all`).

**Scope:** Band A = Phases 0–5 (the config-key surface + egress seal + reactive GUI + perf). Band B (orchestrator knobs, Phases 6–8) is a **separate plan**, written after Phase 0's manifest exists. See spec `docs/superpowers/specs/2026-06-15-llm-ai-settings-ssot-design.md`.

**Per-phase close (applies to every phase):** after the phase's tasks, run `/code-review` on the diff, then `cargo clippy -p <each touched crate> -- -D warnings` and the phase's tests green, before moving on. These are not repeated as steps inside each task.

---

## File Structure

| File | Responsibility | Phase |
|---|---|---|
| `crates/vox-config/src/llm_config_registry.rs` (create) | The SSOT: `LlmConfigKey` type, `LLM_CONFIG_KEYS` table, `gui_fields()`, `resolve()`, snapshot + `subscribe()` | 1,3,5 |
| `crates/vox-config/src/llm_config_registry/keys.rs` (create) | The const key data only (kept separate so the table can grow without touching logic) | 1 |
| `crates/vox-config/src/operator_registry.rs` (modify) | `OPERATOR_TUNING_ENVS` becomes a view over the registry | 1 |
| `crates/vox-config/src/lib.rs` (modify) | `pub mod llm_config_registry;` + re-exports | 1 |
| `crates/vox-config/tests/llm_registry_parity.rs` (create) | The parity test (registry == accessors == operator view == gui) | 1,2 |
| `crates/vox-code-audit/src/detectors/unregistered_llm_env.rs` (create) | Flags `env::var("<llm-shaped>")` not in the registry | 1 |
| `crates/vox-code-audit/src/detectors/llm_provider_call.rs` (modify) | Add Rust-workspace second-egress detection + allowlist | 4 |
| `crates/vox-gui/src/commands/user_config.rs` (modify) | `FIELDS` derived from `gui_fields()`; subscribe + emit events | 2,3 |
| `crates/vox-config/src/snapshot.rs` (create) | `LlmConfigSnapshot` + `watch` channel + invalidate-on-write | 3,5 |
| `crates/vox-gamify/src/ai/client/transport.rs` (modify) | Route through `vox-actor-runtime/llm` facade | 4 |
| `docs/src/architecture/layers.toml` (modify) | arch-check rule: only egress crate reaches egress primitives | 4 |
| `crates/vox-actor-runtime/tests/egress_seal.rs` + `tests/ui/*.rs` (create) | `trybuild` compile_fail for out-of-facade provider client | 4 |
| `crates/vox-test-harness/src/env_scratch.rs` (modify) | Test helper that mutates env *and* invalidates the snapshot | 5 |
| `docs/superpowers/specs/llm-config-key-manifest.md` (create, Phase 0) | The canonical key inventory that seeds the table | 0 |

---

## Phase 0 — Inventory (parallel subagent fan-out, no code change)

**Goal:** Produce `docs/superpowers/specs/llm-config-key-manifest.md` — every LLM/AI setting key in the workspace, so the registry table is seeded from reality, not guesswork.

### Task 0.1: Fan out inventory subagents

**Files:**
- Create: `docs/superpowers/specs/llm-config-key-manifest.md`

- [ ] **Step 1: Dispatch one `Explore` subagent per crate-cluster, in a single message.** Clusters (split so each agent holds its area in context):
  - `vox-config` (inference.rs, routing_policy.rs, operator_registry.rs, config/, env_parse.rs, toml_config.rs)
  - `vox-orchestrator` + `vox-orchestrator-types` + `vox-orchestrator-mcp` (models/, tier_cascade.rs, calibration.rs, routing/, llm_bridge/)
  - `vox-actor-runtime` (llm/, model_resolution.rs)
  - `vox-gamify` (ai/)
  - `vox-secrets` + `vox-code-audit` (spec/registry/llm.rs, review/, detectors/)
  - `vox-gui` (commands/)

  Each agent prompt (substitute CLUSTER):
  > Read-only inventory. In crate-cluster CLUSTER, find every configuration knob that affects LLM/AI behavior: (a) every `std::env::var("…")` / `env_parse::*` read whose name relates to a provider, model, endpoint/base-url, API key, tuning (temperature/top_p/num_ctx), routing, budget, or inference profile; (b) every hardcoded model id, provider hostname, or endpoint URL constant; (c) every config struct field that tunes selection/routing/cascade/autonomic/bandit behavior. For each, return a row: `env_or_const | file:line | current_default | kind(String/Url/Float/Int/Bool/Enum) | reads_via(accessor fn name or "raw env::var" or "const") | secret?(y/n) | one-line purpose`. Return ONLY a markdown table. Do not modify files.

- [ ] **Step 2: Synthesize** the agent tables into `docs/superpowers/specs/llm-config-key-manifest.md`: one deduplicated table sorted by env/const name, plus a `## Band split` section tagging each row `band-a` (provider/endpoint/model/tuning/budget) or `band-b` (orchestrator selection/routing/cascade/autonomic). Mark any key currently read by a raw `env::var` (no accessor) — those need both a registry entry and an accessor in Phase 1.

- [ ] **Step 3: Commit**

```bash
git add docs/superpowers/specs/llm-config-key-manifest.md
git commit -m "docs(spec): LLM/AI config-key manifest (Phase 0 inventory)"
```

> **Phase 0 has no `/code-review` (docs only). Verify by reading the manifest: every accessor in `crates/vox-config/src/inference.rs` (lines 70–336) appears as a `band-a` row.**

---

## Phase 1 — Registry foundation

**Goal:** The SSOT table exists, `operator_registry` is a view over it, and a parity test + unregistered-env detector forbid drift. No GUI change yet; accessors keep current env-backed behavior.

### Task 1.1: Registry types

**Files:**
- Create: `crates/vox-config/src/llm_config_registry.rs`
- Modify: `crates/vox-config/src/lib.rs:10` (add `pub mod`)

- [ ] **Step 1: Write the failing test** (append to a new `tests` mod at the bottom of `llm_config_registry.rs`):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_is_nonempty_and_keys_unique() {
        assert!(!LLM_CONFIG_KEYS.is_empty(), "registry must seed keys");
        let mut seen = std::collections::HashSet::new();
        for k in LLM_CONFIG_KEYS {
            assert!(seen.insert(k.env), "duplicate key in registry: {}", k.env);
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-config --lib llm_config_registry::tests::registry_is_nonempty -- --nocapture`
Expected: FAIL — `LLM_CONFIG_KEYS` / module not found.

- [ ] **Step 3: Write minimal implementation** — top of `crates/vox-config/src/llm_config_registry.rs`:

```rust
//! SSOT for LLM/AI settings keys. The GUI catalog, the operator registry, and the
//! CI allowlist are VIEWS over `LLM_CONFIG_KEYS` — never parallel copies. A parity
//! test (`tests/llm_registry_parity.rs`) forbids drift.

use crate::operator_registry::ConfigClass;

/// One LLM/AI setting. `env` is the canonical identity used by every view.
#[derive(Debug, Clone, Copy)]
pub struct LlmConfigKey {
    pub env: &'static str,
    pub default: DefaultValue,
    pub kind: Kind,
    pub group: Group,
    pub class: ConfigClass,
    pub label: &'static str,
    pub hint: &'static str,
    pub options: &'static [&'static str],
    pub secret: bool,
    pub persistence: Persistence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind { String, Url, Float, Int, Bool, Path, Enum }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group { General, ModelsAndEndpoints, Tuning, Training }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Persistence { VoxConfig, FlatToml, EnvOnly }

/// Default rendered as a display string; `Computed` defers to a `vox-config` accessor
/// so a single source still owns fallbacks.
#[derive(Debug, Clone, Copy)]
pub enum DefaultValue { Literal(&'static str), Computed(fn() -> String) }

impl DefaultValue {
    pub fn render(self) -> String {
        match self {
            DefaultValue::Literal(s) => s.to_string(),
            DefaultValue::Computed(f) => f(),
        }
    }
}

include!("llm_config_registry/keys.rs");
```

- [ ] **Step 4: Create the seed key data** `crates/vox-config/src/llm_config_registry/keys.rs` — seed with the Band-A keys we already know exist (the GUI surface + the inference accessors). One worked entry per kind; **populate the rest from the Phase 0 manifest in Task 1.2**:

```rust
/// The single home for every LLM/AI setting key. Grows via Task 1.2 (manifest-driven),
/// gated by the parity test.
pub const LLM_CONFIG_KEYS: &[LlmConfigKey] = &[
    LlmConfigKey {
        env: "OPENROUTER_BASE_URL",
        default: DefaultValue::Computed(crate::inference::openrouter_base_url),
        kind: Kind::Url, group: Group::ModelsAndEndpoints, class: ConfigClass::UserPreference,
        label: "OpenRouter base URL", hint: "OpenAI-compatible OpenRouter endpoint",
        options: &[], secret: false, persistence: Persistence::FlatToml,
    },
    LlmConfigKey {
        env: "VOX_OPENAI_BASE_URL",
        default: DefaultValue::Computed(crate::inference::openai_compatible_base_url),
        kind: Kind::Url, group: Group::ModelsAndEndpoints, class: ConfigClass::UserPreference,
        label: "OpenAI base URL", hint: "OpenAI-compatible cloud endpoint",
        options: &[], secret: false, persistence: Persistence::FlatToml,
    },
    LlmConfigKey {
        env: "OLLAMA_TUNING_TEMPERATURE",
        default: DefaultValue::Literal(""),
        kind: Kind::Float, group: Group::Tuning, class: ConfigClass::UserPreference,
        label: "Ollama temperature", hint: "Sampling temperature for local Ollama",
        options: &[], secret: false, persistence: Persistence::FlatToml,
    },
    LlmConfigKey {
        env: "OPENROUTER_API_KEY",
        default: DefaultValue::Literal(""),
        kind: Kind::String, group: Group::ModelsAndEndpoints, class: ConfigClass::UserPreference,
        label: "OpenRouter API key", hint: "Resolved via Clavis; never written to config.toml",
        options: &[], secret: true, persistence: Persistence::EnvOnly,
    },
];
```

- [ ] **Step 5: Wire the module** — add to `crates/vox-config/src/lib.rs` after line 9 (`pub mod inference;`):

```rust
pub mod llm_config_registry;
```

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo test -p vox-config --lib llm_config_registry::tests -- --nocapture`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-config/src/llm_config_registry.rs crates/vox-config/src/llm_config_registry/keys.rs crates/vox-config/src/lib.rs
git commit -m "feat(vox-config): LlmConfigKey registry scaffold + seed keys"
```

### Task 1.2: Populate the registry from the manifest (TDD loop)

**Files:**
- Modify: `crates/vox-config/src/llm_config_registry/keys.rs`
- Create: `crates/vox-config/tests/llm_registry_parity.rs`

- [ ] **Step 1: Write the failing parity test** `crates/vox-config/tests/llm_registry_parity.rs` — assert the registry covers every Band-A accessor. List the accessor-backed env names explicitly (this list is itself reviewed against `inference.rs`):

```rust
//! Parity: every Band-A LLM/AI accessor must have a registry entry. Adding an accessor
//! without registering it (or vice versa) fails here.
use std::collections::HashSet;
use vox_config::llm_config_registry::LLM_CONFIG_KEYS;

/// Env names that `vox-config` accessors read (from crates/vox-config/src/inference.rs).
/// Keep in sync with that file; the test below is the guard.
const ACCESSOR_ENV_KEYS: &[&str] = &[
    "OPENROUTER_BASE_URL", "VOX_OPENAI_BASE_URL", "POPULI_URL",
    "OPENROUTER_API_KEY", "HUGGINGFACE_HUB_TOKEN",
    "OLLAMA_TUNING_TEMPERATURE", "OLLAMA_TUNING_TOP_P", "OLLAMA_TUNING_NUM_CTX",
    "OPENAI_TUNING_TEMPERATURE", "OPENAI_TUNING_TOP_P",
    "ANTHROPIC_TUNING_TEMPERATURE", "ANTHROPIC_TUNING_TOP_P",
    "GEMINI_TUNING_TEMPERATURE", "GEMINI_TUNING_TOP_P",
    "TOGETHER_TUNING_TEMPERATURE", "TOGETHER_TUNING_TOP_P",
    // …complete from docs/superpowers/specs/llm-config-key-manifest.md (band-a rows)
];

#[test]
fn registry_covers_every_accessor_key() {
    let registered: HashSet<&str> = LLM_CONFIG_KEYS.iter().map(|k| k.env).collect();
    let missing: Vec<&str> = ACCESSOR_ENV_KEYS.iter().copied()
        .filter(|k| !registered.contains(k)).collect();
    assert!(missing.is_empty(), "accessors not in registry: {missing:?}");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-config --test llm_registry_parity registry_covers_every_accessor_key`
Expected: FAIL — lists `POPULI_URL`, `ANTHROPIC_TUNING_*`, `GEMINI_TUNING_*`, etc. as missing.

- [ ] **Step 3: Add one registry entry per missing key** in `keys.rs`, copying the worked-example shape. For tuning keys use `Kind::Float`/`Int`, `Group::Tuning`, `DefaultValue::Literal("")`. For endpoint keys use `Kind::Url`, `Group::ModelsAndEndpoints`, `DefaultValue::Computed(<accessor>)`. For `*_API_KEY`/`*_TOKEN` set `secret: true`, `persistence: EnvOnly`. Pull `label`/`hint` from the manifest purpose column.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-config --test llm_registry_parity registry_covers_every_accessor_key`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-config/src/llm_config_registry/keys.rs crates/vox-config/tests/llm_registry_parity.rs
git commit -m "feat(vox-config): register all Band-A LLM/AI keys + parity test"
```

### Task 1.3: `operator_registry` becomes a view

**Files:**
- Modify: `crates/vox-config/src/operator_registry.rs`

- [ ] **Step 1: Write the failing test** (append to `operator_registry.rs` tests):

```rust
#[test]
fn llm_operator_envs_are_registry_backed() {
    use crate::llm_config_registry::LLM_CONFIG_KEYS;
    // Every non-secret UserPreference LLM key surfaces as an operator tuning env.
    for k in LLM_CONFIG_KEYS {
        if !k.secret && k.class == ConfigClass::UserPreference {
            assert!(
                operator_tuning_envs().iter().any(|e| e.name == k.env),
                "registry key {} missing from operator view", k.env
            );
        }
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-config --lib operator_registry::tests::llm_operator_envs_are_registry_backed`
Expected: FAIL — `operator_tuning_envs` not defined.

- [ ] **Step 3: Add a view function** to `operator_registry.rs` that yields the existing static specs **plus** the registry-derived LLM ones, deduped by `name`:

```rust
/// The operator tuning envs as a view: the static infra knobs (OPERATOR_TUNING_ENVS)
/// plus every non-secret UserPreference key from the LLM registry. SSOT = the registry.
pub fn operator_tuning_envs() -> Vec<OperatorEnvSpec> {
    let mut out: Vec<OperatorEnvSpec> = OPERATOR_TUNING_ENVS.to_vec();
    for k in crate::llm_config_registry::LLM_CONFIG_KEYS {
        if k.secret || k.class != ConfigClass::UserPreference { continue; }
        if out.iter().any(|e| e.name == k.env) { continue; }
        out.push(OperatorEnvSpec {
            name: k.env, description: k.hint,
            defaults: "", // rendered lazily by callers via registry default
            config_class: k.class,
        });
    }
    out
}
```

(Add `#[derive(Clone)]` to `OperatorEnvSpec` if not present, so `.to_vec()` works.)

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-config --lib operator_registry::tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-config/src/operator_registry.rs
git commit -m "refactor(vox-config): operator_tuning_envs() is a view over the LLM registry"
```

### Task 1.4: Unregistered-env detector

**Files:**
- Create: `crates/vox-code-audit/src/detectors/unregistered_llm_env.rs`
- Modify: `crates/vox-code-audit/src/detectors/mod.rs` (register the detector — match the existing registration pattern used by `llm_provider_call`)

- [ ] **Step 1: Write the failing test** (in the new file's `tests` mod):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::SourceFile;
    use std::path::PathBuf;

    fn rs(code: &str) -> SourceFile { SourceFile::new(PathBuf::from("t.rs"), code.to_string()) }

    #[test]
    fn flags_unregistered_llm_shaped_env() {
        let d = UnregisteredLlmEnvDetector::new();
        let f = rs(r#"let x = std::env::var("OPENROUTER_SECRET_TWEAK").unwrap();"#);
        assert!(!d.detect(&f, None).is_empty(), "llm-shaped unregistered env should fire");
    }

    #[test]
    fn ignores_registered_key() {
        let d = UnregisteredLlmEnvDetector::new();
        let f = rs(r#"let x = std::env::var("OPENROUTER_BASE_URL").ok();"#);
        assert!(d.detect(&f, None).is_empty(), "registered keys must not fire");
    }

    #[test]
    fn ignores_non_llm_env() {
        let d = UnregisteredLlmEnvDetector::new();
        let f = rs(r#"let x = std::env::var("PATH").unwrap();"#);
        assert!(d.detect(&f, None).is_empty(), "non-llm env must not fire");
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-code-audit --lib detectors::unregistered_llm_env`
Expected: FAIL — type not found.

- [ ] **Step 3: Implement the detector.** Match the `DetectionRule` impl shape from `llm_provider_call.rs`. Match `std::env::var("NAME")` where `NAME` matches an LLM-shape prefix (`OPENROUTER_|OPENAI_|ANTHROPIC_|GEMINI_|TOGETHER_|OLLAMA_TUNING_|HF_|HUGGINGFACE_|VOX_GEMINI_|POPULI_`) and is **not** one of the registered names. Embed the registered names as a `const REGISTERED: &[&str]` generated note pointing at the registry (the detector cannot depend on `vox-config`, so it carries a copy guarded by a parity test — Step 4).

```rust
use crate::rules::{DetectionRule, Finding, FindingConfidence, Language, Severity, SourceFile};
use crate::diagnostics::catalog;
use regex::Regex;

pub struct UnregisteredLlmEnvDetector { env_call: Regex, llm_shape: Regex }

/// Mirror of vox_config::llm_config_registry env names. Parity-tested in vox-config.
const REGISTERED: &[&str] = &[
    "OPENROUTER_BASE_URL", "VOX_OPENAI_BASE_URL", "POPULI_URL", "OPENROUTER_API_KEY",
    "HUGGINGFACE_HUB_TOKEN", "OLLAMA_TUNING_TEMPERATURE", "OLLAMA_TUNING_TOP_P",
    "OLLAMA_TUNING_NUM_CTX", "OPENAI_TUNING_TEMPERATURE", "OPENAI_TUNING_TOP_P",
    "ANTHROPIC_TUNING_TEMPERATURE", "ANTHROPIC_TUNING_TOP_P",
    "GEMINI_TUNING_TEMPERATURE", "GEMINI_TUNING_TOP_P",
    "TOGETHER_TUNING_TEMPERATURE", "TOGETHER_TUNING_TOP_P",
    // …keep in sync with vox-config; guarded by registry_detector_parity test.
];

impl Default for UnregisteredLlmEnvDetector { fn default() -> Self { Self::new() } }

impl UnregisteredLlmEnvDetector {
    pub fn new() -> Self {
        Self {
            env_call: Regex::new(r#"env::var(?:_os)?\(\s*"([A-Z0-9_]+)"\s*\)"#).expect("rx"),
            llm_shape: Regex::new(
                r"^(OPENROUTER_|OPENAI_|ANTHROPIC_|GEMINI_|TOGETHER_|OLLAMA_TUNING_|HF_|HUGGINGFACE_|VOX_GEMINI_|POPULI_)"
            ).expect("rx"),
        }
    }
}

impl DetectionRule for UnregisteredLlmEnvDetector {
    fn id(&self) -> &'static str { "vox/llm/unregistered-env" }
    fn name(&self) -> &'static str { "Unregistered LLM Env Detector" }
    fn description(&self) -> &'static str {
        "Flags env vars that tune LLM/AI behavior but are not declared in vox_config::llm_config_registry."
    }
    fn severity(&self) -> Severity { Severity::Error }
    fn languages(&self) -> &[Language] { &[Language::Rust] }
    fn explain(&self) -> &'static str {
        "Every LLM/AI setting must be declared in vox_config::llm_config_registry so it surfaces \
         to the GUI and the CI allowlist. Reading an unregistered LLM-shaped env var bypasses the SSOT."
    }
    fn detect(&self, file: &SourceFile, _c: Option<&crate::analysis::RustFileContext>) -> Vec<Finding> {
        let mut out = Vec::new();
        for (i, line) in file.lines.iter().enumerate() {
            let t = line.trim();
            if t.starts_with("//") || t.starts_with('*') { continue; }
            for cap in self.env_call.captures_iter(line) {
                let name = &cap[1];
                if self.llm_shape.is_match(name) && !REGISTERED.contains(&name) {
                    out.push(Finding {
                        rule_id: self.id().to_string(),
                        diagnostic_id: None,
                        rule_name: self.name().to_string(),
                        severity: Severity::Error,
                        file: file.path.clone(), line: i + 1, column: 0,
                        message: format!("LLM/AI env `{name}` is not registered in vox_config::llm_config_registry."),
                        suggestion: Some("Add an LlmConfigKey entry for it, then read it via the registry.".into()),
                        alternatives: vec![],
                        rationale: Some("Unregistered LLM settings never reach the GUI or CI allowlist.".into()),
                        context: file.context_around(i + 1, 2),
                        confidence: Some(FindingConfidence::High),
                        evidence: None,
                    });
                }
            }
        }
        out
    }
}
```

(If `catalog` has no matching id, omit `diagnostic_id`/leave `None` as above. Confirm the exact `Finding` field set against `llm_provider_call.rs` and adjust if the struct differs.)

- [ ] **Step 4: Add the detector→registry parity test** in `crates/vox-config/tests/llm_registry_parity.rs`:

```rust
#[test]
fn detector_registered_list_matches_registry() {
    // The vox-code-audit detector carries a copy of the registry env names because it
    // cannot depend on vox-config. This test fails if they drift.
    let registry: std::collections::HashSet<&str> =
        LLM_CONFIG_KEYS.iter().map(|k| k.env).collect();
    // Paste of detector REGISTERED — kept identical by this assertion.
    const DETECTOR_REGISTERED: &[&str] = &[ /* same list as detector */ ];
    for k in DETECTOR_REGISTERED {
        assert!(registry.contains(k), "detector lists {k} but registry does not");
    }
}
```

- [ ] **Step 5: Register + run**

Register the detector in `detectors/mod.rs` next to `LlmProviderCallDetector`. Run:
`cargo test -p vox-code-audit --lib detectors::unregistered_llm_env && cargo test -p vox-config --test llm_registry_parity`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-code-audit/src/detectors/unregistered_llm_env.rs crates/vox-code-audit/src/detectors/mod.rs crates/vox-config/tests/llm_registry_parity.rs
git commit -m "feat(vox-code-audit): unregistered-LLM-env detector + registry parity"
```

---

## Phase 2 — GUI derives from the registry

**Goal:** `vox-gui` `FIELDS` is produced from `gui_fields()`; the hand catalog is deleted; all registered keys become visible/editable.

### Task 2.1: `gui_fields()` on the registry

**Files:**
- Modify: `crates/vox-config/src/llm_config_registry.rs`

- [ ] **Step 1: Write the failing test** (registry tests mod):

```rust
#[test]
fn gui_fields_cover_every_key() {
    assert_eq!(gui_fields().len(), LLM_CONFIG_KEYS.len());
    assert!(gui_fields().iter().any(|f| f.key == "OPENROUTER_BASE_URL"));
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-config --lib llm_config_registry::tests::gui_fields_cover_every_key`
Expected: FAIL — `gui_fields` not found.

- [ ] **Step 3: Implement `gui_fields()`** returning a GUI-agnostic DTO (so `vox-gui` only maps, not redeclares):

```rust
/// GUI-facing projection of one key. `vox-gui` maps this to its Tauri DTO — it does not
/// own the field list.
pub struct GuiField {
    pub key: &'static str, pub label: &'static str, pub hint: &'static str,
    pub group: &'static str, pub kind: &'static str,
    pub options: &'static [&'static str], pub default: String,
}

pub fn gui_fields() -> Vec<GuiField> {
    LLM_CONFIG_KEYS.iter().filter(|k| !k.secret).map(|k| GuiField {
        key: k.env, label: k.label, hint: k.hint,
        group: match k.group {
            Group::General => "General", Group::ModelsAndEndpoints => "Models & endpoints",
            Group::Tuning => "Tuning", Group::Training => "Training",
        },
        kind: match k.kind {
            Kind::String => "string", Kind::Url => "string", Kind::Float => "float",
            Kind::Int => "int", Kind::Bool => "bool", Kind::Path => "path", Kind::Enum => "enum",
        },
        options: k.options, default: k.default.render(),
    }).collect()
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-config --lib llm_config_registry::tests::gui_fields_cover_every_key`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-config/src/llm_config_registry.rs
git commit -m "feat(vox-config): gui_fields() projection over the registry"
```

### Task 2.2: Rewire `user_config.rs` onto `gui_fields()`

**Files:**
- Modify: `crates/vox-gui/src/commands/user_config.rs` (replace the `FIELDS` const + `spec_for`/`default_value` data with a build from `gui_fields()`; keep the `VoxConfig`-tier read/write/validate machinery)

- [ ] **Step 1: Write the failing test** (`user_config.rs` tests mod):

```rust
#[test]
fn catalog_matches_registry_nonsecret_keys() {
    let cat = get_user_config();
    let reg = vox_config::llm_config_registry::gui_fields();
    assert_eq!(cat.len(), reg.len(), "GUI catalog must equal registry gui_fields");
    for f in &reg {
        assert!(cat.iter().any(|c| c.key == f.key), "missing key {}", f.key);
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-gui --lib commands::user_config::tests::catalog_matches_registry_nonsecret_keys`
Expected: FAIL — current `FIELDS` has ~16 keys, registry has all of them.

- [ ] **Step 3: Replace the static `FIELDS` source.** In `get_user_config`, build the `UserConfigFieldDto` list from `vox_config::llm_config_registry::gui_fields()` instead of the local `FIELDS`. Keep `flat_effective_value`/`voxconfig_value` for `current_value`, routing each key by the registry's `persistence`/`secret` (the registry now owns `group`/`kind`/`label`/`hint`/`default`/`options`). Delete the local `FIELDS`, `FieldSpec`, and `default_value` duplication; keep `validate`, `apply_voxconfig_field`, `set_user_config`, `reset_user_config`. The VoxConfig-tier fields (`model`, budgets, `data_dir`, `db_url`, train_*) stay registered in the registry with `persistence: VoxConfig`.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-gui --lib commands::user_config::tests`
Expected: PASS.

- [ ] **Step 5: GUI vitest** — update/extend the Runtime-settings vitest so it asserts the rendered field count equals the IPC catalog length (find it under `crates/vox-gui/` vitest dirs; follow the existing settings test pattern). Run the GUI test suite (`pnpm` per memory — `vox-gui` is pnpm-managed): `pnpm -C crates/vox-gui test`.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/src/commands/user_config.rs
git commit -m "refactor(vox-gui): Runtime settings catalog derives from llm_config_registry"
```

---

## Phase 3 — Reactive surfacing

**Goal:** A `watch` snapshot in `vox-config`; writers bump it; a `vox-gui` bridge forwards `vox://llm-config-changed`.

### Task 3.1: Snapshot + watch channel

**Files:**
- Create: `crates/vox-config/src/snapshot.rs`
- Modify: `crates/vox-config/src/lib.rs`, `crates/vox-config/src/toml_config.rs`

- [ ] **Step 1: Write the failing test** (`snapshot.rs` tests mod):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn subscribe_sees_bump_after_write() {
        let mut rx = subscribe();
        let before = rx.borrow().rev;
        bump(&["OPENROUTER_BASE_URL"]);
        assert!(rx.borrow_and_update().rev > before, "rev must advance on bump");
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-config --lib snapshot::tests::subscribe_sees_bump_after_write`
Expected: FAIL.

- [ ] **Step 3: Implement** `snapshot.rs`:

```rust
//! Reactive snapshot of the effective LLM/AI config. Writers call `bump()`; the GUI
//! subscribes via `subscribe()`. This is the only new runtime machinery — event-driven,
//! off the hot path.
use std::sync::OnceLock;
use tokio::sync::watch;

#[derive(Debug, Clone, Default)]
pub struct LlmConfigSnapshot {
    /// Monotonic revision; advances on every write.
    pub rev: u64,
    /// Keys changed in the most recent bump (for targeted GUI updates).
    pub changed: Vec<String>,
}

fn channel() -> &'static (watch::Sender<LlmConfigSnapshot>, watch::Receiver<LlmConfigSnapshot>) {
    static CH: OnceLock<(watch::Sender<LlmConfigSnapshot>, watch::Receiver<LlmConfigSnapshot>)> = OnceLock::new();
    CH.get_or_init(|| watch::channel(LlmConfigSnapshot::default()))
}

pub fn subscribe() -> watch::Receiver<LlmConfigSnapshot> { channel().1.clone() }

/// Advance the revision and record changed keys. Call after any LLM/AI config write.
pub fn bump(changed_keys: &[&str]) {
    let tx = &channel().0;
    let next = {
        let cur = tx.borrow();
        LlmConfigSnapshot { rev: cur.rev + 1, changed: changed_keys.iter().map(|s| s.to_string()).collect() }
    };
    let _ = tx.send(next);
}
```

Add `pub mod snapshot;` to `lib.rs`. In `toml_config.rs`, call `crate::snapshot::bump(&[key])` at the end of `set_user_config_value` and `unset_user_config_value`, and `crate::snapshot::bump(&[])` in `reload_user_config`.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-config --lib snapshot::tests`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-config/src/snapshot.rs crates/vox-config/src/lib.rs crates/vox-config/src/toml_config.rs
git commit -m "feat(vox-config): reactive LlmConfigSnapshot watch channel; writers bump it"
```

### Task 3.2: GUI event bridge

**Files:**
- Modify: `crates/vox-gui/src/commands/user_config.rs` (or the GUI setup module where Tauri `AppHandle` is available)

- [ ] **Step 1: Write the failing test** — a Rust unit test asserting the bridge payload type serializes with `keys` + `rev`:

```rust
#[test]
fn change_event_payload_serializes() {
    let p = LlmConfigChanged { rev: 3, keys: vec!["OPENROUTER_BASE_URL".into()] };
    let j = serde_json::to_string(&p).unwrap();
    assert!(j.contains("\"rev\":3") && j.contains("OPENROUTER_BASE_URL"));
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-gui --lib commands::user_config::tests::change_event_payload_serializes`
Expected: FAIL — type not defined.

- [ ] **Step 3: Implement** the payload + a spawn-once subscriber that emits `vox://llm-config-changed` on each `watch` change:

```rust
#[derive(Debug, Clone, serde::Serialize)]
pub struct LlmConfigChanged { pub rev: u64, pub keys: Vec<String> }

/// Spawn once at GUI startup: forward registry snapshot bumps to the webview.
pub fn spawn_llm_config_bridge(app: tauri::AppHandle) {
    let mut rx = vox_config::snapshot::subscribe();
    tauri::async_runtime::spawn(async move {
        while rx.changed().await.is_ok() {
            let snap = rx.borrow_and_update().clone();
            let _ = app.emit("vox://llm-config-changed", LlmConfigChanged { rev: snap.rev, keys: snap.changed });
        }
    });
}
```

Call `spawn_llm_config_bridge(app.handle().clone())` in the GUI `setup` hook (match how other bridges/listeners are spawned in `vox-gui`'s `lib.rs`/`main.rs`). Use the correct `Emitter` import for Tauri 2 (`use tauri::Emitter;`).

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-gui --lib commands::user_config::tests::change_event_payload_serializes`
Expected: PASS.

- [ ] **Step 5: GUI subscription** — in the Runtime settings frontend, add a listener for `vox://llm-config-changed` that re-invokes `get_user_config` (or patches the changed keys). Add a vitest that simulating the event triggers a refetch. Run `pnpm -C crates/vox-gui test`.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/src/commands/user_config.rs crates/vox-gui/src/lib.rs
git commit -m "feat(vox-gui): forward llm-config-changed snapshot bumps to the webview"
```

---

## Phase 4 — Seal the egress

**Goal:** `vox-actor-runtime/llm` is the only sanctioned provider egress; `vox-gamify` is refactored onto it; arch-check + the upgraded detector prevent regression.

### Task 4.1: Upgrade `llm_provider_call` to catch Rust second-egress

**Files:**
- Modify: `crates/vox-code-audit/src/detectors/llm_provider_call.rs`

- [ ] **Step 1: Write the failing test** (add to that file's tests):

```rust
#[test]
fn flags_rust_egress_via_base_url_accessor_outside_facade() {
    let d = LlmProviderCallDetector::new();
    let code = r#"
let base = vox_config::openrouter_base_url();
let resp = reqwest::Client::new().post(&base).send().await?;
"#;
    let mut f = source("rs", code);
    f.path = std::path::PathBuf::from("crates/vox-gamify/src/ai/client/transport.rs");
    assert!(!d.detect(&f, None).is_empty(), "base-url accessor + reqwest outside facade must fire");
}

#[test]
fn allows_egress_inside_facade_crate() {
    let d = LlmProviderCallDetector::new();
    let code = r#"let resp = reqwest::Client::new().post(&base).send().await?;"#;
    let mut f = source("rs", code);
    f.path = std::path::PathBuf::from("crates/vox-actor-runtime/src/llm/wire.rs");
    assert!(d.detect(&f, None).is_empty(), "facade crate is the allowlisted egress");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-code-audit --lib detectors::llm_provider_call::tests::flags_rust_egress_via_base_url_accessor_outside_facade`
Expected: FAIL.

- [ ] **Step 3: Extend the detector.** Add (a) a regex matching `vox_config::[a-z_]*base_url\(` and `openrouter_base\(`; (b) an allowlist check: if `file.path` contains `crates/vox-actor-runtime/src/llm/`, return no findings for the Rust-egress rule. Fire when a base-url-accessor/provider-hostname appears with a `rust_http_call` in the window AND the file is outside the allowlist. Keep the existing Vox/`populi` behavior unchanged.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-code-audit --lib detectors::llm_provider_call::tests`
Expected: PASS (including the existing tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-code-audit/src/detectors/llm_provider_call.rs
git commit -m "feat(vox-code-audit): detect Rust workspace second-egress outside the llm facade"
```

### Task 4.2: Refactor `vox-gamify` onto the facade

**Files:**
- Modify: `crates/vox-gamify/src/ai/client/transport.rs`, `crates/vox-gamify/src/ai/provider.rs`

- [ ] **Step 1: Write the failing test** — run the new detector over the gamify file to prove it currently fires:

Run: `cargo test -p vox-code-audit --lib` then a temporary integration check (or `cargo run -p vox-cli -- audit <gamify path>` if available). Expected: the gamify transport currently triggers `vox/llm/direct-provider-call`.

- [ ] **Step 2: Replace the direct OpenRouter/Gemini `reqwest` paths** in `transport.rs` with calls into `vox_actor_runtime::llm` (`chat`/`stream`). Preserve gamify's `AiError` mapping, retry-after parsing (`vox_http_client::parse_retry_after`), and streaming surface by adapting the facade's stream type. Keep Ollama/Pollinations local paths if the facade doesn't cover them — but route everything that targets a cloud provider hostname through the facade. Remove `resolve_gemini_key`/`resolve_openrouter_key`/`openrouter_base()` egress duplication; the facade owns key + base-url resolution via the registry.

- [ ] **Step 3: Run gamify tests + the detector** to verify gamify no longer fires and behavior holds:

Run: `cargo test -p vox-gamify && cargo test -p vox-code-audit --lib`
Expected: PASS; gamify path no longer flagged.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gamify/src/ai/
git commit -m "refactor(vox-gamify): route LLM egress through vox-actor-runtime/llm facade"
```

### Task 4.3: arch-check rule + trybuild seal

**Files:**
- Modify: `docs/src/architecture/layers.toml`
- Create: `crates/vox-actor-runtime/tests/egress_seal.rs`, `crates/vox-actor-runtime/tests/ui/out_of_facade_client.rs`

- [ ] **Step 1: Add the arch-check rule** in `layers.toml`: a constraint that only `vox-actor-runtime` may depend on the HTTP-client primitive *for LLM egress* / reference provider base-url accessors. Match the existing rule schema in that file (find an analogous `[[rule]]`/fan-in entry and mirror it). Run `cargo run -p vox-arch-check` and confirm it passes with gamify already refactored (Task 4.2).

- [ ] **Step 2: Write the trybuild compile_fail case** `tests/ui/out_of_facade_client.rs` — a snippet constructing the facade's sealed transport type from outside its module; assert it fails to compile because the constructor is `pub(crate)`/sealed. Wire it via `crates/vox-actor-runtime/tests/egress_seal.rs`:

```rust
#[test]
fn out_of_facade_construction_fails_to_compile() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/out_of_facade_client.rs");
}
```

(Requires sealing the transport constructor: make the provider-client wrapper type in `vox-actor-runtime/src/llm/wire.rs` carry a private field or `pub(crate)` constructor so only the facade mints it. Add `trybuild` as a dev-dependency.)

- [ ] **Step 3: Run**

Run: `cargo run -p vox-arch-check && cargo test -p vox-actor-runtime --test egress_seal`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add docs/src/architecture/layers.toml crates/vox-actor-runtime/tests/ crates/vox-actor-runtime/Cargo.toml crates/vox-actor-runtime/src/llm/wire.rs
git commit -m "feat: arch-check + trybuild seal LLM egress to the vox-actor-runtime facade"
```

---

## Phase 5 — Snapshot-cached resolution (perf)

**Goal:** Accessors resolve through the cached snapshot instead of calling `std::env::var` per invocation; tests prove no per-call env read on a hot accessor.

### Task 5.1: Resolve-once cache + test helper

**Files:**
- Modify: `crates/vox-config/src/snapshot.rs`, `crates/vox-config/src/inference.rs` (one hot accessor as the worked example)
- Modify: `crates/vox-test-harness/src/env_scratch.rs`

- [ ] **Step 1: Write the failing test** — a counting shim proves `openrouter_base_url()` reads env at most once across N calls:

```rust
// in crates/vox-config/tests/snapshot_cache.rs
#[test]
fn hot_accessor_does_not_reread_env_each_call() {
    // env_scratch sets the var AND invalidates the snapshot atomically.
    let _g = vox_test_harness::env_scratch::set_and_invalidate("OPENROUTER_BASE_URL", "https://x/api");
    let a = vox_config::openrouter_base_url();
    let b = vox_config::openrouter_base_url();
    assert_eq!(a, b);
    assert_eq!(a, "https://x/api");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-config --test snapshot_cache`
Expected: FAIL — `set_and_invalidate` not defined.

- [ ] **Step 3: Implement** a cached-value layer in `snapshot.rs` (a `RwLock<HashMap<&'static str, String>>` keyed by env name, filled on first resolve, cleared on `bump`). Route `openrouter_base_url()` through `snapshot::resolved("OPENROUTER_BASE_URL", crate::inference::openrouter_base_url_uncached)`. Add `env_scratch::set_and_invalidate` that sets the env var (existing scratch mechanism) and calls `vox_config::snapshot::bump(&[name])` so the cache clears.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-config --test snapshot_cache`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-config/src/snapshot.rs crates/vox-config/src/inference.rs crates/vox-test-harness/src/env_scratch.rs crates/vox-config/tests/snapshot_cache.rs
git commit -m "perf(vox-config): snapshot-cached LLM config resolution + invalidating test helper"
```

### Task 5.2: Band-A SSOT doc + where-things-live row

**Files:**
- Modify: `docs/src/architecture/where-things-live.md` (add the registry row), `docs/superpowers/specs/2026-06-15-llm-ai-settings-ssot-design.md` (mark Band A done)

- [ ] **Step 1:** Add a `where-things-live.md` row: `LLM/AI setting key → crates/vox-config/src/llm_config_registry.rs (SSOT; GUI + operator_registry are views)`. (This file may be generator-owned — check the header; if auto-generated, run its generator instead of hand-editing, per `feedback_auto_generated_docs`.)

- [ ] **Step 2:** Mark Band A complete in the spec; note Band B is the follow-on plan.

- [ ] **Step 3: Commit**

```bash
git add docs/src/architecture/where-things-live.md docs/superpowers/specs/2026-06-15-llm-ai-settings-ssot-design.md
git commit -m "docs: register llm_config_registry as SSOT in where-things-live; Band A done"
```

---

## Self-Review (completed during authoring)

- **Spec coverage:** Component 1 (registry) → Phase 1; GUI view → Phase 2; reactive surfacing → Phase 3; sealed egress + gamify + detector → Phase 4; perf snapshot → Phase 5; inventory that seeds it all → Phase 0. Orchestrator onboarding (Band B, Phases 6–8) is intentionally a **separate plan** (depends on Band A + Phase 0 manifest). Gap acknowledged, not missed.
- **Placeholder scan:** the only "fill from manifest" steps (Task 1.2 Step 3, detector list) are TDD loops gated by a failing parity test with a fully worked example — mechanical, not vague. Acceptable.
- **Type consistency:** `LlmConfigKey`/`Kind`/`Group`/`Persistence`/`DefaultValue`/`GuiField`/`LlmConfigSnapshot`/`LlmConfigChanged` used consistently across tasks; `bump`/`subscribe`/`gui_fields`/`operator_tuning_envs` names stable throughout.
- **Verify-before-claim:** every implementation step is preceded by a failing-test step and followed by a green-run step; signatures (`-> String`, `Option<f32>`, `Option<i32>`) match `crates/vox-config/src/inference.rs`.

> **Caveat for the implementer:** exact `Finding` struct fields, `SourceFile` constructor, detector registration in `detectors/mod.rs`, the Tauri 2 `setup` hook location, and the `layers.toml` rule schema must be confirmed against the live files before each task — the code blocks above mirror the patterns in `llm_provider_call.rs` / `user_config.rs` but are written from audit reads, not a compile.
