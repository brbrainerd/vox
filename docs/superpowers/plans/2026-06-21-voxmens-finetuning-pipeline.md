# VoxMens Fine-Tuning Architecture & Spot Pipeline — Implementation Plan (rev 2, adversarially reviewed)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax. **This rev fixes codebase-fit blockers, an adapter-provenance correctness bug, and a parallel-write bug found in adversarial review — read "How to execute" before fanning out.**

**Goal:** A resource-scalable hub-and-spoke system — Qwen3 dense-ladder hub (CPU→96 GB) + a tiny embedder hub, 3 v1 QLoRA adapters (vox-lang, rust, mono-harness; decomposed harness as a measured arm), a no-retrain tool/skill retrieval+schema layer, RunPod-default spot training with full provenance, and offline eval acceptance.

**V1 success = pipeline runs end-to-end AND each adapter beats its own base rung on its gate metric.** Flash/Sonnet parity is a tracked north-star gap report, not a pass/fail gate.

**Architecture:** One pinned Qwen3 dense base (revision-locked) + LoRA adapters; tools/skills served by an embedder-backed retrieval + schema-constrained decoding (never fine-tuned); cloud spot training via the existing trait-based `cloud/` module with provenance manifests; adapters validated offline (vLLM serving wired but gated behind a compat spike, off the v1 critical path).

**Tech Stack:** Rust (vox-populi, vox-corpus, vox-ml-cli, vox-orchestrator-mcp), Qwen3 + QLoRA (CandleQlora plugin local; container on cloud), a ~0.6B embedder, vLLM (serving + guided decoding, v2-promoted), RunPod/Vast.ai REST, BFCL + MultiPL-E evals, existing `vox-similarity` for dedup.

**Spec:** `docs/superpowers/specs/2026-06-21-voxmens-finetuning-architecture-design.md` · **Research:** `docs/src/architecture/voxmens-finetuning-boundaries-research-2026-06-21.md`

---

## How to execute (workflow shape — corrected for parallel-write safety)

```
B0 (foundations/contracts/provenance) ── SEQUENTIAL, FIRST
        │
   ┌────┴───────── parallel TRACKS, each in ITS OWN GIT WORKTREE ──────────┐
   │  T1: B1 (harness corpora)   ── 2 sub-agents (selection | arg-gen)      │
   │  T2: B2 (vox/rust readiness + B2.5 data-sufficiency spike)             │
   │  T3: B3 (embedder + tool retrieval)                                    │
   │  T5: B5 (cloud spot pipeline) ── long pole                            │
   └───────────────────────────────────────────────────────────────────────┘
        │   (B4 schema-guided needs B0;  B6 serving needs B0 AND B4 → NOT parallel with B4)
        ▼
B-INT (integration): merge worktrees, resolve mod.rs/lib.rs, run full workspace test  ── SEQUENTIAL
        ▼
B4 (schema-guided decoding) → B6 (serving compat spike + wiring)  ── SEQUENTIAL chain
        ▼
B7 (evals: baseline-first, leakage guard, gates)  ── after B-INT
        ▼
B8 (smoke spoke → fan-out train → offline-validate → parity gap report)  ── SEQUENTIAL, LAST, money-gated
```

**Parallel-write rule (this was a bug in rev 1).** `crates/vox-corpus/src/corpus/mod.rs` (touched by B1.1/B1.2/B1.3/B2.1) and `crates/vox-orchestrator-mcp/src/lib.rs` (B3/B4) are shared files. Do **not** let parallel agents edit them concurrently. Two enforced mechanisms:
1. **Worktree-per-track** (repo uses `superpowers:using-git-worktrees`): T1/T2/T3/T5 each run in their own worktree off the B0 commit.
2. **A single sequential `B-INT` integration task** merges the tracks, resolves the `mod.rs`/`lib.rs` module-declaration conflicts by hand, and runs `cargo test --workspace --exclude vox-gui --locked`. No track edits another track's files.
- `B6` depends on `B4` (it calls `attach_guided_decoding`) — sequence them, never parallel.
- All `min_readiness`/`domain-profiles.yaml` edits live in **B0** (not a parallel phase) — it is the SSOT everyone reads.

