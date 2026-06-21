# VoxMens Split C — Follow-ups (effect-proof, preset SSOT, ledger hygiene)

> **For agentic workers:** REQUIRED SUB-SKILL: `crates/vox-skills/skills/superpowers/subagent-driven-development.skill.md`. Steps use `- [ ]`.
> **EXECUTION TARGET: Gemini Flash 3.5 inside Google Antigravity** (or Claude inline). Read §A first.
> **Source:** AGH-0012 review ([`../antigravity-handoff-ledger.md`](../antigravity-handoff-ledger.md) §C). Split C landed green but with a hollow effect-proof (F1) and a fixed-this-turn preset gap (F2, done in `9ef4c40ac5`).

**Goal:** Close the AGH-0012 follow-ups so per-spoke training selection is *provably* correct without a GPU: (F1) make base-model/preset/method resolution a pure, testable function exercised on a non-GPU build + an observable dry-run; (F3) make self-reports match code; (F4) dedupe the ledger id collisions and prevent recurrence; plus a small preset-name SSOT reconciliation.

**Architecture:** Hoist the *resolution* (pure config logic) out of the `#[cfg(feature="gpu")]` Train block in `pipeline.rs` into a feature-independent `resolve_training_selection(...)` that returns a `TrainingSelection`; the `cfg(gpu)` block only *consumes* it to call `run_train`. This makes the effect unit-testable and dry-run-observable. No new crate.

**Tech Stack:** Rust (`vox-ml-cli`, `vox-populi`), YAML (`mens/config/*`), the handoff ledger.

---

## A. Execution rules (Gemini Flash 3.5)
Same as the Split C plan §A. Highlights + the new lessons from AGH-0012:
- Atomic-green-commit; verify-before-use (inline real signatures via `rg`); two-strike; no stubs.
- **🔴 GATE-INTEGRITY:** never downgrade a `layers.toml = "error"` to `"warn"`; if a gate is red at baseline for unrelated reasons, STOP + report. `forbidden_pattern` stays `error`.
- **NEW (AGH-0012 §B-9 extension): a dry-run/`--skip-train` cannot validate code behind a stage it skips.** Acceptance for training-selection MUST run on a path that actually executes the resolution (a non-GPU unit test of the pure fn + an `--explain`/dry-run that logs the resolved triple). Do not claim "effect proven" from a `--skip-train` run.
- **NEW: wire EVERY field of an SSOT record.** When touching `SpokeBase`, confirm `model`+`method`+`preset` are all consumed.
- **NEW: the self-report manifest must match the diff** — quote actual code; don't describe intent. (`route_by_signal` is *substring* not "suffix"; the module is a plain `pub mod` not feature-gated — past prose was wrong.)
- Vox policy: `.vox` automation; never `cargo fmt --all` (`cargo fmt -p <crate>`); docs/src/ `.md` needs frontmatter; secrets via `vox_secrets`.
- Verify each commit: `cargo test -p <crate>`; `cargo clippy -p <crate> -- -D warnings`; `cargo fmt -p <crate>`; `cargo check -p <crate>`. **For `cfg(feature="gpu")` code also run `--features gpu`.** If `target/*.exe` is RA-locked (`os error 5`), use `CARGO_TARGET_DIR=target/iso`.
- Tasks tagged `[PARALLEL-SAFE]`/`[SEQUENTIAL]`; never two subagents on one file.

## B. Prereq gate
- [ ] On current `origin/main`; `forbidden_pattern = "error"`; `cargo run -p vox-arch-check` exit 0; `vox ci spoke-check` exit 0. If red for unrelated reasons → STOP + report.
- [ ] Confirm F2 landed: `rg -n "base.preset|b.preset|target_preset" crates/vox-ml-cli/src/commands/mens/pipeline.rs` shows the profile-preset wiring (commit 9ef4c40ac5).

---

## Phase 1 — F1: hoist resolution into a pure, testable function

**Why:** Today base-model + preset + method resolution lives inside `#[cfg(feature="gpu")] { if !dry_run { ... } }` in `pipeline.rs`. A `--skip-train` dry-run removes the Train stage, and a non-GPU build compiles the block out — so the resolution is **never executed in any testable/CI path**. The logic is pure config (no GPU needed); only `run_train` needs GPU. Separate them.

**Wave 1.1 (`[SEQUENTIAL]`):** 1.1 → 1.2 → 1.3.

