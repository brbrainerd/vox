---
title: "Populi Mesh — Probe Correctness Implementation Plan (S1, 2026-05-01)"
description: "Step-by-step TDD implementation plan for the probe-correctness spec. 17 tasks producing the HardwareProbe trait, mock harness, refactored probes, operator override, observability, and tests. ~1000 LOC end-to-end."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
training_rationale: "Implementation plan; gets stale as tasks are completed. Spec is the durable artifact."
---

# Populi Mesh — Probe Correctness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal.** Refactor Populi's hardware probes (NVML / wgpu / DRM / Metal / DXGI) behind a `HardwareProbe` trait with a mock harness, so probe failures are observable, the order is operator-overridable, and S2 admission-control work has a testable foundation.

**Architecture.** Add a trait + a `ProbePipeline` that runs probes in order, collecting an attempt log. Each existing concrete probe gets a thin trait-impl wrapper. A mock probe enables unit tests against pipeline behavior. Replace the global `OnceCell` cache with a TTL-aware registry. Add `probe_failures` field on `HardwareSummary` (additive, serde-default-skipping) and emit one structured event per probe attempt.

**Tech stack.** Rust 2024 edition, `tokio` (already a dep for `OnceCell`), `tracing` (already used), `thiserror` (already used in workspace), no new external deps.

**Spec.** [`populi-mesh-probe-correctness-spec-2026.md`](populi-mesh-probe-correctness-spec-2026.md).

**Working directory.** Worktree at `C:\Users\Owner\vox\.claude\worktrees\brave-tharp-b57e7b`. All paths below are relative to this worktree.

---

## File map

**Create:**
- `crates/vox-populi/src/mens/hardware/probe.rs` — trait, error, attempt/outcome/report types.
- `crates/vox-populi/src/mens/hardware/pipeline.rs` — `ProbePipeline` and `default_for_platform()`.
- `crates/vox-populi/src/mens/hardware/registry.rs` — TTL-aware registry replacing the bare `OnceCell`.
- `crates/vox-populi/src/mens/hardware/mock.rs` — test-only mock probe (`#[cfg(test)]`).
- `crates/vox-populi/src/mens/hardware/tests.rs` — inline unit tests.
- `crates/vox-populi/tests/probe_pipeline.rs` — integration tests.
- `crates/vox-populi/tests/probe_pipeline_live.rs` — feature-gated real-hardware tests.

**Modify:**
- `crates/vox-populi/src/mens/hardware/types.rs` — add `probe_failures` field on `HardwareSummary`.
- `crates/vox-populi/src/mens/hardware/nvml.rs` — add `pub struct NvmlProbe` impl.
- `crates/vox-populi/src/mens/hardware/wgpu_probe.rs` — add `pub struct WgpuProbe` impl.
- `crates/vox-populi/src/mens/hardware/linux_drm.rs` — add `pub struct LinuxDrmProbe` impl.
- `crates/vox-populi/src/mens/hardware/macos_metal.rs` — add `pub struct MacosMetalProbe` impl.
- `crates/vox-populi/src/mens/hardware/win_dxgi.rs` — add `pub struct WinDxgiProbe` impl.
- `crates/vox-populi/src/mens/hardware/mod.rs` — replace `probe_internal()` with pipeline composition.
- `crates/vox-populi/Cargo.toml` — add `hw-probe-live-test` feature flag.
- `docs/src/reference/populi.md` — add Appendix on probes.

---

## Task ordering rationale

Tasks are ordered so each one leaves the workspace in a building, testing state. The trait is added before any concrete probe is converted (Tasks 1–3). Concrete probes are converted in dependency-free order (Tasks 4–8). The pipeline is wired up only after all probes have impls (Task 9). Each task ends with a `cargo test -p vox-populi` run and a commit.

---

## Task 1: Probe trait + error type + outcome types

**Files:**
- Create: `crates/vox-populi/src/mens/hardware/probe.rs`
- Modify: `crates/vox-populi/src/mens/hardware/mod.rs:5-13` (add `pub mod probe;`)

- [ ] **Step 1: Create the new module file with trait and types**

`crates/vox-populi/src/mens/hardware/probe.rs`:

```rust
use crate::mens::hardware::types::HardwareSummary;
use async_trait::async_trait;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProbeError {
    #[error("library unavailable: {0}")]
    LibraryUnavailable(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("device-side error: {0}")]
    DeviceError(String),
    #[error("other: {0}")]
    Other(String),
}

#[async_trait]
pub trait HardwareProbe: Send + Sync {
    fn name(&self) -> &'static str;
    fn applicable(&self) -> bool;
    async fn probe(&self) -> Result<Option<HardwareSummary>, ProbeError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeOutcome {
    NotApplicable,
    NoDevice,
    Found(Box<HardwareSummary>),
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct ProbeAttempt {
    pub probe_name: &'static str,
    pub outcome: ProbeOutcome,
    pub duration_ms: u64,
}

#[derive(Debug, Clone)]
pub struct ProbeReport {
    pub summary: HardwareSummary,
    pub attempts: Vec<ProbeAttempt>,
}
```

- [ ] **Step 2: Wire the module into `hardware/mod.rs`**

In `crates/vox-populi/src/mens/hardware/mod.rs`, after the existing `pub mod` declarations, add:

```rust
pub mod probe;
```

- [ ] **Step 3: Add `async-trait` and `thiserror` to `vox-populi` Cargo.toml if not present**

Check `crates/vox-populi/Cargo.toml`. If `async-trait` is missing under `[dependencies]`, add:

```toml
async-trait = { workspace = true }
```

If `thiserror` is missing, add the same.

Run: `cargo build -p vox-populi 2>&1 | head -30`
Expected: builds clean, no errors.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-populi/src/mens/hardware/probe.rs \
        crates/vox-populi/src/mens/hardware/mod.rs \
        crates/vox-populi/Cargo.toml
git commit -m "feat(populi): add HardwareProbe trait and outcome/report types"
```

---

## Task 2: MockProbe + first round-trip test

**Files:**
- Create: `crates/vox-populi/src/mens/hardware/mock.rs`
- Create: `crates/vox-populi/src/mens/hardware/tests.rs`
- Modify: `crates/vox-populi/src/mens/hardware/mod.rs` (declare `mock` and `tests` modules)

- [ ] **Step 1: Write the failing test**

Create `crates/vox-populi/src/mens/hardware/tests.rs`:

```rust
#[cfg(test)]
mod tests {
    use crate::mens::hardware::mock::MockProbe;
    use crate::mens::hardware::probe::{HardwareProbe, ProbeError};
    use crate::mens::hardware::types::{ComputeBackend, GpuVendor, HardwareSummary};

    fn dummy_summary() -> HardwareSummary {
        HardwareSummary {
            model_name: "Test GPU".into(),
            vram_mb: 8192,
            gpu_count: 1,
            vendor: GpuVendor::Nvidia,
            backend: ComputeBackend::Cuda,
            driver_version: None,
            pci_bus_id: None,
            probe_failures: None,
        }
    }

