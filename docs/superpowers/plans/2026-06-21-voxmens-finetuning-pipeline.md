# VoxMens Fine-Tuning Architecture & Spot Pipeline — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make VoxMens a resource-scalable hub-and-spoke system — 4 fine-tuned QLoRA spokes on a shared Qwen3 ladder, a dynamic (no-retrain) tool/skill layer, vLLM multi-LoRA serving, and a RunPod-default cloud spot training pipeline — reaching per-spoke Flash/Sonnet parity on each spoke's domain.

**Architecture:** One Qwen3 dense-ladder hub; 4 LoRA adapters (`vox-lang`, `rust`, `tool-selection`, `argument-generation`); tools/skills/plugins served by retrieval + schema-constrained decoding (never fine-tuned); training on cloud spot GPUs via the existing `cloud/` module; serving via vLLM runtime LoRA hot-swap wired to `DomainRouter`.

**Tech Stack:** Rust (vox-populi, vox-corpus, vox-ml-cli, vox-orchestrator-mcp), Qwen3 + QLoRA (CandleQlora plugin local; container image on cloud), vLLM (serving + guided decoding), RunPod/Vast.ai REST APIs, BFCL + MultiPL-E evals.

**Spec:** `docs/superpowers/specs/2026-06-21-voxmens-finetuning-architecture-design.md`
**Research:** `docs/src/architecture/voxmens-finetuning-boundaries-research-2026-06-21.md`

---

## How to execute (workflow shape for Sonnet 4.6)

Phases are labelled **B0..B8** with explicit dependency + parallelism markers:

```
B0 (foundations/contracts)  ── sequential, FIRST (everyone depends on it)
        │
        ├──► B1 (harness corpora)        [PARALLEL track 1]  ── 2 sub-agents (selection | arg-gen)
        ├──► B2 (vox/rust corpus gating) [PARALLEL track 2]  ── 2 sub-agents (vox | rust)
        ├──► B3 (tool retrieval)         [PARALLEL track 3]
        ├──► B4 (schema-constrained dec) [PARALLEL track 4]
        └──► B5 (cloud spot pipeline)    [PARALLEL track 5]  ── the long pole
        │
        ├──► B6 (vLLM multi-LoRA serving) [needs B0; parallel to corpora/cloud]
        └──► B7 (BFCL + per-spoke gates)  [needs B0; parallel]
        │
        ▼
B8 (end-to-end: train 4 adapters on RunPod → gate → register → serve → validate parity)
        └── sequential, LAST (depends on B1,B2,B3,B4,B5,B6,B7)
```

**Recommended workflow (one phase per `agent()` stage, fan-out inside a phase):**
- A `pipeline()` is wrong here (phases have a barrier at B8). Use `parallel()` for B1–B7 after B0, then a final B8 stage.
- Within B1 and B2, fan out 2 sub-agents each (one per corpus). Within B5, the sub-tasks are sequential (job-submit → logs → checkpoints) — single agent.
- **Each code phase ends in a green `cargo test -p <crate>` gate; each ML phase ends in a corpus-readiness or eval gate.** Never advance a phase on red.
- **Cost guard:** B5/B8 spend real money. The eval-gate + budget ledger (`VOX_CLOUD_MAX_BUDGET`) must pass before any `--remote` run; B8 is the ONLY phase that provisions paid GPUs.

**Windows/CI gotchas (this repo):** never pipe `cargo` to `head`/`grep` (process leak — redirect to a file); never `cargo fmt --all` (use `cargo fmt -p <crate>`); `vox ci` is freshness-gated (`VOX_SKIP_FRESHNESS_CHECK=1` for in-loop runs); docs under `docs/src/` need `category:` frontmatter (these plan/spec files do not).

---

## File structure map