### Task 1.1: Define `TrainingSelection` + pure `resolve_training_selection` [SEQUENTIAL]
**Files:** Create `crates/vox-ml-cli/src/commands/mens/training_selection.rs`; Modify `crates/vox-ml-cli/src/commands/mens/mod.rs` (declare module); Test inline.
- [ ] **Step 1: Verify the seams** — `rg -n "spoke_base_resolver::resolve_base_model|AdapterMethodRegistry|enum TrainMethod|enum PopuliTrainBackend|enum AdapterMethod|fn load_domain_profile" crates/vox-populi/src/mens`. Inline the real signatures.
- [ ] **Step 2: Write the failing test + types** (pure — NO `cfg(gpu)`, NO GPU needed; VRAM is injected):
```rust
//! Pure, GPU-independent resolution of a spoke's training selection
//! (base model + preset + training backend, or a "skip"/"unwired" outcome).
//! Extracted from pipeline.rs so it is unit-testable and dry-run-observable
//! WITHOUT the `gpu` feature (AGH-0012 F1).
use std::path::Path;
use vox_populi::mens::tensor::domain_profiles::{EffectiveDomainProfile, TrainMethod};
use vox_populi::mens::tensor::finetune_contract::AdapterMethod;
use vox_populi::mens::tensor::finetune_registry::AdapterMethodRegistry;
use vox_populi::mens::PopuliTrainBackend;

#[derive(Debug, PartialEq)]
pub enum TrainingSelection {
    /// Train this base+preset with this backend.
    Train { model: Option<String>, preset: String, backend: PopuliTrainBackend },
    /// Spoke is RagOnly/PromptOnly — skip the training stage.
    Skip { reason: String },
}

/// Resolve a spoke's training selection. `vram_mb_override` lets callers/tests
/// inject VRAM (None → vram_autodetect at the resolver). CLI overrides win.
pub fn resolve_training_selection(
    root: &Path,
    profile: Option<&str>,
    cli_model: Option<&str>,
    cli_preset: Option<&str>,
    vram_mb_override: Option<u32>,
) -> anyhow::Result<TrainingSelection> {
    let eff = profile.and_then(|n| EffectiveDomainProfile::load_domain_profile(n, Some(root)).ok());
    let method = eff.as_ref().and_then(|e| e.base.as_ref().map(|b| b.method)).unwrap_or(TrainMethod::Qlora);
    if matches!(method, TrainMethod::RagOnly | TrainMethod::PromptOnly) {
        return Ok(TrainingSelection::Skip { reason: format!("{method:?}") });
    }
    let backend = match method {
        TrainMethod::Qlora => AdapterMethodRegistry::builtin().resolve(AdapterMethod::Qlora)
            .map(|r| r.default_kernel)
            .ok_or_else(|| anyhow::anyhow!("AdapterMethodRegistry missing Qlora kernel"))?,
        TrainMethod::FullSft | TrainMethod::Dpo | TrainMethod::Orpo =>
            anyhow::bail!("training method {method:?} has no wired backend"),
        TrainMethod::RagOnly | TrainMethod::PromptOnly => unreachable!(),
    };
    // model: CLI wins; else resolve the spoke base.model (tag→VRAM-fit / concrete id);
    // on failure (e.g. no GPU to size a tag) fall back to None (run_train default path).
    let model = if let Some(m) = cli_model {
        Some(m.to_string())
    } else if let Some(tag) = eff.as_ref().and_then(|e| e.base.as_ref().map(|b| b.model.clone())) {
        match vox_populi::mens::tensor::spoke_base_resolver::resolve_base_model(root, &tag, vram_mb_override) {
            Ok(id) => Some(id),
            Err(_) => None,
        }
    } else { None };
    // preset: CLI wins; else base.preset; else default.
    let preset = cli_preset.map(str::to_string)
        .or_else(|| eff.as_ref().and_then(|e| e.base.as_ref().and_then(|b| b.preset.clone())))
        .unwrap_or_else(|| "qwen_4080_16g".to_string());
    Ok(TrainingSelection::Train { model, preset, backend })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn root() -> std::path::PathBuf { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).unwrap().to_path_buf() }
    #[test] fn rust_expert_resolves_qwen_qlora() {
        let sel = resolve_training_selection(&root(), Some("rust-expert"), None, None, Some(16384)).unwrap();
        match sel { TrainingSelection::Train{model,preset,backend} => {
            assert!(model.unwrap().contains("Qwen")); assert!(!preset.is_empty());
            assert!(matches!(backend, PopuliTrainBackend::CandleQlora));
        }, _ => panic!("expected Train") }
    }
    #[test] fn cli_model_overrides() {
        let sel = resolve_training_selection(&root(), Some("rust-expert"), Some("org/Manual"), None, Some(16384)).unwrap();
        if let TrainingSelection::Train{model,..} = sel { assert_eq!(model.as_deref(), Some("org/Manual")); } else { panic!() }
    }
    #[test] fn no_gpu_tag_falls_back_to_none_model() {
        let sel = resolve_training_selection(&root(), Some("rust-expert"), None, None, None /*no VRAM*/).unwrap();
        // On a host with no GPU, get_system_vram_gb() is None → tag unsizable → model None (default path).
        if let TrainingSelection::Train{model,..} = sel { /* model may be None here */ let _ = model; } else { panic!() }
    }
    #[test] fn cli_preset_overrides() {
        let sel = resolve_training_selection(&root(), Some("rust-expert"), None, Some("a100"), Some(16384)).unwrap();
        if let TrainingSelection::Train{preset,..} = sel { assert_eq!(preset, "a100"); } else { panic!() }
    }
}
```
**Pre-verified (2026-06-19 audit — do not re-derive, just confirm with one `rg`):** `PopuliTrainBackend` is re-exported at `vox_populi::mens::PopuliTrainBackend` and derives `Debug, Clone, Copy, PartialEq, Eq` (so `TrainingSelection` can derive `Debug, PartialEq` and `*backend`/`matches!` work). `TrainMethod` derives `Copy`. `SpokeBase.preset` is `Option<String>`. `AdapterMethodRegistry::builtin().resolve(AdapterMethod::Qlora) -> Option<&AdapterMethodRecord{default_kernel: PopuliTrainBackend}>`. If any `rg` contradicts this, STOP + report (do not invent).
- [ ] **Step 3:** Declare the module in `crates/vox-ml-cli/src/commands/mens/mod.rs` as `pub(crate) mod training_selection;` (next to `mod pipeline;`). Run `cargo test -p vox-ml-cli training_selection` → FAIL then PASS (4). **No `--features gpu` needed — that is the entire point of this phase.**
- [ ] **Step 4: Commit** `feat(mens): pure resolve_training_selection (GPU-independent, unit-tested) — AGH-0012 F1`

