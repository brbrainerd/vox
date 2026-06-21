# VoxMens Split C (Convergent) — Selection + Routing over Existing Infra

> **For agentic workers:** REQUIRED SUB-SKILL: `crates/vox-skills/skills/superpowers/subagent-driven-development.skill.md`. Steps use `- [ ]`.
> **EXECUTION TARGET: Gemini Flash 3.5 inside Google Antigravity.** Read §A and §D before any task.
> **SUPERSEDES** `2026-06-18-voxmens-split-C-selection-routing-serving.md`. Design: [`../specs/2026-06-19-voxmens-model-selection-convergence-design.md`](../specs/2026-06-19-voxmens-model-selection-convergence-design.md).
> **Ledger:** open/fill **AGH-0010** in [`../antigravity-handoff-ledger.md`](../antigravity-handoff-ledger.md) — see §C.

**Goal:** Fill the three real gaps in MENS per-spoke selection/routing by **converging onto existing infrastructure** — NOT by building a new model-registry/resolver/router. A spoke resolves its base model (capability tag + host-VRAM fit) over a minimal overlay; its `base.method` drives the training kernel via the existing `AdapterMethodRegistry`; its `router.triggers` drive a deterministic lane router on the existing `DomainRouter`.

**Architecture:** Reuse `vram_autodetect`, `domain_router`, `finetune_registry::AdapterMethodRegistry`, the `domain-profiles.yaml` SSOT, and the `model-catalog.bootstrap.v1.json` *contract* (data, layer-safe). Add only: a small `spoke_base_resolver`, a `train_bases:` overlay in the existing `gpu-specs.yaml`, a `route_by_signal` method on the existing router, and base-model + method wiring at the pipeline Train stage. The inference `select()`/egress stack is **untouched**.

**Tech Stack:** Rust (`vox-populi` L3, `vox-ml-cli` L3 binary), YAML (`mens/config/*`).