**Create:**
- `crates/vox-corpus/src/corpus/tool_selection_synth.rs` — selection-adapter SFT corpus from the real tool surface
- `crates/vox-corpus/src/corpus/argument_generation_synth.rs` — arg-gen SFT corpus (schema-grounded)
- `crates/vox-corpus/src/corpus/corpus_readiness.rs` — volume+diversity gate per spoke
- `crates/vox-orchestrator-mcp/src/tool_retrieval.rs` — semantic top-K tool selection
- `crates/vox-orchestrator-mcp/src/schema_guided.rs` — schema→grammar bridge for constrained decoding
- `crates/vox-populi/src/mens/cloud/job.rs` — remote job submission/poll/log/checkpoint flow
- `crates/vox-populi/src/mens/serving/mod.rs` + `vllm_lora.rs` — vLLM multi-LoRA client + DomainRouter wiring
- `crates/vox-ml-cli/src/commands/mens/eval_gate/bfcl.rs` — BFCL metric producer + gate handler
- `mens/config/mix-tool-selection.yaml`, `mens/config/mix-argument-generation.yaml`
- `mens/config/eval-gates-bfcl.yaml`

**Modify:**
- `mens/config/domain-profiles.yaml` — 4-spoke set; retire others; Qwen3 base aliases
- `mens/config/gpu-specs.yaml` + `contracts/mens/training-presets.v1.yaml` — Qwen3 rung presets
- `crates/vox-populi/src/mens/tensor/spoke_base_resolver.rs` — Qwen3 ladder tags
- `crates/vox-populi/src/mens/tensor/domain_router.rs` — adapter-name lookup for serving
- `crates/vox-ml-cli/src/commands/mens/` — `train --remote` wiring; new pipeline stages for the 2 corpora
- `crates/vox-orchestrator-mcp/src/lib.rs` — register tool_retrieval + schema_guided in the request path

---

## Phase B0 — Foundations & contracts  `[SEQUENTIAL — FIRST]`

**Goal:** Land the SSOT changes every other phase reads. No model training. Small, fast, blocks everything.