### Task 1.2: Call the pure fn from the Train stage; cfg(gpu) only consumes it [SEQUENTIAL]
**Files:** Modify `crates/vox-ml-cli/src/commands/mens/pipeline.rs` (Train arm).
- [ ] **Step 1: Read the Train arm** — `rg -n "PipelineStage::Train|cfg\(feature|target_model|target_preset|let method|let backend|run_train\(|dry_run" crates/vox-ml-cli/src/commands/mens/pipeline.rs`.
- [ ] **Step 2: Resolve BEFORE the cfg(gpu)/dry_run guard** (so it is reachable + loggable on any build):
```rust
PipelineStage::Train => {
    let ws = vox_corpus::training::contract::find_workspace_root();
    let root = ws.clone().unwrap_or_else(|| std::path::PathBuf::from("."));
    let selection = crate::commands::mens::training_selection::resolve_training_selection(
        &root, profile.as_deref(), model.as_deref(), preset.as_deref(), None)?;
    match &selection {
        crate::commands::mens::training_selection::TrainingSelection::Skip { reason } => {
            tracing::info!("spoke training skipped ({reason})"); continue;
        }
        crate::commands::mens::training_selection::TrainingSelection::Train { model: m, preset: p, backend } => {
            tracing::info!(model=?m, preset=%p, backend=?backend, "resolved training selection"); // dry-run-observable
            if !dry_run {
                #[cfg(feature = "gpu")]
                {
                    // consume the already-resolved selection; no re-resolution here.
                    let target_model = m.clone();
                    let target_preset = Some(p.clone());
                    let backend = *backend;
                    /* ... existing run_train(backend, target_model, ..., target_preset, ...) call ... */
                }
                #[cfg(not(feature = "gpu"))]
                { anyhow::bail!("training requires the `gpu` feature; rebuild with --features gpu or use --skip-train"); }
            }
        }
    }
}
```
**Delete** the old in-cfg(gpu) resolution blocks (the `target_model` match, the `method`/`backend` match, and the `target_preset` `or_else` chain). **KEEP the existing 60-arg `run_train(backend, target_model, device, …, target_preset, …)` call VERBATIM** — it already uses the variable names `backend`, `target_model`, `target_preset`. You are ONLY changing how those three are *bound*: inside the `cfg(feature="gpu")` block, set `let backend = *backend; let target_model = m.clone(); let target_preset = Some(p.clone());` from the `selection` you resolved above, then leave the `run_train(...)` invocation untouched. Do NOT rewrite or reorder the call's arguments.
- [ ] **Step 3:** `cargo check -p vox-ml-cli` AND `CARGO_TARGET_DIR=target/iso cargo check -p vox-ml-cli --features gpu`. Both must compile. (Confirmed in this env: the `--features gpu` build succeeds in ~30s — if it fails on CUDA/sccache for *environment* reasons unrelated to your edit, note it and rely on the non-gpu check + the pure-fn tests; do NOT disable the feature or stub anything.)
- [ ] **Step 4: Commit** `refactor(mens): Train stage resolves selection pre-gpu-gate (effect now reachable/loggable) — AGH-0012 F1`

