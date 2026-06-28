# VoxMens — Spoke/Hub Architecture Decision + Tracked Backlog

> **Status:** Live architecture decision + consolidated to-do backlog for the VoxMens hub-and-spoke. Synthesized from this session's per-spoke + hub/router audits (the parallel mapping workflow `wf_f1866546-7cf` did not survive a multi-hour session gap; this is the equivalent synthesis, branch-aware).
> **Operator note:** the build-out tracks are Claude/human or carefully-scoped Gemini; the consolidation prerequisite (Track 0) is Claude/human.

## 0. The architecture decision (the central question, resolved)

**"The harness" is TWO things, not one — and only one of them is a trained spoke:**

| Concern | What it does | Kind | Why |
|---|---|---|---|
| **Router / dispatcher** | Per task: pick **which spoke** + **whether to use a cloud AI or a local fine-tuned pathway**, then dispatch | **HUB (a router, NOT a trained model)** | Selection is a *decision function*, not a generation task. It's largely **already built** as the inference engine `crates/vox-orchestrator/src/models/select.rs` (`select()`/`decide()`, 3-axis intent, **`CandidateScope::{LocalOnly,CloudOnly,AllProviders}` == the cloud-vs-local choice**, capability filter, premium aliases) + `key_guard::available_inference_providers` (availability) + the training-side `route_by_signal`/`domain_router` (spoke-by-lane/trigger). |
| **Agentic execution** | Emit MCP tool calls / skill invocations / `vox.exe` commands; operate the harness to manage the codebase | **SPOKE (a trained model)** | This *is* a generation task with its own corpus (`mix-agents`, `vox_tooling` lane, `TOOL_REGISTRY`). It's the "executes and runs our harness" part. |

**Decision:** the harness = **HUB (router) + the agentic-execution SPOKE**, kept distinct. The router *chooses*; the agentic spoke *acts*. "Selects from separate AIs / separate fine-tuned local pathways" = the **router** reusing `select()`'s `CandidateScope` + `key_guard` (cloud-vs-local) and `route_by_signal` (which spoke/adapter). "Executes the harness" = the **agentic spoke**. Do **not** build a third selector — unify the two existing routers behind one task→`(spoke, model, local|cloud)` entry point.

## 1. The four tracks

| # | Track | Kind | Completion definition | Est. % | Where it lives |
|---|---|---|---|---|---|
| T1 | **VoxScript authoring** | spoke | Trainable + diversity-gated + eval-gated (parse/pass@k/anti-stub/coverage); declared in the spoke SSOT | ~85% (mature) | green/voxmens branches |
| T2 | **Rust authoring & review** | spoke | Real authoring/review lanes (not just Rust→Vox), `cargo check`/clippy-verified corpus, `eval-gates-rust` that actually fires (producer+handler) | ~40% | voxmens-split-c-followups |
| T3 | **Harness / agentic execution** | spoke | Corpus from `TOOL_REGISTRY` synthesis + mined real traces; trains to emit valid Vox tool/skill/CLI calls; `eval-gates-agents` (tool-call JSON validity + tool-name-exists) fires | ~25% | mix-agents skeleton; corpus mostly unbuilt |
| T4 | **Hub / Router** | hub (not a spoke) | One task→`(spoke, model, cloud\|local)` dispatcher unifying `select()` (cloud-vs-local via CandidateScope) + `route_by_signal` (spoke via lane/trigger); deterministic first, learned later | ~50% | `select()` mature; `route_by_signal`/`training_selection` on voxmens |

**"Vox coding" vs "VoxScript coding":** one spoke (T1). No evidence of a meaningful split — both are VoxScript authoring under the `vox_codegen`/`vox_lang_tier_b` lanes. Keep as one; revisit only if a distinct sub-domain corpus emerges.

## 2. The meta-blocker (Track 0 — prerequisite)

**None of T1–T4 is on `main`, and the work is scattered across unmerged branches** (`voxmens-split-c-followups`, `claude/auto-gui-debug-plans`, etc.; the current working branch `vox-frontend-ssot-subproject-b` has almost none of it). Until the **repo consolidation** ([`2026-06-19-repo-consolidation-to-main.md`](2026-06-19-repo-consolidation-to-main.md)) lands the VoxMens branches onto one base, "completing the tracks" can't be verified or shipped. **Track 0 gates everything below.**

## 3. Consolidated backlog (dependency-ordered)

Status: ✅ done (built on some branch) · 🟡 partial · ⬜ todo. Priority: P0 unblocks others / closes a critical gap.

### Track 0 — Consolidation (prerequisite)
- [ ] **C0.1** (P0) Freeze + total backup (bundle --all + tag all branches/stashes). *(repo-consolidation Phase 0)*
- [ ] **C0.2** (P0) Branch/stash contribution map; pick integration base; layer LIVE VoxMens branches onto it; green. *(consolidation Phases 1–3)*
- [ ] **C0.3** (P0) Re-slice → ≤140-file CodeRabbit-reviewed PRs → merge to `main`. *(consolidation Phases 4–6)*
  - Completion: VoxMens T1–T4 artifacts all present on `main`, arch-check green (`forbidden_pattern=error` + guard engine), `vox ci spoke-check` exit 0.