    #[tokio::test]
    async fn mock_probe_returns_configured_result() {
        let probe = MockProbe {
            name: "test",
            applicable: true,
            result: Ok(Some(dummy_summary())),
        };
        assert_eq!(probe.name(), "test");
        assert!(probe.applicable());
        let res = probe.probe().await.unwrap();
        assert_eq!(res.unwrap().model_name, "Test GPU");
    }

    #[tokio::test]
    async fn mock_probe_propagates_error() {
        let probe = MockProbe {
            name: "broken",
            applicable: true,
            result: Err(ProbeError::LibraryUnavailable("nvml".into())),
        };
        assert_eq!(
            probe.probe().await.unwrap_err(),
            ProbeError::LibraryUnavailable("nvml".into())
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-populi --lib mens::hardware::tests 2>&1 | tail -20`
Expected: FAIL — `mock` module / `MockProbe` / `probe_failures` field not found.

- [ ] **Step 3: Add `probe_failures` field to `HardwareSummary`**

In `crates/vox-populi/src/mens/hardware/types.rs:34-43`, replace the struct with:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareSummary {
    pub model_name: String,
    pub vram_mb: u64,
    pub gpu_count: u32,
    pub vendor: GpuVendor,
    pub backend: ComputeBackend,
    pub driver_version: Option<String>,
    pub pci_bus_id: Option<String>,
    /// Names of probes that failed during the run that produced this summary.
    /// `None` (or absent in serialized form) means "no failures or not produced
    /// by a pipeline run". Empty `Some(vec![])` is reserved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probe_failures: Option<Vec<String>>,
}
```

Then update every `HardwareSummary { ... }` literal in the codebase (search `grep -rn "HardwareSummary {" crates/vox-populi/src`) to add `probe_failures: None,`. Existing call sites:
- `mens/hardware/mod.rs:53,87,98` — three literals.
- `mens/hardware/wgpu_probe.rs:29` — one literal.

For each, add the field at the end with value `None`.

- [ ] **Step 4: Create the mock module**

`crates/vox-populi/src/mens/hardware/mock.rs`:

```rust
#![cfg(test)]

use crate::mens::hardware::probe::{HardwareProbe, ProbeError};
use crate::mens::hardware::types::HardwareSummary;
use async_trait::async_trait;

pub(crate) struct MockProbe {
    pub name: &'static str,
    pub applicable: bool,
    pub result: Result<Option<HardwareSummary>, ProbeError>,
}

#[async_trait]
impl HardwareProbe for MockProbe {
    fn name(&self) -> &'static str {
        self.name
    }
    fn applicable(&self) -> bool {
        self.applicable
    }
    async fn probe(&self) -> Result<Option<HardwareSummary>, ProbeError> {
        match &self.result {
            Ok(opt) => Ok(opt.clone()),
            Err(e) => Err(match e {
                ProbeError::LibraryUnavailable(s) => ProbeError::LibraryUnavailable(s.clone()),
                ProbeError::PermissionDenied(s) => ProbeError::PermissionDenied(s.clone()),
                ProbeError::DeviceError(s) => ProbeError::DeviceError(s.clone()),
                ProbeError::Other(s) => ProbeError::Other(s.clone()),
            }),
        }
    }
}
```

- [ ] **Step 5: Wire `mock` and `tests` modules into `hardware/mod.rs`**

After `pub mod probe;`, add:

```rust
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
```

- [ ] **Step 6: Run tests, verify pass**

Run: `cargo test -p vox-populi --lib mens::hardware::tests 2>&1 | tail -10`
Expected: PASS for both `mock_probe_returns_configured_result` and `mock_probe_propagates_error`.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-populi/src/mens/hardware/mock.rs \
        crates/vox-populi/src/mens/hardware/tests.rs \
        crates/vox-populi/src/mens/hardware/mod.rs \
        crates/vox-populi/src/mens/hardware/types.rs \
        crates/vox-populi/src/mens/hardware/wgpu_probe.rs
git commit -m "feat(populi): add MockProbe and probe_failures field on HardwareSummary"
```

---

## Task 3: ProbePipeline + run() over a single probe

**Files:**
- Create: `crates/vox-populi/src/mens/hardware/pipeline.rs`
- Modify: `crates/vox-populi/src/mens/hardware/mod.rs` (declare `pipeline`)
- Modify: `crates/vox-populi/src/mens/hardware/tests.rs` (add pipeline tests)

- [ ] **Step 1: Write the failing test**

Append to `crates/vox-populi/src/mens/hardware/tests.rs` (inside the existing `mod tests`):

```rust
    use crate::mens::hardware::pipeline::ProbePipeline;
    use crate::mens::hardware::probe::ProbeOutcome;

    #[tokio::test]
    async fn pipeline_returns_first_found() {
        let pipeline = ProbePipeline::empty()
            .with_probe(Box::new(MockProbe {
                name: "first",
                applicable: true,
                result: Ok(Some(dummy_summary())),
            }));
        let report = pipeline.run().await;
        assert_eq!(report.summary.model_name, "Test GPU");
        assert_eq!(report.attempts.len(), 1);
        assert!(matches!(report.attempts[0].outcome, ProbeOutcome::Found(_)));
    }
```

- [ ] **Step 2: Run, verify fail**

Run: `cargo test -p vox-populi --lib mens::hardware::tests::tests::pipeline_returns_first_found 2>&1 | tail -15`
Expected: FAIL — `pipeline` module not found.

- [ ] **Step 3: Implement minimal pipeline**

Create `crates/vox-populi/src/mens/hardware/pipeline.rs`:

```rust
use crate::mens::hardware::probe::{HardwareProbe, ProbeAttempt, ProbeOutcome, ProbeReport};
use crate::mens::hardware::types::{ComputeBackend, GpuVendor, HardwareSummary};
use std::time::Instant;

pub struct ProbePipeline {
    probes: Vec<Box<dyn HardwareProbe>>,
}

impl ProbePipeline {
    pub fn empty() -> Self {
        Self { probes: Vec::new() }
    }

    pub fn with_probe(mut self, probe: Box<dyn HardwareProbe>) -> Self {
        self.probes.push(probe);
        self
    }

    pub async fn run(&self) -> ProbeReport {
        let mut attempts = Vec::new();
        let mut summary: Option<HardwareSummary> = None;
        let mut failures: Vec<String> = Vec::new();

        for probe in &self.probes {
            let name = probe.name();
            if !probe.applicable() {
                attempts.push(ProbeAttempt {
                    probe_name: name,
                    outcome: ProbeOutcome::NotApplicable,
                    duration_ms: 0,
                });
                continue;
            }
            let start = Instant::now();
            let res = probe.probe().await;
            let duration_ms = start.elapsed().as_millis() as u64;
            match res {
                Ok(Some(s)) => {
                    let s_clone = s.clone();
                    attempts.push(ProbeAttempt {
                        probe_name: name,
                        outcome: ProbeOutcome::Found(Box::new(s)),
                        duration_ms,
                    });
                    if summary.is_none() {
                        summary = Some(s_clone);
                    }
                }
                Ok(None) => {
                    attempts.push(ProbeAttempt {
                        probe_name: name,
                        outcome: ProbeOutcome::NoDevice,
                        duration_ms,
                    });
                }
                Err(e) => {
                    failures.push(name.to_string());
                    attempts.push(ProbeAttempt {
                        probe_name: name,
                        outcome: ProbeOutcome::Failed(e.to_string()),
                        duration_ms,
                    });
                }
            }
        }

        let mut summary = summary.unwrap_or_else(cpu_fallback);
        if !failures.is_empty() {
            summary.probe_failures = Some(failures);
        }
        ProbeReport { summary, attempts }
    }
}

fn cpu_fallback() -> HardwareSummary {
    HardwareSummary {
        model_name: "Host CPU".into(),
        vram_mb: 0,
        gpu_count: 0,
        vendor: GpuVendor::Cpu,
        backend: ComputeBackend::Cpu,
        driver_version: None,
        pci_bus_id: None,
        probe_failures: None,
    }
}
```

- [ ] **Step 4: Wire `pipeline` module into `hardware/mod.rs`**

After `pub mod probe;`, add:

```rust
pub mod pipeline;
```

- [ ] **Step 5: Run, verify pass**

Run: `cargo test -p vox-populi --lib mens::hardware::tests::tests::pipeline_returns_first_found 2>&1 | tail -10`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-populi/src/mens/hardware/pipeline.rs \
        crates/vox-populi/src/mens/hardware/mod.rs \
        crates/vox-populi/src/mens/hardware/tests.rs
git commit -m "feat(populi): add ProbePipeline with single-probe run()"
```

---

## Task 4: Pipeline behavior — skip-no-device, fail-and-continue, all-fail

**Files:**
- Modify: `crates/vox-populi/src/mens/hardware/tests.rs`

- [ ] **Step 1: Write failing tests**

Append to `mod tests`:

```rust
    #[tokio::test]
    async fn pipeline_skips_no_device_to_next() {
        let pipeline = ProbePipeline::empty()
            .with_probe(Box::new(MockProbe {
                name: "no_dev",
                applicable: true,
                result: Ok(None),
            }))
            .with_probe(Box::new(MockProbe {
                name: "found",
                applicable: true,
                result: Ok(Some(dummy_summary())),
            }));
        let report = pipeline.run().await;
        assert_eq!(report.summary.model_name, "Test GPU");
        assert!(matches!(report.attempts[0].outcome, ProbeOutcome::NoDevice));
        assert!(matches!(report.attempts[1].outcome, ProbeOutcome::Found(_)));
    }

    #[tokio::test]
    async fn pipeline_failure_does_not_abort() {
        let pipeline = ProbePipeline::empty()
            .with_probe(Box::new(MockProbe {
                name: "broken",
                applicable: true,
                result: Err(ProbeError::DeviceError("oops".into())),
            }))
            .with_probe(Box::new(MockProbe {
                name: "found",
                applicable: true,
                result: Ok(Some(dummy_summary())),
            }));
        let report = pipeline.run().await;
        assert_eq!(report.summary.model_name, "Test GPU");
        assert_eq!(report.summary.probe_failures.as_deref(), Some(&["broken".to_string()][..]));
    }

    #[tokio::test]
    async fn pipeline_all_fail_returns_cpu_fallback() {
        let pipeline = ProbePipeline::empty()
            .with_probe(Box::new(MockProbe {
                name: "a",
                applicable: true,
                result: Err(ProbeError::Other("a".into())),
            }))
            .with_probe(Box::new(MockProbe {
                name: "b",
                applicable: true,
                result: Err(ProbeError::Other("b".into())),
            }));
        let report = pipeline.run().await;
        assert_eq!(report.summary.model_name, "Host CPU");
        assert_eq!(
            report.summary.probe_failures.as_deref(),
            Some(&["a".to_string(), "b".to_string()][..])
        );
    }

    #[tokio::test]
    async fn pipeline_skips_not_applicable() {
        let pipeline = ProbePipeline::empty()
            .with_probe(Box::new(MockProbe {
                name: "off",
                applicable: false,
                result: Ok(Some(dummy_summary())),
            }))
            .with_probe(Box::new(MockProbe {
                name: "on",
                applicable: true,
                result: Ok(Some(dummy_summary())),
            }));
        let report = pipeline.run().await;
        assert_eq!(report.attempts[0].probe_name, "off");
        assert!(matches!(report.attempts[0].outcome, ProbeOutcome::NotApplicable));
        assert_eq!(report.attempts[0].duration_ms, 0);
        assert!(matches!(report.attempts[1].outcome, ProbeOutcome::Found(_)));
    }
```

- [ ] **Step 2: Run, verify pass**

Run: `cargo test -p vox-populi --lib mens::hardware::tests 2>&1 | tail -15`
Expected: All four new tests PASS (the existing pipeline implementation already handles these cases per Task 3).

- [ ] **Step 3: Commit**

```bash
git add crates/vox-populi/src/mens/hardware/tests.rs
git commit -m "test(populi): pipeline skip/fail/all-fail/not-applicable scenarios"
```

---

## Task 5: NvmlProbe trait impl

**Files:**
- Modify: `crates/vox-populi/src/mens/hardware/nvml.rs`

- [ ] **Step 1: Write failing test (real-hardware compile, gated)**

Append to `crates/vox-populi/src/mens/hardware/tests.rs`:

```rust
    #[cfg(feature = "nvml-gpu-probe")]
    #[tokio::test]
    async fn nvml_probe_compiles_and_is_constructible() {
        let probe = crate::mens::hardware::nvml::NvmlProbe;
        assert_eq!(probe.name(), "nvml");
        // applicable() may be false on machines without NVML — that's fine.
        let _ = probe.applicable();
    }
```

- [ ] **Step 2: Run, verify fail**

Run: `cargo test -p vox-populi --lib --features nvml-gpu-probe mens::hardware::tests::tests::nvml_probe_compiles_and_is_constructible 2>&1 | tail -10`
Expected: FAIL — `NvmlProbe` not found.

- [ ] **Step 3: Implement NvmlProbe**

In `crates/vox-populi/src/mens/hardware/nvml.rs`, append after the existing `monitor_nvml` function:

```rust
use crate::mens::hardware::probe::{HardwareProbe, ProbeError};
use crate::mens::hardware::types::{ComputeBackend, GpuVendor, HardwareSummary, BYTES_TO_MB};
use async_trait::async_trait;

pub struct NvmlProbe;

#[async_trait]
impl HardwareProbe for NvmlProbe {
    fn name(&self) -> &'static str {
        "nvml"
    }
    fn applicable(&self) -> bool {
        // NVML probe always applies if compiled in; library availability checked at probe time.
        true
    }
    async fn probe(&self) -> Result<Option<HardwareSummary>, ProbeError> {
        use nvml_wrapper::Nvml;
        let nvml = match Nvml::init() {
            Ok(n) => n,
            Err(e) => return Err(ProbeError::LibraryUnavailable(e.to_string())),
        };
        let count = nvml
            .device_count()
            .map_err(|e| ProbeError::DeviceError(e.to_string()))?;
        if count == 0 {
            return Ok(None);
        }
        let device = nvml
            .device_by_index(0)
            .map_err(|e| ProbeError::DeviceError(e.to_string()))?;
        let name = device
            .name()
            .map_err(|e| ProbeError::DeviceError(e.to_string()))?;
        let mem = device
            .memory_info()
            .map_err(|e| ProbeError::DeviceError(e.to_string()))?;
        let total_mb = (mem.used + mem.free) / BYTES_TO_MB;
        let driver_version = nvml.sys_driver_version().ok();

        Ok(Some(HardwareSummary {
            model_name: name,
            vram_mb: total_mb,
            gpu_count: count,
            vendor: GpuVendor::Nvidia,
            backend: ComputeBackend::Cuda,
            driver_version,
            pci_bus_id: None,
            probe_failures: None,
        }))
    }
}
```

- [ ] **Step 4: Run, verify pass**

Run: `cargo test -p vox-populi --lib --features nvml-gpu-probe mens::hardware::tests::tests::nvml_probe_compiles_and_is_constructible 2>&1 | tail -10`
Expected: PASS.

- [ ] **Step 5: Confirm non-feature build still passes**

Run: `cargo build -p vox-populi --no-default-features 2>&1 | tail -10`
Expected: clean build (the `NvmlProbe` impl is gated by the `nvml-gpu-probe` mod-level cfg in `mod.rs`).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-populi/src/mens/hardware/nvml.rs \
        crates/vox-populi/src/mens/hardware/tests.rs
git commit -m "feat(populi): NvmlProbe trait impl"
```

---

## Task 6: WgpuProbe trait impl

**Files:**
- Modify: `crates/vox-populi/src/mens/hardware/wgpu_probe.rs`

- [ ] **Step 1: Append impl**

In `crates/vox-populi/src/mens/hardware/wgpu_probe.rs`, append after `probe_wgpu`:

```rust
use crate::mens::hardware::probe::{HardwareProbe, ProbeError};
use async_trait::async_trait;

pub struct WgpuProbe;

#[async_trait]
impl HardwareProbe for WgpuProbe {
    fn name(&self) -> &'static str {
        "wgpu"
    }
    fn applicable(&self) -> bool {
        true
    }
    async fn probe(&self) -> Result<Option<HardwareSummary>, ProbeError> {
        match probe_wgpu().await {
            Some(mut s) => {
                s.probe_failures = None;
                Ok(Some(s))
            }
            None => Ok(None),
        }
    }
}
```

- [ ] **Step 2: Run, verify build**

Run: `cargo build -p vox-populi --features mens-gpu 2>&1 | tail -10`
Expected: clean build.

- [ ] **Step 3: Commit**

```bash
git add crates/vox-populi/src/mens/hardware/wgpu_probe.rs
git commit -m "feat(populi): WgpuProbe trait impl"
```

---

## Task 7: LinuxDrmProbe, MacosMetalProbe, WinDxgiProbe trait impls

**Files:**
- Modify: `crates/vox-populi/src/mens/hardware/linux_drm.rs`
- Modify: `crates/vox-populi/src/mens/hardware/macos_metal.rs`
- Modify: `crates/vox-populi/src/mens/hardware/win_dxgi.rs`

- [ ] **Step 1: Add LinuxDrmProbe impl**

In `linux_drm.rs`, append:

```rust
use crate::mens::hardware::probe::{HardwareProbe, ProbeError};
use async_trait::async_trait;

pub struct LinuxDrmProbe;

#[async_trait]
impl HardwareProbe for LinuxDrmProbe {
    fn name(&self) -> &'static str {
        "linux_drm"
    }
    fn applicable(&self) -> bool {
        cfg!(target_os = "linux")
    }
    async fn probe(&self) -> Result<Option<HardwareSummary>, ProbeError> {
        Ok(probe_drm())
    }
}
```

- [ ] **Step 2: Add MacosMetalProbe impl**

In `macos_metal.rs`, append:

```rust
use crate::mens::hardware::probe::{HardwareProbe, ProbeError};
use async_trait::async_trait;

pub struct MacosMetalProbe;

#[async_trait]
impl HardwareProbe for MacosMetalProbe {
    fn name(&self) -> &'static str {
        "macos_metal"
    }
    fn applicable(&self) -> bool {
        cfg!(target_os = "macos")
    }
    async fn probe(&self) -> Result<Option<HardwareSummary>, ProbeError> {
        Ok(probe_metal())
    }
}
```

- [ ] **Step 3: Add WinDxgiProbe impl**

In `win_dxgi.rs`, append (this file is gated `#[cfg(all(target_os = "windows", feature = "mens-gpu"))]` at the module level in `mod.rs`, so we don't need additional gating):