**Recommended workflow primitive:** `parallel()` for the 4 tracks (barrier), each thunk using `isolation: 'worktree'`; then a sequential `agent()` for B-INT; then the B4→B6 chain; then B7; then B8.

**Money/safety gates (machine-enforceable, not prose):** B5.5 and B8 provisioning require the env sentinel `VOX_MENS_ALLOW_SPEND=1` AND `--apply`; absent either, the orchestration prints the plan and exits 0 without provisioning. The executor cannot set `VOX_MENS_ALLOW_SPEND` itself.

**Repo gotchas:** never pipe `cargo` to `head`/`grep` (redirect to file); `cargo fmt -p <crate>` only (never `--all`); `VOX_SKIP_FRESHNESS_CHECK=1` for in-loop `vox ci`; docs under `docs/src/` need `category:` frontmatter (these files don't); secrets via `vox_secrets::resolve_secret`, never `std::env::var`.

---

## File structure map

**Create:** `crates/vox-corpus/src/corpus/{tool_selection_synth.rs, argument_generation_synth.rs, harness_union.rs, corpus_readiness.rs, eval_split.rs}` · `crates/vox-orchestrator-mcp/src/{tool_retrieval.rs, schema_guided.rs}` · `crates/vox-populi/src/mens/serving/{mod.rs, vllm_lora.rs}` (v2-gated) · `crates/vox-populi/src/mens/tensor/adapter_card.rs` · `crates/vox-ml-cli/src/commands/mens/eval_gate/{bfcl.rs, baseline.rs, leakage.rs}` · `mens/config/{mix-tool-selection.yaml, mix-argument-generation.yaml, mix-harness.yaml, eval-gates-bfcl.yaml}`

**Modify:** `mens/config/{domain-profiles.yaml, gpu-specs.yaml}` · `contracts/mens/training-presets.v1.yaml` + `crates/vox-populi/src/mens/tensor/preset_schema.rs` · `crates/vox-populi/src/mens/tensor/{spoke_base_resolver.rs, domain_router.rs}` · `crates/vox-populi/src/mens/cloud/{part_jobs.rs, mod.rs, runpod_provider.rs, resolver.rs}` · `crates/vox-ml-cli/src/commands/mens/{train_arm.rs, eval_gate/{check_run.rs, policy.rs}}` · `crates/vox-corpus/src/corpus/{mod.rs, agentic_synth.rs}` · `crates/vox-orchestrator-mcp/src/{input_schemas.rs (widen `tool_input_schema` to `pub`), constrained_decoding.rs (add `SchemaGuided`), lib.rs}`

---

## Phase B0 — Foundations, contracts, provenance  `[SEQUENTIAL — FIRST; single agent]`

### Task B0.0: Confirm the 4 load-bearing 🟡 research claims (no code)
The architecture rests on these; the research's adversarial pass was rate-limited. Confirm each with **2–3 targeted WebFetch in small clusters** (the repo's documented rate-safe pattern), record results in the research doc:
- [ ] Qwen3 dense ladder rungs + native function-calling (re-fetch Qwen3 repo). [ ] "New tools without retraining via retrieval" (2509.20415 full text). [ ] vLLM runtime-LoRA + guided_json viability + version. [ ] Rust-vs-Vox separate-vs-shared adapter (Agnostics). **Gate:** any claim that fails confirmation triggers a design note before building on it. Commit the updated research doc.

