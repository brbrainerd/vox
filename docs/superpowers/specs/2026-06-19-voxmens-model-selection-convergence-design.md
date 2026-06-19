---
title: "VoxMens Model-Selection & Routing — Convergence Design (Split C, revised)"
description: "Audit-grounded design that collapses the planned 'Split C' new model-registry/resolver/router into the EXISTING selection+routing infrastructure: one shared catalog of model facts, reuse of vram_autodetect / domain_router / execution_planner, and a documented boundary between inference-egress selection and training-base selection."
category: "architecture"
status: "current"
---

# VoxMens Model-Selection & Routing — Convergence Design

> **Status:** Design spec (revises the original `2026-06-18-voxmens-split-C-selection-routing-serving.md`). Feeds the convergent Split C plan.
> **Date:** 2026-06-19
> **Basis:** 4-agent codebase audit (2026-06-19) of every model-selection / routing / egress / catalog surface.

## 1. Problem

The original Split C plan would add a **third** model-selection/routing system: `mens/config/model-registry.yaml` + `model_registry.rs` (VRAM resolver) + `router.rs`. The audit shows **two already exist**, and most of Split C's "new" pieces are already built. Building a third would create the exact duplicate-SSOT drift the repo fights.

## 2. What already exists (do not duplicate)

### 2.1 Inference selection + routing (vox-orchestrator, L3) — the mature "intelligent" engine
- `crates/vox-orchestrator/src/models/select.rs` — canonical `select()`/`decide()`: 3-axis intent (cost/responsiveness/intelligence), pre-baked intents, capability filtering, premium aliases, confidence gating.
- `models/registry.rs` `ModelRegistry` (telemetry-calibrated, Thompson-bandit), `routing/engine.rs`, `tier_cascade.rs` (Economy/Standard/Strong), `subagent_dispatch.rs`.
- SSOT contracts: `contracts/orchestration/model-pins.v1.yaml`, `model-routing.v1.yaml`, **`model-catalog.bootstrap.v1.json`** (100+ models: id, provider, params, capabilities, strengths, pricing, `is_free`).
- `crates/vox-config/src/resolve_egress.rs` (egress SSOT) + `key_guard::available_inference_providers()` + the L0 `vox-llm-config` keys registry.

### 2.2 Training selection (MENS) — most of Split C is already here
- **Spoke SSOT** (Plan A, landed): `mens/config/domain-profiles.yaml` + `crates/vox-populi/src/mens/tensor/domain_profiles.rs` (`DomainProfile`, `EffectiveDomainProfile`, `TrainMethod`, `SpokeBase{model,method,preset}`, `SpokeRouter{triggers,priority}`).
- **VRAM detection + preset selection — BUILT:** `vram_autodetect::get_system_vram_gb() -> Option<f32>` and `auto_preset(device_is_cuda, vram_gb) -> Option<&'static str>` (VRAM ladder), `HardwareRegistryV2::probe()`, `mens/config/gpu-specs.yaml` (presets + `max_vram_mb`).
- **Adapter router — BUILT:** `domain_router.rs` `DomainRouter` (`register` / `route(domain) -> Option<&PathBuf>` / `discover(artifacts_dir)`).
- **Method → kernel dispatch — BUILT:** `execution_planner::resolve_kernel(&FineTuneContract) -> PopuliTrainBackend` (Qlora→CandleQlora, Lora→BurnLora).
- **Default base id:** `vox-populi::mens::DEFAULT_MODEL_ID = "Qwen/Qwen2.5-Coder-7B-Instruct"`, `resolve_default_model_id()`.

## 3. Constraints that shape convergence