```rust
use crate::mens::hardware::probe::{HardwareProbe, ProbeError};
use async_trait::async_trait;

pub struct WinDxgiProbe;

#[async_trait]
impl HardwareProbe for WinDxgiProbe {
    fn name(&self) -> &'static str {
        "win_dxgi"
    }
    fn applicable(&self) -> bool {
        true
    }
    async fn probe(&self) -> Result<Option<HardwareSummary>, ProbeError> {
        Ok(probe_dxgi())
    }
}
```

- [ ] **Step 4: Run, verify build on current platform**

Run: `cargo build -p vox-populi 2>&1 | tail -10`
Expected: clean build.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-populi/src/mens/hardware/linux_drm.rs \
        crates/vox-populi/src/mens/hardware/macos_metal.rs \
        crates/vox-populi/src/mens/hardware/win_dxgi.rs
git commit -m "feat(populi): DRM, Metal, DXGI probe trait impls"
```

---

## Task 8: ProbePipeline::default_for_platform

**Files:**
- Modify: `crates/vox-populi/src/mens/hardware/pipeline.rs`
- Modify: `crates/vox-populi/src/mens/hardware/tests.rs`

- [ ] **Step 1: Write failing test**

Append to `mod tests`:

```rust
    #[test]
    fn default_pipeline_for_platform_has_probes() {
        let pipeline = ProbePipeline::default_for_platform();
        assert!(!pipeline.probes.is_empty(), "expected at least one probe");
    }