### Task B0.1: 4-spoke set + retire others + route review→base
**Files:** Modify `mens/config/domain-profiles.yaml`, test in `crates/vox-populi/src/mens/tensor/domain_profiles.rs`
- [ ] **Failing test:** the fine-tuned set (profiles with a `base.method: qlora`) is exactly `{vox-lang, rust, tool-selection, argument-generation}`; `harness` exists as a union profile; retired profiles (`chat/research/research-expert/rocks/populi-meta`) have **no `base`**; a profile/router entry maps `lane:vox_rust_review` to base (no adapter).
- [ ] **Run → FAIL.**
- [ ] **Edit YAML:** split the current `agents` profile into `tool-selection` + `argument-generation` (+ a `harness` union profile for the v1 mono smoke), base alias `agentic_default`; keep `vox-lang`/`rust`; strip `base:` from retired profiles; add the review→base routing row; header comment documenting the v1 mono-first sequencing.
- [ ] **Run → PASS.** Then `VOX_SKIP_FRESHNESS_CHECK=1 vox ci spoke-check` OK.
- [ ] **Commit.**

### Task B0.2: Retired-spoke consumer audit (correctness)
- [ ] Grep every consumer of the retired profiles (`rocks/research/...`) and assert none resolves a `base`/adapter for them (a stripped `base:` must fail-closed, not panic/fail-open). Add a regression test if a consumer assumes a base. Commit.

### Task B0.3: Qwen3 ladder + per-tier table in resolver  `[CODEBASE-FIT: real API]`
**Files:** Modify `crates/vox-populi/src/mens/tensor/spoke_base_resolver.rs` (real fns are `pick_base(overlay, tag, vram_mb)` and `resolve_base_model(root, base_model, vram_override)` — do NOT invent `resolve_base_for_vram`), `mens/config/gpu-specs.yaml` (add a `train_bases:` overlay section)
- [ ] **Failing test** using the REAL fn: `pick_base(&overlay, "qwen3_code", 24_000)` returns a concrete `Qwen/Qwen3-14B@<revision>` (revision-pinned), and `pick_base(&overlay, "qwen3_code", 4_000)` returns `Err` (fail-closed below floor). Add a `prefer (larger rung, then less quantization)` ordering test that picks 14B-LoRA over 14B-QLoRA at 48 GB.
- [ ] **Run → FAIL.**
- [ ] **Implement:** add the full CPU/16/24/48/96 GB `train_bases:` table (rung → pinned HF id+revision → min VRAM → method) to `gpu-specs.yaml`; extend the resolver tag table + ordering. Map `small_code_default`→smallest fitting, `strong_code_default`/`agentic_default`→largest fitting rung.
- [ ] **Run → PASS. Commit.**

### Task B0.4: Qwen3 presets (incl. dev/CPU tier)  `[CODEBASE-FIT]`
**Files:** Modify `contracts/mens/training-presets.v1.yaml` + `KNOWN_PRESETS` in `crates/vox-populi/src/mens/tensor/preset_schema.rs` (existing parity test enforces YAML↔Rust)
- [ ] **Failing test:** `KNOWN_PRESETS` contains `qwen3_dev_cpu`, `qwen3_16g`, `qwen3_24g`, `qwen3_48g`, `qwen3_96g`; parity test red until both sides match.
- [ ] **Add presets** (seq_len, batch, grad_accum, rank/alpha per rung; `qwen3_dev_cpu` = 0.6B r8 for smoke). **Run parity → PASS. Commit.**

### Task B0.5: Adapter provenance contract (correctness — the BLOCK item)
**Files:** Create `crates/vox-populi/src/mens/tensor/adapter_card.rs`; Modify `domain_router.rs`
- [ ] **Failing test:** `AdapterCard { base_hf_id, base_revision, base_rung, quantization, lora_rank, lora_alpha, seed, corpus_hash, preset_version, metrics, cost_usd, provider, git_sha, created }`; `DomainRouter::register` requires a card and **errors if base_rung/quantization/base_revision are missing**; a `card.is_compatible_with(serve_rung, serve_quant)` returns false on mismatch.
- [ ] **Run → FAIL.**
- [ ] **Implement** `AdapterCard` + `adapter_card.json` sidecar writer/reader; change `DomainRouter::register(&mut self, domain, adapter_path, card)` (update existing callers/tests). 
- [ ] **Run → PASS. Commit.**