1. **Layer wall.** `vox-populi` (L3) must NOT depend on `vox-orchestrator` (L3 sibling). So the inference `ModelRegistry` **code** is unreachable from training. The catalog **JSON contract is reachable** (it's data) via a small read-only loader.
2. **Different model populations.** Inference models are cloud/OpenRouter-served (claude/gpt/gemini, slugs like `qwen/...`). Training bases are **local, fine-tunable HF repos** (`Qwen/Qwen2.5-Coder-7B-Instruct`) with QLoRA-VRAM floors. These overlap in *family* but not in *identifier or required metadata*. Forcing training onto the inference catalog 1:1 is a poor fit.

**Therefore convergence ≠ "import the inference registry."** It means: one shared *fact source*, reuse of the training infra, no parallel catalog/resolver/router, and a documented boundary.

## 4. The convergent design

### 4.1 One shared source of model FACTS
`contracts/orchestration/model-catalog.bootstrap.v1.json` remains the single catalog of model facts (params, capabilities, family, `is_free`). The training side gains a **tiny read-only loader** (in `vox-populi`, reading the JSON contract — data, not a code dep) for the fields training needs. **No new `model-registry.yaml` catalog is created.**

### 4.2 Training-base selection = thin policy over facts + minimal overlay
A spoke's `base.model` (in `domain-profiles.yaml`) is a **capability tag** (`small_code` / `strong_code` / `agentic`) or a concrete HF id. A small `spoke_base_resolver` (new, `vox-populi`) resolves it to a concrete HF id that **fits host VRAM**, by:
1. taking the spoke's desired capability tag,
2. consulting a **minimal training overlay** — a `train_bases:` section **added to the existing `gpu-specs.yaml`** (NOT a new file): `{ tag → [{hf_id, qlora_vram_floor_mb, methods[]}] }`, referencing catalog families where they overlap,
3. picking the largest variant whose `qlora_vram_floor_mb <= get_system_vram_gb()*1024` (reusing `vram_autodetect`), fail-closed if none fit.

This mirrors the inference `select()` *fit pattern* without importing it. The overlay is minimal (a handful of trainable bases) because the trainable-local population is small and distinct.

### 4.3 Method dispatch = wire the existing kernel resolver
Wire `EffectiveDomainProfile.base.method` (`TrainMethod`) → the training path so `execution_planner::resolve_kernel` selects the backend. `RagOnly`/`PromptOnly` skip training; methods with no real backend **fail closed** with a clear error (no silent QLoRA fallback). No new dispatch logic.

### 4.4 Routing = extend the existing router
Add `DomainRouter::route_by_signal(signal, profiles) -> Option<String>` that consumes `SpokeRouter.triggers` + `priority` from `domain-profiles.yaml` (deterministic name tie-break on equal priority), complementing the existing path-based `route(domain)`. No new router module.

### 4.5 Validation
Extend `spoke_validate` so a spoke's `base.model` capability tag (or id) is resolvable in the overlay, surfaced via the existing `vox ci spoke-check`.

## 5. The boundary (why inference and training selection stay distinct)

| | Inference-egress selection | Training-base selection |
|---|---|---|
| Owner | `vox-orchestrator` `select()`/`ModelRegistry` | `vox-populi` `domain-profiles` + `spoke_base_resolver` |
| Picks | request → served model → provider egress | spoke → fine-tunable HF base + preset + method |
| Population | cloud/OpenRouter served | local trainable HF repos |
| Shared | **model-catalog.bootstrap.v1.json (facts)** | same JSON (read-only, for overlapping facts) |
| Signals | intent axes, capabilities, telemetry/bandit | capability tag, host VRAM, method |

They are **not force-merged**: different populations + the L3↔L3 layer wall make a single facade higher-risk than its value (the "full unification" option was rejected). The shared catalog JSON is the SSOT seam.

## 6. Gains / losses

**Gains:** no third system; one model-facts SSOT (the catalog JSON); reuse of `vram_autodetect` + `domain_router` + `execution_planner` (Split C shrinks ~70%); spoke SSOT stays `domain-profiles.yaml`; drift-guarded by the existing `vox ci spoke-check` + config-registry parity.
**Losses / accepted costs:** a *minimal* training overlay (`train_bases:` in `gpu-specs.yaml`) is still needed because the inference catalog doesn't carry HF-repo ids or QLoRA VRAM floors — but it's a small policy table, not a parallel catalog. Inference and training selection remain two thin policies (intentional, per §5).

## 7. Scope delta vs original Split C
- **Dropped:** new `mens/config/model-registry.yaml`, new `model_registry.rs`, new `router.rs`, new `detect_available_vram_mb()`.
- **Kept (real gaps):** (1) capability/VRAM base resolver over shared facts + overlay; (2) `base.method` → `execution_planner` wiring; (3) `domain_router` triggers/priority extension; (4) `spoke_validate` + e2e dry-run.
- **Untouched:** the entire inference `select()`/egress stack.