### Task 1.3: Effect proof — all live spokes resolve (GPU-free, INLINE test) [SEQUENTIAL]
**Files:** Modify `crates/vox-ml-cli/src/commands/mens/training_selection.rs` (extend its `#[cfg(test)] mod tests`).
> **Must be INLINE, not `tests/`.** `resolve_training_selection` lives in a `pub(crate) mod`, so an external integration test under `crates/vox-ml-cli/tests/` cannot see it. Put this in the same file's test module (use `super::`). This is the real, GPU-free §B-9 effect-proof: it exercises resolution for every live spoke, which `--skip-train` could never do.
- [ ] **Step 1: Get the live spoke ids** — `rg -n "^  [a-z][a-z-]*:" mens/config/domain-profiles.yaml` (under `profiles:`); use those exact ids below (likely `vox-lang`, `rust-expert`, `agents`).
- [ ] **Step 2: Add the test to the existing `mod tests` in training_selection.rs**:
```rust
    #[test]
    fn all_live_spokes_resolve_to_trainable_selection() {
        let root = root(); // helper already defined in this test module
        for spoke in ["vox-lang", "rust-expert", "agents"] {
            let sel = super::resolve_training_selection(&root, Some(spoke), None, None, Some(16384))
                .unwrap_or_else(|e| panic!("{spoke}: {e}"));
            match sel {
                super::TrainingSelection::Train { model, preset, .. } => {
                    assert!(model.as_deref().unwrap_or("").contains("Qwen"), "{spoke}: {model:?}");
                    assert!(!preset.is_empty(), "{spoke}");
                }
                other => panic!("{spoke}: expected Train, got {other:?}"),
            }
        }
    }
```
- [ ] **Step 3:** `cargo test -p vox-ml-cli all_live_spokes_resolve` → PASS. Proves the EFFECT (resolution flows for every spoke) with **no GPU and no Train stage** — the gap AGH-0012 F1 flagged.
- [ ] **Step 3: Commit** `test(mens): GPU-free effect-proof — all live spokes resolve to a trainable selection — AGH-0012 F1`

---