### Task B0.6: Declare the embedder hub
**Files:** Modify `mens/config/domain-profiles.yaml` (add `hub: { base: <qwen3 alias>, embedder: <pinned ~0.6B embedder id@rev> }`)
- [ ] **Failing test:** the loader exposes `hub.embedder` as a required, non-empty, revision-pinned id. Implement. **PASS. Commit.**

### Task B0.7: Local-training backwards-compatibility guard (4080 SUPER 16 GB)
**Files:** test in `crates/vox-ml-cli/src/commands/mens/train_arm.rs`; Modify `contracts/mens/training-presets.v1.yaml` (retain, do not remove, `qwen_4080_16g`)
- [ ] **Failing test:** the default/`--cloud local` path resolves to the CandleQlora plugin backend and the `qwen_4080_16g` preset still loads (parity test green with BOTH the old `qwen_*` and new `qwen3_*` presets present); a local run emits an `AdapterCard` with `provider: "local"` and registers via the same `DomainRouter::register(spoke, adapter, card)`.
- [ ] **Run → FAIL → ensure additive (old presets kept, local path untouched, card emitted locally) → PASS → Commit.**

**B0 gate:** `cargo test -p vox-populi --lib` green; `vox ci spoke-check` OK; research open-items recorded; **local 4080/CPU training path proven backwards-compatible (B0.7)**.

---

## Phase B1 — Harness corpora + union  `[TRACK T1 — own worktree; 2 sub-agents]`

### Task B1.1: tool-selection corpus  `[sub-agent A]`
**Files:** Create `tool_selection_synth.rs`, `mens/config/mix-tool-selection.yaml`; the `corpus/mod.rs` `pub mod` declaration is added in **B-INT** (not here — avoids the shared-file conflict; in-worktree, declare via a track-local module path so tests compile)
- [ ] **Failing test:** `generate_tool_selection_rows(&surface, 50)` rows are `{task, candidate_tools[], chosen_tool, lane:"vox_tool_selection"}` with ≥4 candidates incl. hard negatives (same `product_lane` siblings) and `chosen ∈ candidates`. Use real `input_schemas::tool_input_schema` (widen to `pub`) + `tool_aliases::canonical_tool_name` for names.
- [ ] **Run → FAIL → implement → PASS → commit (in worktree).**
- [ ] **Licensing assertion:** add a test that rows are rule-based-from-surface (no frontier-teacher text fields). 
- [ ] **Curriculum:** `mix-tool-selection.yaml` carries an explicit schedule (single-tool → large-catalog hard-negatives).

### Task B1.2: argument-generation corpus  `[sub-agent B]`
**Files:** Create `argument_generation_synth.rs`, `mens/config/mix-argument-generation.yaml`
- [ ] **Failing test:** rows `{task, tool_name, tool_schema, arguments, lane:"vox_argument_generation"}` where `arguments` validates against `tool_schema` (shared `schema_validate` helper, draft-07 subset). Curriculum: flat → nested/enum/optional. Licensing assertion as B1.1.
- [ ] **FAIL → implement → PASS → commit.**

### Task B1.3: harness union corpus (for the v1 mono smoke)
**Files:** Create `harness_union.rs`, `mens/config/mix-harness.yaml`
- [ ] **Failing test:** `generate_harness_rows` = selection + arg-gen rows merged into a single tool-call SFT format (`lane:"vox_harness"`), preserving the curriculum order. Implement. PASS. Commit.

### Task B1.4: seeded train/eval split at generation time (leakage guard)
**Files:** Create `eval_split.rs`
- [ ] **Failing test:** `split_surface(seed, eval_frac)` partitions tools/skills **by identity** (a tool is wholly in train xor eval, never split by row); emits `split_manifest.json`; deterministic for a fixed seed. Implement; wire all B1 generators to honor the split. PASS. Commit.

