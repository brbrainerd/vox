# Vox Resource-Adaptive Dispatch — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Run in a git worktree** (the `vox-broker` shim breaks `cargo` in the main repo dir). All `cargo` commands target the new crate with `-p vox-dispatch`.

**Goal:** Build `vox-dispatch` — a pure-logic crate that probes the host's CPU/GPU/VRAM, picks the largest VoxMens precision-variant that fits (else routes to cloud), and routes each task to local vs cloud/frontier — plugging into the existing `vox_actor_runtime::llm` facade via a trait.

**Architecture:** Four small units behind clean interfaces — `HwProfile`+`CapabilityProbe` (sensing), `VariantCatalog` (the precision ladder, loaded from a manifest), `VariantSelector` (pure: profile → variant|cloud), `DispatchRouter` (pure: task-class + selection → backend). The real LLM call goes through an injected `LlmBackend` trait, so all decision logic is tested without network or GPU.

**Tech Stack:** Rust, `serde`/`serde_json` (catalog manifest), `sysinfo` (RAM), `std::thread::available_parallelism` (threads), `nvidia-smi` shell-out (VRAM). Tests: `cargo test -p vox-dispatch`.

**Spec:** [`docs/superpowers/specs/2026-06-21-vox-local-tower-and-adaptive-dispatch-design.md`](../specs/2026-06-21-vox-local-tower-and-adaptive-dispatch-design.md)

---

## File Structure
| File | Responsibility |
|---|---|
| `crates/vox-dispatch/Cargo.toml` | crate manifest + deps |
| `crates/vox-dispatch/src/lib.rs` | re-exports + shared types (`TaskClass`, `Backend`, `Choice`) |
| `crates/vox-dispatch/src/profile.rs` | `HwProfile`, `GpuInfo`, `total_vram_gb()` |
| `crates/vox-dispatch/src/probe.rs` | `CapabilityProbe` trait + `SystemProbe` (real) + `FakeProbe` (tests) |
| `crates/vox-dispatch/src/catalog.rs` | `Variant`, `Precision`, `VariantKind`, `load_catalog()` |
| `crates/vox-dispatch/src/selector.rs` | `select_variant()` (pure) |
| `crates/vox-dispatch/src/router.rs` | `route()` (pure) + `Dispatcher` (uses `LlmBackend`) |
| `crates/vox-dispatch/variants.json` | the precision-ladder manifest (sample) |

---

## Task 1: Scaffold crate + shared types

**Files:**
- Create: `crates/vox-dispatch/Cargo.toml`
- Create: `crates/vox-dispatch/src/lib.rs`
- Modify: root `Cargo.toml` (add `"crates/vox-dispatch"` to `[workspace] members`)

- [ ] **Step 1: Write the failing test** — `crates/vox-dispatch/src/lib.rs`

```rust
//! Resource-adaptive model dispatch for Vox.
pub mod profile;

/// How demanding / latency-sensitive a unit of work is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskClass {
    /// High-frequency, latency-bound (route/classify/retrieve-augment/quick gen).
    Micro,
    /// VoxScript-specific work (local VoxMens knows it natively).
    VoxScript,
    /// Quality-bound / hard — may need frontier.
    Hard,
}

/// Where a task should run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Backend {
    /// Run locally on the named VoxMens variant id.
    Local(String),
    /// Cheap cloud model (Flash/Haiku tier).
    CloudCheap,
    /// Frontier cloud model (Opus/Fable/Claude Code) with VoxScript RAG context.
    CloudFrontier,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn task_classes_are_distinct() {
        assert_ne!(TaskClass::Micro, TaskClass::Hard);
        assert_eq!(Backend::Local("v".into()), Backend::Local("v".into()));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-dispatch`
Expected: FAIL — `error: package ID specification 'vox-dispatch' did not match any packages` (crate not in workspace yet).

- [ ] **Step 3: Write the manifest + register**

`crates/vox-dispatch/Cargo.toml`:
```toml
[package]
name = "vox-dispatch"
version.workspace = true
edition = "2021"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sysinfo = "0.30"

[dev-dependencies]
```
Add to root `Cargo.toml` under `[workspace] members = [ ... ]`: the line `"crates/vox-dispatch",`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-dispatch`
Expected: PASS (1 test).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-dispatch/Cargo.toml crates/vox-dispatch/src/lib.rs Cargo.toml
git commit -m "feat(vox-dispatch): scaffold crate + TaskClass/Backend types"
```

