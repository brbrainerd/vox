---
title: "Vox-Populi Extraction Follow-Up Plan (2026)"
description: "Plan for the remaining code-motion work after the plugin system foundation landed. Covers vox-populi mens/tensor residual, vox-populi transport, vox-tensor, vox-oratio Whisper, and vox-browser extraction into their respective plugin scaffolds."
category: "Architecture SSOTs"
status: "research"
training_eligible: true
training_rationale: "Honest accounting of what plugin extractions are still pending after the foundation landed; sequencing for follow-up sessions."
---

# Vox-Populi Extraction Follow-Up Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Parent spec:** [`plugin-system-redesign-2026.md`](plugin-system-redesign-2026.md)

**Predecessor work:** SP1–SP8 implementation plans + the SP3 sub-batches A–D, NVML extraction, and mesh scaffold all landed on the `claude/infallible-lalande-baf300` branch (~70 commits). This follow-up plan picks up where they stopped.

> **Status update (2026-05-08):** Browser, oratio, and transport extractions all landed end-to-end in this session. Verified slim-core: `chromiumoxide`, `symphonia`, `rubato`, `candle-core`, `burn`, `wgpu`, `nvml-wrapper` are all OUT of `cargo tree -p vox-cli` for default builds. `populi-transport` is now opt-in (vox-orchestrator's default no longer pulls axum's mesh router, JWT, dashmap, blake3, ed25519-dalek, turso). vox-skills retirement is partial: parser/bundle/manifest types moved to `vox-plugin-host::skill_*`; `SkillRegistry` (DB-backed install/search/hydrate) remains in vox-skills as it has different semantics. Remaining work: vox-populi mens/tensor candle module cleanup (still feature-gated, kept for the in-tree merge CLI's adapter-schema bridge), full `SkillRegistry` unification.
>
> **Status update (2026-05-07):** Completed candle/NVML extraction from vox-populi mens: deleted `candle_qlora_merge.rs`, `adapter_schema_v3.rs`, `hardware/nvml.rs`. Migrated `vox-mens schola merge-qlora` CLI to `cached_code_plugin("mens-candle-cuda")` dispatch. Inline serde-only v2/v3 schema in CLI (no candle deps). Burn/wgpu files (`burn_stack.rs`, `lora/`) remain — they are `mens-gpu` feature-gated and not candle (Unit 3 scope). All 3 CI gates pass; `cargo tree -p vox-cli` shows no candle/burn/wgpu/nvml.

## Honest accounting of what landed vs. what's deferred

The foundation is sound: ABI-versioned `vox-plugin-host` loader works end-to-end (proven by 11 host integration tests + the candle-cuda gating spike), seven extension-point traits exist, the SSOT catalog is enforced by build-time + runtime validators, and `vox plugin` / `vox bundle` CLI commands work for install / remove / build / verify. But several "extraction complete" claims overstated reality. This plan corrects the record and scopes what remains.

### What's actually in plugins

| Capability | Plugin crate | Status |
|---|---|---|
| Mens/Candle training loop | `vox-plugin-mens-candle-cuda` | ✅ `run_full_training` real (3,128 LOC moved) |
| NVML hardware probe | `vox-plugin-nvml-probe` | ✅ Full extraction |
| Compiler skill | `vox-plugin-skill-compiler` | ✅ Full migration |
| 7 other agent skills | `vox-plugin-skill-{git,memory,orchestrator,rag,testing,testing-validate,v0}` | ✅ Full migration |

### What's *scaffolded only* (trait + plugin shell, zero code moved)

| Capability | Plugin crate | LOC still in source crate |
|---|---|---|
| Tensor backend (Burn/wgpu) | `vox-plugin-tensor-burn-wgpu` | 3,054 LOC in `vox-tensor/src/` |
| Audio capture (Whisper) | `vox-plugin-oratio` | 2,275 LOC in `vox-oratio/src/backends/` |
| Audio capture (mic) | `vox-plugin-oratio-mic` | (subset of vox-oratio) |
| Cloud sync | `vox-plugin-cloud` | mens-cloud feature in vox-populi |
| Script execution | `vox-plugin-script-execution` | (vox-eval, vox-exec-grammar) |
| Mesh transport | `vox-plugin-populi-mesh` (composite) | 4,990 LOC in `vox-populi/src/transport/` |

### What's *partially extracted*

- **vox-populi mens residual**: Candle files deleted in this session (2026-05-07): `candle_qlora_merge.rs`, `adapter_schema_v3.rs` removed from `vox-populi/src/mens/tensor/`; `nvml.rs` removed from `vox-populi/src/mens/hardware/`. `vox-mens` `schola merge-qlora` CLI now dispatches via `cached_code_plugin("mens-candle-cuda")`. Remaining in `mens/tensor/`: Burn/wgpu files (`burn_stack.rs`, `lora/`, `optim.rs`, `train.rs`, `tensor.rs`) are feature-gated by `mens-gpu` and are NOT candle — they belong to the Burn/wgpu training path (Unit 3 scope). `backend_candle_qlora.rs` remains but is 100% plugin dispatch (no direct candle dep).

### What's not a plugin yet but probably should be

- **vox-browser**: chromiumoxide CDP wrapper, pulled into `vox-cli` unconditionally (no feature gate). No plugin scaffold exists.
- **vox-dashboard**: Axum SPA host with embedded assets.
- **vox-publisher**: social/news publishing adapters.
- **vox-scientia-***: scholarly ingestion + publishing pipeline.
- **vox-gamify**: gamification.
- **vox-audio-ingress**: standalone binary that unconditionally pulls `stt-candle` from vox-oratio.

## Slim-core diagnostic

Default `cargo build -p vox-cli` produces a **66 MB debug binary** without ML deps. The feature-gating discipline works: candle/burn/wgpu/qlora do NOT compile in the default build. But:

- `vox-cli` enables `vox-populi/transport` **unconditionally**, pulling axum, tower-http, jwt, ed25519-dalek, blake3, dashmap, turso into every CLI build.
- `vox-audio-ingress` enables `vox-oratio/stt-candle` **unconditionally**, pulling the full Candle Whisper stack.
- `vox-cli` depends on `vox-browser` **unconditionally** (no feature gate).

True slim-core requires extracting these into plugins so even consumers' default features don't pull them in.

## Five extraction units

Each unit is independently shippable. Sequence them or parallelize across worktrees.

### Unit 1: vox-populi mens/tensor residual extraction (~7,852 LOC)

**Scope:** Move the remaining candle-using files from `vox-populi/src/mens/tensor/` into `vox-plugin-mens-candle-cuda`:
- `candle_model_qwen.rs` (764 LOC) — Qwen3.5/Qwen2 transformer block, attention, MLP, RoPE, KV cache
- `candle_inference_serve.rs` (494 LOC) — inference server
- `candle_qlora_merge.rs` (389 LOC) — adapter merge logic
- `burn_stack.rs` (128 LOC) — Burn tensor stack
- Supporting files in `mens/tensor/lora/`, `mens/tensor/ce_*`, etc.

**Approach:** Add `MlBackend::run_inference(model_handle, prompt_json) -> RString` and `MlBackend::merge_adapter(base_path, adapter_path, dest_path) -> ()` methods. Bump ABI 6→7. Move the implementations into the plugin behind those methods. Update `vox-populi`'s `mens` module to dispatch via plugin host (similar to SP3 sub-batch D for training).

**Acceptance:** `vox-populi/src/mens/tensor/` no longer contains direct candle-core imports. The plugin's integration tests cover load_model + run_inference + merge_adapter end-to-end.

**Risk:** Inference server is HTTP-bound (probably axum) — the plugin would need to spin up its own server or the host would need a `serve_model` callback. Decide architecture before code-moving.

### Unit 2: vox-populi transport extraction (~4,990 LOC)

**Scope:** Move `vox-populi/src/transport/` into `vox-plugin-populi-mesh` (currently a stub). Files: `mod.rs`, `auth.rs`, `router.rs`, `mesh_replay.rs`, `result_attestation.rs`, `handlers/` (764 LOC), `store/` (1,153 LOC). Plus `http_client.rs`, `http_lifecycle.rs`, `node_registry.rs` (~1,105 LOC).

**Approach:** The `MeshDriver` trait already exists from SP7 with method shapes (`start_transport`, `stop_transport`, `dispatch`, `node_join`, `list_nodes`). Real implementations replace the stub `RErr("not yet implemented")` returns. The plugin pulls the heavy deps (axum, tower-http, jwt, dashmap, turso). `vox-cli` drops `vox-populi/transport` from its default features.

**Acceptance:** `vox-populi/src/transport/` deleted. `vox-cli` builds without axum in the dep graph (verify with `cargo tree`). Existing populi integration tests pass via the plugin path.

**Risk:** Heavy. The transport module is tightly coupled to `vox-populi`'s NodeRegistry, store backends (Turso schema), and JWT secret resolution (vox-secrets). Extraction may require the plugin to depend on vox-secrets + vox-db too — that's accepted (the same pattern as mens-candle-cuda's vox-tensor + vox-corpus deps).

### Unit 3: vox-tensor extraction (~3,054 LOC)

**Scope:** Move all of `vox-tensor/src/` into `vox-plugin-tensor-burn-wgpu`. Files: `data.rs`, `lora.rs`, `optim.rs`, `vox_nn.rs`, `train.rs`, plus `tensor/{activations,ctor,elemwise,slice_reduce,cat_reshape}.rs`.

**Approach:** TensorBackend trait exists from SP7. Real implementations replace stubs. `vox-tensor` either becomes a thin types-only crate or is deleted entirely.

**Acceptance:** `vox-tensor` is empty or deleted. Burn+wgpu no longer compile in default builds of any consumer.

**Risk:** vox-tensor is consumed by vox-populi mens (Unit 1's territory) and possibly other crates. Sequence after Unit 1 so the consumers exist as plugin-host dispatches before vox-tensor's body moves.

### Unit 4: vox-oratio Whisper extraction (~2,275 LOC)

**Scope:** Move `vox-oratio/src/backends/{candle_engine,candle_whisper,logit_processors,audio_io,multilingual}.rs` into `vox-plugin-oratio`.

**Approach:** AudioCapture trait exists from SP7. Add a sibling trait or method group for speech-to-text (the current trait covers capture, not transcription). Possibly: split into `AudioCapture` (mic / device) and `SpeechToText` (Whisper). Bump ABI.

**Acceptance:** `vox-oratio/src/backends/` no longer has candle-* deps. `vox-audio-ingress` switches to plugin dispatch and drops its `stt-candle` feature.

**Risk:** Audio pipelines are state-heavy (sample rates, frame buffers, streaming). The cdylib boundary needs to support streaming reads — `RVec<u8>` chunked reads work, but design carefully.

### Unit 5: vox-browser extraction (~thin wrapper + chromiumoxide indirect deps)

**Scope:** Wrap `vox-browser`'s public API in a new `vox-plugin-browser` cdylib + a new `BrowserAutomation` extension-point trait (currently no scaffold).

**Approach:** Define `BrowserAutomation` trait in `vox-plugin-api/src/extensions/browser_automation.rs`. Bump ABI. Move chromiumoxide-using code into the plugin. Make `vox-browser` either a thin types-only crate or delete it.

**Acceptance:** `vox-cli` no longer transitively depends on chromiumoxide. CDP automation works via plugin dispatch.

**Risk:** Lower than 1-4 — vox-browser is smaller and has fewer consumers.

## Sequencing recommendation

```
Unit 1 (mens residual)   ←── Unit 3 (tensor) depends on Unit 1's consumer migration
                              Unit 2 (transport) is independent — can run in parallel
Unit 4 (oratio whisper)  ←── Independent of mesh/tensor
Unit 5 (browser)         ←── Independent
```

Total estimated LOC to move: **~18,000+** across 5 units. Total estimated effort: **3–5 focused sessions** (Unit 2 is the biggest).

## Cross-cutting items not unit-specific

These accompany or follow Unit 1–5:

### CC-1: Retire `vox-skills` shim entirely

**ARS shim relocation — DONE (2026-05-03).**
Created `crates/vox-openclaw-runtime` as the ARS/OpenClaw facade (successor to the retired three-letter ARS crate name).
All external consumers (vox-cli, vox-actor-runtime, vox-orchestrator) migrated to canonical OpenClaw runtime import paths.
Physical move of the 12 `ars_shim/` files out of vox-skills is **deferred** — `ars_shim/mod.rs` re-exports
`crate::parser::parse_skill_md`, `crate::SkillRegistry`, and `crate::install_builtins` from the vox-skills
crate root, creating a circular dependency if extracted. Those types need to move into `vox-plugin-host` first.

**Full vox-skills deletion still deferred.** Remaining blocker: `parser.rs`, `registry.rs`, `plugin.rs`
consumed by vox-orchestrator's `plugin_skills_bridge` — requires those types to be replicated or moved
into vox-plugin-host before vox-skills can be deleted.

### CC-2: Adopt AgentSkills open standard at SKILL.md level

The interop reviewer found that **agentskills.io** is the de facto universal format with 34+ adopters (Claude Code, Codex, Gemini CLI, Copilot, Cursor, JetBrains, etc.). Vox's format is ~60% compatible already.

Concrete steps (small, ship as one commit):
- Add `name` field to each skill plugin's SKILL.md frontmatter (alias from existing `id`; satisfy the spec's lowercase-hyphen + match-dirname constraint).
- Move Vox extensions into a `metadata.vox-*` block when exporting.
- Document the convention in `docs/src/reference/plugin-manifest.md`.

### CC-3: OpenClaw publish (export) path

`vox-skills/src/ars_shim/openclaw.rs` implements import. Add export: `OpenClawClient::publish_skill()` POSTs to `/v1/skills`. Unlocks publishing to ClawHub (44k+ community skills marketplace).

### CC-4: AgentSkills export shim

`vox skill export --format agentskills <id>` flattens Plugin.toml + SKILL.md into a spec-compliant directory. Makes Vox skills publishable to any AgentSkills-compatible host.

### CC-5: Honest deferred-status notes in existing plans

Already done in this branch — SP3, SP7, populi-mesh source comments, and parent spec all carry status notes acknowledging deferred work.

## Acceptance for the whole follow-up

When all 5 units + CC-1 land:

- `cargo build -p vox-cli` default features: zero candle/burn/wgpu/chromiumoxide/axum/jwt in the dep graph
- `vox-tensor`, `vox-skills` deleted from workspace
- `vox-populi` shrinks to mesh-types + lifecycle (~few hundred LOC)
- All 7 SP7 + 1 SP3-residual + 1 mesh + 1 browser extension points have working plugin implementations
- CC-2 lands (AgentSkills compliance) → Vox skills work in 34+ external tools
- CC-3 + CC-4 land → bidirectional skill exchange with ClawHub and any AgentSkills-compatible host

The result: a genuinely slim Vox core + first-class membership in the cross-vendor agent-skill ecosystem.