**B1 gate:** `cargo test -p vox-corpus --lib` (in worktree) green for the new modules.

---

## Phase B2 — Vox/Rust readiness + data-sufficiency spike  `[TRACK T2 — own worktree; 2 sub-agents]`

### Task B2.1: corpus-readiness gate
**Files:** Create `corpus_readiness.rs`
- [ ] **Failing test:** `assess_corpus_readiness(jsonl, MinReadiness{rows, ast_diversity})` → `{rows, ast_diversity, rows_ok, diversity_ok, ready}`; reuse `vox_eval::eval_semantic_entropy` for diversity (VERIFY it exists; if not, compute n-gram/AST entropy locally). Implement. PASS. Commit.

### Task B2.2: `vox mens corpus readiness --spoke <name>` command + thresholds
**Files:** Modify `crates/vox-ml-cli/src/commands/corpus/mod.rs`; thresholds already added to `domain-profiles.yaml` in **B0**.
- [ ] TDD a `CorpusAction::Readiness` writing `corpus_readiness.json`, exit non-zero when not ready. Commit.

### Task B2.5: data-sufficiency spike (the real critical path — runs BEFORE any spend)
**Files:** none new (procedure + a `data_sufficiency.json` report)
- [ ] **Run every synth generator at full scale** (vox, rust, tool-selection, arg-gen, harness) and report actual rows + diversity vs the B0 thresholds. **Decision branch, recorded in the report:** (a) all ready → proceed to B8; (b) any thin → v1 deliverable becomes "pipeline + the corpora we can build," that spoke is **blocked from training** (grow corpus first), and parity for it is explicitly out of reach. **No cloud GPU may be provisioned until this report shows ≥1 spoke ready.** Commit the report.

**B2 gate:** readiness command works; `data_sufficiency.json` committed; sufficiency decision recorded.

---

## Phase B3 — Embedder + semantic tool retrieval  `[TRACK T3 — own worktree]`

### Task B3.1: tool-retrieval module (embedder-backed, BM25 = warned degraded mode)
**Files:** Create `tool_retrieval.rs`; `lib.rs` `pub mod` added **in B-INT**
- [ ] **Failing test 1 (lexical-friendly):** `select_tools(&reg, "what files changed in the repo", 5)` returns ≤5 with the git-status tool in **top-K** (assert in-top-K, not top-1 — brittle otherwise).
- [ ] **Failing test 2 (semantics, the important one):** a **paraphrased** task ("show me my uncommitted edits") that BM25 misses but the embedder surfaces — proves semantics are wired, not lexical luck. Skip-with-warning if `hub.embedder` unavailable, and the gate **warns** on degraded mode.
- [ ] **Run → FAIL → implement** using the declared `hub.embedder` (B0.6); BM25 fallback emits a degraded-mode warning. **PASS → commit.**

**B3 gate:** both retrieval tests green with the embedder present; degraded-mode path warns.

---

## Phase B-INT — Integration  `[SEQUENTIAL — after T1/T2/T3/T5 worktrees]`
- [ ] Merge the track worktrees. Resolve `corpus/mod.rs` (`pub mod tool_selection_synth/argument_generation_synth/harness_union/corpus_readiness/eval_split`) and `lib.rs` (`pub mod tool_retrieval`) by hand. Add `CorpusAction`/`PipelineStage` variants (`ToolSelectionSynth`, `ArgumentGenerationSynth`, `HarnessSynth`) to the enum **and** its `as_str` match in `action_prelude.rs` (there is no `all_possible_stages()` — edit the enum + match directly), ordered before `Mix`.
- [ ] Run `cargo test --workspace --exclude vox-gui --locked` (redirect to a file). Gate: green. Commit the integration.

---

