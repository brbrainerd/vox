---
title: "Plugin System Redesign — SP3 Implementation Plan (2026)"
description: "Step-by-step implementation plan for Sub-Project 3: define the MlBackend extension-point trait, extract candle-cuda from vox-populi into a standalone vox-plugin-mens-candle-cuda cdylib plugin, and wire vox-populi to consume the backend through vox-plugin-host."
category: "Architecture SSOTs"
status: "research"
training_eligible: true
training_rationale: "Concrete TDD task plan for SP3; companion to the parent design spec."
---

# Plugin System Redesign — SP3 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **Status (2026-05-03):** **PARTIAL — batches 1–2 landed, batches 3–11 deferred.** Batch 1 defined the `MlBackend` `#[sabi_trait]`, bumped host ABI 1→2, and added the `as_ml_backend()` accessor on `VoxPlugin` (commit `780811cea`). Batch 2 scaffolded the `vox-plugin-mens-candle-cuda` cdylib, extracted `model.rs` (~764 lines of candle-only code) cleanly from `vox-populi/src/mens/tensor/candle_model_qwen.rs`, but discovered that the **training loop and checkpoint logic are deeply tangled** with non-candle vox-populi/vox-tensor/vox-secrets/VoxDB/vox-corpus types (commit `6642dadbd`). The plugin scaffold builds and exports `MlBackend`; `train_step`/`eval_step`/`save_checkpoint` return `RErr("not yet implemented")`. The CUDA-cdylib pattern is fully proven; the architectural boundary problem is what remains. A follow-up plan (`plugin-system-redesign-sp3-training-extraction-plan-2026.md`, TBD) will design either (a) full code-motion of the training loop + dependent types into the plugin or (b) a JSON wire format that lets the plugin own only the math while vox-populi keeps orchestration. Batches 3–11 below are kept verbatim for reference but should be re-derived from the follow-up plan.

**Parent spec:** [`plugin-system-redesign-2026.md`](plugin-system-redesign-2026.md)
**Predecessor plans:** [`SP1`](plugin-system-redesign-sp1-plan-2026.md) (catalog) AND [`SP2`](plugin-system-redesign-sp2-plan-2026.md) (host ABI). Both must be merged before SP3 starts.

**Goal:** Prove the plugin model works for the most-tangled current capability — Mens / Candle / CUDA training. Define `MlBackend` as the first real extension-point trait, extract the existing `mens-candle-qlora` + `mens-candle-qlora-cuda` features from [`vox-populi`](../../../crates/vox-populi/Cargo.toml) into a new standalone `vox-plugin-mens-candle-cuda` cdylib, wire `vox-populi` to consume `MlBackend` through `vox-plugin-host`'s registry, and verify behavioral parity (training produces equivalent checkpoints).

**Architecture:** `MlBackend` is a `#[sabi_trait]` in `vox-plugin-api::extensions::ml_backend` exposing methods derived from current candle-qlora callsites: `load_model`, `train_step`, `eval_step`, `save_checkpoint`. The new `vox-plugin-mens-candle-cuda` cdylib owns the candle-core / candle-nn / qlora-rs / peft-rs / safetensors / tokenizers / memmap2 deps that today live behind `vox-populi`'s `mens-candle-qlora-cuda` feature. `vox-populi`'s old direct candle calls become `host.ml_backend().ok_or(PluginMissingError { plugin_id: "mens-candle-cuda", … })?.method(...)`.