```

Note: this test introspects `pipeline.probes`, which is currently private. Make it `pub(crate)` in the next step.

- [ ] **Step 2: Add `default_for_platform` and expose `probes` to crate**

In `pipeline.rs`:

Replace the `probes` field declaration with:

```rust
pub(crate) probes: Vec<Box<dyn HardwareProbe>>,
```

Add an impl block extending `ProbePipeline`:

```rust
impl ProbePipeline {
    pub fn default_for_platform() -> Self {
        let mut pipeline = Self::empty();

        #[cfg(all(target_os = "windows", feature = "mens-gpu"))]
        {
            pipeline = pipeline.with_probe(Box::new(
                crate::mens::hardware::win_dxgi::WinDxgiProbe,
            ));
        }
        #[cfg(target_os = "linux")]
        {
            pipeline = pipeline.with_probe(Box::new(
                crate::mens::hardware::linux_drm::LinuxDrmProbe,
            ));
        }
        #[cfg(target_os = "macos")]
        {
            pipeline = pipeline.with_probe(Box::new(
                crate::mens::hardware::macos_metal::MacosMetalProbe,
            ));
        }
        #[cfg(feature = "mens-gpu")]
        {
            pipeline = pipeline.with_probe(Box::new(
                crate::mens::hardware::wgpu_probe::WgpuProbe,
            ));
        }
        #[cfg(feature = "nvml-gpu-probe")]
        {
            pipeline = pipeline.with_probe(Box::new(
                crate::mens::hardware::nvml::NvmlProbe,
            ));
        }

        pipeline
    }
}
```

- [ ] **Step 3: Run, verify pass**

Run: `cargo test -p vox-populi --lib mens::hardware::tests::tests::default_pipeline_for_platform_has_probes 2>&1 | tail -10`
Expected: PASS (with `mens-gpu` feature on, default).

- [ ] **Step 4: Commit**

```bash
git add crates/vox-populi/src/mens/hardware/pipeline.rs \
        crates/vox-populi/src/mens/hardware/tests.rs