## Phase B4 — Schema-constrained decoding at generation  `[SEQUENTIAL — after B-INT]`
**Files:** Create `schema_guided.rs`; Modify `constrained_decoding.rs` (add variant)
- [ ] **Failing test:** `to_guided_decoding_spec(&schema)` → `{guided_json: <schema>, guided_decoding_backend: <from config>}` (backend from config, **not** a hard-coded string). Add `ConstrainedDecodingMode::SchemaGuided(serde_json::Value)` to the real enum (currently only `None/JsonPrefix/StrictJson`). Add seam fn `attach_guided_decoding(req, tool_schema)`.
- [ ] **FAIL → implement → PASS → commit.**

---

## Phase B5 — Cloud spot pipeline (RunPod default)  `[TRACK T5 — own worktree; sequential sub-tasks]`  `[CODEBASE-FIT: trait-based]`
**Reality:** job code lives in `cloud/part_jobs.rs` with trait `CloudProvider::{dispatch(offer, spec)->JobHandle, poll_status(handle)->JobStatus}`, `CloudJobSpec`, `BudgetLedger`, `estimator::TimeEstimator`, `CloudResolver`. Extend these — do NOT create `cloud/job.rs`/`submit_job`/`Budget{max_usd}`.

### Task B5.1: budget-gated dispatch on the real trait
- [ ] **Failing test (mock provider):** a `dispatch_training(resolver, CloudJobSpec, &mut BudgetLedger)` wrapper refuses when `BudgetLedger` cumulative + estimate exceeds the cap **before** calling `CloudProvider::dispatch` (assert 0 provider calls). Implement using `TimeEstimator::estimate` + `BudgetLedger`. Secrets via `vox_secrets::resolve_secret("runpod-api-key")` (env fallback). PASS. Commit.

### Task B5.2: poll + log streaming + retention
- [ ] TDD a poll loop over `CloudProvider::poll_status` that streams logs and **persists them** next to the adapter; on `Failed` surfaces the provider reason. Commit.

### Task B5.3: checkpoint sync + resume + idempotent submit + orphan cleanup
- [ ] TDD: `sync_corpus_up`/`sync_checkpoint_down` (configurable interval, default ≤10 min); `resume_from_checkpoint(uri)` restarts a fresh pod mid-training (not from scratch); `dispatch` carries an **idempotency key** (retry ≠ second paid pod); an error-path guard/reconciler **terminates orphaned pods**; budget checked against **cumulative** spend incl. retries; provider-side **max-price auto-terminate** set on dispatch. Commit.

### Task B5.4: `vox mens train --cloud <provider>` orchestration  `[CODEBASE-FIT: --cloud not --remote]`
**Files:** Modify `crates/vox-ml-cli/src/commands/mens/train_arm.rs` (the real `cloud` param, checked `!= "local"`)
- [ ] TDD ordering (mocked): estimate → cumulative-budget gate → `--dry-run` prints plan+ready-spokes and exits without provisioning → (with `VOX_MENS_ALLOW_SPEND=1 && --apply`) sync up → dispatch → poll+log → checkpoint down → **eval-gate (beat-base) BEFORE register** → write `training_manifest.json` + `adapter_card.json` → `DomainRouter::register(spoke, adapter, card)` as **challenger**. Assert a failed gate does NOT register. Commit.

### Task B5.5: gated live smoke (real money, tiny)
- [ ] **Procedure (requires `VOX_MENS_ALLOW_SPEND=1 --apply`, human go-ahead):** train `harness` (mono) on a tiny subset via RunPod for <$5; confirm full loop → registered challenger adapter + manifest + card. This is the executable-pipeline proof before B8.

**B5 gate:** `cargo test -p vox-populi --lib cloud::` green; live smoke deferred to human approval.

---

## Phase B6 — Serving (vLLM multi-LoRA)  `[SEQUENTIAL — after B4; v1 = wired + spike, promotion is v2]`