**Verified baseline (audit 2026-06-19 — inline, but re-confirm per §A):**
- `pipeline::run(..., profile: Option<String>, ...)` EXISTS (`crates/vox-ml-cli/src/commands/mens/pipeline.rs:25`); `--profile` clap flag EXISTS (`mens/populi/action_populi_enum.rs:47-48`, `#[arg(long)] profile: Option<String>`) and is wired through `dispatch.rs` → `pipeline::run`. **Do NOT add it.**
- Train arm today (`pipeline.rs:365-388`): `let target_model = model.clone();` then `run_train(PopuliTrainBackendCli::Qlora.into(), target_model, ...)`. Base-model resolution + method dispatch are the gaps.
- `PopuliTrainBackend = { CandleQlora, BurnLora }` (only two). `AdapterMethod = { Lora, Qlora }`. `AdapterMethodRegistry::builtin().resolve(AdapterMethod) -> AdapterMethodRecord{default_kernel}` (Qlora→CandleQlora, Lora→BurnLora) is the **method→kernel SSOT** (`finetune_registry.rs`). There is **no** Dpo/Orpo/FullSft backend.
- `domain_profiles`: `EffectiveDomainProfile::load_domain_profile`, `DomainProfilesFile::load`, `pub profiles: HashMap<String, DomainProfile>`, `SpokeBase{model,method,preset}`, `SpokeRouter{triggers,priority}`, `TrainMethod{Qlora,FullSft,Dpo,Orpo,RagOnly,PromptOnly}` — all EXIST.
- `vram_autodetect::get_system_vram_gb() -> Option<f32>` (returns **None on non-CUDA / CPU-only hosts** — see §E), `auto_preset(...)`.
- Live `domain-profiles.yaml` base tags: **`small_code_default`, `strong_code_default`, `agentic_default`** (overlay keys MUST match these).
- `gpu-specs.yaml` is parsed into `GpuSpecsFile` with **no `deny_unknown_fields`** → adding `train_bases:` is safe; this plan reads it with its own struct.
- `vox ci spoke-check` EXISTS. `vox ci handoff-ledger` does **NOT** exist (don't run it). `agy_ledger.rs` auto-appends a ledger entry per agy delegation.

---

## A. Execution rules (Gemini Flash 3.5) — non-negotiable

Source: [`gemini-3-5-flash-antigravity-limitations-2026-06-18.md`](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md), [`antigravity-handoff-and-skill-gaps-2026-06-18.md`](../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md), and ledger §B (lessons baked in below).

1. **Atomic-green-commit.** Every task ends compiling, tested, committed. A kill wastes ≤1 task. Never split a compile-breaking change across commits.
2. **Verify-before-use.** Every code step referencing a symbol/path is preceded by its `rg`/read step; inline the REAL signature. A missing symbol → **STOP + handoff note**; never invent. (Ledger §B-6.)
3. **Prove the EFFECT, not the SHAPE** (ledger §B-9). A unit test asserting a resolver returns the string `"big"` is hollow. Acceptance must exercise that the resolved model id actually flows through the pipeline to `run_train` (Phase 2 + Phase 4 dry-run). A "virtual"/placeholder id is a red flag it is not wired.
4. **Self-contained tasks; two-strike circuit breaker** (fail twice → STOP + note, don't loop). Weak long-context recall: repeat needed context per task.
5. **No stubs/placeholders/`unimplemented!()`** in committed code.
6. **🔴 GATE-INTEGRITY** (ledger §B-10). You may **NOT** change any `= "error"` severity in `layers.toml` to `"warn"`, add `--warn-only`, `|| true`, `--no-verify`, or narrow a gate to make it pass. `forbidden_pattern` is `error` — keep it. If a gate is red at baseline for reasons unrelated to your change, **STOP and report**.
7. **No unplanned shared-config edits** (ledger §B-2). Do NOT edit `layers.toml`, `where-things-live.md`, or another crate's `Cargo.toml` unless a task says so. This plan adds NO new crate, so no layer/WTL rows are needed.
8. **Branch isolation** (ledger §B-3). Work on a branch off CURRENT `origin/main` containing ONLY this plan's commits.
9. **Delivery manifest** (ledger §B-4). In the final ledger fill (§C), list EVERY file you changed, including config.
10. **Vox policy:** `.vox` automation only (never `.ps1`/`.sh`/`.py`); never `cargo fmt --all` (use `cargo fmt -p <crate>`); `.md` under `docs/src/` needs YAML frontmatter; secrets via `vox_secrets::resolve_secret` (never `std::env::var`).
11. **Verification ritual before each commit (PASTE output):** `cargo test -p <crate>`, `cargo clippy -p <crate> -- -D warnings`, `cargo fmt -p <crate>`, `cargo check -p <crate>`.
12. **Windows build note:** if a build fails to relink only because `target/...exe` is RA-locked (`os error 5`), retry with `CARGO_TARGET_DIR=target/iso cargo <cmd> -p <crate>`.
13. Each task is `[PARALLEL-SAFE]` or `[SEQUENTIAL]`; never two subagents on one file. Waves declared per phase.

## B. Prereq gate (STOP if any fails)
- [ ] Plan A: `rg -n "pub base|enum TrainMethod|pub struct SpokeRouter|fn load_domain_profile|fn load\b" crates/vox-populi/src/mens/tensor/domain_profiles.rs` → all present.
- [ ] Plan B: `vox ci spoke-check` exits 0; `mens/config/eval-gates-rust.yaml` + `eval-gates-agents.yaml` exist.
- [ ] Guard intact: `rg -n '^forbidden_pattern' docs/src/architecture/layers.toml` shows `= "error"` (NOT `"warn"`). If `"warn"`, STOP — fix the AGH-0008 regression first.
- [ ] Baseline green: `cargo run -p vox-arch-check` exits 0; `cargo check -p vox-populi -p vox-ml-cli` compiles. If red for unrelated reasons, STOP + report (do not "fix" by weakening anything).

## C. Ledger automation (open at start, fill at end)
This plan's outcome MUST be recorded in [`../antigravity-handoff-ledger.md`](../antigravity-handoff-ledger.md) §C.
- **If executed via `agy` delegation:** `crates/vox-orchestrator-mcp/src/agy_ledger.rs` (`append_entry_locked`) auto-appends a base entry (subsystem/task/outcome/exit/files). You still must enrich it (below).
- **If executed manually in Antigravity:** allocate the next id (it will be **AGH-0010** unless taken) and append a `# --- AGH-0010 ---` YAML block per the §C schema with `plan`, `prompt_artifact`, `claude_inputs`, `subsystem: "VoxMens Split C — convergent selection/routing"`.
- [ ] **First task of the run:** open the entry (id + plan + prompt fields).
- [ ] **Final task (Phase 4.3):** fill `delivered[]` (the delivery manifest — every file changed, §B-4), `outcome`, `verification{tests,clippy,arch_check,spoke_check}`, `errors_encountered[]`, `agent_deviations[]`, `commits[]`. Leave `review_findings`/`verdict`/`prompt_lessons` for the Claude review pass. Do NOT run `vox ci handoff-ledger` (it doesn't exist).

---

## Phase 0 — Confirm the reuse seams (read-only, anti-hallucination)
**Wave 0.1:** Task 0.1 (`[SEQUENTIAL]`).
### Task 0.1: Verify every reused symbol [SEQUENTIAL]
**Files:** none.
- [ ] **Step 1:** Run; each must match:
```bash
rg -n "pub fn get_system_vram_gb|pub fn auto_preset" crates/vox-populi/src/mens/tensor/vram_autodetect.rs
rg -n "pub struct DomainRouter|pub fn route\b|pub fn register" crates/vox-populi/src/mens/tensor/domain_router.rs
rg -n "AdapterMethodRegistry|pub fn resolve\b|default_kernel|enum AdapterMethod" crates/vox-populi/src/mens/tensor/finetune_registry.rs crates/vox-populi/src/mens/tensor/finetune_contract.rs
rg -n "pub fn load\b|pub struct EffectiveDomainProfile|pub struct SpokeBase|pub struct SpokeRouter|enum TrainMethod|pub fn validate" crates/vox-populi/src/mens/tensor/domain_profiles.rs crates/vox-populi/src/mens/tensor/spoke_validate.rs
rg -n "PipelineStage::Train|let target_model|run_train\(|profile: Option" crates/vox-ml-cli/src/commands/mens/pipeline.rs
rg -n "profile: Option<String>" crates/vox-ml-cli/src/commands/mens/populi/action_populi_enum.rs
rg -n "^presets:|max_vram_mb|^gpus:" mens/config/gpu-specs.yaml
```
Any miss → STOP + handoff note (codebase drifted). No commit.

---

## Phase 1 — Spoke base resolver (capability + VRAM fit)
**Wave 1.1 (`[SEQUENTIAL]`):** 1.1 → 1.2 → 1.3 → 1.4 → 1.5.

### Task 1.1: `train_bases:` overlay in `gpu-specs.yaml` (keys MUST match live profiles) [SEQUENTIAL]
**Files:** Modify `mens/config/gpu-specs.yaml`.
- [ ] **Step 1:** Confirm live tags: `rg -n "model:" mens/config/domain-profiles.yaml` → expect `small_code_default`, `strong_code_default`, `agentic_default`. **The overlay keys below MUST equal these exactly** (no profile edits needed).
- [ ] **Step 2:** Append to `gpu-specs.yaml`:
```yaml
# Training base candidates per capability tag (referenced by domain-profiles base.model).
# floor_mb = approx QLoRA VRAM floor on the standard preset. Resolver picks the largest
# variant whose floor_mb <= detected VRAM. methods = train methods this base supports.
# Keys match domain-profiles.yaml base.model values exactly.
train_bases:
  small_code_default:
    - { hf_id: "Qwen/Qwen2.5-Coder-3B-Instruct", floor_mb: 6000, methods: [qlora, full_sft] }
  strong_code_default:
    - { hf_id: "Qwen/Qwen2.5-Coder-7B-Instruct", floor_mb: 11000, methods: [qlora, dpo, orpo] }
  agentic_default:
    - { hf_id: "Qwen/Qwen2.5-Coder-7B-Instruct", floor_mb: 11000, methods: [qlora, dpo] }
```
(`Qwen2.5-Coder` matches the existing `DEFAULT_MODEL_ID`; ids/floors are the starting policy.)
- [ ] **Step 3: Parse-regression guard** — the existing `TimeEstimator` parses `gpu-specs.yaml`. Confirm the new key doesn't break it: `CARGO_TARGET_DIR=target/iso cargo test -p vox-populi estimator 2>&1 | tail -5` (or any test that loads gpu-specs). Expected: PASS (serde ignores unknown keys; verified no `deny_unknown_fields`). If it FAILS, STOP — do not delete the key; report.
- [ ] **Step 4: Commit** `feat(mens): train_bases overlay in gpu-specs (tag->fine-tunable bases + VRAM floor)`

### Task 1.2: Pure resolver `pick_base` [SEQUENTIAL]
**Files:** Create `crates/vox-populi/src/mens/tensor/spoke_base_resolver.rs`; Modify `.../tensor/mod.rs`; Test inline.
- [ ] **Step 1: Write the failing test + types** (same as the prior plan revision):
```rust
//! Resolve a spoke capability tag -> concrete fine-tunable base that fits VRAM.
//! Overlay source: `train_bases:` in mens/config/gpu-specs.yaml. Pure core +
//! a thin disk loader; reuses vram_autodetect for the live VRAM number.
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct TrainBase { pub hf_id: String, pub floor_mb: u32, #[serde(default)] pub methods: Vec<String> }

/// Largest candidate for `tag` whose `floor_mb <= vram_mb`. Errors if the tag is
/// unknown or nothing fits (fail-closed — never silently pick a too-big base).
pub fn pick_base<'a>(overlay: &'a HashMap<String, Vec<TrainBase>>, tag: &str, vram_mb: u32) -> anyhow::Result<&'a TrainBase> {
    let candidates = overlay.get(tag)
        .ok_or_else(|| anyhow::anyhow!("unknown base tag '{tag}' (not in gpu-specs train_bases)"))?;
    candidates.iter().filter(|b| b.floor_mb <= vram_mb).max_by_key(|b| b.floor_mb)
        .ok_or_else(|| anyhow::anyhow!("no '{tag}' base fits {vram_mb}MB VRAM"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn overlay() -> HashMap<String, Vec<TrainBase>> {
        let mut m = HashMap::new();
        m.insert("strong_code_default".into(), vec![
            TrainBase{hf_id:"small".into(),floor_mb:6000,methods:vec!["qlora".into()]},
            TrainBase{hf_id:"big".into(),floor_mb:11000,methods:vec!["qlora".into()]},
        ]); m
    }
    #[test] fn picks_largest_that_fits(){ assert_eq!(pick_base(&overlay(),"strong_code_default",16384).unwrap().hf_id,"big"); assert_eq!(pick_base(&overlay(),"strong_code_default",8000).unwrap().hf_id,"small"); }
    #[test] fn errors_when_none_fit(){ assert!(pick_base(&overlay(),"strong_code_default",4000).is_err()); }
    #[test] fn errors_unknown_tag(){ assert!(pick_base(&overlay(),"nope",16384).is_err()); }
}
```
- [ ] **Step 2:** `rg -n "pub mod domain_router" crates/vox-populi/src/mens/tensor/mod.rs`; add `pub mod spoke_base_resolver;`.
- [ ] **Step 3:** `cargo test -p vox-populi spoke_base_resolver` → FAIL then PASS (3).
- [ ] **Step 4: Commit** `feat(mens): pure VRAM-fit base resolver over train_bases overlay`

### Task 1.3: Disk loader + resolve (with explicit no-GPU handling) [SEQUENTIAL]
**Files:** Modify `spoke_base_resolver.rs`; Test inline.
- [ ] **Step 1: Verify** `rg -n "fn get_system_vram_gb" crates/vox-populi/src/mens/tensor/vram_autodetect.rs`.
- [ ] **Step 2: Add loader + resolve** — note the `RequiresVram` distinction for §E:
```rust
#[derive(Debug, Deserialize)]
struct GpuSpecsTrainBases { #[serde(default)] train_bases: HashMap<String, Vec<TrainBase>> }

pub fn load_overlay(root: &std::path::Path) -> anyhow::Result<HashMap<String, Vec<TrainBase>>> {
    let p = root.join("mens/config/gpu-specs.yaml");
    let s = std::fs::read_to_string(&p).map_err(|e| anyhow::anyhow!("read {}: {e}", p.display()))?;
    let parsed: GpuSpecsTrainBases = serde_yaml::from_str(&s)
        .map_err(|e| anyhow::anyhow!("parse train_bases in gpu-specs.yaml: {e}"))?;
    Ok(parsed.train_bases)
}

/// Resolve `base.model` to a concrete HF id.
/// - concrete id (contains '/') -> pass-through (no VRAM needed).
/// - capability tag -> overlay + VRAM fit.
/// `vram_mb_override`: Some(v) for tests / known hosts; None -> vram_autodetect.
/// On None VRAM with a tag, returns Err (fail-closed) — callers that must NOT
/// require a GPU (e.g. --skip-train dry-runs) should treat Err as "defer to the
/// existing default-model path" rather than aborting (see Phase 2 / §E).
pub fn resolve_base_model(root: &std::path::Path, base_model: &str, vram_mb_override: Option<u32>) -> anyhow::Result<String> {
    if base_model.contains('/') { return Ok(base_model.to_string()); }
    let overlay = load_overlay(root)?;
    let vram_mb = match vram_mb_override {
        Some(v) => v,
        None => {
            let gb = crate::mens::tensor::vram_autodetect::get_system_vram_gb()
                .ok_or_else(|| anyhow::anyhow!("no GPU VRAM detected; cannot size base tag '{base_model}'"))?;
            (gb * 1024.0) as u32
        }
    };
    Ok(pick_base(&overlay, base_model, vram_mb)?.hf_id.clone())
}
```
Adjust the `vram_autodetect` path exactly to Step 1.
- [ ] **Step 3: Tests** (effect over the REAL repo overlay + pass-through + tag-not-found):
```rust
    #[test] fn resolves_repo_tag_with_injected_vram() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).unwrap();
        let id = resolve_base_model(root, "strong_code_default", Some(16384)).unwrap();
        assert!(id.contains("Qwen"), "got {id}");
    }
    #[test] fn concrete_id_passthrough_needs_no_vram() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).unwrap();
        assert_eq!(resolve_base_model(root, "org/My-Model", None).unwrap(), "org/My-Model");
    }
```
- [ ] **Step 4:** `cargo test -p vox-populi spoke_base_resolver` → PASS.
- [ ] **Step 5: Commit** `feat(mens): train_bases loader + VRAM-aware resolve_base_model (concrete-id passthrough)`

---

## Phase 2 — Wire base.model + base.method into the Train stage
**Wave 2.1 (`[SEQUENTIAL]`):** 2.1 → 2.2.

### Task 2.1: Resolve the profile's base model (GPU-optional) [SEQUENTIAL]
**Files:** Modify `crates/vox-ml-cli/src/commands/mens/pipeline.rs` (Train arm only).
- [ ] **Step 1: Read the Train arm** — `rg -n "PipelineStage::Train|let target_model|run_train\(|let ws\b|find_workspace_root" crates/vox-ml-cli/src/commands/mens/pipeline.rs`. Note `target_model = model.clone()` precedes `run_train(...)`; `ws` is `vox_corpus::training::contract::find_workspace_root()` (already used in the Mix arm — reuse the same).
- [ ] **Step 2: Resolve base, CLI-override-wins, GPU-optional** — replace `let target_model = model.clone();` with:
```rust
// Per-spoke base model: CLI --model wins; else resolve the profile's base.model
// (tag -> VRAM-fit HF id, or concrete id passthrough). On a host with no GPU,
// a TAG cannot be sized — fall back to the existing default-model path rather
// than aborting a --skip-train dry-run (effect, not abort). §E.
let target_model = if model.is_some() {
    model.clone()
} else if let Some(name) = profile.as_deref() {
    let ws = vox_corpus::training::contract::find_workspace_root();
    let root = ws.clone().unwrap_or_else(|| std::path::PathBuf::from("."));
    match vox_populi::mens::tensor::domain_profiles::EffectiveDomainProfile::load_domain_profile(name, ws.as_deref()) {
        Ok(eff) => match eff.base.as_ref().map(|b| b.model.clone()) {
            Some(tag) => match vox_populi::mens::tensor::spoke_base_resolver::resolve_base_model(&root, &tag, None) {
                Ok(id) => Some(id),
                // No GPU to size a tag, or tag unfit: defer to run_train's default-model path.
                Err(e) => { tracing::warn!("spoke '{name}' base unresolved ({e}); using default model"); None }
            },
            None => None, // profile has no base -> default path
        },
        Err(_) => None,
    }
} else { model.clone() };
```
Verify the exact surrounding lines and adapt (do not duplicate an existing `ws` binding).
- [ ] **Step 3:** `cargo check -p vox-ml-cli` (use `CARGO_TARGET_DIR=target/iso` if RA-locked). Compiles.
- [ ] **Step 4: Commit** `feat(mens): pipeline resolves per-spoke base model (tag->HF id, VRAM-fit; CLI/default fallbacks)`

### Task 2.2: Dispatch backend from base.method via AdapterMethodRegistry [SEQUENTIAL]
**Files:** Modify `pipeline.rs` (Train arm).
- [ ] **Step 1: Verify the SSOT** — `rg -n "AdapterMethodRegistry|pub fn resolve|default_kernel|enum AdapterMethod" crates/vox-populi/src/mens/tensor/finetune_registry.rs crates/vox-populi/src/mens/tensor/finetune_contract.rs`. Confirm `AdapterMethod = {Lora, Qlora}` and `resolve(m) -> Option<&AdapterMethodRecord{default_kernel: PopuliTrainBackend}>`.
- [ ] **Step 2: Map TrainMethod -> backend via the registry** (reuse the SSOT; do not hand-roll the kernel choice). Replace the hard-coded `PopuliTrainBackendCli::Qlora.into()` first arg of `run_train` with `backend`:
```rust
use vox_populi::mens::tensor::domain_profiles::TrainMethod;
use vox_populi::mens::tensor::finetune_contract::AdapterMethod;
use vox_populi::mens::tensor::finetune_registry::AdapterMethodRegistry;
let method = profile.as_deref()
    .and_then(|n| vox_populi::mens::tensor::domain_profiles::EffectiveDomainProfile::load_domain_profile(n, vox_corpus::training::contract::find_workspace_root().as_deref()).ok())
    .and_then(|e| e.base.map(|b| b.method))
    .unwrap_or(TrainMethod::Qlora);
let backend = match method {
    // Qlora is the only TrainMethod that maps to a real AdapterMethod+kernel today.
    TrainMethod::Qlora => AdapterMethodRegistry::builtin().resolve(AdapterMethod::Qlora)
        .map(|r| r.default_kernel)
        .ok_or_else(|| anyhow::anyhow!("AdapterMethodRegistry missing Qlora kernel"))?,
    // Full SFT / preference-tuning (DPO/ORPO) have no wired training backend.
    TrainMethod::FullSft | TrainMethod::Dpo | TrainMethod::Orpo =>
        anyhow::bail!("training method {:?} has no wired backend; wire a kernel before selecting it in domain-profiles.yaml", method),
    TrainMethod::RagOnly | TrainMethod::PromptOnly => {
        tracing::info!("spoke uses {:?}; skipping training stage", method); continue;
    }
};
```
Verify `run_train`'s first param type is `PopuliTrainBackend` (it is) and that `PopuliTrainBackendCli::Qlora.into()` currently produces it; `backend` is already that type.
- [ ] **Step 3:** `cargo check -p vox-ml-cli`. Compiles.
- [ ] **Step 4: Commit** `feat(mens): dispatch train backend from base.method via AdapterMethodRegistry (rag/prompt skip; unwired fail-closed)`

---

## Phase 3 — Lane-tag routing on the existing DomainRouter
**Wave 3.1 (`[SEQUENTIAL]`):** 3.1.
### Task 3.1: `route_by_signal` (triggers + priority, deterministic) [SEQUENTIAL]
**Files:** Modify `crates/vox-populi/src/mens/tensor/domain_router.rs`; Test inline.
- [ ] **Step 1: Verify** `rg -n "DomainProfilesFile|pub profiles|struct SpokeRouter|triggers|priority" crates/vox-populi/src/mens/tensor/domain_profiles.rs`.
- [ ] **Step 2: Add the fn** (same logic as the prior revision — substring match, priority desc, name asc tie-break):
```rust
use crate::mens::tensor::domain_profiles::DomainProfilesFile;

/// Pick the spoke whose router.triggers substring-match `signal`, by highest
/// priority, breaking ties on the lexicographically smaller spoke name
/// (profiles is a HashMap -> name tie-break required for determinism). A
/// trigger's leading `*` is stripped before matching.
pub fn route_by_signal(file: &DomainProfilesFile, signal: &str) -> Option<String> {
    let mut best: Option<(i32, &str)> = None;
    for (name, p) in &file.profiles {
        let Some(r) = &p.router else { continue };
        let hit = r.triggers.iter().any(|t| { let n = t.trim_start_matches('*'); !n.is_empty() && signal.contains(n) });
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
    fn file() -> DomainProfilesFile { serde_yaml::from_str(r#"
profiles:
  rust-expert: { description: x, router: { triggers: ["*.rs", "lane:vox_rust_authoring"], priority: 10 } }
  agents:      { description: x, router: { triggers: ["lane:vox_tooling"], priority: 5 } }
"#).unwrap() }
    #[test] fn rs_routes_rust(){ assert_eq!(route_by_signal(&file(),"src/main.rs").as_deref(),Some("rust-expert")); }
    #[test] fn tool_routes_agents(){ assert_eq!(route_by_signal(&file(),"lane:vox_tooling x").as_deref(),Some("agents")); }
    #[test] fn no_match_none(){ assert_eq!(route_by_signal(&file(),"zzz"),None); }
    #[test] fn equal_priority_name_tiebreak(){ let f: DomainProfilesFile = serde_yaml::from_str("profiles:\n  zeta:  { description: x, router: { triggers: [\"x\"], priority: 5 } }\n  alpha: { description: x, router: { triggers: [\"x\"], priority: 5 } }\n").unwrap(); for _ in 0..20 { assert_eq!(route_by_signal(&f,"x").as_deref(),Some("alpha")); } }
}
```
Comment that substring `.rs` also matches `foo.rsync` — acceptable v1.
- [ ] **Step 3:** `cargo test -p vox-populi route_by_signal` → FAIL then PASS (4).
- [ ] **Step 4: Commit** `feat(mens): DomainRouter::route_by_signal (triggers+priority, deterministic)`

---

## Phase 4 — Validation, end-to-end (effect), serving doc, ledger fill
**Wave 4.1 (`[SEQUENTIAL]`):** 4.1 → 4.2 → 4.3.

### Task 4.1: Extend `spoke_validate` (base.model resolvable) [SEQUENTIAL]
**Files:** Modify `crates/vox-populi/src/mens/tensor/spoke_validate.rs`; Test inline.
- [ ] **Step 1: Verify** `rg -n "pub fn validate|SpokeViolation|fine_tune|base" crates/vox-populi/src/mens/tensor/spoke_validate.rs`.
- [ ] **Step 2:** For each spoke with a `base`, add a violation unless `base.model` is EITHER a concrete id (`contains('/')`) OR a key present in `spoke_base_resolver::load_overlay(root)`. Unit test: bogus tag → violation; a real live tag (`strong_code_default`) → none.
- [ ] **Step 3:** `cargo test -p vox-populi spoke_validate` → PASS. Then **`vox ci spoke-check` exits 0** (the live `*_default` tags now resolve against the overlay from Task 1.1; if any profile tag isn't in the overlay, add it to the overlay — do NOT loosen the check).
- [ ] **Step 4: Commit** `feat(mens): spoke-check validates base.model resolves (overlay tag or concrete id)`

### Task 4.2: End-to-end EFFECT dry-run (all three spokes) + serving doc [SEQUENTIAL]
**Files:** Create `docs/src/architecture/voxmens-serving-topology-decision-2026-06-19.md` (YAML frontmatter).
- [ ] **Step 1: Full gates** — `cargo test -p vox-populi -p vox-ml-cli`; `cargo run -p vox-arch-check` (must stay green at `forbidden_pattern = error`); `vox ci spoke-check` exits 0. PASTE output.
- [ ] **Step 2: EFFECT dry-run per spoke** (proves the resolved id flows through, §B-9). The flag is `--profile` (EXISTS). Find the exact subcommand: `rg -n "Pipeline|profile: Option" crates/vox-ml-cli/src/commands/mens/populi/action_populi_enum.rs` then run the dry-run for each spoke, e.g.:
```
vox mens populi pipeline --profile vox-lang    --skip-train --stages generate,extract,validate,pairs,mix,eval
vox mens populi pipeline --profile rust-expert --skip-train --stages generate,extract,validate,pairs,mix,eval
vox mens populi pipeline --profile agents      --skip-train --stages generate,extract,validate,pairs,mix,eval
```
(Use the real subcommand path from the rg.) Expected: each resolves its base model + method + eval gate with no error. On a no-GPU host, a tag falls back to the default model (logged) — that is correct (§E), NOT a failure. If a spoke is `RagOnly`/`PromptOnly` the Train stage is skipped (but it's not in the current three; all are `qlora`).
- [ ] **Step 3: Serving-topology doc** (`category: "architecture"`): record that **training base selection is local/offline** (no inference-egress serving topology needed to fine-tune); **inference of trained adapters reuses `domain_router`** (adapter hot-swap by domain on a shared base); heterogeneous bases would need per-base adapter sets (note the constraint). No S-LoRA dependency decision is required for the training pipeline. Cross-link the convergence spec §5 boundary.
- [ ] **Step 4: Commit** `docs(mens): serving-topology decision + Split C convergent e2e validated`

### Task 4.3: Fill the ledger entry (delivery manifest + outcome) [SEQUENTIAL]
**Files:** Modify `docs/superpowers/antigravity-handoff-ledger.md` (§C, the AGH-0010 block opened in §C).
- [ ] **Step 1:** Fill `delivered:` with EVERY file changed across Phases 1–4 (the delivery manifest, §B-4): `mens/config/gpu-specs.yaml`, `crates/vox-populi/src/mens/tensor/{spoke_base_resolver.rs,mod.rs,domain_router.rs,spoke_validate.rs}`, `crates/vox-ml-cli/src/commands/mens/pipeline.rs`, `docs/src/architecture/voxmens-serving-topology-decision-2026-06-19.md`.
- [ ] **Step 2:** Fill `outcome` (green|partial), `verification{tests, clippy, arch_check, spoke_check}`, `errors_encountered[]` (anything you hit + root cause + who), `agent_deviations[]` (any place you departed from this plan + why), `commits[]`. Leave `review_findings`/`verdict`/`prompt_lessons` for the Claude review pass.
- [ ] **Step 3: Commit** `docs(ledger): AGH-0010 — Split C convergent delivery (self-report)`

---

## E. Scenarios & edge cases (handle, don't be surprised)
| Scenario | Required behavior | Where |
|---|---|---|
| **No GPU / CPU-only host** (`get_system_vram_gb()→None`) | A capability TAG can't be sized → `resolve_base_model` Errs; pipeline **falls back to run_train's default-model path**, does NOT abort `--skip-train`. | T1.3, T2.1 |
| **CLI `--model X` + a profile** | `--model` wins, no resolution, no VRAM needed. | T2.1 |
| **Profile has no `base`** | Resolution returns None → default-model path. | T2.1 |
| **`base.model` is a concrete id** (`Qwen/...`) | Pass-through, no overlay/VRAM. | T1.3 |
| **`base.model` tag absent from overlay** (typo) | `spoke-check` fails at CI (T4.1); at runtime resolution Errs → default path + warn. | T4.1, T2.1 |
| **VRAM too small for any candidate** | `pick_base` Errs ("none fit") → fail-closed (training) / default path (dry-run). | T1.2, T2.1 |
| **`RagOnly`/`PromptOnly` spoke** | Train stage `continue`s (skipped); earlier stages (mix/eval) still run for corpus building. | T2.2 |
| **DPO/ORPO/FullSft method selected** | `anyhow::bail!` — no wired backend (don't silently QLoRA). | T2.2 |
| **gpu-specs.yaml parser** | New `train_bases:` key ignored by `GpuSpecsFile` (no `deny_unknown_fields`); regression-checked. | T1.1 |
| **Two spokes, equal router priority** | Deterministic name tie-break. | T3.1 |
| **Concurrent session edits `pipeline.rs`** | Pull latest before editing; Train arm is small + localized. If `pipeline::run` lost the `profile` param, STOP + report. | T2.* |
| **arch-check red at baseline (unrelated)** | STOP + report; never downgrade `forbidden_pattern`. | §A.6, B |

## F. Green boundary (initiative complete)
- Spoke resolves `base.model` (tag→VRAM-fit HF id / concrete id), unit-tested with injected VRAM; no-GPU falls back gracefully.
- `base.method` drives the backend via `AdapterMethodRegistry` (rag/prompt skip; unwired fail-closed).
- `DomainRouter::route_by_signal` resolves signal→spoke deterministically; tested.
- `vox ci spoke-check` + `cargo test` green; `forbidden_pattern` stays `error`; e2e `--profile` dry-run succeeds for all three spokes (effect proven).
- AGH-0010 ledger entry opened + filled with a delivery manifest.
- **No** new model-registry/resolver/router; the inference `select()`/egress stack untouched.

## G. Launch statement (copy-paste into Antigravity)
```
Execute docs/superpowers/plans/2026-06-19-voxmens-split-C-convergent.md in this vox repo, task by task.
- Check out a branch off current origin/main with ONLY this plan's commits.
- Read §A (execution rules) and §E (edge cases) first; obey them exactly.
- Every task ends compiling + tested + committed (atomic-green). Verify-before-use: run each task's rg/read step and inline the REAL signature; if a symbol is missing, STOP and write a handoff note — do not invent.
- Prove EFFECT not shape: the Phase 4 --profile dry-run must actually resolve each spoke's base model through the pipeline.
- DO NOT weaken any gate: never change a layers.toml `= "error"` to `"warn"`, never add --warn-only/|| true/--no-verify. forbidden_pattern stays "error". If arch-check is red at baseline for unrelated reasons, STOP and report.
- DO NOT edit layers.toml / where-things-live.md / other crates' Cargo.toml (this plan adds no crate).
- Before each commit paste: cargo test -p <crate>; cargo clippy -p <crate> -- -D warnings; cargo fmt -p <crate>; cargo check -p <crate>. Never cargo fmt --all. If target/*.exe is RA-locked (os error 5), use CARGO_TARGET_DIR=target/iso.
- Open ledger entry AGH-0010 (§C) at start; fill it with a full delivery manifest + outcome at the end (Phase 4.3).
- Two-strike: if a verification fails twice, STOP and surface a handoff note; do not loop.
Goal: per-spoke base-model selection + method dispatch + lane routing, converged onto existing infra (no new registry/resolver/router), all gates green.
```

## H. Spec-coverage check
- Converge (no 3rd system) → P1–P3 reuse vram_autodetect / domain_router / AdapterMethodRegistry + overlay. ✓
- Real gaps: base resolver (P1), method dispatch (P2), trigger routing (P3), validation+e2e (P4). ✓
- Dropped: model-registry.yaml, model_registry.rs, router.rs, detect_available_vram_mb. ✓
- Gemini-hardened: verify-before-use, effect-not-shape, two-strike, gate-integrity, branch isolation, delivery manifest, no-GPU scenario, parse-regression guard. ✓
- Ledger automation: open + fill AGH-0010; agy_ledger auto-append noted; handoff-ledger lint absence noted. ✓
- Boundary documented (P4.3 + spec §5). ✓