### Spoke SSOT + shared infra (cross-track foundation)
- [ ] **S.1** (P0) 🟡 `domain-profiles.yaml` + `DomainProfile.base/method/eval_gate/router` SSOT + `spoke_validate` + `vox ci spoke-check`. *(Plan A — built on voxmens; absent on current branch → consolidate)* — done when present on `main` and spoke-check gates drift.
- [ ] **S.2** (P0) 🟡 `spoke_base_resolver` (tag→VRAM-fit HF id) + `train_bases` overlay in `gpu-specs.yaml`, reusing `vram_autodetect`. *(Plan B)* — done when each spoke resolves a concrete base, no-GPU falls back.
- [ ] **S.3** (P1) 🟡 Per-spoke method dispatch via `AdapterMethodRegistry` (QLoRA wired; DPO/ORPO/FullSft fail-closed; RAG/prompt skip). *(F1)* — done when `base.method` drives the kernel.
- [ ] **S.4** (P0) 🟡 Restore the arch-check guard engine (`exempt_tests`/`cfg_test_line_mask`) wherever the integration base lacks it; keep `forbidden_pattern=error`. *(AGH-0008)*

### T1 — VoxScript spoke (harden)
- [ ] **V.1** (P2) ✅ Confirm synthetic_gen + ast_mutator + doc-mining + diversity gate present on `main` post-consolidation.
- [ ] **V.2** (P2) ⬜ Pick the small/fast VoxScript base model (model-registry entry) — *needs live model re-research (open decision)*.
- [ ] **V.3** (P2) ⬜ Promote vox-lang to an explicit spoke SSOT record (reference impl for the schema).

### T2 — Rust spoke (fill)
- [ ] **R.1** (P1) 🟡 `rust_authoring` corpus (instruction→idiomatic Rust) + `cargo check`/clippy verifier (batched, workspace-context, not per-snippet). *(convergent/F-plan)*
- [ ] **R.2** (P1) ⬜ Rust-review lane mined from PR `review_findings` (DPO/ORPO preference pairs).
- [ ] **R.3** (P1) 🟡 `eval-gates-rust.yaml` + **metric producer** (`rust_compile_rate`/`clippy_clean_rate`) + **`check_run` handler** (the gate must actually fire, not just declare thresholds).
- [ ] **R.4** (P2) ⬜ Strong-code base-model pick (model-registry) — *open decision, live research*.

### T3 — Harness / agentic spoke (build)
- [ ] **H.1** (P0) ⬜ **Trace-capture in the harness** (a2a/workflow/dogfood → `agent_trace_record` schema). *Prerequisite — the corpus literally doesn't exist yet.*
- [ ] **H.2** (P1) 🟡 `agentic_synth` from the REAL Vox surface — reuse `generate_tool_pairs(TOOL_REGISTRY_SLIM)` + `SkillRegistry` + `vox.exe` CLI tree (NOT Claude's `Skill` tool).
- [ ] **H.3** (P1) ⬜ `trace_ingest` (traces→SFT/DPO) + diversity gate (`eval_semantic_entropy`).
- [ ] **H.4** (P1) ⬜ `eval-gates-agents.yaml` + producer + handler: `tool_call_valid_json_rate` + **`tool_name_exists_rate` against `vox_mcp_registry::TOOL_REGISTRY`** (hard gate — registry exists).
- [ ] **H.5** (P2) ⬜ Make `mix-agents.yaml` strict (no silent-optional phantom sources).
- [ ] **H.6** (P2) ⬜ Agentic/tool-use base-model pick — *open decision, live research*.

### T4 — Hub / Router (unify)
- [ ] **U.1** (P1) 🟡 `route_by_signal` (lane/trigger → spoke, deterministic, name tie-break). *(built on voxmens)*
- [ ] **U.2** (P0) ⬜ **Unify the two routers**: one entry point `route(task) → {spoke, model_id, scope: Local|Cloud}` that calls `route_by_signal` for the spoke and reuses `select()`+`CandidateScope`+`key_guard` for cloud-vs-local + availability. This is the harness's "selects spokes + chooses cloud-or-local AI" capability.
- [ ] **U.3** (P1) ⬜ Wire the agentic spoke (T3) as a *local fine-tuned pathway* the router can pick (VoxLocal/PopuliMesh) vs a cloud AI, per task.
- [ ] **U.4** (P2) ⬜ Serving topology decision (shared-base adapter hot-swap vs separate servers) — gated on whether spokes share a base; `domain_router` already maps adapter-by-domain.
- [ ] **U.5** (P2) ⬜ Optional: upgrade the deterministic router to a learned classifier once misroute rate is measurable.

## 4. Open decisions (need you / live research)
1. **Cloud-vs-local default policy:** when does the router prefer a local fine-tuned spoke over a cloud AI (cost? latency? privacy? capability floor)? Drives U.2.
2. **Per-spoke base models** (V.2/R.4/H.6): the model-landscape research was rate-limited; re-research live before committing model-registry entries.
3. **Consolidation base branch** (C0.2): which of the ~280-commit branches becomes the integration base.

## 5. Suggested execution order
**Track 0 (C0.1→C0.3) FIRST** — without it, nothing is verifiable or shippable. Then the P0 foundation (S.1, S.2, S.4, U.2, H.1), then fill T2/T3 corpora in parallel (R.1–R.3 ∥ H.2–H.4), then T4 unification (U.3), then base-model picks (after the open-decision research). T1 is mostly hardening (low priority).
