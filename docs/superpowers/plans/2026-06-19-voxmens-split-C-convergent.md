# VoxMens Split C (Convergent) — Selection + Routing over Existing Infra

> **For agentic workers:** REQUIRED SUB-SKILL: `crates/vox-skills/skills/superpowers/subagent-driven-development.skill.md`. Steps use `- [ ]`.
> **EXECUTION TARGET: Gemini 3.5 Flash inside Google Antigravity.** Read §A before any task.
> **SUPERSEDES** `2026-06-18-voxmens-split-C-selection-routing-serving.md`. Design: [`../specs/2026-06-19-voxmens-model-selection-convergence-design.md`](../specs/2026-06-19-voxmens-model-selection-convergence-design.md).

**Goal:** Fill the three real gaps in MENS per-spoke selection/routing by **converging onto existing infrastructure** — NOT by building a new model-registry/resolver/router. A spoke resolves its base model (capability + host-VRAM fit) over the shared catalog + a minimal overlay; its `base.method` drives the training kernel; its `router.triggers` drive a deterministic lane router.

**Architecture:** Reuse `vram_autodetect`, `domain_router`, `execution_planner`, the `domain-profiles.yaml` SSOT, and the `model-catalog.bootstrap.v1.json` *contract* (data, cross-layer-safe). Add only: a small `spoke_base_resolver`, a `train_bases:` overlay in the existing `gpu-specs.yaml`, a `route_by_signal` method on the existing router, and method-dispatch wiring. The inference `select()`/egress stack is **untouched**.

**Tech Stack:** Rust (`vox-populi`, `vox-ml-cli`), YAML (`mens/config/*`), JSON contract (`contracts/orchestration/`).

---

## A. Execution rules (Gemini 3.5 Flash)

Source: [`gemini-3-5-flash-antigravity-limitations-2026-06-18.md`](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md), [`antigravity-handoff-and-skill-gaps-2026-06-18.md`](../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md).