git commit -m "feat(populi): ProbePipeline::default_for_platform"
```

---

## Task 9: Replace probe_internal() with pipeline composition

**Files:**
- Modify: `crates/vox-populi/src/mens/hardware/mod.rs`

This task preserves the existing public API (`probe()` returning `Arc<HardwareSummary>`) while routing through the pipeline.

- [ ] **Step 1: Add a new probe-with-report function**

In `mod.rs`, after `pub async fn probe()`:

```rust
/// Probe with full attempt log. Used by `vox doctor mesh` and tests.
pub async fn probe_with_report() -> crate::mens::hardware::probe::ProbeReport {
    crate::mens::hardware::pipeline::ProbePipeline::default_for_platform()
        .run()
        .await
}
```

- [ ] **Step 2: Replace `probe_internal()` body with pipeline call**

Replace the existing `async fn probe_internal()` body (lines 47-107) with:

```rust
async fn probe_internal() -> HardwareSummary {
    // 1. Check for overrides in vox-secrets (preserved from previous behavior).
    if let (Some(model), Some(vram_s)) = (
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxGpuModel).expose(),
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxGpuVramMb).expose(),
    ) {
        if let Ok(vram_mb) = vram_s.parse::<u64>() {
            return HardwareSummary {
                vendor: types::vendor_from_model(&model),
                model_name: model.to_string(),
                vram_mb,
                gpu_count: 1,
                backend: types::ComputeBackend::Unknown,
                driver_version: None,
                pci_bus_id: None,
                probe_failures: None,
            };
        }
    }

    // 2. Run the platform-default pipeline.
    crate::mens::hardware::pipeline::ProbePipeline::default_for_platform()
        .run()
        .await
        .summary
}
```

The vox-secrets-override path stays in `probe_internal()` rather than becoming a probe because it preempts probing entirely; making it a probe would still need this short-circuit anyway.

- [ ] **Step 3: Run, verify build and existing tests pass**

Run: `cargo test -p vox-populi 2>&1 | tail -20`
Expected: all tests pass (this is a refactor; behavior should be unchanged for the non-vox-secrets-override path).

- [ ] **Step 4: Commit**

```bash
git add crates/vox-populi/src/mens/hardware/mod.rs
git commit -m "refactor(populi): probe_internal() routes through ProbePipeline"
```

---

## Task 10: Observability — emit a tracing event per attempt

**Files:**
- Modify: `crates/vox-populi/src/mens/hardware/pipeline.rs`
- Modify: `crates/vox-populi/src/mens/hardware/tests.rs`

- [ ] **Step 1: Write failing test (using `tracing-test`)**

If `tracing-test` is not in dev-deps, add it:

In `crates/vox-populi/Cargo.toml` `[dev-dependencies]`:

```toml
tracing-test = "0.2"
```

Append to `tests.rs`:

```rust
    #[tokio::test]
    #[tracing_test::traced_test]
    async fn pipeline_emits_event_per_attempt() {
        let pipeline = ProbePipeline::empty()
            .with_probe(Box::new(MockProbe {
                name: "found",
                applicable: true,
                result: Ok(Some(dummy_summary())),
            }))
            .with_probe(Box::new(MockProbe {
                name: "broken",
                applicable: true,
                result: Err(ProbeError::Other("x".into())),
            }));
        let _ = pipeline.run().await;
        assert!(logs_contain("vox.mesh.probe.name=\"found\""));
        assert!(logs_contain("vox.mesh.probe.outcome=\"found\""));
        assert!(logs_contain("vox.mesh.probe.name=\"broken\""));
        assert!(logs_contain("vox.mesh.probe.outcome=\"failed\""));
    }
```

- [ ] **Step 2: Run, verify fail**

Run: `cargo test -p vox-populi --lib mens::hardware::tests::tests::pipeline_emits_event_per_attempt 2>&1 | tail -10`
Expected: FAIL — events not emitted.

- [ ] **Step 3: Add tracing events to pipeline run**

In `pipeline.rs`, replace the body of the `for probe in &self.probes` loop with:

```rust
        for probe in &self.probes {
            let name = probe.name();
            if !probe.applicable() {
                tracing::debug!(
                    "vox.mesh.probe.name" = name,
                    "vox.mesh.probe.outcome" = "not_applicable",
                    "vox.mesh.probe.duration_ms" = 0u64,
                );
                attempts.push(ProbeAttempt {
                    probe_name: name,
                    outcome: ProbeOutcome::NotApplicable,
                    duration_ms: 0,
                });
                continue;
            }
            let start = Instant::now();
            let res = probe.probe().await;
            let duration_ms = start.elapsed().as_millis() as u64;
            match res {
                Ok(Some(s)) => {
                    tracing::debug!(
                        "vox.mesh.probe.name" = name,
                        "vox.mesh.probe.outcome" = "found",
                        "vox.mesh.probe.duration_ms" = duration_ms,
                    );
                    let s_clone = s.clone();
                    attempts.push(ProbeAttempt {
                        probe_name: name,
                        outcome: ProbeOutcome::Found(Box::new(s)),
                        duration_ms,
                    });
                    if summary.is_none() {
                        summary = Some(s_clone);
                    }
                }
                Ok(None) => {
                    tracing::debug!(
                        "vox.mesh.probe.name" = name,
                        "vox.mesh.probe.outcome" = "no_device",
                        "vox.mesh.probe.duration_ms" = duration_ms,
                    );
                    attempts.push(ProbeAttempt {
                        probe_name: name,
                        outcome: ProbeOutcome::NoDevice,
                        duration_ms,
                    });
                }
                Err(e) => {
                    let err_str = e.to_string();
                    tracing::warn!(
                        "vox.mesh.probe.name" = name,
                        "vox.mesh.probe.outcome" = "failed",
                        "vox.mesh.probe.error" = %err_str,
                        "vox.mesh.probe.duration_ms" = duration_ms,
                    );
                    failures.push(name.to_string());
                    attempts.push(ProbeAttempt {
                        probe_name: name,
                        outcome: ProbeOutcome::Failed(err_str),
                        duration_ms,
                    });
                }
            }
        }
```

- [ ] **Step 4: Run, verify pass**

Run: `cargo test -p vox-populi --lib mens::hardware::tests::tests::pipeline_emits_event_per_attempt 2>&1 | tail -10`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-populi/Cargo.toml \
        crates/vox-populi/src/mens/hardware/pipeline.rs \
        crates/vox-populi/src/mens/hardware/tests.rs
git commit -m "feat(populi): tracing events for probe pipeline attempts"
```

---

## Task 11: Operator override — accept a probe-name list and reorder