### Task B6.0: vLLM compatibility spike (real, non-mocked — gates everything serving)
- [ ] **Procedure:** stand up vLLM on the pinned version with a real Qwen3 base + one trained QLoRA adapter; confirm (a) runtime LoRA load/unload, (b) `guided_json` + the configured backend, (c) **serve quantization matches the QLoRA training quantization** (the silent killer). Record versions in `gpu-specs.yaml`. **Gate:** if any fails, serving stays v2 and B8 validates **offline only** (still a green v1).

### Task B6.1: vLLM LoRA client + provenance enforcement
**Files:** Create `serving/{mod.rs, vllm_lora.rs}`
- [ ] **Failing test (mock):** `ensure_adapter_loaded(name, path, card)` loads once, is idempotent (LRU), and **rejects on rung/quantization mismatch** via `AdapterCard::is_compatible_with`; `build_chat(task, adapter)` sets `model=<adapter>` + attaches `guided_json`. Add `load_rejects_rung_mismatch` test. Implement. PASS. Commit.

### Task B6.2: `route_by_signal` → adapter + champion/challenger + telemetry
**Files:** Modify `domain_router.rs` (create `route_by_signal`)
- [ ] TDD: `route_by_signal(file_or_lane)` → spoke → `DomainRouter::route` → adapter; unknown lane → base (no adapter), never errors; a **challenger** serves only behind a flag, **champion** is the promoted default; emit routing telemetry (adapter, fallback rate, evictions, latency). Commit.

**B6 gate:** client tests green; compat spike recorded; promotion deferred to v2 unless the spike passes.

---

## Phase B7 — Evals: baseline-first, leakage-guarded, beat-base  `[after B-INT]`

### Task B7.0: leakage assertion (runs before any gate is trusted)
**Files:** Create `eval_gate/leakage.rs`
- [ ] **Failing test:** intersect train-corpus fingerprints with eval-pack fingerprints (from `split_manifest.json`) → **fail build if non-empty**; near-dup check via existing `vox-similarity` (simhash/minhash). Implement. PASS. Commit.

### Task B7.1: baseline capture (base rung + Flash/Sonnet reference)
**Files:** Create `eval_gate/baseline.rs`
- [ ] TDD `baseline_report.json` per spoke: metric, pass@k **with k stated**, **sample size**, **bootstrap CI**, judge identity for any LLM-judged metric. Run untrained base + the reference on the held-out packs. Commit.

### Task B7.2: BFCL gate + per-rung thresholds + regression guard  `[CODEBASE-FIT]`
**Files:** Create `eval_gate/bfcl.rs`, `mens/config/eval-gates-bfcl.yaml`; Modify `eval_gate/{check_run.rs, policy.rs}` (add `bfcl_accuracy` field — absent today)
- [ ] **Failing test (mirror the existing "not applicable when metric absent" handler):** `bfcl_accuracy` blocks below threshold, "not applicable" when absent; **gate = beat-base** (trained metric > `baseline_report.json` base by margin, CIs considered), not a hand-set number; **per-rung** thresholds (4B spoke not held to 32B bar); **regression guard** vs the prior registered adapter's stored metrics. Implement producer + handler. PASS. Commit.

### Task B7.3: harness safety eval (mocked executor)
- [ ] TDD: harness evals run against a **mocked/dry-run tool executor** — assert no real side-effecting tool is invoked in any eval/smoke path. Commit.

### Task B7.4: planning/dispatch eval (base-only; evidences a v2 planning spoke)
- [ ] TDD a multi-step "did the harness sequence the right tools" metric on the base; write to `baseline_report.json`. (No planning spoke in v1 — this is the evidence to decide v2.) Commit.

**B7 gate:** `cargo test -p vox-ml-cli --lib mens::eval_gate` green; leakage assertion green; baselines captured.

---

## Phase B8 — End-to-end: smoke → fan-out → offline-validate → parity gap  `[SEQUENTIAL — LAST; money-gated]`