- **Atomic-green-commit:** every task ends compiling, tested, committed. A kill wastes ≤1 task.
- **Verify-before-use:** every code step referencing a symbol/path is preceded by the `rg`/read step shown; inline the REAL signature. Missing symbol → STOP + handoff note, never invent.
- **Self-contained tasks; two-strike circuit breaker** (fail twice → STOP + note, don't loop).
- **No stubs/placeholders** in committed code.
- **🔴 GATE-INTEGRITY (AGH-0008 lesson):** You may **NOT** change any `= "error"` severity in `layers.toml` to `"warn"`, add `--warn-only`, `|| true`, or narrow a gate to make it pass. If `cargo run -p vox-arch-check` (or `vox ci spoke-check`) is red at baseline for unrelated reasons, **STOP and report**. The `forbidden_pattern` guard is `error` — keep it so.
- **Vox policy:** `.vox` automation only; never `cargo fmt --all` (use `cargo fmt -p <crate>`); `.md` under `docs/src/` needs frontmatter; secrets via `vox_secrets::resolve_secret`.
- **Verification ritual before each commit (paste output):** `cargo test -p <crate>`, `cargo clippy -p <crate> -- -D warnings`, `cargo fmt -p <crate>`, `cargo check -p <crate>`.
- **Windows build note:** the main `target/` may be RA-locked (`os error 5`). If a build fails to relink only because of that, retry with an isolated dir: `CARGO_TARGET_DIR=target/iso cargo test -p <crate>`.
- Each task tagged `[PARALLEL-SAFE]`/`[SEQUENTIAL]`; never two subagents on one file.

## B. Prereq gate (run first; STOP if any fails)
- [ ] Plan A landed: `rg -n "pub base|enum TrainMethod|pub struct SpokeRouter|fn load_domain_profile" crates/vox-populi/src/mens/tensor/domain_profiles.rs` → all present.
- [ ] Plan B landed: `vox ci spoke-check` exits 0; `mens/config/eval-gates-rust.yaml` + `eval-gates-agents.yaml` exist.
- [ ] Guard intact: `rg -n '^forbidden_pattern' docs/src/architecture/layers.toml` shows `= "error"`. If it shows `"warn"`, STOP — a regression must be fixed first (see ledger AGH-0008).

---

## Phase 0 — Confirm the reuse seams (read-only, anti-hallucination)

**Wave 0.1:** Task 0.1 (`[SEQUENTIAL]`).

### Task 0.1: Verify every existing symbol this plan reuses [SEQUENTIAL]
**Files:** none.
- [ ] **Step 1:** Run and confirm each prints a match:
```bash
rg -n "pub fn get_system_vram_gb|pub fn auto_preset" crates/vox-populi/src/mens/tensor/vram_autodetect.rs
rg -n "pub struct DomainRouter|pub fn route\b|pub fn register|pub fn discover" crates/vox-populi/src/mens/tensor/domain_router.rs
rg -n "pub fn resolve_kernel|enum AdapterMethod|enum PopuliTrainBackend" crates/vox-populi/src/mens
rg -n "pub fn load\b|pub struct EffectiveDomainProfile|pub struct SpokeBase|pub struct SpokeRouter|enum TrainMethod" crates/vox-populi/src/mens/tensor/domain_profiles.rs
rg -n "DEFAULT_MODEL_ID|resolve_default_model_id" crates/vox-populi/src/mens/mod.rs
ls contracts/orchestration/model-catalog.bootstrap.v1.json
rg -n "^presets:|max_vram_mb" mens/config/gpu-specs.yaml
```
Expected: all resolve. Any miss → STOP + handoff note (the codebase drifted from this plan). No commit.

---

## Phase 1 — Spoke base resolver (capability + VRAM fit over shared facts)

**What:** A spoke's `base.model` tag → a concrete fine-tunable HF id that fits host VRAM, using a minimal overlay co-located in `gpu-specs.yaml` (NOT a new catalog file).

**Wave 1.1 (`[SEQUENTIAL]`):** 1.1 → 1.2 → 1.3 → 1.4.

### Task 1.1: Add the `train_bases:` overlay to `gpu-specs.yaml` [SEQUENTIAL]
**Files:** Modify `mens/config/gpu-specs.yaml`.
Context (verified): `gpu-specs.yaml` already has `gpus:` and `presets:` (with `max_vram_mb`). Add a sibling `train_bases:` map — capability tag → candidate fine-tunable bases with QLoRA VRAM floors. These reference the same model families as `contracts/orchestration/model-catalog.bootstrap.v1.json` but carry the training-only fields (HF repo id, floor, methods) the catalog lacks.
- [ ] **Step 1:** Append to `gpu-specs.yaml`:
```yaml
# Training base candidates per capability tag (referenced by domain-profiles base.model).
# floor = approximate QLoRA VRAM floor in MB on the standard preset. Resolver picks
# the largest variant whose floor <= detected VRAM. methods = train methods this base supports.
train_bases:
  small_code:
    - { hf_id: "Qwen/Qwen2.5-Coder-3B-Instruct", floor_mb: 6000, methods: [qlora, full_sft] }
  strong_code:
    - { hf_id: "Qwen/Qwen2.5-Coder-7B-Instruct", floor_mb: 11000, methods: [qlora, dpo, orpo] }
  agentic:
    - { hf_id: "Qwen/Qwen2.5-Coder-7B-Instruct", floor_mb: 11000, methods: [qlora, dpo] }
```
(Concrete ids/floors are the starting policy — refine against live benchmarks before production; `Qwen2.5-Coder` matches the existing `DEFAULT_MODEL_ID`.)
- [ ] **Step 2: Commit** `git add mens/config/gpu-specs.yaml && git commit -m "feat(mens): train_bases overlay in gpu-specs (capability tag -> fine-tunable bases + VRAM floor)"`

### Task 1.2: Pure resolver `pick_base(tag, vram_mb, overlay)` [SEQUENTIAL]
**Files:** Create `crates/vox-populi/src/mens/tensor/spoke_base_resolver.rs`; Modify `crates/vox-populi/src/mens/tensor/mod.rs`; Test inline.
- [ ] **Step 1: Write the failing test + types**
```rust
//! Resolve a spoke capability tag -> concrete fine-tunable base that fits VRAM.
//! Overlay source: `train_bases:` in mens/config/gpu-specs.yaml. Pure core +
//! a thin disk loader; reuses vram_autodetect for the live VRAM number.
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct TrainBase {
    pub hf_id: String,
    pub floor_mb: u32,
    #[serde(default)]
    pub methods: Vec<String>,
}

/// Largest candidate for `tag` whose `floor_mb <= vram_mb`. Errors if the tag is
/// unknown or nothing fits (fail-closed — never silently pick a too-big base).
pub fn pick_base<'a>(
    overlay: &'a HashMap<String, Vec<TrainBase>>,
    tag: &str,
    vram_mb: u32,
) -> anyhow::Result<&'a TrainBase> {
    let candidates = overlay
        .get(tag)
        .ok_or_else(|| anyhow::anyhow!("unknown base tag '{tag}' (not in gpu-specs train_bases)"))?;
    candidates
        .iter()
        .filter(|b| b.floor_mb <= vram_mb)
        .max_by_key(|b| b.floor_mb)
        .ok_or_else(|| anyhow::anyhow!("no '{tag}' base fits {vram_mb}MB VRAM"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn overlay() -> HashMap<String, Vec<TrainBase>> {
        let mut m = HashMap::new();
        m.insert("strong_code".into(), vec![
            TrainBase { hf_id: "small".into(), floor_mb: 6000, methods: vec!["qlora".into()] },
            TrainBase { hf_id: "big".into(),   floor_mb: 11000, methods: vec!["qlora".into()] },
        ]);
        m
    }
    #[test] fn picks_largest_that_fits() {
        assert_eq!(pick_base(&overlay(), "strong_code", 16384).unwrap().hf_id, "big");
        assert_eq!(pick_base(&overlay(), "strong_code", 8000).unwrap().hf_id, "small");
    }
    #[test] fn errors_when_none_fit() { assert!(pick_base(&overlay(), "strong_code", 4000).is_err()); }
    #[test] fn errors_unknown_tag()  { assert!(pick_base(&overlay(), "nope", 16384).is_err()); }
}
```
- [ ] **Step 2:** `rg -n "pub mod domain_router" crates/vox-populi/src/mens/tensor/mod.rs` to find the module-decl block; add `pub mod spoke_base_resolver;`.
- [ ] **Step 3:** `cargo test -p vox-populi spoke_base_resolver` → FAIL then PASS (3 tests).
- [ ] **Step 4: Commit** `feat(mens): pure VRAM-fit base resolver over train_bases overlay`

### Task 1.3: Disk loader that wires overlay + live VRAM [SEQUENTIAL]
**Files:** Modify `spoke_base_resolver.rs`; Test inline (loader test reads the repo `gpu-specs.yaml`).
- [ ] **Step 1: Verify the VRAM + gpu-specs path** — `rg -n "fn get_system_vram_gb" crates/vox-populi/src/mens/tensor/vram_autodetect.rs` and confirm `gpu-specs.yaml` location.
- [ ] **Step 2: Add the loader**
```rust
#[derive(Debug, Deserialize)]
struct GpuSpecsTrainBases { #[serde(default)] train_bases: HashMap<String, Vec<TrainBase>> }

/// Load the `train_bases:` overlay from mens/config/gpu-specs.yaml under `root`.
pub fn load_overlay(root: &std::path::Path) -> anyhow::Result<HashMap<String, Vec<TrainBase>>> {
    let p = root.join("mens/config/gpu-specs.yaml");
    let s = std::fs::read_to_string(&p).map_err(|e| anyhow::anyhow!("read {}: {e}", p.display()))?;
    let parsed: GpuSpecsTrainBases = serde_yaml::from_str(&s)
        .map_err(|e| anyhow::anyhow!("parse train_bases in gpu-specs.yaml: {e}"))?;
    Ok(parsed.train_bases)
}

/// Resolve a spoke tag to a concrete HF id using the live host VRAM.
/// `vram_mb_override` lets tests inject a value (None → vram_autodetect).
pub fn resolve_hf_id(root: &std::path::Path, tag: &str, vram_mb_override: Option<u32>) -> anyhow::Result<String> {
    let overlay = load_overlay(root)?;
    let vram_mb = match vram_mb_override {
        Some(v) => v,
        None => {
            let gb = crate::mens::tensor::vram_autodetect::get_system_vram_gb()
                .ok_or_else(|| anyhow::anyhow!("cannot detect GPU VRAM; set base.model to a concrete HF id or pass --model"))?;
            (gb * 1024.0) as u32
        }
    };
    Ok(pick_base(&overlay, tag, vram_mb)?.hf_id.clone())
}
```
**Verify** the `vram_autodetect` path/fn name from Step 1 and adjust the call exactly.
- [ ] **Step 3: Add loader test** (reads repo overlay with injected VRAM):
```rust
    #[test]
    fn loads_repo_overlay_and_resolves_with_injected_vram() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).unwrap();
        let id = resolve_hf_id(root, "strong_code", Some(16384)).unwrap();
        assert!(id.contains("Qwen"), "got {id}");
    }
```
- [ ] **Step 4:** `cargo test -p vox-populi spoke_base_resolver` → PASS.
- [ ] **Step 5: Commit** `feat(mens): gpu-specs train_bases loader + VRAM-aware resolve_hf_id`

### Task 1.4: Treat a concrete HF id in `base.model` as pass-through [SEQUENTIAL]
**Files:** Modify `spoke_base_resolver.rs`; Test inline.
Context: `base.model` may be a tag (`strong_code`) OR a concrete id (`Qwen/...`). If it contains `/`, treat it as a literal id (skip overlay).
- [ ] **Step 1: Test + impl**
```rust
/// If `base_model` looks like a concrete HF id (contains '/'), return it as-is;
/// otherwise treat it as a capability tag and resolve via the overlay + VRAM.
pub fn resolve_base_model(root: &std::path::Path, base_model: &str, vram_mb_override: Option<u32>) -> anyhow::Result<String> {
    if base_model.contains('/') { return Ok(base_model.to_string()); }
    resolve_hf_id(root, base_model, vram_mb_override)
}

#[cfg(test)]
mod passthrough_tests {
    use super::*;
    #[test] fn concrete_id_is_passthrough() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).unwrap();
        assert_eq!(resolve_base_model(root, "org/My-Model", Some(8000)).unwrap(), "org/My-Model");
    }
}
```
- [ ] **Step 2:** `cargo test -p vox-populi spoke_base_resolver` → PASS.
- [ ] **Step 3: Commit** `feat(mens): base.model accepts concrete HF id or capability tag`

---

## Phase 2 — Wire base.model + base.method into training (reuse run_train/execution_planner)

**Wave 2.1 (`[SEQUENTIAL]`):** 2.1 → 2.2.

### Task 2.1: Resolve the profile's base model at the pipeline Train stage [SEQUENTIAL]
**Files:** Modify `crates/vox-ml-cli/src/commands/mens/pipeline.rs` (Train arm).
- [ ] **Step 1: Verify the seam** — `rg -n "PipelineStage::Train|run_train\(|profile|EffectiveDomainProfile|let target_model" crates/vox-ml-cli/src/commands/mens/pipeline.rs`. Confirm the `profile: Option<String>` param (Plan A) and the `run_train(... model ...)` call.
- [ ] **Step 2: Resolve before `run_train`** — using the active profile's `base.model` via the Phase 1 resolver; **CLI `--model` still wins**:
```rust
let resolved_base = if let Some(name) = profile.as_deref() {
    let eff = vox_populi::mens::tensor::domain_profiles::EffectiveDomainProfile
        ::load_domain_profile(name, ws.as_deref())?;
    match eff.base.as_ref().map(|b| b.model.clone()) {
        Some(tag) => Some(vox_populi::mens::tensor::spoke_base_resolver::resolve_base_model(
            ws.as_deref().unwrap_or(std::path::Path::new(".")), &tag, None)?),
        None => None,
    }
} else { None };
let target_model = model.clone().or(resolved_base); // CLI override wins
```
**Verify** `ws` and the existing `target_model` construction; adjust exactly to the real code. Fail-closed VRAM error from the resolver is correct (do not swallow it).
- [ ] **Step 3:** `cargo check -p vox-ml-cli` → compiles. (No GPU train here.)
- [ ] **Step 4: Commit** `feat(mens): pipeline resolves per-spoke base model (tag->HF id, VRAM-fit; CLI override wins)`

### Task 2.2: Dispatch training backend from `base.method` [SEQUENTIAL]
**Files:** Modify `pipeline.rs` (Train arm).
- [ ] **Step 1: Verify backends** — `rg -n "enum PopuliTrainBackend|PopuliTrainBackendCli|Qlora" crates/vox-populi/src crates/vox-ml-cli/src | head`. Note the real variants; map ONLY methods with a real backend.
- [ ] **Step 2: Dispatch**
```rust
use vox_populi::mens::tensor::domain_profiles::TrainMethod;
let method = profile.as_deref()
    .and_then(|n| vox_populi::mens::tensor::domain_profiles::EffectiveDomainProfile
        ::load_domain_profile(n, ws.as_deref()).ok())
    .and_then(|e| e.base.map(|b| b.method))
    .unwrap_or(TrainMethod::Qlora);
let backend = match method {
    TrainMethod::Qlora => crate::commands::mens::PopuliTrainBackendCli::Qlora.into(),
    TrainMethod::FullSft | TrainMethod::Dpo | TrainMethod::Orpo =>
        anyhow::bail!("training method {:?} has no wired backend yet; wire it before selecting it in domain-profiles.yaml", method),
    TrainMethod::RagOnly | TrainMethod::PromptOnly => {
        tracing::info!("spoke uses {:?}; skipping training stage", method); continue;
    }
};
```
Pass `backend` as the `run_train` first arg (replace the hard-coded `Qlora`). **Note:** the deeper integration point is `execution_planner::resolve_kernel` (maps `AdapterMethod`→backend); if the contract path is the right seam, wire there instead — verify with `rg -n "resolve_kernel|AdapterMethod" crates/vox-populi/src/mens` and prefer the existing resolver over a parallel match.
- [ ] **Step 3:** `cargo check -p vox-ml-cli` → compiles.
- [ ] **Step 4: Commit** `feat(mens): dispatch train backend from spoke base.method (rag/prompt skip; unwired fail-closed)`

---

## Phase 3 — Lane-tag routing on the existing DomainRouter

**Wave 3.1 (`[SEQUENTIAL]`):** 3.1.

### Task 3.1: `route_by_signal` over SpokeRouter triggers + priority [SEQUENTIAL]
**Files:** Modify `crates/vox-populi/src/mens/tensor/domain_router.rs`; Test inline.
Context (verified): `DomainRouter` routes by domain name. Add a signal router that reads `domain-profiles.yaml` (`DomainProfilesFile::load`) and picks the spoke whose `router.triggers` substring-match `signal`, highest `priority`, **name tie-break for determinism** (profiles is a HashMap).
- [ ] **Step 1: Write the failing test + fn**
```rust
use crate::mens::tensor::domain_profiles::DomainProfilesFile;

/// Pick the spoke whose router.triggers match `signal` (substring), by highest
/// priority, breaking ties on the lexicographically smaller spoke name
/// (DomainProfilesFile.profiles is a HashMap → name tie-break is required for
/// deterministic routing). A trigger's leading `*` is stripped before matching.
pub fn route_by_signal(file: &DomainProfilesFile, signal: &str) -> Option<String> {
    let mut best: Option<(i32, &str)> = None;
    for (name, p) in &file.profiles {
        let Some(r) = &p.router else { continue };
        let hit = r.triggers.iter().any(|t| {
            let needle = t.trim_start_matches('*');
            !needle.is_empty() && signal.contains(needle)
        });
        if hit {
            let cand = (r.priority, name.as_str());
            let better = match best { None => true, Some((bp, bn)) => cand.0 > bp || (cand.0 == bp && cand.1 < bn) };
            if better { best = Some(cand); }
        }
    }
    best.map(|(_, n)| n.to_string())
}

#[cfg(test)]
mod signal_tests {
    use super::*;
    fn file() -> DomainProfilesFile {
        serde_yaml::from_str(r#"
profiles:
  rust-expert: { description: x, router: { triggers: ["*.rs", "lane:vox_rust_authoring"], priority: 10 } }
  agents:      { description: x, router: { triggers: ["lane:vox_tooling"], priority: 5 } }
"#).unwrap()
    }
    #[test] fn rs_file_routes_to_rust() { assert_eq!(route_by_signal(&file(), "src/main.rs").as_deref(), Some("rust-expert")); }
    #[test] fn tool_lane_routes_to_agents() { assert_eq!(route_by_signal(&file(), "lane:vox_tooling x").as_deref(), Some("agents")); }
    #[test] fn no_match_is_none() { assert_eq!(route_by_signal(&file(), "zzz"), None); }
    #[test] fn equal_priority_breaks_by_name() {
        let f: DomainProfilesFile = serde_yaml::from_str(
            "profiles:\n  zeta:  { description: x, router: { triggers: [\"x\"], priority: 5 } }\n  alpha: { description: x, router: { triggers: [\"x\"], priority: 5 } }\n").unwrap();
        for _ in 0..20 { assert_eq!(route_by_signal(&f, "x").as_deref(), Some("alpha")); }
    }
}
```
**Verify** `DomainProfilesFile`/`profiles`/`SpokeRouter` field names with `rg` before relying. Note in a comment: substring `.rs` also matches `foo.rsync` — acceptable v1; tighten to a suffix check if false routes appear.
- [ ] **Step 2:** `cargo test -p vox-populi route_by_signal` → FAIL then PASS (4 tests).
- [ ] **Step 3: Commit** `feat(mens): DomainRouter::route_by_signal (triggers+priority, deterministic tie-break)`

---

## Phase 4 — Validation + end-to-end

**Wave 4.1 (`[SEQUENTIAL]`):** 4.1 → 4.2.

### Task 4.1: Extend `spoke_validate` to check base.model resolvability [SEQUENTIAL]
**Files:** Modify `crates/vox-populi/src/mens/tensor/spoke_validate.rs`; Test inline.
- [ ] **Step 1: Verify** — `rg -n "pub fn validate|SpokeViolation|fine_tune" crates/vox-populi/src/mens/tensor/spoke_validate.rs`.
- [ ] **Step 2:** Add a check: for each spoke with a `base` whose `method` is a fine-tune method, `base.model` must be EITHER a concrete id (contains `/`) OR a tag present in the `train_bases` overlay (`spoke_base_resolver::load_overlay`). Add a violation otherwise. Add a unit test with a bogus tag → violation; a real tag → none.
- [ ] **Step 3:** `cargo test -p vox-populi spoke_validate` → PASS. Then confirm the gate: `vox ci spoke-check` exits 0 (the live domain-profiles tags `small_code`/`strong_code`/`agentic` must exist in the overlay — if a profile uses a placeholder tag from Plan A like `small_code_default`, reconcile the names here).
- [ ] **Step 4: Commit** `feat(mens): spoke-check validates base.model resolves (tag in overlay or concrete id)`

### Task 4.2: End-to-end dry-run + serving-topology note [SEQUENTIAL]
**Files:** Create `docs/src/architecture/voxmens-serving-topology-decision-2026-06-19.md` (frontmatter).
- [ ] **Step 1: Validate** — `cargo test -p vox-populi -p vox-ml-cli`; `cargo run -p vox-arch-check` (must stay green at `forbidden_pattern = error`); `vox ci spoke-check` exits 0. Paste output.
- [ ] **Step 2: Per-spoke dry-run** — run the pipeline `--profile <spoke> --skip-train` (verify the `--profile` CLI flag exists with `rg -n "profile" crates/vox-ml-cli/src/commands/mens`; if absent, add the clap arg — it's the Plan A seam exposed at the CLI) for `vox-lang`, `rust-expert`, `agents`. Each must resolve base model (tag→HF id), method, and eval gate without error.
- [ ] **Step 3: Write the serving-topology decision** (frontmatter `category: "architecture"`): the convergence spec already sets the boundary — record that **training base selection is local/offline (no inference-egress serving topology needed for fine-tuning)**, and that **inference of trained adapters reuses the existing `domain_router` (adapter hot-swap by domain) on a shared base**; heterogeneous bases would require separate adapter sets per base (note the constraint). No S-LoRA dependency decision needed for the training pipeline itself.
- [ ] **Step 4: Commit** `docs(mens): serving-topology decision + Split C convergent e2e validated`

---

## C. Green boundary (initiative complete)
- A spoke resolves `base.model` (tag→VRAM-fit HF id, or concrete id) via the resolver over the shared `gpu-specs.yaml` overlay; unit-tested with injected VRAM.
- `base.method` drives the training backend (rag/prompt skip; unwired methods fail closed).
- `DomainRouter::route_by_signal` resolves a signal→spoke deterministically; tested.
- `vox ci spoke-check` + `cargo test` green; `forbidden_pattern` stays `error`; e2e `--profile` dry-run succeeds for all three spokes.
- **No** new model-registry/resolver/router system was created; the inference `select()`/egress stack is untouched.

## D. Spec-coverage check
- Converge onto existing (no 3rd system) → Phases 1–3 reuse vram_autodetect/domain_router/execution_planner + the catalog/overlay. ✓
- Real gaps filled: base resolver (P1), method dispatch (P2), trigger routing (P3), validation+e2e (P4). ✓
- Dropped: model-registry.yaml, model_registry.rs, router.rs, detect_available_vram_mb. ✓
- Gate integrity preserved (AGH-0008 lesson in §A). ✓
- Boundary documented (P4 Step 3 + spec §5). ✓