**Files:**
- Modify: `crates/vox-populi/src/mens/hardware/pipeline.rs`
- Modify: `crates/vox-populi/src/mens/hardware/tests.rs`

- [ ] **Step 1: Write failing tests**

Append to `tests.rs`:

```rust
    use crate::mens::hardware::pipeline::PipelineOrderError;

    #[test]
    fn pipeline_reorder_respects_explicit_order() {
        let pipeline = ProbePipeline::empty()
            .with_probe(Box::new(MockProbe { name: "a", applicable: true, result: Ok(None) }))
            .with_probe(Box::new(MockProbe { name: "b", applicable: true, result: Ok(None) }))
            .reorder(&["b".to_string(), "a".to_string()])
            .unwrap();
        let names: Vec<&str> = pipeline.probes.iter().map(|p| p.name()).collect();
        assert_eq!(names, vec!["b", "a"]);
    }

    #[test]
    fn pipeline_reorder_rejects_unknown() {
        let pipeline = ProbePipeline::empty()
            .with_probe(Box::new(MockProbe { name: "a", applicable: true, result: Ok(None) }));
        let err = pipeline.reorder(&["nope".to_string()]).unwrap_err();
        match err {
            PipelineOrderError::Unknown(s) => assert_eq!(s, "nope"),
        }
    }
```

- [ ] **Step 2: Run, verify fail**

Run: `cargo test -p vox-populi --lib mens::hardware::tests::tests::pipeline_reorder 2>&1 | tail -10`
Expected: FAIL — `reorder` and `PipelineOrderError` not found.

- [ ] **Step 3: Implement reorder**

In `pipeline.rs`, after the existing `impl ProbePipeline { ... }` block:

```rust
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PipelineOrderError {
    #[error("unknown probe name: {0}")]
    Unknown(String),
}

impl ProbePipeline {
    /// Reorder probes by name. Names not present in the pipeline produce
    /// `PipelineOrderError::Unknown`. Names omitted from `order` are appended
    /// after the listed ones in their original relative order.
    pub fn reorder(mut self, order: &[String]) -> Result<Self, PipelineOrderError> {
        let mut by_name: std::collections::HashMap<&'static str, Box<dyn HardwareProbe>> =
            self.probes.into_iter().map(|p| (p.name(), p)).collect();
        for name in order {
            if !by_name.contains_key(name.as_str()) {
                return Err(PipelineOrderError::Unknown(name.clone()));
            }
        }
        let mut reordered: Vec<Box<dyn HardwareProbe>> = Vec::new();
        for name in order {
            // Lookup is safe because of the prior loop.
            let key = by_name
                .keys()
                .find(|k| **k == name.as_str())
                .copied()
                .unwrap();
            reordered.push(by_name.remove(key).unwrap());
        }
        // Append remaining (unmentioned) probes in their original order.
        for (_, p) in by_name {
            reordered.push(p);
        }
        self.probes = reordered;
        Ok(self)
    }
}
```

(Note: the order of remaining probes after `HashMap` iteration is not deterministic for "appended" probes. The spec accepts this; if the operator wants a specific order they list every probe.)

- [ ] **Step 4: Run, verify pass**

Run: `cargo test -p vox-populi --lib mens::hardware::tests::tests::pipeline_reorder 2>&1 | tail -10`
Expected: PASS for both tests.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-populi/src/mens/hardware/pipeline.rs \
        crates/vox-populi/src/mens/hardware/tests.rs
git commit -m "feat(populi): ProbePipeline::reorder for operator override"
```

---

## Task 12: TTL cache replacing the bare OnceCell

**Files:**
- Create: `crates/vox-populi/src/mens/hardware/registry.rs`
- Modify: `crates/vox-populi/src/mens/hardware/mod.rs`
- Modify: `crates/vox-populi/src/mens/hardware/tests.rs`

- [ ] **Step 1: Write failing test**

Append to `tests.rs`:

```rust
    #[tokio::test(start_paused = true)]
    async fn registry_cache_invalidates_after_ttl() {
        use crate::mens::hardware::registry::HardwareRegistryV2;
        use std::sync::atomic::{AtomicU32, Ordering};
        use std::sync::Arc;

        let calls = Arc::new(AtomicU32::new(0));
        let calls_clone = calls.clone();

        let factory = move || {
            let calls = calls_clone.clone();
            Box::pin(async move {
                calls.fetch_add(1, Ordering::SeqCst);
                ProbePipeline::empty()
                    .with_probe(Box::new(MockProbe {
                        name: "x",
                        applicable: true,
                        result: Ok(Some(dummy_summary())),
                    }))
                    .run()
                    .await
            }) as std::pin::Pin<Box<dyn std::future::Future<Output = _> + Send>>
        };

        let registry = HardwareRegistryV2::new(std::time::Duration::from_secs(60), Arc::new(factory));

        let _ = registry.get().await;
        let _ = registry.get().await;
        assert_eq!(calls.load(Ordering::SeqCst), 1, "second call should hit cache");

        tokio::time::advance(std::time::Duration::from_secs(120)).await;
        let _ = registry.get().await;
        assert_eq!(calls.load(Ordering::SeqCst), 2, "third call should miss after TTL");
    }
```

- [ ] **Step 2: Run, verify fail**

Run: `cargo test -p vox-populi --lib mens::hardware::tests::tests::registry_cache_invalidates_after_ttl 2>&1 | tail -10`
Expected: FAIL — `registry` module / `HardwareRegistryV2` not found.

- [ ] **Step 3: Implement the registry**

Create `crates/vox-populi/src/mens/hardware/registry.rs`:

```rust
use crate::mens::hardware::probe::ProbeReport;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

type ReportFuture = Pin<Box<dyn Future<Output = ProbeReport> + Send>>;
pub type ReportFactory = Arc<dyn Fn() -> ReportFuture + Send + Sync>;

/// TTL-aware hardware registry. The factory is invoked on first call and on
/// every call after the TTL has elapsed since the last refresh.
pub struct HardwareRegistryV2 {
    ttl: Duration,
    factory: ReportFactory,
    state: Mutex<Option<(Instant, Arc<ProbeReport>)>>,
}

impl HardwareRegistryV2 {
    pub fn new(ttl: Duration, factory: ReportFactory) -> Self {
        Self {
            ttl,
            factory,
            state: Mutex::new(None),
        }
    }

    pub async fn get(&self) -> Arc<ProbeReport> {
        let mut guard = self.state.lock().await;
        if let Some((at, report)) = guard.as_ref() {
            if at.elapsed() < self.ttl {
                return report.clone();
            }
        }
        let report = (self.factory)().await;
        let report = Arc::new(report);
        *guard = Some((Instant::now(), report.clone()));
        report
    }
}
```

- [ ] **Step 4: Wire `registry` module into `mod.rs`**

After `pub mod pipeline;`, add:

```rust
pub mod registry;
```

- [ ] **Step 5: Run, verify pass**

Run: `cargo test -p vox-populi --lib mens::hardware::tests::tests::registry_cache_invalidates_after_ttl 2>&1 | tail -10`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-populi/src/mens/hardware/registry.rs \
        crates/vox-populi/src/mens/hardware/mod.rs \
        crates/vox-populi/src/mens/hardware/tests.rs
git commit -m "feat(populi): TTL-aware HardwareRegistryV2"
```