### Task B8.1: readiness + sufficiency pre-flight (no GPU)
- [ ] From B2.5: train only spokes marked ready. Record the set; thin spokes are flagged "blocked on data," not failed.

### Task B8.2: smoke spoke FIRST (hard prerequisite)
- [ ] **Local 4080 smoke (no money, run this first):** train the **mono `harness`** adapter on the 16 GB tier (`qwen3_16g` / Qwen3-8B QLoRA) **locally on the 4080 SUPER** via the CandleQlora path — full train→gate(beat-base)→register(challenger w/ `provider: local`)→offline-validate. This proves the whole loop end-to-end with zero spend and is the local-testing capability the user requires.
- [ ] **Cloud smoke (gated):** repeat the mono-`harness` end-to-end on RunPod (`VOX_MENS_ALLOW_SPEND=1 --apply`) to prove the cloud path. Only after both is fan-out unlocked.

### Task B8.3: fan-out train the ready spokes  `[4-way sub-agent fan-out]`
- [ ] Train `vox-lang`, `rust`, and the **decomposed** `tool-selection` + `argument-generation` adapters. **Comparison arm:** evaluate decomposed (selection+arg-gen) vs the mono `harness` adapter, both over *base + B3 retrieval + B4 schema-guided* — keep whichever wins; if mono wins, the decomposition is deferred to v2 (records the decision). **Optional arm:** a single language-tagged code adapter on the combined vox+rust corpus vs the two separate adapters (could collapse two spokes in v2).

### Task B8.4: offline validation + parity gap report
- [ ] Load each registered adapter offline; run the held-out eval packs. **V1 acceptance:** each adapter beats its base rung (B7.2). Write `parity_report.json` = each spoke's metric vs the Flash/Sonnet reference (north-star gap, not a gate). Promote champions only on beat-incumbent; keep last-known-good for rollback.
- [ ] (If B6.0 passed) validate live vLLM hot-swap across adapters on one base; else note serving deferred to v2.

**B8 gate (final):** smoke spoke proven end-to-end; ready spokes trained + offline-validated (beat-base); `parity_report.json` + decomposition/shared-code decisions recorded; champions promoted with rollback in place.

---

## Self-review (rev 2 — done)

- **Codebase-fit:** every invented API replaced with the real one (`pick_base`, trait `CloudProvider::dispatch/poll_status`, `BudgetLedger`, `--cloud`, enum-edit instead of `all_possible_stages`, `DomainRouter::register(+card)`, `ConstrainedDecodingMode::SchemaGuided`, `bfcl_accuracy` field, `pub` `tool_input_schema` + `canonical_tool_name`).
- **Correctness:** adapter↔(rung+quantization+revision) provenance pinned and fail-closed (B0.5/B5.4/B6.1); parallel-write bug fixed via worktree-per-track + B-INT; B6 sequenced after B4; `min_readiness` moved to B0.
- **Stability/quality:** provenance manifest (B5.4), base-revision pin (B0.3), seed (B0.5/B1.4), leakage split+guard (B1.4/B7.0), baseline-first (B7.1), cloud resume/idempotent/orphan-cleanup/secrets/max-price (B5.3), regression guard + champion/challenger + rollback (B7.2/B6.2/B8.4), safety eval (B7.3).
- **Scaling:** CPU→96 GB table + dev tier + scale-down floor + 48 GB un-quantize rule (B0.3/B0.4); dense-vs-MoE recorded (spec).
- **Honesty:** success reframed to beat-base; data-sufficiency spike gates spend (B2.5); money gated by machine-enforceable sentinel; serving off the v1 critical path.
- **Type consistency:** `AdapterCard`/`is_compatible_with`, `dispatch_training`/`CloudJobSpec`/`BudgetLedger`, `to_guided_decoding_spec`/`attach_guided_decoding`, `select_tools`, `assess_corpus_readiness`/`MinReadiness`, `split_surface`/`split_manifest.json` used consistently across phases.
