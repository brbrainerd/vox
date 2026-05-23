---
title: "Populi Mesh — Probe Correctness Spec (S1, 2026-05-01)"
description: "Slice S1 child spec for workstream W2 (GPU truth, partial). Refactors hardware probes behind a trait, introduces a mock harness, and establishes correctness criteria for NVML / wgpu / DRM / Metal / DXGI probes. No admission-control integration (deferred to S2)."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Defines the probe trait shape and test pattern that subsequent hardware-related work in vox-populi follows."
---

# Populi Mesh — Probe Correctness (S1 child spec)

**Parent.** [`populi-mesh-north-star-2026.md`](populi-mesh-north-star-2026.md), Slice S1, Workstream W2 partial.

**Goal.** Make `vox-populi`'s hardware-probe output **trustworthy enough** that S2's admission-control work can route on it. That means: probes have tests, probe failures are observable, and probe contracts are explicit.

**Non-goals.**
- Admission-control integration (S2's `populi-mesh-admission-spec`).
- Layer C operator-label demotion (S2).
- Real-time telemetry refactor — `HardwareRegistry::monitor()` stays as-is.
- Multi-GPU device enumeration changes (backlog `MESH-051`).

---

## Part 1 — Current state

`vox-populi/src/mens/hardware/` has six modules ([mod.rs](../../../crates/vox-populi/src/mens/hardware/mod.rs:47), `nvml.rs`, `wgpu_probe.rs`, `linux_drm.rs`, `macos_metal.rs`, `win_dxgi.rs`) and a shared `types.rs`. Dispatch is imperative inside `probe_internal()` with platform `cfg` gates. Result is cached in a `tokio::sync::OnceCell` for the process lifetime. Output type is `HardwareSummary` (model_name, vram_mb, gpu_count, vendor, backend, driver_version, pci_bus_id).

**What's wrong.**
1. Zero inline tests in any probe module ([nvml.rs](../../../crates/vox-populi/src/mens/hardware/mod.rs), wgpu_probe.rs, linux_drm.rs, macos_metal.rs, win_dxgi.rs).
2. Probe failures degrade silently to "Host CPU" with no event emitted — operators cannot tell whether a node *has* no GPU vs. its NVML library is missing.
3. No way to inject a probe for testing — the cache makes test ordering matter, and concrete imperative dispatch defies mocking.
4. Probe order is hard-coded; an operator on a Linux box with both an NVML-visible GPU and DRM cannot influence which one wins (DRM does, today).
5. `pci_bus_id` is plumbed but never populated by any probe.
6. `driver_version` only set by DXGI/DRM paths; NVML and wgpu leave it `None`.

---

## Part 2 — Design

### 2.1 The `HardwareProbe` trait

```rust
// vox:skip
pub trait HardwareProbe: Send + Sync {
    /// Stable identifier for logging and operator override.
    fn name(&self) -> &'static str;

    /// Whether this probe applies to the running platform.
    /// Compile-time gates remain on the *types*, not on this method.
    fn applicable(&self) -> bool;

    /// Run the probe. Returns `Ok(None)` for "applicable but no device found",
    /// `Err` for "applicable and broken" (NVML lib missing, DRM permission denied, etc.).
    async fn probe(&self) -> Result<Option<HardwareSummary>, ProbeError>;
}

#[derive(Debug, thiserror::Error)]
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
```

Each existing probe module exposes a `pub struct NvmlProbe;`, `pub struct WgpuProbe;`, etc. that implements the trait. Existing free functions (`probe_dxgi`, `probe_drm`, `probe_metal`, `probe_wgpu`, `monitor_nvml`) stay public so external callers don't break; the trait impl wraps them.

### 2.2 The probe pipeline

```rust
// vox:skip
pub struct ProbePipeline {
    probes: Vec<Box<dyn HardwareProbe>>,
}

impl ProbePipeline {
    pub fn default_for_platform() -> Self { /* ... */ }

    pub fn with_probe(mut self, probe: Box<dyn HardwareProbe>) -> Self { /* ... */ }

    pub async fn run(&self) -> ProbeReport { /* ... */ }
}

pub struct ProbeReport {
    pub summary: HardwareSummary,        // best-of-applicable, or CPU fallback
    pub attempts: Vec<ProbeAttempt>,     // every probe's name + outcome
}

pub struct ProbeAttempt {
    pub probe_name: &'static str,
    pub outcome: ProbeOutcome,
}

pub enum ProbeOutcome {
    NotApplicable,
    NoDevice,
    Found(HardwareSummary),
    Failed(String),                      // mirror of ProbeError, flattened
}
```

`ProbeReport` is the new public output shape. Existing `probe()` / `HardwareRegistry::probe()` keep returning `Arc<HardwareSummary>` and become thin wrappers that select `report.summary`. New code that wants the full attempt log uses `probe_with_report()`.

### 2.3 Probe ordering and operator override

Default order (unchanged): vox-secrets-override → DXGI → DRM → Metal → wgpu → NVML → CPU fallback.

**New:** an operator can set `[mesh.probe.order]` in `Vox.toml` (validated against known probe names). Useful for forcing wgpu when an NVML probe is misreporting. Validation rejects unknown names with an actionable error.

### 2.4 The mock probe (test-only)

`vox-populi/src/mens/hardware/mock.rs` (gated `#[cfg(test)]` and behind `pub(crate)`):

```rust
// vox:skip
pub(crate) struct MockProbe {
    pub name: &'static str,
    pub applicable: bool,
    pub result: Result<Option<HardwareSummary>, ProbeError>,
}

impl HardwareProbe for MockProbe { /* trivial */ }
```

Tests construct `ProbePipeline::empty().with_probe(...)` and assert on the resulting `ProbeReport`.

### 2.5 Observability

Every `ProbePipeline::run()` invocation emits one structured event per attempt:

- attribute `vox.mesh.probe.name`
- attribute `vox.mesh.probe.outcome` (`not_applicable | no_device | found | failed`)
- attribute `vox.mesh.probe.error` (only on `failed`)
- attribute `vox.mesh.probe.duration_ms`

Plus one summary event per pipeline run with the chosen `vendor`, `backend`, `vram_mb`. No telemetry for `monitor()` (live path) — that's hot.

### 2.6 Fixing the silent-degradation problem

When `probe_internal` falls all the way through to "Host CPU" but at least one probe `Failed`, the fallback `HardwareSummary` carries that information through a new optional field:

```rust
// vox:skip
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareSummary {
    // existing fields...

    /// Names of probes that failed during this run, if any. Empty/None means
    /// "all probes either succeeded or were not applicable".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub probe_failures: Option<Vec<String>>,
}
```

This is additive (default-skipping) so it doesn't break existing serialized payloads. `vox doctor` reads this field and surfaces a remediation hint.

### 2.7 Caching

Replace the global `OnceCell` with an `Arc<RwLock<Option<ProbeReport>>>` on a `HardwareRegistry` instance, plus a TTL configurable via `[mesh.probe.cache_ttl_secs]` (default 300). Tests construct fresh registries; production code keeps a process-global registry via `once_cell::Lazy`. This unblocks `MESH-048` (probe cache) and `MESH-052` (real-hardware feature-flagged tests) without requiring them in this slice.

---

## Part 3 — Test plan

Tests live in `crates/vox-populi/src/mens/hardware/tests.rs` (inline `#[cfg(test)]`) and `crates/vox-populi/tests/probe_pipeline.rs` (integration).

### 3.1 Unit tests (inline)

- `mock_probe_returns_no_device_skips_to_next` — pipeline of [Mock(NoDevice), Mock(Found)] returns the Found summary.
- `mock_probe_failure_does_not_abort_pipeline` — pipeline of [Mock(Failed), Mock(Found)] returns the Found summary, attempts log records the failure.
- `all_probes_failed_returns_cpu_fallback_with_failures_field` — pipeline of [Mock(Failed), Mock(Failed)] returns the CPU summary with `probe_failures = Some(["a", "b"])`.
- `not_applicable_probe_emits_no_event` — pipeline of [Mock(NotApplicable), Mock(Found)] only emits one attempt event.
- `vendor_from_model_table` — table-driven test over the existing `vendor_from_model` heuristics with at least 20 inputs (confirms current behavior; no change to function).
- `vendor_from_id_table` — same for `vendor_from_id`.
- `compute_backend_as_cli_flag` — round-trip every variant.
- `cache_ttl_invalidates` — `tokio::time::pause()` + advance past TTL → cache miss.

### 3.2 Integration tests (`tests/probe_pipeline.rs`)

- `default_pipeline_includes_expected_probes_per_platform` — runs `ProbePipeline::default_for_platform()` and asserts the `probes.iter().map(|p| p.name())` set matches the platform's expected set.
- `operator_override_respected` — supply `[mesh.probe.order] = ["wgpu", "nvml"]` and confirm pipeline applies in that order.
- `operator_override_rejects_unknown` — `[mesh.probe.order] = ["nope"]` returns a config error with actionable message.
- `report_round_trips_serde` — every variant of `ProbeOutcome` survives JSON round-trip.
- `node_record_includes_probe_failures` — call `node_record_for_current_process()` with a test pipeline that has one failing probe; assert the resulting NodeRecord exposes the failure.

### 3.3 Real-hardware tests (feature-gated)

`crates/vox-populi/tests/probe_pipeline_live.rs`, gated `#[cfg(feature = "hw-probe-live-test")]`:

- `nvml_probe_finds_a_gpu` — only runs if `nvml-gpu-probe` feature is on AND env `VOX_TEST_EXPECT_NVIDIA_GPU=1`.
- `wgpu_probe_finds_an_adapter` — only runs if `mens-gpu` is on AND `VOX_TEST_EXPECT_GPU=1`.

These tests are not run in CI by default; they exist for contributors to validate on real hardware.

---

## Part 4 — Acceptance criteria

The spec is "done" when:

1. `cargo test -p vox-populi` exercises every probe code path via mocks and passes with all features off, with `mens-gpu` only, with `nvml-gpu-probe` only, and with both.
2. `vox doctor mesh` (or equivalent — naming TBD by the config-baseline spec) reports the chosen probe summary AND any probe failures.
3. `node_record_for_current_process()` includes `probe_failures` when any probe failed.
4. `vox.mesh.probe.*` span events appear in telemetry on every pipeline run.
5. Operator override `[mesh.probe.order]` is documented in `populi.md` and validated in `vox config check`.
6. Backlog items closed: `MESH-038`–`MESH-046`, `MESH-048`, `MESH-050`, `MESH-052`, `MESH-053`.

---

## Part 5 — Out-of-scope items punted to follow-on specs or backlog

- **Admission control consuming probe output** — S2 spec.
- **Layer A/B/C demotion of operator labels** — S2 spec (admission).
- **Multi-GPU enumeration** — backlog `MESH-051`.
- **NVML library handle lifecycle** — backlog `MESH-047` (related but separable).
- **Replacement of `vendor_from_model` heuristics with PCI ID lookup table** — backlog (new item to add: `MESH-211 [health] Replace vendor_from_model substring heuristics with vendor_id-first lookup; fall back to substring`).

---

## Part 6 — Rough cost

- New trait + pipeline structure: ~200 LOC + ~100 LOC re-wiring existing probes.
- Mock module: ~80 LOC.
- Inline tests: ~300 LOC.
- Integration tests: ~200 LOC.
- Real-hardware tests (feature-gated): ~80 LOC.
- Doc updates: `populi.md` (1 section), `how-to/populi-quickstart.md` reference.

Total: ~1000 LOC, dominated by tests, no new external dependencies.

---

## Revision history

- **2026-05-01.** Initial S1 child spec.