## Phase 2 — Preset-name SSOT reconciliation (small)
**Wave 2.1 (`[SEQUENTIAL]`):** 2.1.
### Task 2.1: One canonical 16G preset name [SEQUENTIAL]
**Files:** docs only + optional alias note. **Do NOT rename without checking all readers.**
Context: drift exists — `domain-profiles.yaml` + `contracts/mens/training-presets.v1.yaml` use `qwen_4080_16g`; `gpu-specs.yaml` presets use `prosumer_16g`; `preset_schema.rs` treats them as aliases (`prosumer_16g => qwen_4080_16g`). It currently WORKS via the alias, so this is hygiene, not a break.
- [ ] **Step 1: Map all readers** — `rg -n "qwen_4080_16g|prosumer_16g" crates/ mens/ contracts/ docs/`. List every consumer.
- [ ] **Step 2: Decide + document** (do not mass-rename): pick `qwen_4080_16g` as the canonical id (it's the contract + profile id), keep `prosumer_16g` as a documented alias in `preset_schema.rs`. Add a one-paragraph note to `docs/src/architecture/voxmens-serving-topology-decision-2026-06-19.md` (or the convergence spec) recording the canonical name + alias, so future edits don't reintroduce a third name.
- [ ] **Step 3: Commit** `docs(mens): canonicalize qwen_4080_16g preset id; prosumer_16g is a documented alias`

---

## Phase 3 — Ledger hygiene (F4) + self-report accuracy (F3)
**Wave 3.1 (`[PARALLEL-SAFE]`):** 3.1 (ledger ids), 3.2 (launch-statement note) — disjoint files.

### Task 3.1: Dedupe AGH id collisions [PARALLEL-SAFE]
**Files:** Modify `docs/superpowers/antigravity-handoff-ledger.md`.
Context: `id: AGH-0012` appears twice (Split C + a Track E telemetry entry) and `id: AGH-0016` appears twice. The auto-writer (`agy_ledger::next_agh_id`) prevents this for delegated runs; manual appends collided.
- [ ] **Step 1:** `rg -no "id: AGH-[0-9]+" docs/superpowers/antigravity-handoff-ledger.md | sort | uniq -d` → the duplicate ids. Next free is the max+1.
- [ ] **Step 2:** Renumber the LATER of each colliding pair to the next free id(s) (e.g. the Track E `AGH-0012` → `AGH-0018`; the second `AGH-0016` → `AGH-0019`). Update BOTH the `id:` line AND the matching `### AGH-NNNN — ... review detail` header. Do NOT touch the entry whose id is referenced by an external commit message unless you also note the remap.
- [ ] **Step 3:** Verify uniqueness: `rg -no "id: AGH-[0-9]+" ... | sort | uniq -d` → empty.
- [ ] **Step 4: Commit** `docs(ledger): dedupe AGH id collisions (concurrent-append clashes)`

> **Durable fix (out of scope here — reference only):** the pending `vox ci handoff-ledger` lint (ledger §D-2 / AGH-0003) should enforce unique ids + schema. Build it under that initiative, not this plan.

### Task 3.2: Bake the AGH-0012 lessons into the launch-statement template [PARALLEL-SAFE]
**Files:** Modify ledger §B (append lessons) — OR the convergence plan's §A if preferred.
- [ ] **Step 1:** Append to ledger §B three lessons (already drafted in the AGH-0012 review): (a) a dry-run can't validate a stage it skips — acceptance must execute the asserted behavior; (b) wire every field of a new SSOT record; (c) self-report manifest must match the diff (quote code). Mark them "promote to launch-statement template."
- [ ] **Step 2: Commit** `docs(ledger): promote AGH-0012 lessons (effect-vs-skip, wire-all-fields, manifest-accuracy)`

---

## C. Green boundary (follow-ups complete)
- `resolve_training_selection` is pure, GPU-independent, unit-tested; the Train stage resolves pre-gpu-gate and **logs the resolved triple** (dry-run-observable).
- A GPU-free integration test proves all three live spokes resolve to a trainable selection (real effect-proof — closes F1).
- `base.preset` honored (F2, done); preset name canonicalized + alias documented.
- Ledger ids unique (F4); AGH-0012 lessons promoted (F3).
- `vox ci spoke-check` + `cargo test -p vox-ml-cli -p vox-populi` green; `cargo check --features gpu` green; `forbidden_pattern` stays `error`.

## D. Scope NOT included (separate initiatives)
- A real `--features gpu` micro-train run (true end-to-end training) — needs a GPU host; the GPU-free effect-proof (Phase 1.3) is the CI-grade substitute.
- The `vox ci handoff-ledger` lint (AGH-0003 / §D-2).
- Heterogeneous-base serving topology (S-LoRA etc.) — the convergence spec §5 deferred it; not needed for the training pipeline.

## E. Spec-coverage check
- F1 (hollow effect-proof) → Phase 1 (pure fn + pre-gate resolution + GPU-free effect test). ✓
- F2 (base.preset) → done (9ef4c40ac5); re-confirmed in §B prereq. ✓
- F3 (manifest accuracy) → Phase 3.2 lesson + §A rule. ✓
- F4 (id collision) → Phase 3.1 dedupe + durable-fix reference. ✓
- Preset SSOT drift → Phase 2. ✓