**Sub-agent fan-out:** none (single agent; it's config + resolver edits that conflict if parallel).

### Task B0.1: Define the 4-spoke set in domain-profiles.yaml
**Files:** Modify `mens/config/domain-profiles.yaml`

- [ ] **Step 1 — write the failing test** (`crates/vox-populi/src/mens/tensor/domain_profiles.rs` tests):
```rust
#[test]
fn v1_spoke_set_is_exactly_four_finetuned() {
    let root = repo_root();
    let file = DomainProfilesFile::load(Some(&root)).expect("load");
    let finetuned: Vec<_> = file.profiles.iter()
        .filter(|(_, p)| p.base.as_ref().map(|b| b.method == TrainMethod::Qlora).unwrap_or(false))
        .map(|(k, _)| k.clone()).collect();
    let mut got = finetuned.clone(); got.sort();
    assert_eq!(got, vec!["argument-generation","rust","tool-selection","vox-lang"],
        "v1 fine-tuned spokes must be exactly these 4; retired spokes must not set a qlora base");
}
```
- [ ] **Step 2 — run, expect FAIL** (`agents` is the only harness spoke today; `tool-selection`/`argument-generation` don't exist): `cargo test -p vox-populi --lib v1_spoke_set_is_exactly_four_finetuned`
- [ ] **Step 3 — edit `domain-profiles.yaml`:** rename/split `agents` into two profiles `tool-selection` and `argument-generation` (each `base.method: qlora`, base alias `agentic_default`, preset to be set in B0.3); keep `vox-lang` + `rust`; **remove `base:` from** `rocks`, `research`, `research-expert`, `populi-meta` (they become dynamic/hub — keep the profiles for context_filter docs but with no `base`). Add a top-of-file comment: "V1 fine-tuned spokes = vox-lang, rust, tool-selection, argument-generation. All others are served by hub + retrieval (see spec)."
- [ ] **Step 4 — run, expect PASS.** Also run `VOX_SKIP_FRESHNESS_CHECK=1 cargo run -q -p vox-cli -- ci spoke-check > /tmp/sc.txt 2>&1; grep -i "OK\|error" /tmp/sc.txt` → spoke-check OK.
- [ ] **Step 5 — commit:** `git add mens/config/domain-profiles.yaml crates/vox-populi/src/mens/tensor/domain_profiles.rs && git commit -m "feat(mens): v1 4-spoke set (vox/rust/tool-selection/argument-generation)"`

### Task B0.2: Qwen3 ladder tags in spoke_base_resolver
**Files:** Modify `crates/vox-populi/src/mens/tensor/spoke_base_resolver.rs`, `mens/config/gpu-specs.yaml`

- [ ] **Step 1 — failing test:** assert `resolve_base("qwen3_code", vram_mb=24000)` returns a concrete Qwen3 HF id (e.g. `Qwen/Qwen3-14B`) and that a too-big rung fails closed:
```rust
#[test]
fn qwen3_ladder_resolves_by_vram_and_fails_closed() {
    let r = resolve_base_for_vram("qwen3_code", 24_000).expect("fits");
    assert!(r.contains("Qwen/Qwen3-"), "got {r}");
    assert!(resolve_base_for_vram("qwen3_code", 6_000).is_err(), "must fail closed when no rung fits");
}
```
- [ ] **Step 2 — run, expect FAIL.**
- [ ] **Step 3 — implement:** add the Qwen3 dense-ladder mapping (rung → HF id → min VRAM at QLoRA) to `gpu-specs.yaml`'s `train_bases` overlay and the resolver's tag table. Map aliases `small_code_default`→smallest fitting Qwen3, `strong_code_default`/`agentic_default`→largest fitting Qwen3 rung.
- [ ] **Step 4 — run, expect PASS.**
- [ ] **Step 5 — commit.**

### Task B0.3: Qwen3 presets
**Files:** Modify `contracts/mens/training-presets.v1.yaml` + `crates/vox-populi/src/mens/tensor/preset_schema.rs`
- [ ] **Step 1 — failing test:** `KNOWN_PRESETS` contains `qwen3_24g` and `qwen3_a100_80g`; the YAML↔Rust parity test fails until both sides match.
- [ ] **Step 2 — run, expect FAIL.**
- [ ] **Step 3 — add presets** to both files (seq_len, batch, grad_accum, rank/alpha per rung) keeping the existing parity contract.
- [ ] **Step 4 — run parity test, expect PASS.**
- [ ] **Step 5 — commit.**

**Phase B0 gate:** `cargo test -p vox-populi --lib` green; `vox ci spoke-check` OK.

---

## Phase B1 — Harness corpora (tool-selection + argument-generation)  `[PARALLEL track 1; 2 sub-agents]`

**Goal:** Generate two SFT corpora from the REAL Vox surface so the harness learns the *grammar* of tool use, not specific tools. Depends on B0. The two corpora are independent → **fan out 2 sub-agents** (one per file).

**Reuse:** `agentic_synth.rs` already builds tool-use rows from `TOOL_REGISTRY_SLIM` + `CLI_COMMANDS` + `SkillRegistry`. B1 *specializes* that into the two decomposed tasks.

### Task B1.1: tool-selection corpus  `[sub-agent A]`
**Files:** Create `crates/vox-corpus/src/corpus/tool_selection_synth.rs`; Create `mens/config/mix-tool-selection.yaml`; Modify `crates/vox-corpus/src/corpus/mod.rs`

- [ ] **Step 1 — failing test:** a generated row is `{task, candidate_tools[], chosen_tool}` where `chosen_tool ∈ candidate_tools`, candidates include hard negatives (sibling tools), and `lane == "vox_tool_selection"`:
```rust
#[test]
fn selection_rows_have_chosen_within_candidates_and_negatives() {
    let rows = generate_tool_selection_rows(&sample_surface(), 50);
    assert!(!rows.is_empty());
    for r in &rows {
        let cands = r["candidate_tools"].as_array().unwrap();
        assert!(cands.len() >= 4, "need distractors");
        assert!(cands.iter().any(|c| c == &r["chosen_tool"]), "chosen must be a candidate");
        assert_eq!(r["lane"], "vox_tool_selection");
    }
}
```
- [ ] **Step 2 — run, expect FAIL.**
- [ ] **Step 3 — implement** `generate_tool_selection_rows(surface, n)`: for each real tool, build a task prompt from its description, sample K-1 hard-negative sibling tools (same product_lane) + the correct one as candidates, emit the selection row. Add `pub mod tool_selection_synth;`.
- [ ] **Step 4 — run, expect PASS.**
- [ ] **Step 5 — commit.**

### Task B1.2: argument-generation corpus  `[sub-agent B]`
**Files:** Create `crates/vox-corpus/src/corpus/argument_generation_synth.rs`; Create `mens/config/mix-argument-generation.yaml`; Modify `mod.rs`

- [ ] **Step 1 — failing test:** a generated row is `{task, tool_name, tool_schema, arguments}` where `arguments` validates against `tool_schema` (use the same JSON-schema check the runtime uses), `lane == "vox_argument_generation"`:
```rust
#[test]
fn arg_rows_validate_against_their_schema() {
    let rows = generate_argument_generation_rows(&sample_surface(), 50);
    assert!(!rows.is_empty());
    for r in &rows {
        let schema = &r["tool_schema"];
        let args = &r["arguments"];
        assert!(crate::corpus::schema_validate(schema, args), "args must satisfy schema for {}", r["tool_name"]);
        assert_eq!(r["lane"], "vox_argument_generation");
    }
}
```
- [ ] **Step 2 — run, expect FAIL.**
- [ ] **Step 3 — implement** `generate_argument_generation_rows`: pull each tool's JSON schema from `input_schemas::tool_input_schema`, synthesize valid argument objects (respecting required/enum/min-max), emit row. Add a small `schema_validate` helper (draft-07 subset) if one isn't already shared.
- [ ] **Step 4 — run, expect PASS.**
- [ ] **Step 5 — commit.**

### Task B1.3: wire both as pipeline stages
**Files:** Modify `crates/vox-ml-cli/src/commands/mens/populi/action_prelude.rs` (+ `pipeline.rs`) — add stages `ToolSelectionSynth`, `ArgumentGenerationSynth` BEFORE `Mix`; Modify `corpus/mod.rs` CorpusAction.
- [ ] **Step 1 — failing test:** `PipelineStage::as_str` covers both new stages; `all_possible_stages` orders them before `Mix`. (Mirror the existing `AgentTraceIngest`/`ReviewToDpo` wiring.)
- [ ] **Step 2-4 — TDD** as in the existing stage pattern.
- [ ] **Step 5 — commit.**

**Phase B1 gate:** `cargo test -p vox-corpus --lib corpus::tool_selection_synth corpus::argument_generation_synth` green; running `vox mens corpus` for both lanes produces ≥ the readiness threshold rows (see B2.3 gate).

---

## Phase B2 — Vox & Rust corpus readiness  `[PARALLEL track 2; 2 sub-agents]`

**Goal:** The vox-lang and rust spokes already have corpora (mix-vox-lang.yaml, mix-rust.yaml) — ensure VOLUME + DIVERSITY so training is worth it. Add a readiness gate. Depends on B0. Two independent spokes → **2 sub-agents**.

### Task B2.1: corpus-readiness gate module
**Files:** Create `crates/vox-corpus/src/corpus/corpus_readiness.rs`; Modify `mod.rs`
- [ ] **Step 1 — failing test:**
```rust
#[test]
fn readiness_requires_min_rows_and_diversity() {
    let report = assess_corpus_readiness(&jsonl_path, MinReadiness{ rows: 2000, ast_diversity: 0.40 });
    assert!(report.rows_ok == (report.rows >= 2000));
    assert!(report.diversity_ok == (report.ast_diversity >= 0.40));
    assert_eq!(report.ready, report.rows_ok && report.diversity_ok);
}
```
- [ ] **Step 2 — run, expect FAIL.**
- [ ] **Step 3 — implement** `assess_corpus_readiness` reusing `vox_eval::eval_semantic_entropy` for `ast_diversity`.
- [ ] **Step 4 — run, expect PASS.**
- [ ] **Step 5 — commit.**

### Task B2.2: `vox mens corpus readiness --spoke <name>` command + CI gate
**Files:** Modify `crates/vox-ml-cli/src/commands/corpus/mod.rs` (+ stats.rs)
- [ ] TDD a `CorpusAction::Readiness` that prints a `corpus_readiness.json` and exits non-zero when not ready. Commit.

### Task B2.3: per-spoke readiness thresholds in config
**Files:** Modify `mens/config/domain-profiles.yaml` (add `min_readiness:` per spoke)
- [ ] Add `min_readiness: { rows: <n>, ast_diversity: 0.40 }` to all 4 fine-tuned spokes (vox/rust: rows 3000; tool-selection/argument-generation: rows 2000). Commit.

**Phase B2 gate:** `vox mens corpus readiness --spoke vox-lang|rust|tool-selection|argument-generation` all report `ready: true`. **This gate blocks B8 training for any spoke that isn't ready** (the executor must grow the corpus first — flag it, don't train on thin data).

---

## Phase B3 — Dynamic layer: semantic tool retrieval  `[PARALLEL track 3]`

**Goal:** Inject only top-K relevant tool schemas per task instead of the whole registry. New skill = new registry row → retrievable immediately, no retraining. Depends on B0. Independent of corpora.

### Task B3.1: tool-retrieval module
**Files:** Create `crates/vox-orchestrator-mcp/src/tool_retrieval.rs`; Modify `lib.rs`
- [ ] **Step 1 — failing test:** given a task string and the tool registry, `select_tools(task, k)` returns ≤ k tools ranked by similarity, and a task clearly about one tool surfaces it in the top-1:
```rust
#[test]
fn retrieval_surfaces_relevant_tool_top1() {
    let reg = test_registry(); // includes vox_git_status, vox_repo_query_text, ...
    let hits = select_tools(&reg, "what files changed in the repo", 5);
    assert!(hits.len() <= 5);
    assert_eq!(hits[0].name, "vox_git_status");
}
```
- [ ] **Step 2 — run, expect FAIL.**
- [ ] **Step 3 — implement** `select_tools`: embed task + tool descriptions (reuse the existing retrieval/embedding infra in `memory_tools/retrieval.rs`; if a local embedder isn't available, fall back to BM25/lexical over name+description — the module already has a lexical fallback path). Rank, return top-K with schema.
- [ ] **Step 4 — run, expect PASS.**
- [ ] **Step 5 — commit.**

### Task B3.2: wire retrieval into the tool-call request path
**Files:** Modify `crates/vox-orchestrator-mcp/src/lib.rs` (the place that assembles tool context for a model turn)
- [ ] TDD: a request with N≫K registered tools sends only top-K schemas to the model; assert the assembled context contains the retrieved subset, not the full registry. Commit.

**Phase B3 gate:** `cargo test -p vox-orchestrator-mcp --lib tool_retrieval` green; an integration test shows context size bounded by K regardless of registry size.

---

## Phase B4 — Dynamic layer: schema-constrained decoding at generation  `[PARALLEL track 4]`

**Goal:** Emitted tool-call args are schema-valid by construction (not post-hoc). Adopt vLLM guided decoding / XGrammar; do not hand-roll. Depends on B0. Independent.

### Task B4.1: schema→grammar bridge
**Files:** Create `crates/vox-orchestrator-mcp/src/schema_guided.rs`; Modify `constrained_decoding.rs`
- [ ] **Step 1 — failing test:** `to_guided_decoding_spec(schema)` produces a vLLM `guided_json` request field equal to the tool's JSON schema (vLLM consumes JSON-schema directly via `guided_json`):
```rust
#[test]
fn schema_becomes_guided_json_request() {
    let schema = serde_json::json!({"type":"object","required":["path"],"properties":{"path":{"type":"string"}}});
    let spec = to_guided_decoding_spec(&schema);
    assert_eq!(spec["guided_json"], schema);
    assert_eq!(spec["guided_decoding_backend"], "xgrammar");
}
```
- [ ] **Step 2 — run, expect FAIL.**
- [ ] **Step 3 — implement** `to_guided_decoding_spec`: map a tool's `input_schemas::tool_input_schema` result into the vLLM sampling-params `guided_json` + `guided_decoding_backend: xgrammar`. Extend `ConstrainedDecodingMode` with a `SchemaGuided(serde_json::Value)` variant.
- [ ] **Step 4 — run, expect PASS.**
- [ ] **Step 5 — commit.**

### Task B4.2: thread the schema for the selected tool into the generation request
**Files:** Modify the serving request builder (B6's `vllm_lora.rs` once it exists, else stub the seam here)
- [ ] TDD: when the harness has selected a tool, the generation request carries that tool's `guided_json`. (If B6 not yet merged, land the pure mapping + a seam fn `attach_guided_decoding(req, tool)`; B6 calls it.) Commit.

**Phase B4 gate:** `cargo test -p vox-orchestrator-mcp --lib schema_guided` green.

---

## Phase B5 — Cloud spot pipeline completion (RunPod default)  `[PARALLEL track 5 — LONG POLE; single agent, sequential sub-tasks]`

**Goal:** Complete the 3 missing pieces in `crates/vox-populi/src/mens/cloud/`: job submission, log streaming, checkpoint sync — exposed as `vox mens train --remote`. RunPod default; Vast opt-in. Depends on B0. **Sub-tasks are sequential** (submit → poll/logs → checkpoints → orchestrate). No paid GPU is provisioned in B5 except one tiny smoke run gated behind `--apply` (see B5.5).

### Task B5.1: job submission (RunPod)
**Files:** Create `crates/vox-populi/src/mens/cloud/job.rs`; Modify `cloud/mod.rs`, `cloud/runpod_provider.rs`
- [ ] **Step 1 — failing test** (mock HTTP): `submit_job(provider, JobSpec{ image, gpu, corpus_uri, spoke, preset, budget })` returns a `JobHandle{ id, provider }`; on budget-exceeded it errors before any API call:
```rust
#[test]
fn submit_refuses_over_budget_before_calling_provider() {
    let spec = JobSpec{ est_cost_usd: 50.0, ..fixture() };
    let err = submit_job(&MockProvider::recording(), &spec, Budget{ max_usd: 10.0 }).unwrap_err();
    assert!(matches!(err, JobError::OverBudget{..}));
    assert_eq!(MockProvider::calls(), 0, "must not hit provider when over budget");
}
```
- [ ] **Step 2 — run, expect FAIL.**
- [ ] **Step 3 — implement** `submit_job`: estimate via existing `estimator.rs`; check `budget.rs`; build the RunPod create-pod request (image `VOX_CLOUD_IMAGE`, GPU from preset rung, onstart that runs `vox mens train --local` against the synced corpus); return handle. Vast path mirrors via the existing `vast.rs` ask flow.
- [ ] **Step 4 — run, expect PASS.**
- [ ] **Step 5 — commit.**

### Task B5.2: log streaming + status poll
**Files:** Modify `cloud/job.rs`
- [ ] TDD `poll_job(handle) -> JobStatus{Running{logs_tail}, Succeeded{checkpoint_uri}, Failed{reason}}` against a mock; on `Failed` it must surface the provider reason (not a generic error). Commit.

### Task B5.3: checkpoint sync (down) + corpus sync (up)
**Files:** Modify `cloud/job.rs`
- [ ] TDD `sync_corpus_up(local_dir) -> uri` and `sync_checkpoint_down(uri, local_dir) -> adapter_path`. For Vast (spot-interruptible) the design syncs checkpoints **every N minutes**; assert the sync interval is configurable and defaults to frequent (≤10 min). Commit.

### Task B5.4: `vox mens train --remote` orchestration
**Files:** Modify `crates/vox-ml-cli/src/commands/mens/` (train command), `cloud/mod.rs`
- [ ] TDD the orchestration ordering with mocked steps: estimate → (budget gate) → sync_corpus_up → submit_job → poll-until-terminal (stream logs) → sync_checkpoint_down → eval-gate → `DomainRouter::register(spoke, adapter_path)`. Assert eval-gate runs BEFORE register and that a failed gate does NOT register. Commit.

### Task B5.5: live smoke (gated, real money, tiny)
**Files:** none (procedure)
- [ ] **Procedure (NOT auto-run):** `vox mens train --remote --spoke tool-selection --provider runpod --preset qwen3_24g --max-budget 5 --apply` on a *tiny* corpus subset; confirm the full loop produces a registered adapter for <$5. **Gate:** loop completes, adapter file present, eval-gate ran. This is the proof the pipeline is executable end-to-end before B8 trains all four. **Do not run without explicit human go-ahead** (spends money).

**Phase B5 gate:** `cargo test -p vox-populi --lib cloud::` green; B5.5 smoke deferred to a human-approved moment.

---

## Phase B6 — Serving: vLLM multi-LoRA hot-swap  `[needs B0; parallel to B1-B5]`

**Goal:** One Qwen3 base + 4 adapters hot-swapped at request time via vLLM runtime LoRA. Wire `DomainRouter`/`route_by_signal` → adapter name. No custom merge.

### Task B6.1: vLLM LoRA client
**Files:** Create `crates/vox-populi/src/mens/serving/mod.rs` + `vllm_lora.rs`
- [ ] **Step 1 — failing test** (mock HTTP): `ensure_adapter_loaded(name, path)` calls vLLM's `/v1/load_lora_adapter` once and is idempotent (LRU cache — second call no-ops); `chat(req, adapter=name)` sets the model field to the adapter name:
```rust
#[test]
fn adapter_load_is_idempotent_and_chat_targets_adapter() {
    let s = VllmLora::new(MockVllm::new());
    s.ensure_adapter_loaded("rust", "/a/rust").unwrap();
    s.ensure_adapter_loaded("rust", "/a/rust").unwrap();
    assert_eq!(MockVllm::load_calls(), 1, "idempotent");
    let req = s.build_chat("write a fn", "rust");
    assert_eq!(req["model"], "rust");
}
```
- [ ] **Step 2 — run, expect FAIL.**
- [ ] **Step 3 — implement** `VllmLora`: POST `/v1/load_lora_adapter` (requires `VLLM_ALLOW_RUNTIME_LORA_UPDATING=1`), LRU of loaded names (cap configurable), `build_chat` sets `model=<adapter>` + attaches `guided_json` via B4's `attach_guided_decoding`.
- [ ] **Step 4 — run, expect PASS.**
- [ ] **Step 5 — commit.**

### Task B6.2: route_by_signal → adapter
**Files:** Modify `crates/vox-populi/src/mens/tensor/domain_router.rs`
- [ ] TDD: `route_by_signal(file, "lane:vox_rust_authoring")` → spoke `rust` → `DomainRouter::route("rust")` → adapter path; serving calls `ensure_adapter_loaded` then `build_chat`. Assert an unknown lane falls back to the base model (no adapter), never errors. Commit.

**Phase B6 gate:** `cargo test -p vox-populi --lib serving:: domain_router::` green.

---

## Phase B7 — Evals: BFCL + per-spoke gates  `[needs B0; parallel]`

**Goal:** Add BFCL for the harness adapters; keep MultiPL-E pass@k + per-spoke gates; target per-spoke Flash/Sonnet parity.

### Task B7.1: BFCL metric producer + gate
**Files:** Create `crates/vox-ml-cli/src/commands/mens/eval_gate/bfcl.rs`; Create `mens/config/eval-gates-bfcl.yaml`; Modify `eval_gate/check_run.rs` + `policy.rs`
- [ ] **Step 1 — failing test:** mirror the existing `rust_gate_not_applicable_when_metric_absent` pattern — a `bfcl_accuracy` gate reads `eval_results.json`, passes when ≥ threshold, is "not applicable" when absent:
```rust
#[test]
fn bfcl_gate_blocks_below_threshold_and_skips_when_absent() {
    write_eval(&dir, r#"{"bfcl_accuracy":0.55}"#);
    assert!(!gate_passes(&dir, "bfcl_accuracy", 0.70));
    write_eval(&dir, r#"{"vox_parse_rate":0.9}"#);
    assert!(gate_result(&dir, "bfcl_accuracy", 0.70).message.contains("not applicable"));
}
```
- [ ] **Step 2 — run, expect FAIL.**
- [ ] **Step 3 — implement** the `bfcl_accuracy` producer (runs a held-out BFCL-style pack against the harness adapters and writes `bfcl_accuracy` into `eval_results.json`) + the gate handler (copy the rust-gate handler shape, including the "not applicable when absent" branch).
- [ ] **Step 4 — run, expect PASS.**
- [ ] **Step 5 — commit.**

### Task B7.2: per-spoke parity thresholds
**Files:** Modify `mens/config/eval-gates-*.yaml`
- [ ] Set per-spoke gate thresholds toward the parity target: vox parse-rate, `rust_compile_rate`, `bfcl_accuracy` (harness), `tool_call_valid_json_rate` (arg-gen). Document each as "block: true" once a baseline is measured (start `block: false`, flip after first real run). Commit.

**Phase B7 gate:** `cargo test -p vox-ml-cli --lib mens::eval_gate` green (incl. new BFCL tests).

---

## Phase B8 — End-to-end: train, gate, register, serve, validate  `[SEQUENTIAL — LAST; needs B1-B7]`

**Goal:** Use the completed pipeline to actually produce the 4 adapters and prove per-spoke parity. **Spends money — human-gated.**

**Sub-agent fan-out:** the 4 trainings are independent → fan out 4 (one per spoke) **only after** each spoke passes its B2 readiness gate. Serving/eval validation is sequential after all four register.

### Task B8.1: readiness pre-flight (no GPU)
- [ ] Run `vox mens corpus readiness --spoke <each>`; **any spoke not ready → STOP and grow its corpus** (re-run B1/B2 synth), do not train. Record which spokes are ready.

### Task B8.2: train each ready spoke on RunPod  `[fan out per ready spoke]`
- [ ] For each ready spoke: `vox mens train --remote --spoke <name> --provider runpod --preset <rung> --max-budget <cap> --apply`. The orchestration (B5.4) runs estimate→budget→sync→train→logs→checkpoint→eval-gate→register. **Gate per spoke:** eval-gate passed AND adapter registered in DomainRouter. Capture cost + metrics.

### Task B8.3: serve all four via vLLM and validate routing
- [ ] Start the vLLM base server (`VLLM_ALLOW_RUNTIME_LORA_UPDATING=1`); for each lane signal, assert `route_by_signal`→correct adapter→`ensure_adapter_loaded`→a real completion. Assert hot-swap across all 4 within one base instance.

### Task B8.4: parity validation
- [ ] Run the per-spoke eval packs (B7) and compare to the Flash/Sonnet parity targets on each domain. Record a `parity_report.json`. **Gate:** each spoke meets or transparently misses (with the gap documented) its domain target. Flip the B7.2 gates to `block: true` for spokes that met target.

**Phase B8 gate (final):** 4 adapters registered + served from one Qwen3 base; `parity_report.json` written; per-spoke gates green or gaps explicitly recorded.

---

## Self-review (author's checklist — done)

- **Spec coverage:** Hub (B0.2/B0.3) · 4 spokes (B0.1) · harness corpora (B1) · vox/rust readiness (B2) · tool retrieval (B3) · schema-constrained decoding (B4) · cloud RunPod pipeline (B5) · vLLM serving (B6) · BFCL + per-spoke gates (B7) · end-to-end parity (B8). Dynamic-layer "no retrain for new skills" = B3 (retrieval over the registry) — covered. V2 deferrals (RLVR/distillation/DPO) intentionally absent.
- **Placeholders:** none — empirical ML tasks (B2/B8) are stated as procedures + explicit gates (correct altitude; "complete code" does not apply to a training run), code tasks carry real signatures/tests.
- **Type consistency:** `JobSpec`/`JobHandle`/`JobStatus`/`JobError` (B5), `VllmLora`/`ensure_adapter_loaded`/`build_chat` (B6), `to_guided_decoding_spec`/`attach_guided_decoding` (B4→B6), `assess_corpus_readiness`/`MinReadiness` (B2), `select_tools` (B3) used consistently across phases.
- **Parallelism markers:** B0 sequential-first; B1-B7 parallel after B0 (fan-out noted per phase); B8 sequential-last with a 4-way fan-out inside.
