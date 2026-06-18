# VoxMens Hub-and-Spoke Build-Out Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `crates/vox-skills/skills/superpowers/subagent-driven-development.skill.md` (recommended) or `executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **EXECUTION TARGET: Gemini 3.5 Flash inside Google Antigravity.** Read §A (Execution Model for Gemini) before starting any task. It is not optional — the task shapes (atomic-green-commit, verify-before-use, `[PARALLEL-SAFE]`/`[SEQUENTIAL]` tags, two-strike circuit breaker) exist specifically because of this model's documented failure modes.

**Goal:** Turn VoxMens from a single-method, single-base, VoxScript-centric pipeline into a config-only hub-and-spoke where each of three spokes (VoxScript, Rust, Harness/agentic) declares its own data mix, base model, training method, and eval gate in one validated SSOT, trains end-to-end, and is selected at inference by a router — with per-spoke base models scaled to the host's VRAM.

**Architecture:** Extend the existing `domain-profiles.yaml` SSOT (already maps a domain → mix + training params) into the full spoke registry by adding `base` (model + method + preset), `eval_gate`, and `router` fields; enforce it with a `vox-arch-check`-style validator; wire per-spoke `method` through `run_train`; build the missing Rust-authoring and agentic corpora with deterministic verifiers; add a lane-tag router; and resolve base-model selection from a committed, VRAM-scaled decision matrix.

**Tech Stack:** Rust (workspace crates `vox-populi`, `vox-corpus`, `vox-ml-cli`, `vox-arch-check`), YAML SSOTs under `mens/config/`, QLoRA/DPO training (Candle), VoxScript automation (`vox run scripts/*.vox`).

---

## A. Execution Model for Gemini 3.5 Flash (READ FIRST)