The CUDA cdylib pattern is already proven by the [SP3 gating spike](plugin-system-redesign-2026.md#sub-project-3-first-code-extension-point-mlbackend) — direct cdylib + `libloading` works on Windows MSVC + CUDA 13.1. SP3 generalizes to a real extraction, no further architectural risk expected.

**Tech Stack:**
- `abi_stable` (workspace dep, added in SP2 Task 13)
- `candle-core`, `candle-nn`, `qlora-rs`, `peft-rs`, `safetensors`, `tokenizers`, `memmap2` (existing workspace deps; move from `vox-populi`'s optional deps to the new plugin's required deps)
- The `vox-plugin-api` and `vox-plugin-host` infrastructure from SP2

---

## File Structure

### New crate

| Path                                                                | Responsibility                                                          |
| ------------------------------------------------------------------- | ----------------------------------------------------------------------- |
| `crates/vox-plugin-api/src/extensions/ml_backend.rs`                | `MlBackend` `#[sabi_trait]` — replaces the placeholder from SP2 Task 6.  |
| `crates/vox-plugin-mens-candle-cuda/Cargo.toml`                     | `[lib] crate-type = ["cdylib", "rlib"]`. Depends on candle-core/cuda + vox-plugin-api. |
| `crates/vox-plugin-mens-candle-cuda/src/lib.rs`                     | `MlBackend` impl + plugin root export.                                  |
| `crates/vox-plugin-mens-candle-cuda/src/training.rs`                | Training step body (lifted from `vox-populi`).                           |
| `crates/vox-plugin-mens-candle-cuda/src/checkpoint.rs`              | Save/load checkpoint logic (lifted from `vox-populi`).                   |
| `crates/vox-plugin-mens-candle-cuda/Plugin.toml`                    | Manifest declaring code payload, MlBackend extension point, native-libs (cudart 12.0+). |
| `crates/vox-plugin-mens-candle-cuda/tests/training_smoke.rs`        | Integration test: load plugin, run one training step, save checkpoint, assert checkpoint bytes match a fixture. |

### Modified

| Path                                                                | Change                                                                  |
| ------------------------------------------------------------------- | ----------------------------------------------------------------------- |
| `crates/vox-plugin-api/src/extensions/ml_backend.rs`                | Replace placeholder with real trait (file already exists from SP2 as a stub). |
| `crates/vox-plugin-api/src/abi.rs`                                  | Add `as_ml_backend()` accessor to the `VoxPlugin` `#[sabi_trait]`.       |
| `crates/vox-populi/Cargo.toml`                                      | Delete `mens-candle-qlora` and `mens-candle-qlora-cuda` features. Drop the candle/qlora/peft optional deps. |
| `crates/vox-populi/src/mens/training.rs` (or wherever candle is called) | Replace direct candle calls with host-mediated MlBackend dispatch. |
| `crates/vox-plugin-catalog/catalog.toml`                            | `mens-candle-cuda` entry already exists from SP1; no change needed unless `default-source` updates. |
| `docs/src/architecture/mens-training-ssot.md`                       | Update invocation: `cargo run -p vox-cli -- mens train ...` no longer needs `--features`. |

---

## Tasks

### Task 1: Define the `MlBackend` `#[sabi_trait]`

**Files:** Replace placeholder in `crates/vox-plugin-api/src/extensions/ml_backend.rs`. Test in `crates/vox-plugin-api/tests/ml_backend_compile.rs`.

The trait shape is derived from the current candle-qlora callsites in `vox-populi`. Read those callsites first to confirm the methods needed.

- [ ] **Step 0 (research):** Find all calls to `candle_core::*` / `candle_nn::*` / `qlora_rs::*` in `vox-populi`:

```
rg "candle_core|candle_nn|qlora_rs|peft_rs" --type rust crates/vox-populi/
```

Group by call shape. The trait methods correspond to these grouped operations. Suspected method set (refine after the rg):
- `load_model(model_path: RStr<'_>) -> RResult<RBox<Model>, RBoxError>` — load a pretrained model
- `train_step(model: &Model, batch: TrainBatch) -> RResult<TrainStepStats, RBoxError>` — one optimization step
- `eval_step(model: &Model, batch: EvalBatch) -> RResult<EvalStats, RBoxError>` — one evaluation step
- `save_checkpoint(model: &Model, dest: RStr<'_>) -> RResult<(), RBoxError>` — write a checkpoint

`Model`, `TrainBatch`, `TrainStepStats`, etc. need stable-ABI representations. Pragmatic: use `RBox<RErasedObj>` opaque handles for `Model`, and serialize batch/stats payloads as JSON `RString` to avoid defining many sabi-stable structs. (Verify against perf budget; if JSON serialization shows up in profiles for hot training-loop calls, switch to `RVec<u8>` with bincode.)

- [ ] **Step 1:** Write a compile-only test that asserts the trait's signature matches expectations:

```rust
use vox_plugin_api::extensions::ml_backend::{MlBackend, MlBackend_TO};

fn assert_object_safe<T: MlBackend>(_: T) {}

#[test]
fn trait_object_compiles() {
    // Compilation alone is the assertion.
}
```

- [ ] **Step 2:** Verify FAIL.
- [ ] **Step 3:** Implement the trait:

```rust
//! MlBackend extension-point trait — first real code-plugin extension.
//!
//! Implementations live in plugins like `vox-plugin-mens-candle-cuda`. The
//! host obtains an instance via `VoxPlugin::as_ml_backend()` and dispatches
//! training / eval / checkpoint operations through it.

use abi_stable::{sabi_trait, std_types::*};

pub const ML_BACKEND_REVISION: u32 = 1;

#[sabi_trait]
pub trait MlBackend: Send + Sync {
    fn revision(&self) -> u32 { ML_BACKEND_REVISION }
    fn load_model(&self, model_path: RStr<'_>) -> RResult<RBox<MlModelHandle>, RBoxError>;
    fn train_step(&self, model: &MlModelHandle, batch_json: RStr<'_>) -> RResult<RString, RBoxError>;
    fn eval_step(&self, model: &MlModelHandle, batch_json: RStr<'_>) -> RResult<RString, RBoxError>;
    fn save_checkpoint(&self, model: &MlModelHandle, dest: RStr<'_>) -> RResult<(), RBoxError>;
}

/// Opaque handle to a backend-owned model. The host never inspects the
/// contents — it only passes it back to the same backend.
#[repr(C)]
pub struct MlModelHandle {
    _opaque: [u8; 0],
}
```

(Note: `MlModelHandle` as a ZST with extern type semantics is tricky in Rust; if `abi_stable` can't carry it as `RBox<MlModelHandle>` cleanly, fall back to `RBox<RErasedObj>` and have the plugin use `Arc<...>` internally. Implementer pick.)

- [ ] **Step 4:** Verify PASS.
- [ ] **Step 5:** Commit: `feat(plugin-api): define MlBackend extension-point trait`.

### Task 2: Wire `as_ml_backend()` into the `VoxPlugin` `#[sabi_trait]`

**Files:** Modify `crates/vox-plugin-api/src/abi.rs`.

In SP2 the `VoxPlugin` trait had only `id()` and `shutdown()`. Add the typed extension accessor.

- [ ] **Step 1:** In `abi.rs`, add to `VoxPlugin`:

```rust
fn as_ml_backend(&self) -> ROption<MlBackend_TO<'static, RBox<()>>> { RNone }
```

(With appropriate `use vox_plugin_api::extensions::ml_backend::MlBackend_TO;` import.)

- [ ] **Step 2:** Run all `vox-plugin-api` tests — they should still pass (default impl returns RNone).
- [ ] **Step 3:** **ABI BUMP:** This is a backwards-incompatible change to the trait surface. Bump `VOX_PLUGIN_ABI_VERSION` from `1` to `2` in `lib.rs`. Update SP2's noop-code dylib to re-export `abi_version: 2` (rebuild). The bad-abi noop now declares `999_999` so it still mismatches.
- [ ] **Step 4:** Run SP2's `cargo test -p vox-plugin-host` battery — all four integration tests should still pass.
- [ ] **Step 5:** Commit: `feat(plugin-api): add as_ml_backend accessor to VoxPlugin trait; bump ABI to 2`.

### Task 3: Scaffold `vox-plugin-mens-candle-cuda` crate

Same pattern as SP1 Task 1. New crate as `cdylib` + `rlib`.

- [ ] **Step 1:** Smoke test.
- [ ] **Step 2:** Verify FAIL.
- [ ] **Step 3:** `Cargo.toml`:

```toml
[package]
name = "vox-plugin-mens-candle-cuda"
version = "0.1.0"
edition.workspace = true
publish = false
description = "ML training backend plugin: Candle + CUDA. Implements MlBackend."

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
vox-plugin-api = { workspace = true }
abi_stable = { workspace = true }
candle-core = { workspace = true, features = ["cuda"] }
candle-nn = { workspace = true, features = ["cuda"] }
qlora-rs = { workspace = true }
peft-rs = { workspace = true }
safetensors = { workspace = true }
tokenizers = { workspace = true }
memmap2 = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
```

- [ ] **Step 4:** `src/lib.rs` with module wiring + plugin root export (mirror SP2 Task 16's noop-code pattern):

```rust
//! vox-plugin-mens-candle-cuda — Candle + CUDA ML backend plugin.

mod backend;
mod checkpoint;
mod training;

use abi_stable::{export_root_module, prefix_type::PrefixTypeTrait, sabi_extern_fn, std_types::*};
use vox_plugin_api::abi::{VoxPlugin, VoxPlugin_TO, VoxPluginRef, VoxPluginRoot, VoxPluginRootRef};
use vox_plugin_api::host::VoxHost_TO;
use vox_plugin_api::VOX_PLUGIN_ABI_VERSION;

#[export_root_module]
fn root_module() -> VoxPluginRootRef {
    VoxPluginRoot {
        abi_version: VOX_PLUGIN_ABI_VERSION,
        manifest_json,
        init,
    }.leak_into_prefix()
}

#[sabi_extern_fn]
fn manifest_json() -> RString {
    RString::from(r#"{"id":"mens-candle-cuda","version":"0.1.0"}"#)
}

#[sabi_extern_fn]
fn init(_host: VoxHost_TO<'static, RBox<()>>) -> RResult<VoxPluginRef, RBoxError> {
    let plugin = backend::CandleCudaPlugin::new();
    let to = VoxPlugin_TO::from_value(plugin, abi_stable::erased_types::TD_Opaque);
    RResult::ROk(to)
}
```

- [ ] **Step 5:** Stub the three modules so the crate compiles:

```rust
// src/backend.rs
use vox_plugin_api::abi::VoxPlugin;
use abi_stable::std_types::*;

pub struct CandleCudaPlugin;
impl CandleCudaPlugin { pub fn new() -> Self { Self } }
impl VoxPlugin for CandleCudaPlugin {
    fn id(&self) -> RString { RString::from("mens-candle-cuda") }
    fn shutdown(&self) -> RResult<(), RBoxError> { RResult::ROk(()) }
    // SP3 Task 5 wires as_ml_backend.
}
```

```rust
// src/training.rs — SP3 Task 5 fills in
```

```rust
// src/checkpoint.rs — SP3 Task 5 fills in
```

- [ ] **Step 6:** `Plugin.toml`:

```toml
[plugin]
id = "mens-candle-cuda"
name = "Mens (Candle + CUDA)"
version = "0.1.0"
description = "ML training backend using Candle with CUDA acceleration."
license = "Apache-2.0"

[plugin.host]
min-vox-version = "0.5.0"

[plugin.payload]
kind = "code"
abi-version = 2

[plugin.payload.provides]
extension-points = ["MlBackend"]

[plugin.payload.requires]
os = ["windows", "linux"]
arch = ["x86_64"]
native-libs = [
    { name = "cudart", min-version = "12.0" },
    { name = "cublas" },
]

[plugin.payload.artifacts]
"windows-x86_64" = "vox_plugin_mens_candle_cuda.dll"
"linux-x86_64"   = "libvox_plugin_mens_candle_cuda.so"
```

- [ ] **Step 7:** `cargo build -p vox-plugin-mens-candle-cuda` (in MSVC env on Windows) — verify the dylib produces.
- [ ] **Step 8:** Commit: `feat(plugin-mens-candle-cuda): scaffold cdylib crate`.

### Task 4: Move candle-using code from `vox-populi` to `vox-plugin-mens-candle-cuda/src/training.rs` and `checkpoint.rs`

**Files:** All `vox-populi` files containing direct `candle_core::*` / `qlora_rs::*` calls (identified in Task 1 Step 0).

Pure code-motion: copy the implementations into the plugin crate's modules, leaving `vox-populi`'s side empty placeholders that the next task wires through the host.

- [ ] **Step 1:** For each candle-using function in `vox-populi`, identify whether it should become:
  - A method on `MlBackend` (training_step, save_checkpoint, etc.) → moves to plugin's training.rs / checkpoint.rs as a private function called from the trait impl.
  - Pre/post processing that doesn't need GPU → stays in `vox-populi`, calls the trait through the host.
- [ ] **Step 2:** Copy candle code into the plugin. Adapt signatures to match `MlBackend`'s opaque-handle + JSON-payload contract.
- [ ] **Step 3:** Cargo build the plugin. Cargo build should succeed but `vox-populi` will likely have compile errors now (broken candle imports). Ignore those; Task 5 fixes them.
- [ ] **Step 4:** Commit: `feat(plugin-mens-candle-cuda): move candle training and checkpoint code from vox-populi`.

### Task 5: Implement `MlBackend` for `CandleCudaPlugin`

**Files:** `crates/vox-plugin-mens-candle-cuda/src/backend.rs`.

- [ ] **Step 1:** Add `MlBackend` impl on `CandleCudaPlugin`:

```rust
use vox_plugin_api::extensions::ml_backend::{MlBackend, MlBackend_TO, MlModelHandle};

impl MlBackend for CandleCudaPlugin {
    fn load_model(&self, model_path: RStr<'_>) -> RResult<RBox<MlModelHandle>, RBoxError> {
        match crate::training::load_model(model_path.as_str()) {
            Ok(model) => RResult::ROk(model),
            Err(e) => RResult::RErr(RBoxError::new(e)),
        }
    }
    // train_step / eval_step / save_checkpoint — same shape, delegating to crate::training and crate::checkpoint
}
```

- [ ] **Step 2:** Also override `VoxPlugin::as_ml_backend()`:

```rust
impl VoxPlugin for CandleCudaPlugin {
    fn id(&self) -> RString { RString::from("mens-candle-cuda") }
    fn shutdown(&self) -> RResult<(), RBoxError> { RResult::ROk(()) }
    fn as_ml_backend(&self) -> ROption<MlBackend_TO<'static, RBox<()>>> {
        ROption::RSome(MlBackend_TO::from_value(self.clone(), abi_stable::erased_types::TD_Opaque))
    }
}
```

(Requires `CandleCudaPlugin: Clone` — make it cloneable, or wrap in `Arc` and clone the arc.)

- [ ] **Step 3:** `cargo build -p vox-plugin-mens-candle-cuda` — green.
- [ ] **Step 4:** Commit: `feat(plugin-mens-candle-cuda): implement MlBackend trait`.

### Task 6: Wire `vox-populi` through the host

**Files:** `crates/vox-populi/Cargo.toml`, `crates/vox-populi/src/mens/training.rs` (or wherever candle was called).

- [ ] **Step 1:** Delete `vox-populi`'s `mens-candle-qlora` and `mens-candle-qlora-cuda` features. Remove the candle / qlora-rs / peft-rs / safetensors / tokenizers / memmap2 from `[dependencies]`. Add `vox-plugin-host = { workspace = true }`.
- [ ] **Step 2:** In each former candle-using function, accept a `&Registry` parameter (or a method on a struct that holds one) and dispatch through `MlBackend`:

```rust
use vox_plugin_host::{Registry, errors::PluginMissingError};

pub fn run_training(registry: &Registry, model_path: &str, batch: &TrainBatch) -> Result<TrainStats, MlError> {
    let plugin = registry.get("mens-candle-cuda").ok_or(PluginMissingError {
        plugin_id: "mens-candle-cuda",
        extension_point: "MlBackend",
    })?;
    let backend = plugin.as_ml_backend().ok_or(/* ... */)?;
    let model = backend.load_model(model_path.into()).into_result().map_err(/* ... */)?;
    let batch_json = serde_json::to_string(batch)?;
    let stats_json = backend.train_step(&model, batch_json.as_str().into()).into_result()?;
    let stats: TrainStats = serde_json::from_str(stats_json.as_str())?;
    Ok(stats)
}
```

- [ ] **Step 3:** `cargo check -p vox-populi` — green.
- [ ] **Step 4:** `cargo check --workspace` — green (deprecation warnings on `vox-build-meta::FEATURES_JSON` are pre-existing from SP1 and OK).
- [ ] **Step 5:** Commit: `refactor(vox-populi): consume MlBackend through vox-plugin-host instead of direct candle calls`.

### Task 7: End-to-end training test

**Files:** `crates/vox-plugin-mens-candle-cuda/tests/training_smoke.rs`.

Reproduces a tiny training loop end-to-end through the plugin and asserts the output checkpoint matches a fixture (within hardware tolerance).

- [ ] **Step 1:** Pick the smallest existing test fixture in `vox-populi`'s test corpus that exercises a single training step. Copy the fixture into `vox-plugin-mens-candle-cuda/tests/fixtures/` if needed.
- [ ] **Step 2:** Write the integration test:

```rust
// Pattern similar to SP2 Task 16's load_noop_code.rs but for the real plugin.
// Build the dylib, copy to tempdir, discover, load, call MlBackend methods,
// compare output checkpoint bytes to a baseline.
```

- [ ] **Step 3:** Run on a CUDA-equipped machine (Windows MSVC + CUDA 13.1 confirmed working from the spike). Capture the baseline checkpoint bytes if not already present.
- [ ] **Step 4:** Verify PASS.
- [ ] **Step 5:** Commit: `feat(plugin-mens-candle-cuda): end-to-end training smoke test`.

### Task 8: Pre-existing vox-populi tests still pass

**Files:** none (verification only).

- [ ] **Step 1:** Run `cargo test -p vox-populi`. If any test directly invokes the now-extracted candle path, it will need either:
  - To be rewritten to use the host registry (for tests that exercised the full pipeline), OR
  - To be moved into `vox-plugin-mens-candle-cuda/tests/` (for tests that were really about the candle layer).
- [ ] **Step 2:** For each affected test, decide which bucket and migrate.
- [ ] **Step 3:** Commit per-test migration as separate commits if substantial; one bulk commit if mechanical.

### Task 9: Update `mens-training-ssot.md`

Replace `cargo run -p vox-cli ... --features mens-candle-cuda` instructions with the plugin-based equivalent: `vox plugin install mens-candle-cuda` (after SP5 lands; for now `vox plugin install --path crates/vox-plugin-mens-candle-cuda/dist/` or similar dev-mode install).

- [ ] **Step 1:** Edit the doc.
- [ ] **Step 2:** Run `cargo run -p vox-doc-pipeline` to regenerate any auto-rolled docs that reference it.
- [ ] **Step 3:** Commit.

### Task 10: Catalog and CI guards

- [ ] **Step 1:** Verify `mens-candle-cuda` is already in the catalog (it was added in SP1 Task 3). Confirm `default-source` is reasonable; update if needed.
- [ ] **Step 2:** Run `cargo run -q -p vox-cli -- ci plugin-catalog-parity` — should pass since `mens-candle-cuda` is in both the catalog and now has a real Plugin.toml.
- [ ] **Step 3:** Run `cargo run -q -p vox-cli -- ci plugin-abi-parity` — should pass: the plugin's ABI matches host (both at 2 after Task 2).
- [ ] **Step 4:** Run `cargo run -q -p vox-cli -- ci generate-plugin-catalog-docs` to regenerate (a new `bundled-in` may have changed). Commit if needed.

### Task 11: Final acceptance

- [ ] **Step 1:** `cargo build --workspace` — green.
- [ ] **Step 2:** `cargo test -p vox-plugin-mens-candle-cuda` — green (training smoke test).
- [ ] **Step 3:** `cargo test -p vox-populi` — green (post-Task-8 fixups).
- [ ] **Step 4:** `cargo test -p vox-plugin-host` — green (SP2 tests still pass after the ABI bump).
- [ ] **Step 5:** All four CI guards green: `plugin-catalog-parity`, `plugin-abi-parity`, `plugin-skill-parity`, `generate-plugin-catalog-docs --check`.
- [ ] **Step 6:** Behavioral parity: run the existing `vox-populi` mens-training integration test (whatever it was before SP3) end-to-end with the plugin installed via dev-mode path. Output checkpoint should be byte-identical to pre-SP3 baseline (or within the documented hardware tolerance).

If green: SP3 done. SP6 (slim defaults / vox-build-meta retirement) becomes possible since `mens-candle-cuda` is now a plugin and no `vox-populi` code references the old features.

---

## Spec coverage check (self-review)

| SP3 spec deliverable                                                             | Plan task |
| -------------------------------------------------------------------------------- | --------- |
| `MlBackend` trait with revision 1.0                                              | 1, 2      |
| `vox-plugin-mens-candle-cuda` cdylib crate                                       | 3         |
| Owns candle/qlora/peft/safetensors/tokenizers/memmap2 deps                       | 3         |
| Implements `MlBackend`                                                           | 5         |
| Plugin.toml + integration test                                                   | 3, 7      |
| Delete `mens-candle-qlora`, `mens-candle-qlora-cuda` features from vox-populi    | 6         |
| Replace direct candle calls with host-mediated MlBackend dispatch                | 6         |
| Update mens-training-ssot.md                                                     | 9         |
| CUDA spike result already proven                                                 | (SP3 spec; precondition met) |
| Behavioral parity                                                                | 11        |

All SP3 deliverables map to tasks. Largest implementation risk: Task 1 (defining the right `MlBackend` shape — too granular and dispatch overhead becomes a problem; too coarse and the trait can't carry future operations). Mitigation: read all current candle-call shapes (Task 1 Step 0) before locking the trait.