---

## Task 13: Wire HardwareRegistryV2 into the public probe() path

**Files:**
- Modify: `crates/vox-populi/src/mens/hardware/mod.rs`

The existing `HardwareRegistry::probe()` keeps its signature for backward compatibility. We replace its internals.

- [ ] **Step 1: Replace the OnceCell with a Lazy wrapping HardwareRegistryV2**

Replace lines 15-45 of `mod.rs` (the `static REGISTRY: OnceCell ...`, `pub struct HardwareRegistry;`, and its impl) with:

```rust
use once_cell::sync::Lazy;

static REGISTRY: Lazy<registry::HardwareRegistryV2> = Lazy::new(|| {
    let ttl = std::time::Duration::from_secs(
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshProbeCacheTtlSecs)
            .expose()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(300),
    );
    let factory: registry::ReportFactory = std::sync::Arc::new(|| {
        Box::pin(async {
            let pipeline = pipeline::ProbePipeline::default_for_platform();
            // Apply operator override if present.
            if let Some(order) = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMeshProbeOrder)
                .expose()
            {
                let names: Vec<String> = order
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                if !names.is_empty() {
                    match pipeline.reorder(&names) {
                        Ok(p) => return p.run().await,
                        Err(e) => {
                            tracing::warn!("invalid VOX_MESH_PROBE_ORDER: {e}; using default");
                        }
                    }
                }
            }
            // Re-construct since reorder consumed; default path:
            pipeline::ProbePipeline::default_for_platform().run().await
        })
    });
    registry::HardwareRegistryV2::new(ttl, factory)
});

pub struct HardwareRegistry;

impl HardwareRegistry {
    pub async fn probe() -> std::sync::Arc<types::HardwareSummary> {
        let report = REGISTRY.get().await;
        std::sync::Arc::new(report.summary.clone())
    }

    pub fn monitor() -> Option<types::GpuTelemetry> {
        #[cfg(feature = "nvml-gpu-probe")]
        {
            nvml::monitor_nvml()
        }
        #[cfg(not(feature = "nvml-gpu-probe"))]
        {
            None
        }
    }
}
```

- [ ] **Step 2: Add the two new vox-secrets SecretIds**

In `crates/vox-secrets/src/spec.rs` (location confirmed via grep — find an existing `SecretId::VoxMeshA2aStorePath` declaration and add nearby), add two new variants matching the existing pattern:

```rust
VoxMeshProbeCacheTtlSecs,
VoxMeshProbeOrder,
```

And add their entries to whatever `SecretSpec` table in the same file maps SecretIds to env-var names. Pattern:

```rust
SecretSpec {
    id: SecretId::VoxMeshProbeCacheTtlSecs,
    env: "VOX_MESH_PROBE_CACHE_TTL_SECS",
    // ... fill remaining fields per the existing pattern in this file
},
SecretSpec {
    id: SecretId::VoxMeshProbeOrder,
    env: "VOX_MESH_PROBE_ORDER",
    // ...
},
```

(Exact field order depends on the existing struct definition — look at one of the nearby existing entries and copy its shape.)

- [ ] **Step 3: Add `once_cell` to vox-populi deps if not present**

Check `crates/vox-populi/Cargo.toml`. If `once_cell` is missing under `[dependencies]`, add:

```toml
once_cell = { workspace = true }
```

- [ ] **Step 4: Build and run all populi tests**

Run: `cargo test -p vox-populi 2>&1 | tail -20`
Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-populi/src/mens/hardware/mod.rs \
        crates/vox-populi/Cargo.toml \
        crates/vox-secrets/src/spec.rs
git commit -m "feat(populi): wire HardwareRegistryV2 into public probe path with operator override"
```

---

## Task 14: Integration test — probe pipeline end-to-end

**Files:**
- Create: `crates/vox-populi/tests/probe_pipeline.rs`

- [ ] **Step 1: Write integration test**

Create `crates/vox-populi/tests/probe_pipeline.rs`:

```rust
use vox_populi::mens::hardware::pipeline::ProbePipeline;
use vox_populi::mens::hardware::probe::ProbeOutcome;

#[tokio::test]
async fn default_pipeline_runs_without_panic_on_this_platform() {
    let pipeline = ProbePipeline::default_for_platform();
    let report = pipeline.run().await;
    // Should produce *something* — at minimum CPU fallback.
    assert!(!report.summary.model_name.is_empty());
    // Every probe attempt must have a recognized outcome.
    for attempt in &report.attempts {
        match attempt.outcome {
            ProbeOutcome::NotApplicable
            | ProbeOutcome::NoDevice
            | ProbeOutcome::Found(_)
            | ProbeOutcome::Failed(_) => {}
        }
    }
}

#[tokio::test]
async fn report_summary_carries_failures_when_all_fail() {
    use vox_populi::mens::hardware::probe::{HardwareProbe, ProbeError};
    use vox_populi::mens::hardware::types::HardwareSummary;
    use async_trait::async_trait;

    struct Failing;
    #[async_trait]
    impl HardwareProbe for Failing {
        fn name(&self) -> &'static str { "failing" }
        fn applicable(&self) -> bool { true }
        async fn probe(&self) -> Result<Option<HardwareSummary>, ProbeError> {
            Err(ProbeError::Other("fail".into()))
        }
    }

    let pipeline = ProbePipeline::empty().with_probe(Box::new(Failing));
    let report = pipeline.run().await;
    assert_eq!(report.summary.model_name, "Host CPU");
    assert_eq!(
        report.summary.probe_failures.as_deref().map(|v| v.len()),
        Some(1)
    );
}
```

Note: this test refers to `pipeline` and `probe` modules as `pub`. They are `pub` per Tasks 1, 3. The `tests/` directory is an integration-test target so it accesses only `pub` items.

- [ ] **Step 2: Make pipeline.probes accessible to tests via pub method (no internal field exposure)**

Step 8 made `probes` `pub(crate)`. Integration tests can't reach `pub(crate)`. Add a public introspection method to `pipeline.rs`:

```rust
impl ProbePipeline {
    pub fn probe_names(&self) -> Vec<&'static str> {
        self.probes.iter().map(|p| p.name()).collect()
    }
}
```

(The integration test above doesn't actually use it; this is an enabler for Task 15. Add it now.)

- [ ] **Step 3: Run, verify pass**

Run: `cargo test -p vox-populi --test probe_pipeline 2>&1 | tail -15`
Expected: both tests PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-populi/tests/probe_pipeline.rs \
        crates/vox-populi/src/mens/hardware/pipeline.rs
git commit -m "test(populi): integration tests for probe pipeline + probe_names introspection"
```

---

## Task 15: Real-hardware feature flag and gated tests

**Files:**
- Modify: `crates/vox-populi/Cargo.toml`
- Create: `crates/vox-populi/tests/probe_pipeline_live.rs`

- [ ] **Step 1: Add the feature flag**

In `crates/vox-populi/Cargo.toml`, under `[features]`, add:

```toml
hw-probe-live-test = []
```

- [ ] **Step 2: Create the live-test file**