Source of these rules: [`docs/src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md) and [`docs/src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md`](../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md).

**Why the plan is shaped this way (model facts):**
- ~48% real-world task completion; **mid-task termination leaves no checkpoint** → every task must end on a compiling, tested, committed tree. A kill wastes at most one task.
- **Hallucinates APIs/symbols with confidence** → every code step that references a symbol/path is preceded by a `rg`/read **verify step**; exact signatures are inlined.
- **Weak long-context recall (MRCR)** → each task repeats the context it needs. Never rely on remembering an earlier task.
- **Poor self-correction (repeats failed actions)** → **two-strike circuit breaker**: if a verification step fails twice, STOP, write a handoff note in the commit body, and surface it. Do not loop.
- **Quota = hard cutoff, no warning** → small tasks, frequent commits.

**Sub-agent & parallelism rules (Antigravity orchestrator + isolated-context subagents):**
- Each task is tagged **`[PARALLEL-SAFE]`** or **`[SEQUENTIAL]`**.
  - `[PARALLEL-SAFE]`: its **Files** block is disjoint from every other parallel-safe task in the same *wave*. The orchestrator may dispatch these as concurrent subagents, one task's text per subagent (isolated window).
  - `[SEQUENTIAL]`: shares a file with, or depends on the output of, an earlier task. Run in order on one agent.
- **Golden rule:** never dispatch two subagents that write the same file — isolated contexts will clobber. When unsure, treat as `[SEQUENTIAL]`.
- Follow `crates/vox-skills/skills/superpowers/dispatching-parallel-agents.skill.md` for dispatch; after a parallel wave, integrate sequentially and run the full crate test suite once.
- **Waves are declared at the top of each Phase.** Do not infer them.

**Vox-specific policy the agent must obey throughout (from `AGENTS.md`/`GEMINI.md`):**
- Automation scripts are `.vox` (`vox run scripts/foo.vox`) — never generate `.ps1`/`.sh`/`.py`.
- **Never `cargo fmt --all`** (Windows arg-limit `os error 206`). Use `cargo fmt -p <crate>`.
- Any `.md` under `docs/src/` needs YAML frontmatter (`title`/`description`/`category`). Plans under `docs/superpowers/` do not.
- No stubs/placeholders in shipped code (`feedback_no_stubs`).
- Before adding a concept, check `docs/src/architecture/where-things-live.md`.

**Verification ritual (the `verification-before-completion` skill) — run before every "Commit" step, paste actual output:**
1. `cargo test -p <crate>` → PASS counts shown.
2. `cargo clippy -p <crate> -- -D warnings` → clean.
3. `cargo fmt -p <crate>` (never `--all`).
4. `cargo check -p <crate>` → compiles.
Evidence before assertion. If any fails twice → two-strike STOP.

---

## B. Research basis & the one decision this plan defers

This plan implements [`docs/src/architecture/voxmens-hub-and-spoke-ssot-research-2026-06-18.md`](../../src/architecture/voxmens-hub-and-spoke-ssot-research-2026-06-18.md). That research **verified** the config-SSOT pattern (Axolotl/torchtune: one declarative file, per-run method selection, list-based data) but the model-selection and serving-topology axes were **rate-limited to unverified**. Therefore:

- **Base-model choice is not hard-coded in this plan.** Phase 3 contains a **decision task** that produces a committed, VRAM-scaled `mens/config/model-registry.yaml`. The plan provides the *mechanism and the candidate matrix*; the agent fills the *verified picks* from a short live re-research (Task 3.1) before wiring. This satisfies "ideal models documented for each, scaled to available resources" without baking in a perishable guess.
- **Serving topology (shared-base adapter hot-swap vs. separate servers) is decided in Phase 7** after the router exists, gated on the same research. The router (Phase 7) is built lane-tag-first so it works regardless of that decision.

---

## C. File Structure (decomposition lock-in)

**Create:**
- `mens/config/model-registry.yaml` — SSOT of allowed base models, their VRAM floor, and method compatibility (Phase 3).
- `mens/config/eval-gates-rust.yaml`, `mens/config/eval-gates-agents.yaml` — per-spoke gates (Phase 2).
- `crates/vox-populi/src/mens/tensor/spoke_validate.rs` — spoke SSOT validator (Phase 1).
- `crates/vox-corpus/src/corpus/rust_authoring.rs` — Rust-authoring pair gen + `cargo check` verifier (Phase 4).
- `crates/vox-corpus/src/corpus/agentic_synth.rs` — tool-use synthesis from skill/CLI/discovery surfaces (Phase 5).
- `crates/vox-corpus/src/corpus/trace_ingest.rs` — agent-trace → SFT/DPO converter (Phase 5).
- `crates/vox-populi/src/mens/router.rs` — lane-tag inference router (Phase 7).
- `mens/schemas/agent_trace_record.schema.json` — trace schema (Phase 5).

**Modify:**
- `crates/vox-populi/src/mens/tensor/domain_profiles.rs` — add `base`/`eval_gate`/`router` fields to `DomainProfile` + `EffectiveDomainProfile` (Phase 1).
- `mens/config/domain-profiles.yaml` — add the three spoke records with new fields (Phase 1, 3).
- `crates/vox-arch-check/src/lib.rs` (+ `main.rs`) — call the spoke validator (Phase 1).
- `crates/vox-ml-cli/src/commands/mens/pipeline.rs` — resolve method/base from profile; strict mix; per-spoke eval (Phases 2, 6).
- `crates/vox-ml-cli/src/commands/schola/train/run_train.rs` — honor profile `method` (Phase 6).

Each file has one responsibility; the corpus generators live beside the existing `extract_rs.rs`/`rust_to_vox.rs` they resemble.

---

## Phase 0 — Handoff preflight (anti-hallucination baseline)

**Wave 0.1:** Task 0.1 only (`[SEQUENTIAL]`, no code).

### Task 0.1: Confirm the world matches the plan [SEQUENTIAL]

**Files:** none (read-only).

- [ ] **Step 1: Verify the SSOT structs exist as the plan assumes**

Run:
```bash
rg -n "pub struct DomainProfile|pub struct EffectiveDomainProfile|pub mix_config|pub context_filter" crates/vox-populi/src/mens/tensor/domain_profiles.rs
rg -n "pub struct MixConfigSchema|pub struct MixRunOptions|pub strict" crates/vox-corpus/src/corpus/mix/mod.rs
rg -n "pub fn check_run" crates/vox-ml-cli/src/commands/mens/eval_gate/check_run.rs
rg -n "pub async fn run_train" crates/vox-ml-cli/src/commands/schola/train/run_train.rs
```
Expected: each prints at least one match. If any returns nothing, the codebase has drifted from this plan — **STOP and write a handoff note**; do not guess replacements.

- [ ] **Step 2: Establish the green baseline**

Run:
```bash
cargo run -p vox-arch-check
cargo check -p vox-populi -p vox-corpus -p vox-ml-cli
```
Expected: arch-check passes; all three crates compile. If not, fix nothing yet — record the failure and STOP (you need a clean baseline to attribute later failures).

- [ ] **Step 3: Confirm Antigravity can see in-repo skills**

Run: `rg -l "skill.md" crates/vox-skills/skills/superpowers/ | head`
Expected: lists `subagent-driven-development.skill.md`, `dispatching-parallel-agents.skill.md`, `verification-before-completion.skill.md`, etc. These are the skills this plan references. No commit (read-only phase).

---

## Phase 1 — The Spoke SSOT (foundation)

**What & why:** Promote `domain-profiles.yaml` to the full spoke registry. Adding a spoke must become a single validated YAML record. This phase adds the `base`/`eval_gate`/`router` fields, a validator that fails CI on drift, and flips mix strictness so a missing corpus is an error, not a silent empty set.

**Wave 1.1 (`[SEQUENTIAL]` chain — all touch `domain_profiles.rs`):** 1.1 → 1.2 → 1.3.
**Wave 1.2 (`[PARALLEL-SAFE]` together — disjoint files):** 1.4 (arch-check), 1.5 (domain-profiles.yaml).

### Task 1.1: Add the `SpokeBase` type and field to `DomainProfile` [SEQUENTIAL]

**Files:**
- Modify: `crates/vox-populi/src/mens/tensor/domain_profiles.rs`
- Test: `crates/vox-populi/src/mens/tensor/domain_profiles.rs` (inline `#[cfg(test)]`)

Context you need (current head of the file, verified):
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainProfile {
    pub description: Option<String>,
    pub context_filter: Option<ContextFilter>,
    pub mix_config: Option<String>,
    // ... existing fields ...
    pub reward_hook: Option<String>,
}
```

- [ ] **Step 1: Write the failing test**

Append to `domain_profiles.rs`:
```rust
#[cfg(test)]
mod spoke_base_tests {
    use super::*;

    #[test]
    fn domain_profile_deserializes_base_block() {
        let yaml = r#"
description: "test"
base:
  model: qwen2_5_coder_7b
  method: qlora
  preset: qwen_4080_16g
"#;
        let p: DomainProfile = serde_yaml::from_str(yaml).expect("parse");
        let base = p.base.expect("base present");
        assert_eq!(base.model, "qwen2_5_coder_7b");
        assert_eq!(base.method, TrainMethod::Qlora);
        assert_eq!(base.preset.as_deref(), Some("qwen_4080_16g"));
    }

    #[test]
    fn base_is_optional_for_backward_compat() {
        let yaml = r#"description: "legacy profile, no base""#;
        let p: DomainProfile = serde_yaml::from_str(yaml).expect("parse");
        assert!(p.base.is_none());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-populi spoke_base_tests -- --nocapture`
Expected: FAIL — `no field 'base'` / `cannot find type 'TrainMethod'`.

- [ ] **Step 3: Add the `TrainMethod` enum and `SpokeBase` struct, and the `base` field**

In `domain_profiles.rs`, after the imports add:
```rust
/// Training method selected per spoke. Mirrors the methods our trainer can
/// dispatch; extend ONLY when the trainer gains a real backend (no stubs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainMethod {
    Qlora,
    FullSft,
    Dpo,
    Orpo,
    /// No fine-tune: spoke is served via retrieval/prompting only.
    RagOnly,
    PromptOnly,
}

/// Per-spoke base model + training method + hardware preset.
/// `model` and `preset` are validated against `model-registry.yaml` /
/// `gpu-specs.yaml` by `spoke_validate` (Phase 1.4) — a typo fails arch-check.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpokeBase {
    pub model: String,
    pub method: TrainMethod,
    #[serde(default)]
    pub preset: Option<String>,
}
```
Then add to `DomainProfile` (after `mix_config`):
```rust
    #[serde(default)]
    pub base: Option<SpokeBase>,
    #[serde(default)]
    pub eval_gate: Option<String>,
    #[serde(default)]
    pub router: Option<SpokeRouter>,
```
And add the router type:
```rust
/// Inference-time routing hints for this spoke (Phase 7 consumes these).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpokeRouter {
    /// Lane tags / glob triggers that route a request to this spoke.
    #[serde(default)]
    pub triggers: Vec<String>,
    /// Higher wins when multiple spokes match.
    #[serde(default)]
    pub priority: i32,
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-populi spoke_base_tests -- --nocapture`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
cargo fmt -p vox-populi
git add crates/vox-populi/src/mens/tensor/domain_profiles.rs
git commit -m "feat(mens): add SpokeBase/TrainMethod/SpokeRouter to DomainProfile SSOT"
```

### Task 1.2: Surface `base`/`eval_gate`/`router` on `EffectiveDomainProfile` [SEQUENTIAL]

**Files:**
- Modify: `crates/vox-populi/src/mens/tensor/domain_profiles.rs`
- Test: same file, inline.

- [ ] **Step 1: Write the failing test**

Add to `spoke_base_tests`:
```rust
    #[test]
    fn effective_profile_carries_base_through() {
        // load_domain_profile reads mens/config/domain-profiles.yaml from the
        // workspace root; this test runs from the crate dir, so point it up.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap()
            .to_path_buf();
        let eff = EffectiveDomainProfile::load_domain_profile("vox-lang", Some(&root))
            .expect("vox-lang profile loads");
        // vox-lang gains a base block in Task 1.5; until then this asserts the
        // field exists and is plumbed (None is acceptable pre-1.5).
        let _ = &eff.base;
        let _ = &eff.eval_gate;
        let _ = &eff.router;
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-populi effective_profile_carries_base_through`
Expected: FAIL — `no field 'base' on EffectiveDomainProfile`.

- [ ] **Step 3: Add the fields to `EffectiveDomainProfile` and populate them in `load_domain_profile`**

Add to the `EffectiveDomainProfile` struct (after `mix_config`):
```rust
    pub base: Option<SpokeBase>,
    pub eval_gate: Option<PathBuf>,
    pub router: Option<SpokeRouter>,
```
In the `Ok(EffectiveDomainProfile { ... })` constructor (after `mix_config:`):
```rust
            base: profile.base.clone(),
            eval_gate: profile.eval_gate.as_ref().map(|p| root.join(p)),
            router: profile.router.clone(),
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-populi effective_profile_carries_base_through`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cargo fmt -p vox-populi
git add crates/vox-populi/src/mens/tensor/domain_profiles.rs
git commit -m "feat(mens): plumb base/eval_gate/router onto EffectiveDomainProfile"
```

### Task 1.3: Add a `list_profiles` helper (validator needs to enumerate spokes) [SEQUENTIAL]

**Files:**
- Modify: `crates/vox-populi/src/mens/tensor/domain_profiles.rs`
- Test: same file, inline.

- [ ] **Step 1: Write the failing test**
```rust
    #[test]
    fn list_profiles_returns_known_spokes() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors().nth(2).unwrap().to_path_buf();
        let file = DomainProfilesFile::load(Some(&root)).expect("load file");
        assert!(file.profiles.contains_key("vox-lang"));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-populi list_profiles_returns_known_spokes`
Expected: FAIL — `no function 'load' on DomainProfilesFile`.

- [ ] **Step 3: Add `DomainProfilesFile::load`**

Refactor the file-read out of `load_domain_profile` into a reusable loader:
```rust
impl DomainProfilesFile {
    /// Read and parse mens/config/domain-profiles.yaml from the workspace root.
    pub fn load(workspace_root: Option<&Path>) -> anyhow::Result<Self> {
        let root = workspace_root.unwrap_or_else(|| Path::new("."));
        let profiles_path = root.join("mens/config/domain-profiles.yaml");
        let content = std::fs::read_to_string(&profiles_path)
            .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", profiles_path.display(), e))?;
        serde_yaml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse domain profiles: {}", e))
    }
}
```
Then make `load_domain_profile` call it (replace its inline read/parse with `let file = DomainProfilesFile::load(workspace_root)?;`).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-populi domain_profiles`
Expected: PASS (all prior tests still green — confirms the refactor didn't break `load_domain_profile`).

- [ ] **Step 5: Commit**

```bash
cargo fmt -p vox-populi
git add crates/vox-populi/src/mens/tensor/domain_profiles.rs
git commit -m "refactor(mens): extract DomainProfilesFile::load for reuse"
```

### Task 1.4: Spoke SSOT validator + arch-check wiring [PARALLEL-SAFE]

**Files:**
- Create: `crates/vox-populi/src/mens/tensor/spoke_validate.rs`
- Modify: `crates/vox-populi/src/mens/tensor/mod.rs` (add `pub mod spoke_validate;`)
- Test: in the new file, inline.

Context: `mod.rs` currently has `pub mod domain_profiles;` (verified at line 47). The validator is pure (operates on a parsed `DomainProfilesFile`), so it is disjoint from Wave 1.1's edits to `domain_profiles.rs` once those are merged — run this in Wave 1.2.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-populi/src/mens/tensor/spoke_validate.rs`:
```rust
//! Drift-proofing for the spoke SSOT (mens/config/domain-profiles.yaml).
//! Called by vox-arch-check so a malformed spoke fails CI, not training.

use crate::mens::tensor::domain_profiles::{DomainProfilesFile, TrainMethod};
use std::collections::BTreeSet;
use std::path::Path;

/// One human-readable validation problem.
#[derive(Debug, PartialEq, Eq)]
pub struct SpokeViolation(pub String);

/// Validate every spoke that declares a `base`. Returns all violations.
/// Rules: (1) a fine-tune method requires `mix_config`; (2) a fine-tune
/// method requires `base.preset`; (3) `eval_gate`, if set, must exist on disk;
/// (4) `mix_config`, if set, must exist on disk.
pub fn validate(file: &DomainProfilesFile, workspace_root: &Path) -> Vec<SpokeViolation> {
    let mut v = Vec::new();
    let fine_tune = |m: TrainMethod| {
        matches!(m, TrainMethod::Qlora | TrainMethod::FullSft | TrainMethod::Dpo | TrainMethod::Orpo)
    };
    for (name, p) in &file.profiles {
        let Some(base) = &p.base else { continue };
        if fine_tune(base.method) {
            if p.mix_config.is_none() {
                v.push(SpokeViolation(format!("spoke '{name}': fine-tune method requires mix_config")));
            }
            if base.preset.is_none() {
                v.push(SpokeViolation(format!("spoke '{name}': fine-tune method requires base.preset")));
            }
        }
        if let Some(mc) = &p.mix_config {
            if !workspace_root.join(mc).is_file() {
                v.push(SpokeViolation(format!("spoke '{name}': mix_config '{mc}' not found")));
            }
        }
        if let Some(eg) = &p.eval_gate {
            if !workspace_root.join(eg).is_file() {
                v.push(SpokeViolation(format!("spoke '{name}': eval_gate '{eg}' not found")));
            }
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mens::tensor::domain_profiles::DomainProfilesFile;

    #[test]
    fn flags_finetune_without_mix_config() {
        let yaml = r#"
profiles:
  broken:
    description: "x"
    base: { model: m, method: qlora }
"#;
        let file: DomainProfilesFile = serde_yaml::from_str(yaml).unwrap();
        let v = validate(&file, std::path::Path::new("/nonexistent"));
        assert!(v.iter().any(|x| x.0.contains("requires mix_config")), "got {v:?}");
    }

    #[test]
    fn rag_only_spoke_needs_no_mix() {
        let yaml = r#"
profiles:
  docs:
    description: "x"
    base: { model: m, method: rag_only }
"#;
        let file: DomainProfilesFile = serde_yaml::from_str(yaml).unwrap();
        let v = validate(&file, std::path::Path::new("/nonexistent"));
        assert!(!v.iter().any(|x| x.0.contains("requires mix_config")), "got {v:?}");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-populi spoke_validate`
Expected: FAIL — module not declared / unresolved import.

- [ ] **Step 3: Wire the module**

In `crates/vox-populi/src/mens/tensor/mod.rs`, after `pub mod domain_profiles;` add:
```rust
pub mod spoke_validate;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-populi spoke_validate`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
cargo fmt -p vox-populi
git add crates/vox-populi/src/mens/tensor/spoke_validate.rs crates/vox-populi/src/mens/tensor/mod.rs
git commit -m "feat(mens): spoke SSOT validator (fine-tune needs mix+preset; paths must exist)"
```

> **RESOLVED (layer check done 2026-06-18):** do **NOT** wire `validate()` into `vox-arch-check`. `vox-arch-check` is **layer 0** (`layers.toml:92`) and depends only on `cargo_metadata`/`toml`/`serde`; `vox-populi` is **layer 3** (`layers.toml:167`). A layer-0→layer-3 dependency is a hard layer violation. Instead expose validation through a new **`vox ci spoke-check`** subcommand in `vox-ml-cli` (layer 3, already deps `vox-populi`) — add it in this task: a thin command that calls `DomainProfilesFile::load(..)` → `spoke_validate::validate(..)` and exits non-zero on any violation. Add `vox ci spoke-check` to the pre-push/CI gate list so drift fails CI exactly as arch-check would.

### Task 1.5: Declare the three spokes in `domain-profiles.yaml` [PARALLEL-SAFE]

**Files:**
- Modify: `mens/config/domain-profiles.yaml`

Context: `vox-lang` profile already exists (verified) with `mix_config: mens/config/mix-vox-lang.yaml`. Add `base`/`eval_gate`/`router` to it and create `rust` and `agents` profiles. **Base `model` values are placeholders that Phase 3 replaces** with verified picks — use the registry keys Phase 3 will define.

- [ ] **Step 1: Add a `base`/`router` block to the existing `vox-lang` profile**

Under `profiles.vox-lang`, add:
```yaml
    base:
      model: small_code_default     # resolved in Phase 3 (model-registry.yaml)
      method: qlora
      preset: qwen_4080_16g
    eval_gate: mens/config/eval-gates.yaml
    router:
      triggers: ["lane:vox_codegen", "lane:vox_lang_tier_b", "*.vox"]
      priority: 10
```

- [ ] **Step 2: Add the `rust` spoke**
```yaml
  rust:
    description: "Idiomatic Rust authoring & review of our own workspace"
    mix_config: mens/config/mix-rust.yaml
    base:
      model: strong_code_default    # resolved in Phase 3
      method: qlora
      preset: qwen_4080_16g
    eval_gate: mens/config/eval-gates-rust.yaml
    router:
      triggers: ["lane:vox_rust_authoring", "lane:vox_rust_review", "*.rs"]
      priority: 10
    min_rating: 4
    seq_len: 2048
```

- [ ] **Step 3: Add the `agents` spoke**
```yaml
  agents:
    description: "Harness/agentic: tool calls, skills, discovery, operating vox.exe"
    mix_config: mens/config/mix-agents.yaml
    base:
      model: agentic_default        # resolved in Phase 3
      method: qlora
      preset: qwen_4080_16g
    eval_gate: mens/config/eval-gates-agents.yaml
    router:
      triggers: ["lane:vox_tooling", "lane:vox_dogfood_agent"]
      priority: 5
    min_rating: 3
    seq_len: 2048
```

- [ ] **Step 4: Verify it parses (no code; uses the loader test from 1.3)**

Run: `cargo test -p vox-populi list_profiles_returns_known_spokes`
Expected: PASS. Then run a parse smoke check:
```bash
cargo test -p vox-populi domain_profiles -- --nocapture
```
Expected: all green (the new profiles must deserialize).

> The validator (1.4) will report `eval_gate ... not found` for `eval-gates-rust.yaml`/`eval-gates-agents.yaml` until Phase 2 creates them. That is expected; **do not run the validator as a blocking gate until Phase 2 completes.** Record this ordering in the commit body.

- [ ] **Step 5: Commit**

```bash
git add mens/config/domain-profiles.yaml
git commit -m "feat(mens): declare vox-lang/rust/agents spokes in domain-profiles SSOT (bases TBD Phase 3)"
```

### Task 1.6: Make the pipeline mix strict (kill the silent-optional gap) [SEQUENTIAL]

**Files:**
- Modify: `crates/vox-ml-cli/src/commands/mens/pipeline.rs:266-284` (the `PipelineStage::Mix` arm)
- Test: `crates/vox-corpus/src/corpus/mix/tests.rs` (strict behavior already supported by `MixRunOptions.strict`, verified)

Context (verified): `run_mix_with_options(config_path, path_base, MixRunOptions{ strict, write_report })` already bails when a **required** (non-`optional`) source is missing under `strict: true`. The pipeline currently calls the lenient `CorpusAction::Mix` path. The fix: the agentic spoke's sources must NOT be silently optional once its corpus exists.

> **Prereq:** this task references the `profile: Option<String>` parameter on `pipeline::run`. If it is not yet threaded, do **Task 2.3 Step 1 (thread `profile`) FIRST**, then return here. (It is listed in Phase 2 only for narrative grouping; the param is a shared seam used by 1.6, 2.3, 3.4, and 6.1.)

- [ ] **Step 1: Write the failing test**

In `crates/vox-corpus/src/corpus/mix/tests.rs`, add:
```rust
#[test]
fn strict_mix_fails_on_missing_required_source() {
    use super::*;
    let dir = tempfile::tempdir().unwrap();
    let cfg = dir.path().join("m.yaml");
    std::fs::write(&cfg, "output: out.jsonl\nsources:\n  - path: does_not_exist.jsonl\n    weight: 1.0\n").unwrap();
    let res = run_mix_with_options(&cfg, Some(dir.path()), MixRunOptions { strict: true, write_report: false });
    assert!(res.is_err(), "strict mix must fail on missing required source");
}
```

- [ ] **Step 2: Run test to verify it fails or passes**

Run: `cargo test -p vox-corpus strict_mix_fails_on_missing_required_source`
Expected: This may already PASS (strict logic exists). If it PASSES, that confirms the mechanism — proceed to wire it. If it FAILS, the strict path regressed: fix `run_mix_with_options` to bail, then continue.

- [ ] **Step 3: Wire strict mix for spoke training in the pipeline**

The pipeline's `Mix` stage builds `train_mixed_*.jsonl`. Leave the broad `mens/config/mix.yaml` and the research mixes (`mix-research*.yaml`, `mix-rocks.yaml`) lenient — they aggregate many legitimately-optional lanes. Strictness must apply **only to the mix of the spoke actually being trained**, identified by the active `EffectiveDomainProfile.mix_config` (the `profile` parameter threaded in Task 2.3 Step 1), **not a filename heuristic** (a `"mix-"` prefix would wrongly force the research mixes strict). Concretely, in the `PipelineStage::Mix` arm, after resolving `mix_config`:
```rust
                        // Strict ONLY when this mix is the active spoke's declared mix.
                        let is_active_spoke_mix = if let Some(name) = profile.as_deref() {
                            let eff = vox_populi::mens::tensor::domain_profiles::EffectiveDomainProfile
                                ::load_domain_profile(name, ws.as_deref())?;
                            eff.mix_config.map(|spoke_mix| spoke_mix == mix_config).unwrap_or(false)
                        } else {
                            false
                        };
                        if is_active_spoke_mix {
                            vox_corpus::corpus::mix::run_mix_with_options(
                                &mix_config,
                                ws.as_deref(),
                                vox_corpus::corpus::mix::MixRunOptions { strict: true, write_report: true },
                            )?;
                        } else {
                            crate::commands::corpus::run(crate::commands::corpus::CorpusAction::Mix {
                                config: mix_config,
                                allow_missing_sources: true,
                            })
                            .await?;
                        }
```
**Verify the import path first:** `rg -n "pub use|pub mod mix" crates/vox-corpus/src/corpus/mod.rs` to confirm `vox_corpus::corpus::mix::run_mix_with_options` is the correct path; adjust to the real re-export.

- [ ] **Step 4: Run tests**

Run: `cargo test -p vox-corpus mix && cargo check -p vox-ml-cli`
Expected: PASS + compiles.

- [ ] **Step 5: Commit**

```bash
cargo fmt -p vox-corpus -p vox-ml-cli
git add crates/vox-ml-cli/src/commands/mens/pipeline.rs crates/vox-corpus/src/corpus/mix/tests.rs
git commit -m "feat(mens): strict mix for per-spoke training (no silent-optional corpus drop)"
```

---

## Phase 2 — Per-spoke eval gates AND the metrics that feed them

> **REVIEW FIX (critical):** `check_run` does **not** generically read arbitrary keys from a gate YAML — it has **bespoke per-gate-name handlers** (verify: `rg -n "supervised_ratio|truncation|pass_at_k|tokens_per_sec" crates/vox-ml-cli/src/commands/mens/eval_gate/`). A new gate name with no handler is **silently ignored**, and nothing currently **produces** Rust/agentic metrics into `eval_results.json`. Therefore this phase has **three layers**, not one: (a) metric **producers** that compute and write the metrics; (b) **`check_run` handlers** for the new gate names; (c) the gate **YAML thresholds**. Skipping (a) or (b) ships an inert gate.

**What & why:** You cannot promote a spoke you cannot measure. `eval-gates.yaml` is VoxScript-only (verified). Build the producer → handler → gate chain for Rust and agentic spokes.

**Wave 2.1 (`[PARALLEL-SAFE]` — disjoint files):** 2.1a (rust metric producer), 2.2a (agentic metric producer). **Wave 2.2 (`[SEQUENTIAL]` — both touch `check_run`/`policy`):** 2.1b → 2.2b (handlers). **Wave 2.3 (`[PARALLEL-SAFE]`):** 2.1c (rust gate yaml), 2.2c (agents gate yaml). **Wave 2.4 (`[SEQUENTIAL]`):** 2.3 (pipeline per-spoke gate selection).

### Task 2.0: Map the existing gate-handler pattern before adding to it [SEQUENTIAL]

**Files:** none (read-only — anti-hallucination for the whole phase).

- [ ] **Step 1:** Run:
```bash
rg -n "supervised_ratio|truncation|eval_local|pass_at_k|anti_stub|review_recurrence" crates/vox-ml-cli/src/commands/mens/eval_gate/check_run.rs
rg -n "fn load_policy|struct .*Policy|mcp_tool_schema" crates/vox-ml-cli/src/commands/mens/eval_gate/policy.rs
rg -n "eval_results.json|construct_coverage|vox_parse_rate" crates/vox-corpus/src crates/vox-ml-cli/src --type rust
```
Expected: shows (1) how each existing gate name is parsed from `eval_results.json`/manifest and turned into a `GateResult`; (2) the policy struct shape; (3) where `eval_results.json` is written today. **Inline the real producer + handler patterns into Tasks 2.1a/2.1b** rather than guessing. If `eval_results.json` is written by `CorpusAction::Eval`, that is where the new metric fields are added. STOP and write a handoff note if the gate-handler dispatch is structured differently than "one match arm per gate name."

### Task 2.1a: Rust eval metric producer [PARALLEL-SAFE]

**Files:**
- Create: `crates/vox-corpus/src/corpus/eval_rust_metrics.rs`
- Modify: `crates/vox-corpus/src/corpus/mod.rs`
- Test: inline.

Context: produce `rust_compile_rate` / `clippy_clean_rate` into the eval report. Reuse the **batched workspace-context compiler** from Task 4.2 (`vox_corpus::corpus::rust_authoring::compile_batch_in_workspace`) so metric and data verification share one implementation (DRY). Inputs are model-output `.rs` snippets from an eval set; output is the fraction that compile / pass clippy.

- [ ] **Step 1: Write the failing test** (pure ratio logic; the heavy compile is injected so the test stays fast):
```rust
//! Compute Rust spoke eval metrics from a batch of model outputs.

/// Fraction of `outputs` for which `verifier` returns true. Empty → 0.0.
pub fn pass_rate(outputs: &[String], verifier: impl Fn(&str) -> bool) -> f64 {
    if outputs.is_empty() { return 0.0; }
    let ok = outputs.iter().filter(|s| verifier(s)).count();
    ok as f64 / outputs.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rate_is_fraction_passing() {
        let outs = vec!["a".to_string(), "b".to_string(), "c".to_string(), "d".to_string()];
        let r = pass_rate(&outs, |s| s == "a" || s == "b");
        assert!((r - 0.5).abs() < 1e-9);
    }
    #[test]
    fn empty_is_zero() {
        assert_eq!(pass_rate(&[], |_| true), 0.0);
    }
}
```

- [ ] **Step 2: Declare module + run test** — add `pub mod eval_rust_metrics;` to `corpus/mod.rs`; `cargo test -p vox-corpus eval_rust_metrics` → FAIL then PASS.

- [ ] **Step 3: Add the report writer** — a function that runs `pass_rate` with the real `compile_batch_in_workspace` verifier (and a clippy variant) and merges `rust_compile_rate`/`clippy_clean_rate` into the existing `eval_results.json` (read–modify–write; **verify the exact path/struct** from Task 2.0 Step 1 before writing). Do not invent a new report file.

- [ ] **Step 4: Commit**
```bash
cargo fmt -p vox-corpus
git add crates/vox-corpus/src/corpus/eval_rust_metrics.rs crates/vox-corpus/src/corpus/mod.rs
git commit -m "feat(corpus): rust eval metric producer (compile_rate/clippy_clean_rate -> eval_results.json)"
```

### Task 2.1b: `check_run` handlers for Rust gate names [SEQUENTIAL]

**Files:**
- Modify: `crates/vox-ml-cli/src/commands/mens/eval_gate/check_run.rs`
- Modify: `crates/vox-ml-cli/src/commands/mens/eval_gate/policy.rs` (if the policy struct enumerates gate names — confirm in Task 2.0)
- Test: inline in `check_run.rs`.

Context: add match arms / parse logic for `rust_compile_rate` (min_pct, block) and `clippy_clean_rate`, reading them from `eval_results.json`, **following the exact pattern Task 2.0 surfaced** for `eval_local`/`supervised_ratio`. Do not assume the dispatch shape — mirror the real one.

- [ ] **Step 1: Write a failing test** that builds a temp `run_dir` with an `eval_results.json` containing `rust_compile_rate: 0.9`, points `check_run` at `eval-gates-rust.yaml`, and asserts a `GateResult { name: "rust_compile_rate", passed: true, .. }` appears. (Inline the real `eval_results.json` shape from Task 2.0.)
- [ ] **Step 2:** Run it → FAIL (handler missing).
- [ ] **Step 3:** Add the handler following the existing per-gate pattern.
- [ ] **Step 4:** Run it → PASS.
- [ ] **Step 5: Commit**
```bash
cargo fmt -p vox-ml-cli
git add crates/vox-ml-cli/src/commands/mens/eval_gate/
git commit -m "feat(mens): check_run handlers for rust_compile_rate/clippy_clean_rate gates"
```

### Task 2.2a: Agentic eval metric producer [PARALLEL-SAFE]

**Files:**
- Create: `crates/vox-corpus/src/corpus/eval_agentic_metrics.rs`
- Modify: `crates/vox-corpus/src/corpus/mod.rs`
- Test: inline.

Context: compute `tool_call_valid_json_rate` (model output parses as a tool-call JSON object with required keys) and `tool_name_exists_rate`. **RESOLVED (2026-06-18): an enumerable registry exists** — `vox_mcp_registry::TOOL_REGISTRY` / `TOOL_REGISTRY_SLIM` (auto-derived by `vox-corpus/build.rs`; verify with `rg -n "TOOL_REGISTRY" crates/vox-corpus/src/synthetic_gen/mod.rs crates/vox-mcp-registry/src`). So `tool_name_exists_rate` is a **hard (blocking) gate**: an emitted `tool_name` must be in that set. Inject the registry slice; do not fabricate a list.

- [ ] **Step 1: Write the failing test**
```rust
//! Agentic spoke eval metrics from model tool-call outputs.
use serde_json::Value;

/// True if `out` parses as a JSON object carrying the 4 tool-call keys.
pub fn is_valid_tool_call(out: &str) -> bool {
    let Ok(v) = serde_json::from_str::<Value>(out) else { return false };
    let Some(o) = v.as_object() else { return false };
    ["tool_name", "arguments", "result", "success"].iter().all(|k| o.contains_key(*k))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn valid_call_detected() {
        assert!(is_valid_tool_call(r#"{"tool_name":"ls","arguments":{},"result":1,"success":true}"#));
    }
    #[test] fn missing_key_rejected() {
        assert!(!is_valid_tool_call(r#"{"tool_name":"ls"}"#));
    }
    #[test] fn non_json_rejected() {
        assert!(!is_valid_tool_call("I will call ls"));
    }
}
```
- [ ] **Step 2: Declare module + run** — add `pub mod eval_agentic_metrics;`; `cargo test -p vox-corpus eval_agentic_metrics` → FAIL then PASS.
- [ ] **Step 3:** Add the report writer merging `tool_call_valid_json_rate` (+ `tool_name_exists_rate` if a registry exists) into `eval_results.json`.
- [ ] **Step 4: Commit**
```bash
cargo fmt -p vox-corpus
git add crates/vox-corpus/src/corpus/eval_agentic_metrics.rs crates/vox-corpus/src/corpus/mod.rs
git commit -m "feat(corpus): agentic eval metric producer (tool-call JSON validity)"
```

### Task 2.2b: `check_run` handlers for agentic gate names [SEQUENTIAL]

**Files:**
- Modify: `crates/vox-ml-cli/src/commands/mens/eval_gate/check_run.rs`
- Test: inline.

- [ ] **Step 1:** Failing test: `eval_results.json` with `tool_call_valid_json_rate: 0.85` + gate `eval-gates-agents.yaml` → expect a passing `GateResult`.
- [ ] **Step 2:** Run → FAIL.
- [ ] **Step 3:** Add handlers for BOTH `tool_call_valid_json_rate` and `tool_name_exists_rate` (the registry exists — both are blocking per `eval-gates-agents.yaml`), mirroring the Task 2.1b pattern.
- [ ] **Step 4:** Run → PASS.
- [ ] **Step 5: Commit**
```bash
cargo fmt -p vox-ml-cli
git add crates/vox-ml-cli/src/commands/mens/eval_gate/check_run.rs
git commit -m "feat(mens): check_run handler for tool_call_valid_json_rate gate"
```

### Task 2.1c: `eval-gates-rust.yaml` [PARALLEL-SAFE]

**Files:**
- Create: `mens/config/eval-gates-rust.yaml`

Context: mirror `eval-gates.yaml`'s structure (verified keys: `version`, then named gates each with thresholds + `block`). The gate names here MUST match the handlers added in Task 2.1b — `rust_compile_rate` measures compile/clippy, not vox parse.

- [ ] **Step 1: Write the gate file**
```yaml
version: "1"
# Per-spoke eval gate for the Rust authoring/review spoke.
# Source file map (written by the eval step, same convention as eval-gates.yaml):
#   eval_results.json → rust_compile_rate, clippy_clean_rate
#   review_metrics.json → review_recurrence (already produced by review pipeline)
rust_compile_rate:
  min_pct: 0.70
  block: true
clippy_clean_rate:
  min_pct: 0.50
  block: false
review_recurrence:
  max_pct: 0.20
  block: false
supervised_ratio:
  min_pct: 10.0
  block: true
```

- [ ] **Step 2: Verify the loader accepts it**

Run: `rg -n "supervised_ratio|min_pct|block" mens/config/eval-gates.yaml`
Expected: confirms the same keys exist in the reference gate (so `check_run`'s parser will accept this shape). No code change.

- [ ] **Step 3: Commit**
```bash
git add mens/config/eval-gates-rust.yaml
git commit -m "feat(mens): Rust spoke eval gate (compile-rate/clippy/review-recurrence)"
```

### Task 2.2c: `eval-gates-agents.yaml` [PARALLEL-SAFE]

**Files:**
- Create: `mens/config/eval-gates-agents.yaml`

Context: gate names MUST match the handlers from Task 2.2b.

- [ ] **Step 1: Write the gate file**
```yaml
version: "1"
# Per-spoke eval gate for the harness/agentic spoke.
# Source file map:
#   eval_results.json → tool_call_valid_json_rate, tool_name_exists_rate
tool_call_valid_json_rate:
  min_pct: 0.80
  block: true
tool_name_exists_rate:
  min_pct: 0.75
  block: true
supervised_ratio:
  min_pct: 10.0
  block: true
```

- [ ] **Step 2: Verify shape parity** — `rg -n "version|block" mens/config/eval-gates.yaml`. Expected: same keys present.

- [ ] **Step 3: Commit**
```bash
git add mens/config/eval-gates-agents.yaml
git commit -m "feat(mens): agentic spoke eval gate (tool-call JSON validity / tool-name existence)"
```

### Task 2.3: Pipeline selects the spoke's eval gate [SEQUENTIAL]

**Files:**
- Modify: `crates/vox-ml-cli/src/commands/mens/pipeline.rs` (the `PipelineStage::Eval` arm, lines ~256-265)

Context: the `Eval` stage currently runs corpus eval to `eval_results.json`. The per-spoke *gate check* runs via `vox mens eval-gate` (`check_run`). This task ensures the gate path is resolved from the active spoke's `EffectiveDomainProfile.eval_gate` (Task 1.2) rather than hard-coded.

- [ ] **Step 1: Verify the gate-check entrypoint and how `pipeline.rs` learns the spoke name**

Run:
```bash
rg -n "eval_gate|EffectiveDomainProfile|domain_profile" crates/vox-ml-cli/src/commands/mens/pipeline.rs
rg -n "pub async fn run" crates/vox-ml-cli/src/commands/mens/pipeline.rs
```
Expected: shows `pipeline::run`'s signature. If it has no spoke/profile parameter, **add one** (`profile: Option<String>`) threaded from the caller — this is the seam the router and per-spoke training both need.

- [ ] **Step 2: Write the failing test**

Add a unit test next to the pipeline (or in `crates/vox-populi`) asserting gate resolution:
```rust
#[test]
fn rust_spoke_resolves_its_eval_gate() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors().nth(2).unwrap().to_path_buf();
    let eff = vox_populi::mens::tensor::domain_profiles::EffectiveDomainProfile
        ::load_domain_profile("rust", Some(&root)).unwrap();
    let gate = eff.eval_gate.expect("rust spoke has an eval gate");
    assert!(gate.ends_with("eval-gates-rust.yaml"));
}
```
Place this in a test module of the crate that already depends on `vox-populi` (e.g. inline in `pipeline.rs` under `#[cfg(test)]`).

- [ ] **Step 3: Run test to verify it fails then passes**

Run: `cargo test -p vox-ml-cli rust_spoke_resolves_its_eval_gate`
Expected: After Phase 1.5 + 2.1 landed, this should PASS immediately (it asserts wiring, not new code). If it FAILS with "not found", the profile/file is missing — fix the path, don't invent a gate.

- [ ] **Step 4: Use the resolved gate in the Eval stage**

In the `Eval` arm, after `eval_results.json` is produced, resolve the active profile's gate and run `check_run`:
```rust
                if let Some(name) = profile.as_deref() {
                    if let Ok(eff) = vox_populi::mens::tensor::domain_profiles::EffectiveDomainProfile
                        ::load_domain_profile(name, ws.as_deref())
                    {
                        if let Some(gate) = eff.eval_gate.as_deref() {
                            let results = crate::commands::mens::eval_gate::check_run::check_run(
                                &output_dir, gate,
                            )?;
                            tracing::info!(?results, "spoke eval gate results");
                        }
                    }
                }
```
**Verify the `check_run` import path** first: `rg -n "mod eval_gate|pub mod check_run|pub use" crates/vox-ml-cli/src/commands/mens/mod.rs`.

- [ ] **Step 5: Commit**
```bash
cargo fmt -p vox-ml-cli
git add crates/vox-ml-cli/src/commands/mens/pipeline.rs
git commit -m "feat(mens): pipeline resolves and runs the active spoke's eval gate"
```

---

## Phase 3 — Model selection SSOT, scaled to available VRAM

**What & why:** Decouple base-model choice from code, document the ideal model per spoke, and scale it to the host GPU. `gpu-specs.yaml` already lists VRAM per GPU (verified: `rtx 4080 super` → 16384 MB). The registry maps a *role key* (used in `domain-profiles.yaml`) → a concrete model + its VRAM floor + method compatibility, and a resolver picks the largest model that fits the detected GPU.

**Wave 3.1 (`[SEQUENTIAL]`):** 3.1 (research/decision) → 3.2 (registry file) → 3.3 (resolver) → 3.4 (wire profiles).

### Task 3.1: Live re-research and record the model decision [SEQUENTIAL]

**Files:**
- Create: `docs/src/architecture/voxmens-model-selection-decision-2026-06-18.md` (needs YAML frontmatter — `docs/src/`)

Context: the research report's model-selection axis was rate-limited (unverified). This task closes it with a small, current search and a committed decision — *not* speculation in code.

- [ ] **Step 1: Use the `brainstorming` micro-skill to frame the decision** (one sentence + 2-3 options per spoke). Do not invent models; if a search tool is unavailable in Antigravity, mark the pick as "PROVISIONAL — verify before training" in the doc and proceed with the most-cited candidate from the research report's source list.

- [ ] **Step 2: Write the decision doc with frontmatter**
```markdown
---
title: "VoxMens Per-Spoke Base Model Selection — Decision"
description: "Verified base-model picks per spoke (VoxScript/Rust/agentic), VRAM floors, and method compatibility, scaled to the RTX 4080 SUPER (16GB) host. Resolves the unverified model-selection axis of the hub-and-spoke research."
category: "Architecture"
status: "roadmap"
training_eligible: false
---

# Per-Spoke Base Model Decision (2026-06-18)

| Role key | Spoke | Chosen model | Params | QLoRA VRAM floor (MB) | Method | Why |
|---|---|---|---|---|---|---|
| small_code_default | VoxScript | <pick> | ~1.5-3B | <measured/estimated> | qlora | DSL, latency-first |
| strong_code_default | Rust | <pick> | ~7B | <measured/estimated> | qlora | hardest code spoke |
| agentic_default | Harness | <pick> | ~7B tool-tuned | <measured/estimated> | qlora | tool-use lineage |

Candidate evidence (from the research report's unverified source set — re-confirm live before locking): Qwen2.5-Coder, DeepSeek-Coder-V2, Codestral, StarCoder2, Granite-Code. Benchmarks to check per spoke: HumanEval/MBPP + Rust-specific for Rust; BFCL/agentic for harness.
```
Fill `<pick>` and VRAM floors from the search; if provisional, say so explicitly.

- [ ] **Step 3: Commit**
```bash
git add docs/src/architecture/voxmens-model-selection-decision-2026-06-18.md
git commit -m "docs(mens): per-spoke base model selection decision (VRAM-scaled)"
```

### Task 3.2: `model-registry.yaml` SSOT [SEQUENTIAL]

**Files:**
- Create: `mens/config/model-registry.yaml`

- [ ] **Step 1: Write the registry**
```yaml
# SSOT for base models a spoke may select. Role keys are referenced by
# domain-profiles.yaml (base.model). The resolver (Task 3.3) picks the
# largest variant whose vram_floor_mb <= detected GPU VRAM (gpu-specs.yaml).
# Add a model here — no Rust changes — and reference its role key in a spoke.
roles:
  small_code_default:
    description: "Latency-first small code model for the VoxScript DSL spoke"
    variants:
      - model_id: <hf-id-from-decision-doc>
        vram_floor_mb: 6000
        methods: [qlora, full_sft]
  strong_code_default:
    description: "Strong code model for the Rust authoring/review spoke"
    variants:
      - model_id: <hf-id-from-decision-doc>
        vram_floor_mb: 11000
        methods: [qlora, dpo, orpo]
  agentic_default:
    description: "Tool-use/agentic model for the harness spoke"
    variants:
      - model_id: <hf-id-from-decision-doc>
        vram_floor_mb: 11000
        methods: [qlora, dpo]
```
Replace `<hf-id-from-decision-doc>` with the Task 3.1 picks.

- [ ] **Step 2: Commit**
```bash
git add mens/config/model-registry.yaml
git commit -m "feat(mens): model-registry SSOT (role -> variants with VRAM floor + methods)"
```

### Task 3.3: VRAM-scaled resolver [SEQUENTIAL]

**Files:**
- Create: `crates/vox-populi/src/mens/tensor/model_registry.rs`
- Modify: `crates/vox-populi/src/mens/tensor/mod.rs` (add `pub mod model_registry;`)
- Test: inline.

- [ ] **Step 1: Write the failing test**
```rust
//! Resolve a spoke's role key -> concrete model_id, scaled to detected VRAM.
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Variant { pub model_id: String, pub vram_floor_mb: u32, #[serde(default)] pub methods: Vec<String> }
#[derive(Debug, Deserialize)]
pub struct Role { #[serde(default)] pub variants: Vec<Variant> }
#[derive(Debug, Deserialize)]
pub struct ModelRegistry { pub roles: HashMap<String, Role> }

impl ModelRegistry {
    pub fn load(workspace_root: &Path) -> anyhow::Result<Self> {
        let p = workspace_root.join("mens/config/model-registry.yaml");
        let s = std::fs::read_to_string(&p)
            .map_err(|e| anyhow::anyhow!("read {}: {e}", p.display()))?;
        serde_yaml::from_str(&s).map_err(|e| anyhow::anyhow!("parse model-registry: {e}"))
    }
    /// Largest variant whose floor fits `available_vram_mb` and that supports `training_method`.
    /// Errors if no variant satisfies both constraints.
    pub fn resolve(&self, role: &str, available_vram_mb: u32, training_method: &str) -> anyhow::Result<&Variant> {
        let r = self.roles.get(role).ok_or_else(|| anyhow::anyhow!("unknown role '{role}'"))?;
        r.variants.iter()
            .filter(|v| v.vram_floor_mb <= available_vram_mb && v.methods.iter().any(|m| m == training_method))
            .max_by_key(|v| v.vram_floor_mb)
            .ok_or_else(|| anyhow::anyhow!("no variant of '{role}' fits {available_vram_mb}MB with method '{training_method}'"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn reg() -> ModelRegistry {
        serde_yaml::from_str(r#"
roles:
  r:
    variants:
      - { model_id: small, vram_floor_mb: 6000, methods: [qlora] }
      - { model_id: big, vram_floor_mb: 11000, methods: [qlora] }
"#).unwrap()
    }
    #[test]
    fn picks_largest_that_fits() {
        assert_eq!(reg().resolve("r", 16384, "qlora").unwrap().model_id, "big");
        assert_eq!(reg().resolve("r", 8000, "qlora").unwrap().model_id, "small");
    }
    #[test]
    fn errors_when_none_fit() {
        assert!(reg().resolve("r", 4000, "qlora").is_err());
    }
    #[test]
    fn errors_when_method_unsupported() {
        assert!(reg().resolve("r", 16384, "dpo").is_err());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-populi model_registry`
Expected: FAIL — module not declared.

- [ ] **Step 3: Declare the module** — add `pub mod model_registry;` to `mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-populi model_registry`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**
```bash
cargo fmt -p vox-populi
git add crates/vox-populi/src/mens/tensor/model_registry.rs crates/vox-populi/src/mens/tensor/mod.rs
git commit -m "feat(mens): VRAM-scaled model resolver (largest variant that fits)"
```

### Task 3.4: Replace role placeholders → resolved model at train time [SEQUENTIAL]

**Files:**
- Modify: `crates/vox-ml-cli/src/commands/mens/pipeline.rs` (the `Train` arm)

Context (verified): the `Train` arm calls `run_train(... model, ... preset ...)`. Today `target_model = model.clone()` (a CLI override). Change it to resolve from the profile's `base.model` role key through the registry, using the detected GPU VRAM from `gpu-specs.yaml`.

- [ ] **Step 1: Verify the VRAM-detection helper exists**

Run: `rg -n "vram_mb|detect.*gpu|available.*vram|gpu-specs" crates/vox-populi/src crates/vox-ml-cli/src --type rust | head`
Expected: find the existing GPU/VRAM detection (`gpu-specs.yaml` is "loaded at runtime by TimeEstimator" per its header). Use that helper. If none is reachable from `pipeline.rs`, **do not invent one** — fall back to reading the active preset's GPU from `gpu-specs.yaml` and STOP to flag the gap if even that is unavailable.

- [ ] **Step 2: Resolve the model before the `run_train` call**

In the `Train` arm, before constructing `target_model`:
```rust
                        let resolved_model = if let Some(name) = profile.as_deref() {
                            let eff = vox_populi::mens::tensor::domain_profiles::EffectiveDomainProfile
                                ::load_domain_profile(name, ws.as_deref())?;
                            match eff.base {
                                Some(b) => {
                                    let reg = vox_populi::mens::tensor::model_registry::ModelRegistry
                                        ::load(ws.as_deref().unwrap_or(std::path::Path::new(".")))?;
                                    // FAIL CLOSED: if VRAM can't be detected, error — never
                                    // assume a large GPU (that picks an OOM-bound model).
                                    let vram = detect_available_vram_mb()
                                        .ok_or_else(|| anyhow::anyhow!(
                                            "cannot detect GPU VRAM; pass --model to override or fix gpu-specs detection"
                                        ))?;
                                    Some(reg.resolve(&b.model, vram, b.method.as_str())?.model_id.clone())
                                }
                                None => None,
                            }
                        } else { None };
                        let target_model = model.clone().or(resolved_model);
```
Replace `detect_available_vram_mb()` with the **real** helper found in Step 1 (returns `Option<u32>`). **Do not** substitute a literal fallback — failing closed is the point: a wrong-direction guess OOMs at train time. **CLI `--model` still wins** (`model.clone().or(...)`) so manual overrides bypass detection entirely.

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p vox-ml-cli`
Expected: compiles. (Training itself needs `--features gpu`; do not attempt a real train run in this task — that's Phase 8 validation.)

- [ ] **Step 4: Commit**
```bash
cargo fmt -p vox-ml-cli
git add crates/vox-ml-cli/src/commands/mens/pipeline.rs
git commit -m "feat(mens): resolve per-spoke base model from registry, scaled to VRAM (CLI override wins)"
```

### Task 3.5: Extend the spoke validator with cross-registry checks [SEQUENTIAL]

> **REVIEW FIX (design):** Phase 1.4's validator only checks intra-profile rules. The plan's promise — "a typo in `base.model`/`preset` fails arch-check" — is unmet until the validator cross-checks the registries created in this phase. Do it here, after `model-registry.yaml` exists.

**Files:**
- Modify: `crates/vox-populi/src/mens/tensor/spoke_validate.rs`
- Test: inline.

- [ ] **Step 1: Write the failing test**
```rust
    #[test]
    fn flags_unknown_model_role_and_method() {
        // registry knows role "good" supporting only qlora; spoke references an
        // unknown role and an unsupported method.
        let reg: crate::mens::tensor::model_registry::ModelRegistry = serde_yaml::from_str(
            "roles:\n  good:\n    variants:\n      - { model_id: m, vram_floor_mb: 6000, methods: [qlora] }\n").unwrap();
        let file: DomainProfilesFile = serde_yaml::from_str(
            "profiles:\n  s:\n    description: x\n    mix_config: mens/config/mix.yaml\n    base: { model: typo_role, method: dpo, preset: p }\n").unwrap();
        let v = validate_with_registry(&file, &reg, std::path::Path::new("/nonexistent"));
        assert!(v.iter().any(|x| x.0.contains("unknown model role")), "got {v:?}");
    }
```

- [ ] **Step 2: Run → FAIL** (`validate_with_registry` undefined).

- [ ] **Step 3: Add `validate_with_registry`** that calls the existing `validate` then adds, per spoke with a `base`: (a) `base.model` must be a key in `registry.roles`; (b) `base.method` must appear in at least one matching variant's `methods`; (c) `base.preset`, if set, should be a known preset (load `gpu-specs.yaml` presets — verify the key path with `rg -n "presets|preset" mens/config/gpu-specs.yaml`). Return combined violations. Keep the old `validate` for callers that don't have a registry.

- [ ] **Step 4: Run → PASS.**

- [ ] **Step 5: Commit**
```bash
cargo fmt -p vox-populi
git add crates/vox-populi/src/mens/tensor/spoke_validate.rs
git commit -m "feat(mens): validator cross-checks base.model/method/preset against registries"
```

---

## Phase 4 — Rust authoring spoke corpus (fill the half-built spoke)

**What & why:** `mix-rust.yaml` teaches Rust→Vox *translation*, not authorship. Add `(instruction → idiomatic Rust)` pairs verified by `cargo check`, mirroring the round-trip verification already used in `run_mutate`/`run_rust_mine` (verified pattern).

**Wave 4.1 (`[SEQUENTIAL]`):** 4.1 → 4.2 → 4.3.

### Task 4.1: `rust_authoring` pair generator with compile verification [SEQUENTIAL]

**Files:**
- Create: `crates/vox-corpus/src/corpus/rust_authoring.rs`
- Modify: `crates/vox-corpus/src/corpus/mod.rs` (declare module)
- Test: inline.

Context (verified): `run_rust_mine` already reads `.rs`, extracts via `vox_corpus::rust_to_vox::extract_translations`, and verifies output. The authoring generator instead pairs a mined function with a synthesized instruction and tags lane `vox_rust_authoring`. Compile verification uses a temp crate + `cargo check`.

- [ ] **Step 1: Write the failing test** (pure pairing logic; the `cargo check` verifier is tested separately to keep this fast):
```rust
//! Generate (instruction -> idiomatic Rust) SFT pairs from workspace .rs files.
use serde_json::json;

/// Build one SFT pair JSON from a function name + its source. Lane: vox_rust_authoring.
pub fn make_authoring_pair(fn_name: &str, rust_src: &str) -> serde_json::Value {
    let instruction = format!("Write an idiomatic Rust function named `{fn_name}`.");
    json!({
        "prompt": instruction,
        "response": format!("```rust\n{rust_src}\n```"),
        "messages": [
            {"role": "user", "content": instruction},
            {"role": "assistant", "content": format!("```rust\n{rust_src}\n```")}
        ],
        "category": "rust_authoring",
        "lane": "vox_rust_authoring",
        "origin": "human",
        "response_mode": "code_only",
        "task_family": "rust_authoring"
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pair_has_lane_and_assistant_turn() {
        let p = make_authoring_pair("add", "fn add(a: i32, b: i32) -> i32 { a + b }");
        assert_eq!(p["lane"], "vox_rust_authoring");
        let msgs = p["messages"].as_array().unwrap();
        assert_eq!(msgs.last().unwrap()["role"], "assistant");
    }
}
```

- [ ] **Step 2: Run test to verify it fails** — `cargo test -p vox-corpus rust_authoring` → FAIL (module undeclared).

- [ ] **Step 3: Declare module** — add `pub mod rust_authoring;` to `crates/vox-corpus/src/corpus/mod.rs` (verify exact location: `rg -n "pub mod" crates/vox-corpus/src/corpus/mod.rs`).

- [ ] **Step 4: Run test to verify it passes** — `cargo test -p vox-corpus rust_authoring` → PASS.

- [ ] **Step 5: Commit**
```bash
cargo fmt -p vox-corpus
git add crates/vox-corpus/src/corpus/rust_authoring.rs crates/vox-corpus/src/corpus/mod.rs
git commit -m "feat(corpus): rust_authoring SFT pair builder (lane vox_rust_authoring)"
```

### Task 4.2: Batched, workspace-context compile verifier [SEQUENTIAL]

> **REVIEW FIX (critical):** the naive design — one `cargo check` per snippet in a throwaway crate — is (a) **O(n) cargo cold-starts** (hours for a real corpus) and (b) **wrong**: workspace functions reference workspace crates/types, so compiled in isolation they fail → mass false-negatives that reject valid data. Fix: compile a **batch** of snippets as sibling modules inside **one throwaway member of the real workspace** (so `vox-*` deps resolve), sharing the workspace `target/` dir (warm incremental cache). One `cargo check` amortizes over N snippets; per-snippet pass/fail comes from parsing the diagnostics' file/module spans.

**Files:**
- Modify: `crates/vox-corpus/src/corpus/rust_authoring.rs`
- Test: inline (`#[ignore]` the cargo-spawning integration test; keep a fast unit test for the batching/diagnostic-parsing logic).

- [ ] **Step 1: Verify the spawn helper + workspace layout first**
```bash
rg -n "CREATE_NO_WINDOW|quiet_command" crates/vox-corpus/src crates/*/src | head
rg -n "^\[workspace\]|members" Cargo.toml | head
```
Expected: confirm (a) the no-flashing-window spawn helper to reuse (`feedback_no_console_windows_on_spawn` — child spawns MUST set `CREATE_NO_WINDOW` on Windows), and (b) the workspace members list, so the throwaway crate can be added as a member that depends on the crates the snippets need. If no `quiet_command` helper exists, add the `#[cfg(windows)]` `CREATE_NO_WINDOW` flag inline.

- [ ] **Step 2: Write the failing unit test for batch result mapping** (pure logic — no cargo):
```rust
/// Map a batch of `cargo check` JSON diagnostics back to per-snippet pass/fail.
/// `n` snippets were emitted as modules `snippet_0..snippet_{n-1}`; a snippet
/// fails iff any error diagnostic's span path contains its module name.
pub fn batch_pass_flags(n: usize, error_modules: &[String]) -> Vec<bool> {
    (0..n)
        .map(|i| {
            !error_modules.iter().any(|m| m == &format!("snippet_{}", i))
        })
        .collect()
}

#[cfg(test)]
mod batch_tests {
    use super::*;
    #[test] fn flags_only_failing_modules() {
        let flags = batch_pass_flags(3, &["snippet_1".to_string()]);
        assert_eq!(flags, vec![true, false, true]);
    }
    #[test] fn empty_errors_all_pass() {
        assert_eq!(batch_pass_flags(2, &[]), vec![true, true]);
    }
}
```

- [ ] **Step 3: Run the unit test** — `cargo test -p vox-corpus batch_tests` → FAIL then PASS after adding `batch_pass_flags`.

- [ ] **Step 4: Add `compile_batch_in_workspace` (the heavy path) + an ignored integration test**
```rust
/// Compile `snippets` together as modules in a throwaway workspace member,
/// reusing the workspace target dir. Returns one pass-flag per snippet.
/// Wraps each snippet in `mod snippet_i { ... }` and parses `cargo check
/// --message-format=json` diagnostics via `batch_pass_flags`.
/// Spawns with the no-flashing-window helper on Windows.
pub fn compile_batch_in_workspace(workspace_root: &std::path::Path, snippets: &[String]) -> Vec<bool> {
    // Implementation: create crates/_corpus_verify_tmp (a workspace member with
    // the deps the snippets need), write src/lib.rs containing
    // `pub mod snippet_0 { <snip> } pub mod snippet_1 { ... }`, run
    // `cargo check -p _corpus_verify_tmp --message-format=json` from
    // workspace_root, collect error spans, call batch_pass_flags. Use the
    // verified quiet-spawn helper. Remove the tmp member afterward.
    // (Fill in using the exact spawn helper + workspace path from Step 1.)
    let _ = (workspace_root, snippets);
    unimplemented!("wire to verified spawn helper from Step 1")
}

#[cfg(test)]
mod verify_tests {
    use super::*;
    #[test] #[ignore] // requires cargo + workspace; run locally with --ignored
    fn batch_accepts_valid_rejects_invalid() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).unwrap();
        let flags = compile_batch_in_workspace(root, &[
            "pub fn add(a: i32, b: i32) -> i32 { a + b }".to_string(),
            "pub fn broken() -> i32 { return }".to_string(),
        ]);
        assert_eq!(flags, vec![true, false]);
    }
}
```
> The `unimplemented!()` body is a **plan marker**, not shippable code — Step 4's commit is blocked until it is filled with the real spawn + diagnostic-parse from Step 1 (no-stubs policy). Do not commit `unimplemented!()`.

- [ ] **Step 5: Run the fast tests, fill the body, verify** — `cargo test -p vox-corpus rust_authoring` (unit tests pass; ignored test skipped). Fill `compile_batch_in_workspace`, then locally: `cargo test -p vox-corpus -- --ignored batch_accepts_valid_rejects_invalid`.

- [ ] **Step 6: Commit** (only after the body is real)
```bash
cargo fmt -p vox-corpus
git add crates/vox-corpus/src/corpus/rust_authoring.rs
git commit -m "feat(corpus): batched workspace-context compile verifier (amortized cargo, no false-negatives)"
```

### Task 4.3: Add the authoring source to `mix-rust.yaml` [SEQUENTIAL]

**Files:**
- Modify: `mens/config/mix-rust.yaml`
- Modify: `mens/config/mix-rust.yaml` lane allow-list (add `vox_rust_authoring`)

- [ ] **Step 1: Add the lane and a source**

Add to `mix-rust.yaml`:
```yaml
include_lanes: [vox_rust_expert_cross, vox_rust_authoring, vox_rust_review]
```
and a source entry (the generator's output path — wire the CLI `CorpusAction` that calls `make_authoring_pair` over `crates/`; if no such action exists, add `vox corpus rust-author` mirroring `run_rust_mine`'s `RustMine` action — verify with `rg -n "RustMine|ExtractRs|rust_mine" crates/vox-ml-cli/src`):
```yaml
  - path: target/dogfood/rust_authoring.validated.jsonl
    weight: 3.0
    optional: false   # strict: this spoke needs real authoring data
```

- [ ] **Step 2: Verify the mix still loads** — `cargo test -p vox-corpus mix`. Expected: PASS.

- [ ] **Step 3: Commit**
```bash
git add mens/config/mix-rust.yaml
git commit -m "feat(mens): wire rust_authoring lane into mix-rust (strict source)"
```

---

## Phase 5 — Harness/agentic spoke corpus (build from zero)

**What & why:** The agentic corpus does not exist. Build it two ways (user-chosen "both"): synthesize from skill/CLI/discovery surfaces (deterministic, no infra) **and** ingest real agent traces. `vox-corpus` already has a `tool_trace` record format + `mens/schemas/tool_trace_record.schema.json` (verified in `mix/mod.rs`) — reuse it.

**Wave 5.1 (`[PARALLEL-SAFE]` — disjoint files):** 5.1 (agentic_synth), 5.2 (trace schema). **Wave 5.2 (`[SEQUENTIAL]`):** 5.3 (trace_ingest, depends on schema) → 5.4 (wire mix-agents).

### Task 5.1: Synthesize tool-use pairs from skill/CLI surfaces [PARALLEL-SAFE]

**Files:**
- Create: `crates/vox-corpus/src/corpus/agentic_synth.rs`
- Modify: `crates/vox-corpus/src/corpus/mod.rs`
- Test: inline.

Context: emit rows in the existing `tool_trace`-compatible shape so `normalize_training_jsonl_line(.., Some("tool_trace"))` consumes them. Reuse `ToolTraceRecord` (verified — exactly 7 fields: `task_prompt`, `tool_name`, `arguments_json`, `result_json`, `success`, `followup_text`, `session_id`).

> **REVIEW FIX + REUSE (2026-06-18):** the spoke must learn the **Vox harness surface** — **not** Claude Code's `Skill` tool. Crucially, **synthesis machinery already exists**: `vox-corpus` has `generate_tool_pairs(&mut buf, TOOL_REGISTRY_SLIM, cfg)` plus `ORCHESTRATOR_TOOLS` / `SKILL_TOOLS` constants (auto-derived from `vox_mcp_registry::TOOL_REGISTRY` by `build.rs`). **Prefer extending/reusing that over writing new synthesis.** This task adds only what's missing (skill/CLI-discovery pairs not already covered), tagged lane `vox_tooling`/`vox_dogfood_agent`.

- [ ] **Step 1: Inventory what synthesis already exists, then fill only the gap** — `rg -n "generate_tool_pairs|TOOL_REGISTRY_SLIM|ORCHESTRATOR_TOOLS|SKILL_TOOLS" crates/vox-corpus/src/synthetic_gen`. If `generate_tool_pairs` already covers MCP tools, your new code should (a) route its output into `mix-agents.yaml`'s sources, and (b) add only skill-invocation / CLI-discovery pairs it omits. Build candidate `(command, args)` from `TOOL_REGISTRY_SLIM` + the skill registry (`vox_skills::SkillRegistry`); do not invent commands.

- [ ] **Step 2: Write the failing test**
```rust
//! Synthesize tool-use SFT rows from the real Vox CLI / skill surface.
use crate::tool_workflow_corpus::ToolTraceRecord;

/// One synthetic supervised tool call over a REAL Vox CLI command (e.g.
/// "vox ci affected-crates"). `command` MUST be a command verified to exist in
/// Step 1; `args` is the JSON arguments object.
pub fn synth_vox_command(task: &str, command: &str, args: serde_json::Value) -> ToolTraceRecord {
    ToolTraceRecord {
        task_prompt: task.to_string(),
        tool_name: command.to_string(),
        arguments_json: args.to_string(),
        result_json: serde_json::json!({ "status": "ok" }).to_string(),
        success: true,
        followup_text: None,
        session_id: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn synth_targets_real_vox_command() {
        let r = synth_vox_command(
            "Find which crates a change affects",
            "vox ci affected-crates",
            serde_json::json!({ "base": "origin/main" }),
        );
        assert_eq!(r.tool_name, "vox ci affected-crates");
        assert!(r.arguments_json.contains("origin/main"));
    }
}
```

- [ ] **Step 3: Declare module + run test** — add `pub mod agentic_synth;` to `corpus/mod.rs`; `cargo test -p vox-corpus agentic_synth` → PASS.

- [ ] **Step 4: Commit**
```bash
cargo fmt -p vox-corpus
git add crates/vox-corpus/src/corpus/agentic_synth.rs crates/vox-corpus/src/corpus/mod.rs
git commit -m "feat(corpus): synthesize tool-use SFT rows from skill/CLI surfaces"
```

### Task 5.2: Agent-trace JSON schema [PARALLEL-SAFE]

**Files:**
- Create: `mens/schemas/agent_trace_record.schema.json`

Context: mirror `mens/schemas/tool_trace_record.schema.json` (verified to exist) but capture multi-turn tool sequences.

- [ ] **Step 1: Verify the sibling schema's shape** — `rg -n "properties|required" mens/schemas/tool_trace_record.schema.json | head`.

- [ ] **Step 2: Write the schema**
```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "AgentTraceRecord",
  "type": "object",
  "required": ["intent", "steps"],
  "properties": {
    "intent": { "type": "string" },
    "steps": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["tool_name", "arguments", "result", "success"],
        "properties": {
          "tool_name": { "type": "string" },
          "arguments": { "type": "object" },
          "result": {},
          "success": { "type": "boolean" }
        }
      }
    },
    "outcome": { "type": "string", "enum": ["success", "failure", "partial"] }
  }
}
```

- [ ] **Step 3: Commit**
```bash
git add mens/schemas/agent_trace_record.schema.json
git commit -m "feat(mens): agent_trace_record JSON schema (multi-turn tool sequences)"
```

### Task 5.3: Trace → SFT/DPO converter with diversity gate [SEQUENTIAL]

**Files:**
- Create: `crates/vox-corpus/src/corpus/trace_ingest.rs`
- Modify: `crates/vox-corpus/src/corpus/mod.rs`
- Test: inline.

Context: convert an `AgentTraceRecord` (per Task 5.2 schema) into `prompt`/`response` SFT rows tagged lane `vox_dogfood_agent`; reuse the existing diversity guard (`vox_eval::eval_semantic_entropy`, verified used in `run_diversity_check`) to drop monoculture before writing.

- [ ] **Step 1: Write the failing test**
```rust
//! Convert captured agent traces (agent_trace_record schema) into SFT rows.
use serde_json::{json, Value};

/// Convert one trace JSON into an SFT row (lane vox_dogfood_agent). Returns
/// None if the trace has no steps (nothing to learn).
pub fn trace_to_sft(trace: &Value) -> Option<Value> {
    let intent = trace.get("intent")?.as_str()?;
    let steps = trace.get("steps")?.as_array()?;
    if steps.is_empty() { return None; }
    let prompt = format!("[vox_agent]\nIntent: {intent}\nEmit the tool-call sequence as JSON.");
    let response = serde_json::to_string(steps).ok()?; // computed once (DRY)
    Some(json!({
        "prompt": prompt,
        "response": response,
        "messages": [
            {"role": "user", "content": prompt},
            {"role": "assistant", "content": response}
        ],
        "category": "agent_trace",
        "lane": "vox_dogfood_agent",
        "origin": "agent",
        "response_mode": "code_only",
        "task_family": "agent_trace"
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_steps_yield_none() {
        assert!(trace_to_sft(&json!({"intent":"x","steps":[]})).is_none());
    }
    #[test]
    fn populated_trace_yields_agent_lane_row() {
        let t = json!({"intent":"list files","steps":[{"tool_name":"ls","arguments":{},"result":"a","success":true}]});
        let row = trace_to_sft(&t).unwrap();
        assert_eq!(row["lane"], "vox_dogfood_agent");
        assert_eq!(row["origin"], "agent");
    }
}
```

- [ ] **Step 2: Declare module + run test** — add `pub mod trace_ingest;`; `cargo test -p vox-corpus trace_ingest` → FAIL then PASS.

- [ ] **Step 3: Apply the diversity gate over the converted batch (anti-mode-collapse)**

Add a batch function that converts many traces then drops the set if it fails the diversity threshold — reusing the **existing** `vox_eval::eval_semantic_entropy` (verified used in `run_diversity_check`):
```rust
/// Convert a batch of traces to SFT rows, then fail if the corpus is a
/// monoculture (semantic entropy below `min_diversity`). Mirrors the guard in
/// `corpus::run_diversity_check`.
pub fn traces_to_sft_gated(traces: &[Value], min_diversity: f64) -> anyhow::Result<Vec<Value>> {
    let rows: Vec<Value> = traces.iter().filter_map(trace_to_sft).collect();
    let responses: Vec<String> = rows.iter()
        .filter_map(|r| r.get("response").and_then(|v| v.as_str()).map(String::from))
        .collect();
    if !responses.is_empty() {
        let report = vox_eval::eval_semantic_entropy(&responses, min_diversity);
        anyhow::ensure!(!report.collapse_warning,
            "agentic trace corpus failed diversity check (mode collapse) — got {:.3}", report.ast_diversity);
    }
    Ok(rows)
}
```
**Verify the `vox_eval` API first:** `rg -n "pub fn eval_semantic_entropy|collapse_warning|ast_diversity" crates/vox-eval/src` — match the real signature/fields (the names above are from `run_diversity_check`; confirm before use). Add a test that a single repeated trace trips `collapse_warning`.

- [ ] **Step 4: Commit**
```bash
cargo fmt -p vox-corpus
git add crates/vox-corpus/src/corpus/trace_ingest.rs crates/vox-corpus/src/corpus/mod.rs
git commit -m "feat(corpus): agent-trace -> SFT converter + diversity gate (lane vox_dogfood_agent)"
```

### Task 5.4: Make `mix-agents.yaml` real and strict [SEQUENTIAL]

**Files:**
- Modify: `mens/config/mix-agents.yaml`

Context: today every source is a non-existent `optional: true` file. Point it at the two real generators' outputs and mark them required.

- [ ] **Step 1: Replace the phantom sources**
```yaml
output: mens/data/train_mixed_agents.jsonl
include_lanes: [vox_tooling, vox_dogfood_agent]
sources:
  - path: mens/data/mix_sources/agentic_synth.jsonl
    weight: 4.0
    record_format: tool_trace
    optional: false
  - path: mens/data/mix_sources/agent_traces_sft.jsonl
    weight: 3.0
    optional: false
```
Keep any genuinely-optional research extras below these, clearly marked `optional: true`.

- [ ] **Step 2: Verify load** — `cargo test -p vox-corpus mix` → PASS.

- [ ] **Step 3: Commit**
```bash
git add mens/config/mix-agents.yaml
git commit -m "feat(mens): mix-agents points at real synth+trace corpora (strict, no phantoms)"
```

---

## Phase 6 — Per-spoke training method (generalize beyond QLoRA)

**What & why:** `pipeline.rs::Train` hard-codes `PopuliTrainBackendCli::Qlora` (verified). Honor the spoke's `base.method` so the Rust-review lane can use DPO, etc. RAG/prompt-only spokes skip training entirely.

**Wave 6.1 (`[SEQUENTIAL]`):** 6.1.

### Task 6.1: Dispatch training backend from profile method [SEQUENTIAL]

**Files:**
- Modify: `crates/vox-ml-cli/src/commands/mens/pipeline.rs` (the `Train` arm)

- [ ] **Step 1: Verify the available backends**

Run: `rg -n "PopuliTrainBackend|enum.*Backend|Qlora|Dpo" crates/vox-populi/src crates/vox-ml-cli/src --type rust | head`
Expected: lists the real backend variants. **Only map methods that have a real backend.** If DPO has no trainer backend yet, map `Dpo`/`Orpo` to a clear `anyhow::bail!("method X not yet supported by trainer")` rather than silently training QLoRA (no stubs).

- [ ] **Step 2: Add the dispatch before the `run_train` call**
```rust
                        let method = if let Some(name) = profile.as_deref() {
                            let eff = vox_populi::mens::tensor::domain_profiles::EffectiveDomainProfile
                                ::load_domain_profile(name, ws.as_deref())?;
                            eff.base.map(|b| b.method)
                                .ok_or_else(|| anyhow::anyhow!("profile '{name}' has no base.method; cannot dispatch training backend"))?
                        } else {
                            vox_populi::mens::tensor::domain_profiles::TrainMethod::Qlora
                        };
                        use vox_populi::mens::tensor::domain_profiles::TrainMethod;
                        let backend = match method {
                            TrainMethod::Qlora => crate::commands::mens::PopuliTrainBackendCli::Qlora.into(),
                            TrainMethod::FullSft | TrainMethod::Dpo | TrainMethod::Orpo =>
                                anyhow::bail!("training method {:?} has no wired backend yet; add it before selecting it in domain-profiles.yaml", method),
                            TrainMethod::RagOnly | TrainMethod::PromptOnly => {
                                tracing::info!("spoke uses {:?}; skipping training stage", method);
                                continue; // no fine-tune for this spoke
                            }
                        };
```
Replace the existing `crate::commands::schola::train::run_train(crate::commands::mens::PopuliTrainBackendCli::Qlora.into(), ...)` first argument with `backend`.

- [ ] **Step 3: Verify it compiles** — `cargo check -p vox-ml-cli` → compiles.

- [ ] **Step 4: Commit**
```bash
cargo fmt -p vox-ml-cli
git add crates/vox-ml-cli/src/commands/mens/pipeline.rs
git commit -m "feat(mens): dispatch training backend from spoke method (rag/prompt skip; unwired methods bail)"
```

---

## Phase 7 — Lane-tag inference router

**What & why:** Route a request to the right spoke using the `router.triggers` already declared in the SSOT. Lane-tag first (cheapest; the research recommends starting here). This works regardless of the Phase 8 serving decision.

**Wave 7.1 (`[SEQUENTIAL]`):** 7.1.

### Task 7.1: `route()` over spoke triggers [SEQUENTIAL]

**Files:**
- Create: `crates/vox-populi/src/mens/router.rs`
- Modify: `crates/vox-populi/src/mens/mod.rs` (declare module — verify path with `rg -n "pub mod" crates/vox-populi/src/mens/mod.rs`)
- Test: inline.

- [ ] **Step 1: Write the failing test**
```rust
//! Lane-tag router: pick the spoke whose triggers best match a request.
use crate::mens::tensor::domain_profiles::DomainProfilesFile;
use std::path::Path;

/// Return the spoke name with the highest-priority matching trigger.
/// A trigger matches if it is a substring of `signal` (lane tag, filename, or
/// keyword). Primary order: `router.priority` (higher wins). **Tie-break:
/// spoke name (lexicographically smallest) — `file.profiles` is a HashMap, so
/// without an explicit tie-break the winner on equal priority would be
/// nondeterministic across runs.**
pub fn route(file: &DomainProfilesFile, signal: &str) -> Option<String> {
    let mut best: Option<(i32, &str)> = None;
    for (name, p) in &file.profiles {
        let Some(r) = &p.router else { continue };
        let matches = r.triggers.iter().any(|t| {
            let needle = t.trim_start_matches('*');
            !needle.is_empty() && signal.contains(needle)
        });
        if matches {
            let cand = (r.priority, name.as_str());
            // Higher priority wins; on tie, smaller name wins (deterministic).
            let better = match best {
                None => true,
                Some((bp, bn)) => cand.0 > bp || (cand.0 == bp && cand.1 < bn),
            };
            if better {
                best = Some(cand);
            }
        }
    }
    best.map(|(_, n)| n.to_string())
}

/// Convenience: load the SSOT and route in one call.
pub fn route_from_disk(workspace_root: &Path, signal: &str) -> anyhow::Result<Option<String>> {
    Ok(route(&DomainProfilesFile::load(Some(workspace_root))?, signal))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn file() -> DomainProfilesFile {
        serde_yaml::from_str(r#"
profiles:
  rust:
    description: x
    router: { triggers: ["*.rs", "lane:vox_rust_authoring"], priority: 10 }
  agents:
    description: x
    router: { triggers: ["lane:vox_tooling"], priority: 5 }
"#).unwrap()
    }
    #[test] fn routes_rust_file_to_rust_spoke() {
        assert_eq!(route(&file(), "src/main.rs").as_deref(), Some("rust"));
    }
    #[test] fn routes_tool_lane_to_agents() {
        assert_eq!(route(&file(), "lane:vox_tooling call").as_deref(), Some("agents"));
    }
    #[test] fn no_match_returns_none() {
        assert_eq!(route(&file(), "unrelated"), None);
    }
    #[test] fn equal_priority_breaks_deterministically_by_name() {
        // Two spokes, equal priority, both match "x". Must ALWAYS return the
        // lexicographically-smaller name regardless of HashMap order.
        let f: DomainProfilesFile = serde_yaml::from_str(r#"
profiles:
  zeta:  { description: x, router: { triggers: ["x"], priority: 5 } }
  alpha: { description: x, router: { triggers: ["x"], priority: 5 } }
"#).unwrap();
        for _ in 0..20 { assert_eq!(route(&f, "x").as_deref(), Some("alpha")); }
    }
}
```

- [ ] **Step 2: Run test to verify it fails** — `cargo test -p vox-populi router` → FAIL (module undeclared).

- [ ] **Step 3: Declare module** — add `pub mod router;` to `crates/vox-populi/src/mens/mod.rs`.

- [ ] **Step 4: Run test to verify it passes** — `cargo test -p vox-populi router` → PASS (3 tests).

- [ ] **Step 5: Commit**
```bash
cargo fmt -p vox-populi
git add crates/vox-populi/src/mens/router.rs crates/vox-populi/src/mens/mod.rs
git commit -m "feat(mens): lane-tag inference router over spoke triggers (priority tiebreak)"
```

---

## Phase 8 — Serving-topology decision + end-to-end validation

**What & why:** Resolve the one deferred research question (shared-base adapter hot-swap vs. separate servers) now that bases (Phase 3) and the router (Phase 7) exist, then prove one spoke trains end-to-end.

**Wave 8.1 (`[SEQUENTIAL]`):** 8.1 (decision doc) → 8.2 (smoke validation).

### Task 8.1: Serving-topology decision doc [SEQUENTIAL]

**Files:**
- Create: `docs/src/architecture/voxmens-serving-topology-decision-2026-06-18.md` (frontmatter required — `docs/src/`)

- [ ] **Step 1: Decide using the research + the now-known per-spoke bases**

If Phase 3 picked the **same base** for all spokes → adapter hot-swap (S-LoRA/LoRAX-style) is viable; if **heterogeneous bases** → separate model servers behind the Phase 7 router. Confirm the S-LoRA/LoRAX "single shared base" constraint with one live source before locking (it was unverified in the research).

- [ ] **Step 2: Write the doc** (frontmatter `title`/`description`/`category: "Architecture"`/`status: "roadmap"`/`training_eligible: false`), recording: chosen topology, the verified constraint, VRAM math from `gpu-specs.yaml`, and the router's role.

- [ ] **Step 3: Commit**
```bash
git add docs/src/architecture/voxmens-serving-topology-decision-2026-06-18.md
git commit -m "docs(mens): serving-topology decision (adapter hot-swap vs separate servers)"
```

### Task 8.2: End-to-end dry-run validation [SEQUENTIAL]

**Files:** none (validation only).

- [ ] **Step 1: Validate the SSOT end to end (no GPU)**

Run:
```bash
cargo test -p vox-populi -p vox-corpus -p vox-ml-cli
cargo run -p vox-arch-check
```
Expected: all green; arch-check clean. Paste output.

- [ ] **Step 2: Dry-run the pipeline for each spoke**

Run the pipeline in `--dry-run` / `--skip-train` for each profile (verify the exact flags first: `rg -n "dry_run|skip_train|profile" crates/vox-ml-cli/src/commands/mens`). Expected: each spoke resolves its mix (strict), its base model (from registry), its method, and its eval gate without error. For `agents` and `rust`, confirm the strict mix now finds real sources (Phases 4–5).

- [ ] **Step 3: One real micro-train (optional, GPU host only)**

On a `--features gpu` build, train the `vox-lang` spoke for 1 epoch on a tiny slice to prove the full path (mix → resolved base → QLoRA → eval gate). **Two-strike rule applies:** if it fails twice, STOP and write a handoff note; do not loop. Paste the eval-gate receipt.

- [ ] **Step 4: Final commit (validation notes)**
```bash
git commit --allow-empty -m "test(mens): hub-and-spoke end-to-end dry-run validated (3 spokes resolve)"
```

---

## D. Spec-coverage self-check (done by plan author)

- SSOT for hub-and-spoke (config-only spoke add) → Phase 1 (extends `domain-profiles.yaml` + validator). ✓
- Extensible for new spokes → new YAML record + validator; no Rust change. ✓
- Best model per spoke, not all same base → Phase 3 registry (role → variants). ✓
- Scaled to available resources → Phase 3 VRAM-scaled resolver vs `gpu-specs.yaml`. ✓
- Generalize beyond QLoRA / cross-method SSOT → Phase 1 `TrainMethod` + Phase 6 dispatch. ✓
- Train all three spoke types → Phases 4 (Rust corpus), 5 (agentic corpus), 1/6 (VoxScript + method). ✓
- Router (MoE-ish) → Phase 7 lane-tag router; topology in Phase 8. ✓
- Gemini/Antigravity orchestration, sub-agents, parallel/sequential, context limits → §A + per-task tags. ✓
- Research best-practices enforced → strict mix, per-spoke eval, no-stubs, verify-before-use throughout. ✓
- Kill silent-optional corpus gap → Phase 1.6 + Phases 4.3/5.4 strict sources. ✓

**Known deferrals (by design, see §B):** concrete base-model IDs and serving topology are filled by decision tasks (3.1, 8.1) with live re-verification, because the research left them unverified. The plan supplies the mechanism and the candidate matrix; it does not hard-code a perishable guess.

### Revision log (post code-review, 2026-06-18)

Applied after a Rust-ecosystem review of the embedded code:
- **Critical — inert eval gates fixed.** `check_run` has per-gate-name handlers and does NOT read arbitrary YAML keys; nothing produced Rust/agentic metrics. Phase 2 now has the full **producer → handler → gate** chain (Tasks 2.0, 2.1a/b/c, 2.2a/b/c) and the false "loads unchanged" claim is removed.
- **Critical — Rust verifier rebuilt.** Replaced per-snippet `cargo check` (O(n) cold-starts + isolation false-negatives) with **batched workspace-context compile** (Task 4.2), shared by the eval producer (Task 2.1a) for DRY.
- **Critical — deterministic routing.** `route()` now breaks priority ties by spoke name (was `HashMap`-order nondeterministic); regression test added (Task 7.1).
- **Fail-closed VRAM.** Task 3.4 errors if VRAM can't be detected instead of assuming 16 GB (which would OOM smaller GPUs).
- **Validator promise honored.** New Task 3.5 cross-checks `base.model`/`method`/`preset` against `model-registry.yaml` + `gpu-specs.yaml`.
- **Strict-mix precision.** Task 1.6 keys strictness on the active profile's resolved `mix_config`, not a `"mix-"` filename prefix (which would have wrongly forced research mixes strict).
- **Correct tool surface.** Task 5.1 synthesizes the **Vox** CLI/skill surface, not Claude's `Skill` tool.
- **Diversity gate wired.** Task 5.3 now actually calls `eval_semantic_entropy` (Step 3) and de-duplicates a redundant serialization.
- `ToolTraceRecord` literal confirmed complete (7 fields); speculative `// NOTE` removed.

**Follow-ups resolved (2026-06-18):**
- **Validator placement (Task 1.4):** arch-check is layer 0, vox-populi is layer 3 → wiring it there is a layer violation. Resolved to a new **`vox ci spoke-check`** subcommand in `vox-ml-cli` (layer 3), added to the CI/pre-push gate list.
- **`tool_name_exists_rate` (Tasks 2.2a/b):** an enumerable registry exists (`vox_mcp_registry::TOOL_REGISTRY` / `vox-corpus TOOL_REGISTRY_SLIM`) → promoted from "drop if absent" to a **hard blocking gate**.
- **Agentic synthesis reuse (Task 5.1):** `vox-corpus` already has `generate_tool_pairs` over `TOOL_REGISTRY_SLIM` + `ORCHESTRATOR_TOOLS`/`SKILL_TOOLS` → reuse/extend rather than rebuild; new code fills only skill/CLI-discovery gaps.