---

## Task 2: HwProfile + total VRAM

**Files:**
- Create: `crates/vox-dispatch/src/profile.rs`

- [ ] **Step 1: Write the failing test** — append to `profile.rs`

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GpuInfo {
    pub name: String,
    pub vram_gb: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HwProfile {
    pub cpu_threads: u32,
    pub ram_gb: u32,
    pub gpus: Vec<GpuInfo>,
}

impl HwProfile {
    /// Total usable VRAM across all GPUs, in GB.
    pub fn total_vram_gb(&self) -> u32 {
        self.gpus.iter().map(|g| g.vram_gb).sum()
    }
    pub fn has_gpu(&self) -> bool {
        !self.gpus.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn total_vram_sums_all_gpus() {
        let p = HwProfile {
            cpu_threads: 48,
            ram_gb: 128,
            gpus: vec![
                GpuInfo { name: "RTX 3090".into(), vram_gb: 24 },
                GpuInfo { name: "RTX 3090".into(), vram_gb: 24 },
            ],
        };
        assert_eq!(p.total_vram_gb(), 48);
        assert!(p.has_gpu());
    }
    #[test]
    fn no_gpu_is_zero_vram() {
        let p = HwProfile { cpu_threads: 8, ram_gb: 16, gpus: vec![] };
        assert_eq!(p.total_vram_gb(), 0);
        assert!(!p.has_gpu());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-dispatch profile`
Expected: FAIL — `profile.rs` has the code but `lib.rs` already declares `pub mod profile;` (Task 1) so it should compile; if you split the test before the impl, FAIL with "no method `total_vram_gb`". (Write the test first, impl after, per TDD.)

- [ ] **Step 3: Write minimal implementation** — already shown above (the `impl HwProfile` block). Ensure it's present.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-dispatch profile`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-dispatch/src/profile.rs
git commit -m "feat(vox-dispatch): HwProfile + total_vram_gb"
```

---

## Task 3: VariantCatalog (the precision ladder)

**Files:**
- Create: `crates/vox-dispatch/src/catalog.rs`
- Create: `crates/vox-dispatch/variants.json`
- Modify: `crates/vox-dispatch/src/lib.rs` (add `pub mod catalog;`)

- [ ] **Step 1: Write the failing test** — `catalog.rs`

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Precision { Fp16, Fp8, Awq4 }

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum VariantKind { Base, Spoke }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Variant {
    pub id: String,
    pub precision: Precision,
    pub min_vram_gb: u32,
    pub tokens_per_s: u32,
    pub kind: VariantKind,
}

/// Parse a catalog manifest (JSON array of variants).
pub fn parse_catalog(json: &str) -> Result<Vec<Variant>, serde_json::Error> {
    serde_json::from_str(json)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_a_ladder() {
        let json = r#"[
          {"id":"voxmens-32b-fp16","precision":"fp16","min_vram_gb":70,"tokens_per_s":50,"kind":"base"},
          {"id":"voxmens-32b-fp8","precision":"fp8","min_vram_gb":40,"tokens_per_s":90,"kind":"base"},
          {"id":"voxmens-32b-awq4","precision":"awq4","min_vram_gb":20,"tokens_per_s":110,"kind":"base"}
        ]"#;
        let v = parse_catalog(json).unwrap();
        assert_eq!(v.len(), 3);
        assert_eq!(v[1].precision, Precision::Fp8);
        assert_eq!(v[1].min_vram_gb, 40);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-dispatch catalog`
Expected: FAIL — `module 'catalog' not found` until you add `pub mod catalog;` to `lib.rs`.

- [ ] **Step 3: Write minimal implementation**

Add `pub mod catalog;` to `lib.rs`. The `catalog.rs` content above is the implementation. Also create `variants.json` (the real manifest):
```json
[
  {"id":"voxmens-32b-fp16","precision":"fp16","min_vram_gb":70,"tokens_per_s":50,"kind":"base"},
  {"id":"voxmens-32b-fp8","precision":"fp8","min_vram_gb":40,"tokens_per_s":90,"kind":"base"},
  {"id":"voxmens-32b-awq4","precision":"awq4","min_vram_gb":20,"tokens_per_s":110,"kind":"base"}
]
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-dispatch catalog`
Expected: PASS (1 test).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-dispatch/src/catalog.rs crates/vox-dispatch/variants.json crates/vox-dispatch/src/lib.rs
git commit -m "feat(vox-dispatch): VariantCatalog + precision-ladder manifest"
```

---

## Task 4: VariantSelector (pure: profile → variant | cloud)

**Files:**
- Create: `crates/vox-dispatch/src/selector.rs`
- Modify: `crates/vox-dispatch/src/lib.rs` (add `pub mod selector;` + `pub enum Choice`)

- [ ] **Step 1: Write the failing test** — `selector.rs`

```rust
use crate::catalog::Variant;
use crate::profile::HwProfile;
use crate::Choice;

/// Pick the highest-quality variant that fits in available VRAM; else cloud.
/// "Highest quality" = lowest tokens_per_s among fitting base variants is NOT the rule;
/// we prefer the LARGEST model that fits, i.e. the highest min_vram_gb that is <= available.
pub fn select_variant(profile: &HwProfile, catalog: &[Variant]) -> Choice {
    let vram = profile.total_vram_gb();
    let best = catalog
        .iter()
        .filter(|v| matches!(v.kind, crate::catalog::VariantKind::Base))
        .filter(|v| v.min_vram_gb <= vram)
        .max_by_key(|v| v.min_vram_gb);
    match best {
        Some(v) => Choice::Local(v.id.clone()),
        None => Choice::Cloud,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::parse_catalog;
    use crate::profile::{GpuInfo, HwProfile};

    fn ladder() -> Vec<Variant> {
        parse_catalog(r#"[
          {"id":"voxmens-32b-fp16","precision":"fp16","min_vram_gb":70,"tokens_per_s":50,"kind":"base"},
          {"id":"voxmens-32b-fp8","precision":"fp8","min_vram_gb":40,"tokens_per_s":90,"kind":"base"},
          {"id":"voxmens-32b-awq4","precision":"awq4","min_vram_gb":20,"tokens_per_s":110,"kind":"base"}
        ]"#).unwrap()
    }
    fn profile(vram: u32) -> HwProfile {
        HwProfile { cpu_threads: 48, ram_gb: 128,
            gpus: if vram == 0 { vec![] } else { vec![GpuInfo{name:"g".into(), vram_gb: vram}] } }
    }

    #[test]
    fn picks_fp8_on_48gb_tower() {
        assert_eq!(select_variant(&profile(48), &ladder()), Choice::Local("voxmens-32b-fp8".into()));
    }
    #[test]
    fn picks_awq4_on_24gb_card() {
        assert_eq!(select_variant(&profile(24), &ladder()), Choice::Local("voxmens-32b-awq4".into()));
    }
    #[test]
    fn picks_fp16_on_96gb_box() {
        assert_eq!(select_variant(&profile(96), &ladder()), Choice::Local("voxmens-32b-fp16".into()));
    }
    #[test]
    fn no_gpu_falls_back_to_cloud() {
        assert_eq!(select_variant(&profile(0), &ladder()), Choice::Cloud);
    }
    #[test]
    fn tiny_gpu_below_floor_is_cloud() {
        assert_eq!(select_variant(&profile(8), &ladder()), Choice::Cloud);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-dispatch selector`
Expected: FAIL — `Choice` not defined / `module selector not found`.

- [ ] **Step 3: Write minimal implementation**

In `lib.rs` add:
```rust
pub mod catalog;
pub mod selector;

/// Outcome of variant selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    /// Run locally on this variant id.
    Local(String),
    /// No local variant fits — use cloud.
    Cloud,
}
```
(The `selector.rs` body above is the implementation.)

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-dispatch selector`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-dispatch/src/selector.rs crates/vox-dispatch/src/lib.rs
git commit -m "feat(vox-dispatch): VariantSelector picks largest-fitting variant else cloud"
```

---

## Task 5: DispatchRouter (pure: task-class + selection → backend)

**Files:**
- Create: `crates/vox-dispatch/src/router.rs`
- Modify: `crates/vox-dispatch/src/lib.rs` (add `pub mod router;`)

- [ ] **Step 1: Write the failing test** — `router.rs`

```rust
use crate::{Backend, Choice, TaskClass};

/// Decide the backend for a task given the local-variant selection.
/// Policy (spec §3.3):
///  - Micro: local if available (latency), else CloudCheap.
///  - VoxScript: local if available (native knowledge), else CloudFrontier (+RAG).
///  - Hard: CloudFrontier (escalate), regardless of local availability.
pub fn route(task: TaskClass, local: &Choice) -> Backend {
    match (task, local) {
        (TaskClass::Hard, _) => Backend::CloudFrontier,
        (TaskClass::Micro, Choice::Local(id)) => Backend::Local(id.clone()),
        (TaskClass::Micro, Choice::Cloud) => Backend::CloudCheap,
        (TaskClass::VoxScript, Choice::Local(id)) => Backend::Local(id.clone()),
        (TaskClass::VoxScript, Choice::Cloud) => Backend::CloudFrontier,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn micro_prefers_local() {
        assert_eq!(route(TaskClass::Micro, &Choice::Local("v".into())), Backend::Local("v".into()));
    }
    #[test]
    fn micro_without_gpu_uses_cheap_cloud() {
        assert_eq!(route(TaskClass::Micro, &Choice::Cloud), Backend::CloudCheap);
    }
    #[test]
    fn voxscript_runs_local_when_available() {
        assert_eq!(route(TaskClass::VoxScript, &Choice::Local("v".into())), Backend::Local("v".into()));
    }
    #[test]
    fn voxscript_without_local_escalates_to_frontier() {
        assert_eq!(route(TaskClass::VoxScript, &Choice::Cloud), Backend::CloudFrontier);
    }
    #[test]
    fn hard_always_frontier_even_with_local() {
        assert_eq!(route(TaskClass::Hard, &Choice::Local("v".into())), Backend::CloudFrontier);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-dispatch router`
Expected: FAIL — `module router not found`.

- [ ] **Step 3: Write minimal implementation**

Add `pub mod router;` to `lib.rs`. `router.rs` above is the implementation.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-dispatch router`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-dispatch/src/router.rs crates/vox-dispatch/src/lib.rs
git commit -m "feat(vox-dispatch): DispatchRouter routes by task class + local availability"
```

---

## Task 6: CapabilityProbe trait + real SystemProbe

**Files:**
- Create: `crates/vox-dispatch/src/probe.rs`
- Modify: `crates/vox-dispatch/src/lib.rs` (add `pub mod probe;`)

- [ ] **Step 1: Write the failing test** — `probe.rs`

```rust
use crate::profile::{GpuInfo, HwProfile};

/// Produces an `HwProfile` for the current host. Trait so logic is testable.
pub trait CapabilityProbe {
    fn probe(&self) -> HwProfile;
}

/// Parse `nvidia-smi --query-gpu=name,memory.total --format=csv,noheader,nounits` output
/// (memory in MiB) into GpuInfo list. Pure — unit-testable without a GPU.
pub fn parse_nvidia_smi(output: &str) -> Vec<GpuInfo> {
    output
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(2, ',');
            let name = parts.next()?.trim().to_string();
            let mib: u64 = parts.next()?.trim().parse().ok()?;
            if name.is_empty() { return None; }
            Some(GpuInfo { name, vram_gb: (mib / 1024) as u32 })
        })
        .collect()
}

/// Test double with a fixed profile.
pub struct FakeProbe(pub HwProfile);
impl CapabilityProbe for FakeProbe {
    fn probe(&self) -> HwProfile { self.0.clone() }
}

/// Real probe: threads via std, RAM via sysinfo, GPUs via nvidia-smi.
pub struct SystemProbe;
impl CapabilityProbe for SystemProbe {
    fn probe(&self) -> HwProfile {
        let cpu_threads = std::thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(1);
        let mut sys = sysinfo::System::new();
        sys.refresh_memory();
        let ram_gb = (sys.total_memory() / 1024 / 1024 / 1024) as u32;
        let gpus = std::process::Command::new("nvidia-smi")
            .args(["--query-gpu=name,memory.total", "--format=csv,noheader,nounits"])
            .output()
            .ok()
            .map(|o| parse_nvidia_smi(&String::from_utf8_lossy(&o.stdout)))
            .unwrap_or_default();
        HwProfile { cpu_threads, ram_gb, gpus }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_two_3090s() {
        let out = "NVIDIA GeForce RTX 3090, 24576\nNVIDIA GeForce RTX 3090, 24576\n";
        let gpus = parse_nvidia_smi(out);
        assert_eq!(gpus.len(), 2);
        assert_eq!(gpus[0].vram_gb, 24);
        assert_eq!(gpus[0].name, "NVIDIA GeForce RTX 3090");
    }
    #[test]
    fn empty_output_means_no_gpu() {
        assert!(parse_nvidia_smi("").is_empty());
    }
    #[test]
    fn fake_probe_returns_its_profile() {
        let p = HwProfile { cpu_threads: 48, ram_gb: 128, gpus: vec![] };
        assert_eq!(FakeProbe(p.clone()).probe(), p);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-dispatch probe`
Expected: FAIL — `module probe not found`.

- [ ] **Step 3: Write minimal implementation**

Add `pub mod probe;` to `lib.rs`. `probe.rs` above is the implementation.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-dispatch probe`
Expected: PASS (3 tests). (The real `SystemProbe` is not exercised by unit tests — only its pure `parse_nvidia_smi`.)

- [ ] **Step 5: Commit**

```bash
git add crates/vox-dispatch/src/probe.rs crates/vox-dispatch/src/lib.rs
git commit -m "feat(vox-dispatch): CapabilityProbe trait + SystemProbe + nvidia-smi parser"
```

---

## Task 7: Dispatcher (end-to-end decision) over an LlmBackend trait

**Files:**
- Modify: `crates/vox-dispatch/src/router.rs` (add `LlmBackend` trait + `Dispatcher`)

- [ ] **Step 1: Write the failing test** — append to `router.rs`

```rust
use crate::catalog::Variant;
use crate::profile::HwProfile;
use crate::probe::CapabilityProbe;
use crate::selector::select_variant;

/// The seam to the real model facade (`vox_actor_runtime::llm`). Implemented in the
/// integration crate; here it's a trait so the Dispatcher is fully testable.
pub trait LlmBackend {
    /// Run `prompt` on `backend`, returning the model's text.
    fn run(&self, backend: &Backend, prompt: &str) -> String;
}

/// Ties probe + catalog + selector + router + backend into one call.
pub struct Dispatcher<P: CapabilityProbe, B: LlmBackend> {
    pub probe: P,
    pub catalog: Vec<Variant>,
    pub backend: B,
}

impl<P: CapabilityProbe, B: LlmBackend> Dispatcher<P, B> {
    pub fn dispatch(&self, task: TaskClass, prompt: &str) -> (Backend, String) {
        let profile: HwProfile = self.probe.probe();
        let choice = select_variant(&profile, &self.catalog);
        let backend = route(task, &choice);
        let out = self.backend.run(&backend, prompt);
        (backend, out)
    }
}

#[cfg(test)]
mod dispatcher_tests {
    use super::*;
    use crate::catalog::parse_catalog;
    use crate::probe::FakeProbe;
    use crate::profile::{GpuInfo, HwProfile};

    struct EchoBackend;
    impl LlmBackend for EchoBackend {
        fn run(&self, backend: &Backend, prompt: &str) -> String {
            format!("{:?}:{}", backend, prompt)
        }
    }
    fn ladder() -> Vec<Variant> {
        parse_catalog(r#"[{"id":"voxmens-32b-fp8","precision":"fp8","min_vram_gb":40,"tokens_per_s":90,"kind":"base"}]"#).unwrap()
    }

    #[test]
    fn tower_routes_micro_to_local_variant() {
        let probe = FakeProbe(HwProfile { cpu_threads: 48, ram_gb: 128,
            gpus: vec![GpuInfo{name:"3090".into(),vram_gb:24}, GpuInfo{name:"3090".into(),vram_gb:24}] });
        let d = Dispatcher { probe, catalog: ladder(), backend: EchoBackend };
        let (backend, out) = d.dispatch(TaskClass::Micro, "route this");
        assert_eq!(backend, Backend::Local("voxmens-32b-fp8".into()));
        assert!(out.starts_with("Local"));
    }
    #[test]
    fn gpuless_routes_hard_to_frontier() {
        let probe = FakeProbe(HwProfile { cpu_threads: 8, ram_gb: 16, gpus: vec![] });
        let d = Dispatcher { probe, catalog: ladder(), backend: EchoBackend };
        let (backend, _) = d.dispatch(TaskClass::Hard, "hard");
        assert_eq!(backend, Backend::CloudFrontier);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-dispatch dispatcher`
Expected: FAIL — `LlmBackend`/`Dispatcher` not defined.

- [ ] **Step 3: Write minimal implementation** — the `LlmBackend` trait + `Dispatcher` block above (added to `router.rs`).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-dispatch`
Expected: PASS — all tests across profile/catalog/selector/router/probe/dispatcher.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-dispatch/src/router.rs
git commit -m "feat(vox-dispatch): Dispatcher wires probe->select->route->backend"
```

---

## Task 8: Facade integration + full gate

**Files:**
- Modify: `crates/vox-dispatch/src/router.rs` (doc the integration contract)
- (Integration impl lives in the orchestrator crate that already depends on `vox_actor_runtime::llm`.)

- [ ] **Step 1: Audit the facade signature**

Run: `rg -n "pub fn (llm_chat|llm_stream|infer_with_retry)" crates/`
Expected: prints the real facade fn signatures. Record them — the `LlmBackend` impl maps `Backend::Local(id)`→local model id, `Backend::CloudCheap`→Flash/Haiku model id, `Backend::CloudFrontier`→Opus/Fable via the facade's model-selection. Do NOT duplicate model selection — pass the chosen model id to the existing scorer/registry.

- [ ] **Step 2: Write the integration adapter (in the orchestrator crate)**

In the crate that owns the facade dependency, add:
```rust
// e.g. crates/vox-orchestrator/src/dispatch_adapter.rs
use vox_dispatch::router::LlmBackend;
use vox_dispatch::Backend;

pub struct FacadeBackend;   // holds whatever the facade needs (client/config)
impl LlmBackend for FacadeBackend {
    fn run(&self, backend: &Backend, prompt: &str) -> String {
        let model_id = match backend {
            Backend::Local(id) => id.as_str(),
            Backend::CloudCheap => "google/gemini-3-flash",        // or Haiku, per config
            Backend::CloudFrontier => "anthropic/claude-opus-4.8", // per config/scorer
        };
        // Call the EXISTING facade with model_id; do not re-implement selection.
        // e.g. vox_actor_runtime::llm::llm_chat(model_id, prompt)  (await/sync per real API)
        vox_actor_runtime::llm::llm_chat(model_id, prompt)
    }
}
```
Add `vox-dispatch = { path = "../vox-dispatch" }` to that crate's `Cargo.toml`.

- [ ] **Step 3: Build to verify integration compiles**

Run: `cargo build -p vox-orchestrator` (or whichever crate hosts the adapter)
Expected: compiles; if the facade fn is async, wrap with the crate's existing runtime helper.

- [ ] **Step 4: Run the full crate gate**

Run: `cargo test -p vox-dispatch -- --nocapture`
Expected: PASS — all unit tests green.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-dispatch crates/vox-orchestrator/src/dispatch_adapter.rs crates/vox-orchestrator/Cargo.toml
git commit -m "feat(dispatch): wire vox-dispatch Dispatcher into the model facade"
```

---

## Self-Review

**Spec coverage:** CapabilityProbe (§3.1)→T6; VariantCatalog/precision-ladder (§3.1–3.2)→T3; VariantSelector (§3.1)→T4; DispatchRouter + policy (§3.1, §3.3)→T5; end-to-end Dispatcher + facade seam (§3)→T7–T8. Out of scope (not code in this crate, noted in spec): hardware BOM (buy/build), build-acceleration (process), economics (analysis), and the *retrieve-dynamically skills/tool catalog* (§3.4) which is the existing `vox-search` + skill registry — this crate consumes those, it doesn't reimplement them.

**Placeholder scan:** none — every step has runnable test + implementation code; the only "audit" step (T8.1) is a real `rg` command because the facade's exact async signature must be read from the repo, not guessed.

**Type consistency:** `TaskClass`/`Backend` (lib.rs) used unchanged in router/dispatcher; `Choice` (lib.rs) used in selector + router; `HwProfile`/`GpuInfo` consistent across profile/probe/selector; `Variant`/`Precision`/`VariantKind` consistent across catalog/selector; fn names stable: `total_vram_gb`, `parse_catalog`, `select_variant`, `route`, `parse_nvidia_smi`, `Dispatcher::dispatch`, `LlmBackend::run`.