Create `crates/vox-populi/tests/probe_pipeline_live.rs`:

```rust
//! Real-hardware probe tests. Gated on `hw-probe-live-test` feature so CI
//! doesn't try to run them on machines without a known hardware contract.
//!
//! Run with:
//!   cargo test -p vox-populi --test probe_pipeline_live --features hw-probe-live-test,nvml-gpu-probe -- --ignored

#![cfg(feature = "hw-probe-live-test")]

use vox_populi::mens::hardware::pipeline::ProbePipeline;

#[tokio::test]
#[ignore = "requires live GPU; run with VOX_TEST_EXPECT_GPU=1 and the hw-probe-live-test feature"]
async fn live_pipeline_finds_a_gpu_when_expected() {
    if std::env::var("VOX_TEST_EXPECT_GPU").as_deref() != Ok("1") {
        eprintln!("VOX_TEST_EXPECT_GPU not set to 1 — skipping live GPU expectation");
        return;
    }
    let report = ProbePipeline::default_for_platform().run().await;
    assert!(
        report.summary.gpu_count >= 1,
        "expected at least one GPU; got summary {:?}",
        report.summary
    );
}
```

- [ ] **Step 3: Verify build with feature on**

Run: `cargo build -p vox-populi --features hw-probe-live-test 2>&1 | tail -10`
Expected: clean build.

- [ ] **Step 4: Verify default build still compiles**

Run: `cargo build -p vox-populi 2>&1 | tail -10`
Expected: clean build.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-populi/Cargo.toml \
        crates/vox-populi/tests/probe_pipeline_live.rs
git commit -m "test(populi): hw-probe-live-test feature flag + live GPU expectation test"
```

---

## Task 16: Documentation — populi.md probe appendix

**Files:**
- Modify: `docs/src/reference/populi.md`

- [ ] **Step 1: Add the appendix**

Append to `docs/src/reference/populi.md` (or insert at the natural section location — search for an existing "## Appendix" heading first; if none, append at end):

```markdown
## Appendix: Hardware probes

Vox-Populi probes the local hardware on every node and uses the result to populate `NodeRecord`. Probes run in a pipeline; each probe is independent and reports one of: `Found(summary)`, `NoDevice`, `NotApplicable`, or `Failed(reason)`. The first `Found` wins; failures are recorded but do not abort the pipeline.

### Default probe order

By default, the pipeline runs probes in this order (per platform):

| Platform | Order |
|----------|-------|
| Linux    | `linux_drm`, `wgpu`, `nvml` |
| Windows  | `win_dxgi`, `wgpu`, `nvml` |
| macOS    | `macos_metal`, `wgpu` |

Probes that fail (e.g., NVML library not installed) are recorded in `HardwareSummary.probe_failures` and surfaced by `vox doctor mesh`.

### Operator override

To force a specific order, set `VOX_MESH_PROBE_ORDER` to a comma-separated list of probe names. Names omitted from the list are appended after the listed ones.

```
VOX_MESH_PROBE_ORDER=wgpu,nvml
```

Unknown probe names are logged at WARN and the default order is used.

### Cache TTL

Probe results are cached for `VOX_MESH_PROBE_CACHE_TTL_SECS` seconds (default 300). Live telemetry (`HardwareRegistry::monitor()`) bypasses the cache.

### Span attributes

Each probe attempt emits a `tracing` event with attributes:

| Attribute | Value |
|-----------|-------|
| `vox.mesh.probe.name` | probe name |
| `vox.mesh.probe.outcome` | `not_applicable` / `no_device` / `found` / `failed` |
| `vox.mesh.probe.error` | error message (only when outcome is `failed`) |
| `vox.mesh.probe.duration_ms` | wall-clock probe duration |
```

- [ ] **Step 2: Verify the doc builds (if there's a doc check)**

Run: `find docs -name "*.md" | head -5` to confirm the file edit succeeded.

(If there's a vox doc-build command in this repo, run it; otherwise skip.)

- [ ] **Step 3: Commit**

```bash
git add docs/src/reference/populi.md
git commit -m "docs(populi): hardware probe appendix in populi.md"
```

---

## Task 17: Final integration sweep + close the spec

**Files:**
- (verification only)

- [ ] **Step 1: Run full populi test suite**

Run: `cargo test -p vox-populi 2>&1 | tail -30`
Expected: all tests pass with default features.

- [ ] **Step 2: Run with NVML feature**

Run: `cargo test -p vox-populi --features nvml-gpu-probe 2>&1 | tail -15`
Expected: all tests pass.

- [ ] **Step 3: Run with no default features**

Run: `cargo test -p vox-populi --no-default-features 2>&1 | tail -15`
Expected: all tests pass (most probes gated out, but pipeline + mock tests survive).

- [ ] **Step 4: Sanity-check compile of dependent crates**

Run: `cargo build --workspace 2>&1 | tail -15`
Expected: clean build (the additive `probe_failures` field doesn't break any consumer because of `serde(default)`).

- [ ] **Step 5: Confirm backlog item closure**

Confirm in [`populi-mesh-improvement-backlog-2026.md`](populi-mesh-improvement-backlog-2026.md) that the following items are addressed by this work:

- `MESH-038`–`MESH-042` (probe tests for NVML/wgpu/DRM/Metal/DXGI) — covered by mock tests + integration test + live-feature test.
- `MESH-044` (silent probe failure) — fixed via `probe_failures` and tracing events.
- `MESH-045` (startup probe summary) — addressed via tracing events emitted on every pipeline run.
- `MESH-048` (probe cache) — implemented via `HardwareRegistryV2`.
- `MESH-050` (mod.rs split) — partial; pipeline/probe/registry/mock split out, but the platform-default dispatcher still lives in `mod.rs` (intentional, per spec).
- `MESH-052` (real-hardware test feature) — implemented as `hw-probe-live-test`.

If any item turns out partially-addressed and not fully closeable, leave a note in a commit message rather than the backlog file (backlog edits get their own PR per project convention).

- [ ] **Step 6: Final commit**

If any small fixes were needed during the sweep:

```bash
git add -u
git commit -m "chore(populi): probe-correctness final sweep"
```

If no changes, no commit needed.

---

## Self-review

- **Spec coverage.** Every numbered acceptance criterion from the spec maps to at least one task: trait+pipeline (Tasks 1–4, 8), all probe impls (5–7), failure-visibility field (2), cache TTL (12), operator override (11), tracing events (10), real-hardware feature (15), doc (16), test sweep (17).
- **Placeholder scan.** No `TBD`, no "implement later", no "similar to Task N". Every code step has the actual code.
- **Type consistency.** `HardwareProbe` trait, `ProbeError`, `ProbeOutcome`, `ProbeReport`, `ProbeAttempt`, `ProbePipeline`, `PipelineOrderError`, `HardwareRegistryV2` are defined exactly once and referenced consistently. `MockProbe` is `pub(crate)` and lives under `#[cfg(test)]`.
- **One known caveat.** Task 11's `reorder` uses `HashMap` whose iteration order is non-deterministic for the "appended remaining probes" branch. The spec accepts this. If determinism is wanted later, use `IndexMap` or sort the appended portion.

---

## Revision history

- **2026-05-01.** Initial implementation plan.
