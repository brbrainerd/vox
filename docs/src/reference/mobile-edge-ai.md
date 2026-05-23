---
title: "Mobile and edge AI — SSOT"
description: "Official documentation for Mobile and edge AI — SSOT for the Vox language. Detailed technical reference, architecture guides, and impleme"
category: "Language Reference"
last_updated: "2026-03-24"
training_eligible: true

schema_type: "TechArticle"
---

# Mobile and edge AI — SSOT

This page is the **single place** for how Vox treats **Android / iOS / browser** relative to desktop Mens training, **Ollama**, **mens** coordination, and **GPU** advertisement. It complements [Mens training SSOT](mens-training.md), [mens SSOT](populi.md), and [unified orchestration](orchestration-unified.md).

## Non-goals (near term)

- Running **Ollama** or a full **Ollama-compatible** daemon on stock consumer phones.
- Running **`vox mens train`** with **Candle QLoRA** or **Burn LoRA** *on* the phone (Rust + wgpu/Candle stacks are workstation targets).
- Promising **end-to-end LLM LoRA fine-tuning** on-device with the same maturity as workstation `vox mens train` (industry runtimes still steer operators toward **train off-device, infer on-device** for LLMs).

## Industry context (2025–2026)

- **On-device LLM inference:** Google **LiteRT-LM** is the cross-platform direction for Android, iOS, web, and desktop with hardware acceleration; see [LiteRT-LM](https://ai.google.dev/edge/litert-lm) and [LLM inference (AI Edge)](https://ai.google.dev/edge/mediapipe/solutions/genai/llm_inference). Older MediaPipe-only flows are being superseded; plan migrations against current AI Edge docs.
- **LoRA / adapters:** Practical path is **fine-tune on a workstation or cloud**, then **ship base + adapter** (or converted bundle) -> the device. LiteRT LLM LoRA on-device is still **integration-heavy** (see discussion in [LiteRT issue #1420](https://github.com/google-ai-edge/LiteRT/issues/1420)).
- **Web tier:** **WebGPU** helps browser-side compute but is **not universal** (OS version, browser policy, and security modes can disable it). Treat PWA / WebGPU as an **optional** tier, not the only mobile story.

## Vox tiers

| Tier | Train | Infer | Mens node | Notes |
|------|--------|--------|-----------|--------|
| **Workstation** | `vox mens train` (Burn / Candle) | `vox mens serve`, Ollama, cloud OpenAI-compatible | Yes (`vox-mcp`, `vox run`, `vox populi`) | Default SSOT paths. |
| **Mobile native** | **Off-device** (`mobile_edge` contract / preset) | LiteRT-LM, Core ML, vendor SDKs | Yes — HTTP control plane + [`NodeRecord`](../../../crates/vox-populi/src/lib.rs) | Register capabilities from the app; see mens env vars below. |
| **Browser** | Off-device | WebGPU + WASM (when available) | Optional (HTTP client to mens) | Not WASI `vox run --isolation wasm` (that is desktop Wasmtime). |

## Mobile support boundary (normative)

Mobile support is split across distinct product surfaces. Do not collapse them into one claim.

| Surface | Status | In scope now | Out of scope now |
|---------|--------|--------------|------------------|
| **Mobile browser for Vox-built apps** | Supported direction | `.vox` compiles to web apps that run in mobile browsers; mobile compatibility is a web-stack contract concern | Native-phone parity with server-script runtime semantics |
| **Phone as remote management client** | Supported direction | Phone/browser controls a **remote** Vox host (MCP/orchestrator/Codex) over authenticated network APIs | Local phone execution of the full Vox CLI/toolchain |
| **Native mobile inference participation** | Partially supported | App-owned runtime (LiteRT/Core ML), mens HTTP registration, capability hints (`mobile`, `npu`, `gpu_vulkan`) | On-device Mens training, on-device Ollama daemon |
| **Direct on-device `.vox` script runtime** | Experimental / deferred | Narrow future R&D subset only, if explicitly versioned and capability-scoped | Full parity with workstation `vox run` / Cargo-backed native runtime |

This SSOT does **not** define Vox as a replacement for Kotlin or Swift. The recommended product path is:

- Vox for browser-first full-stack app generation.
- Remote phone management for planning, editing, validation, and orchestration against a remote Vox host.
- Native mobile only where thin wrappers or inference SDK integration are the right boundary.

## Training pathway for mobile (`mobile_edge`)

1. On a **GPU or CPU workstation**, run:

   `vox mens train … --deployment-target mobile_edge`

   or `--preset mobile_edge` (implies the same deployment target).

2. The **execution planner** applies **gates**: bounded `seq_len` / `rank` / `batch_size`, no `--qlora-require-full-proxy-stack`, and **`--device cpu`** is **required** so adapters are trained without binding to a desktop-only GPU stack (see planner errors for the exact message).

3. **Artifacts** (`adapter_schema_v3`, `training_manifest.json`) record `training_deployment_target` and an operator **note** pointing here and to [HF finetune capability matrix](../archive/research-2026-q1/hf-finetune-capability-matrix.md). **Conversion** to LiteRT / Core ML / TFLite is **out of tree** until a supported exporter exists.

Canonical trainer documentation remains [mens-training.md](mens-training.md).

## Export contract (out of tree)

Training emits artifacts that are consumed by an **exporter outside this repository** until a first supported exporter lands in-tree.

### Inputs (already produced by the Mens pipeline)

- `adapter_schema_v3`
- `training_manifest.json`
- `training_deployment_target` (for example `mobile_edge`)

### Outputs

*TBD by the chosen on-device runtime* (for example LiteRT bundle layout, Core ML, or vendor-specific packages).

### Definition of done (first supported exporter)

- [ ] Documented output format(s) and a version pin for the target runtime.
- [ ] Reproducible build: same inputs and toolchain version produce artifacts described by a checksum or manifest.
- [ ] `training_manifest.json` (or its successor) records exporter version and output checksums (or equivalent integrity fields).
- [ ] Documented validation step (for example a dry-run load in the target runtime, or a future `vox mens` verify subcommand when one exists).

Further context: [HF finetune capability matrix](../archive/research-2026-q1/hf-finetune-capability-matrix.md), [Mens training SSOT](mens-training.md).

## Inference profiles (no Ollama on loopback for mobile)

Desktop MCP and CLI default to a **local Ollama** URL for **workstation** use only. Mobile apps should set an explicit profile (environment) so routing does not assume `localhost:11434`.

**`vox-mcp` HTTP inference:** local Ollama calls and cloud→Ollama fallback are enabled only when the profile is **`desktop_ollama`** or **`lan_gateway`**. Other profiles skip Ollama probes and reject `ProviderType::Ollama` with a clear error unless you switch profile or model.

| Profile | Meaning |
|---------|---------|
| `desktop_ollama` | Default when unset: `OLLAMA_HOST` / `POPULI_URL` / `http://localhost:11434` (see [`vox_config::inference`](../../../crates/vox-config/src/inference.rs)). |
| `cloud_openai_compatible` | Use `OPENROUTER_*`, `HF_*`, or dedicated OpenAI-compatible URLs from config. |
| `mobile_litert` | On-device LiteRT-LM (app-owned); Vox tooling does not spawn the runtime. |
| `mobile_coreml` | Apple Core ML (app-owned). |
| `lan_gateway` | Ollama or Mens HTTP on **LAN** (explicit base URL). |

Registry: [Environment variables (SSOT)](env-vars.md) (`VOX_INFERENCE_PROFILE`).

## Mens and GPU / NPU advertisement

Mens nodes embed [`TaskCapabilityHints`](../../../crates/vox-orchestrator/src/contract.rs). **CUDA** and **Metal** are not sufficient for Android **Vulkan** phones or **NPU** classes.

- **Legacy:** `VOX_MESH_ADVERTISE_GPU=1` still sets **`gpu_cuda`** (workstation-oriented; unchanged for backward compatibility).
- **Additive:** `VOX_MESH_ADVERTISE_VULKAN`, `VOX_MESH_ADVERTISE_WEBGPU`, `VOX_MESH_ADVERTISE_NPU` (each `1` / `true`) set the matching capability flags.
- **Class label:** `VOX_MESH_DEVICE_CLASS` — optional free-form hint (`server`, `desktop`, `mobile`, `browser`, …) stored in `TaskCapabilityHints.device_class`.

See [mens SSOT](populi.md) for the full `VOX_MESH_*` table.

## GPU probing (Mens vs mens)

- **Mens training** uses [`probe_gpu`](../../../crates/vox-populi/src/mens/tensor/device.rs) for VRAM heuristics. Overrides: **`VOX_GPU_MODEL`**, **`VOX_GPU_VRAM_MB`**. **Windows:** `wmic`; **Linux:** best-effort `nvidia-smi` / `lspci`. **Android / iOS:** no in-crate probe — the **host app** should set env overrides or pass capabilities into mens JSON.
- **Mens** does not require Mens; capability flags come from **env + host** as above.

## Related

- [Cross-platform Vox — lanes & Docker matrix (SSOT)](../archive/research-2026-q1/vox-cross-platform-runbook.md) — script worker vs app vs mobile; Docker feature matrix.
- [Deployment compose SSOT](deployment-compose.md) — server/container Compose vs mobile (inference profiles, no phone OCI).
- [Orchestration unified SSOT](orchestration-unified.md) — capability merge rules.
- [Environment variables (SSOT)](env-vars.md).
- [vox-mcp API](../reference/cli.md) — Ollama fallback is **desktop-oriented**.

## Direct on-device `.vox` runtime (experimental boundary)

If Vox later explores direct on-device `.vox` execution, treat it as a reduced, versioned subset and not parity with workstation/server runtime semantics.

Initial unsupported-by-default classes should include:

- actors/workflows/activities
- server/query/mutation function surfaces
- MCP tool declarations in script bodies
- async `main` in wasm isolation lanes
- host-assumed builtins without mobile/browser-safe shims (for example current `std.http.*` wasm guardrails)

Use the existing WASI guardrails and diagnostics as a baseline contract source, not as a claim of stock-phone parity.

