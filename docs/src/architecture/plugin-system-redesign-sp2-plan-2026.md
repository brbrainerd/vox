---
title: "Plugin System Redesign — SP2 Implementation Plan (2026)"
description: "Step-by-step implementation plan for Sub-Project 2: vox-plugin-api shared types, vox-plugin-host loader for both code and skill payloads, dual-payload registry, two noop test plugins, and ABI-mismatch CI guards."
category: "Architecture SSOTs"
status: "research"
training_eligible: true
training_rationale: "Concrete TDD task plan for SP2; companion to the parent design spec."
---

# Plugin System Redesign — SP2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Parent spec:** [`plugin-system-redesign-2026.md`](plugin-system-redesign-2026.md)
**Predecessor plan:** [`plugin-system-redesign-sp1-plan-2026.md`](plugin-system-redesign-sp1-plan-2026.md) (must be merged before SP2 starts).

**Goal:** Land the two host-side runtime crates — `vox-plugin-api` (shared trait/type surface) and `vox-plugin-host` (loader + registry) — with proven end-to-end loading of both a noop code plugin (`cdylib` via `libloading` + `abi_stable`) and a noop skill plugin (directory with `Plugin.toml` + `SKILL.md`). No real extension points yet (those land in SP3 onward); no CLI plugin commands (those land in SP5). This unblocks SP3 (MlBackend) and SP4 (skill-compiler migration).

**Architecture:** `vox-plugin-api` exposes `VOX_PLUGIN_ABI_VERSION: u32`, `VoxHost` and `VoxPlugin` `#[sabi_trait]` traits, the `VoxPluginRoot` `abi_stable` prefix struct, plain-Rust `SkillManifest` / `LoadedSkill` types, and the discriminated-union `PluginManifest` matching the `Plugin.toml` schema documented in SP1. `vox-plugin-host` provides `Registry` (in-memory, keyed by plugin id, holds both loaded code plugins and parsed skills), `discover()` (scans an install root for `Plugin.toml` files), `Loader` (calls `libloading::Library::new`, asserts ABI match, invokes the plugin's `init` to obtain a trait object), `SkillRegistry` (parses + stores skill payloads), `PluginMissingError` / `SkillNotInstalledError`, and lifecycle telemetry.

**Tech Stack:**
- `abi_stable` (new workspace dep) for the typed code-plugin boundary
- `libloading` 0.8+ (new workspace dep) for `dlopen`
- `serde` + `toml` (existing workspace deps) for manifest parsing
- `thiserror` (existing) for errors
- `tracing` (existing) for telemetry-event emission
- Project's existing telemetry pattern per [`telemetry-trust-ssot.md`](telemetry-trust-ssot.md)

The CUDA cdylib spike at [`crates/vox-plugin-cuda-spike/`](plugin-system-redesign-2026.md#sub-project-3-first-code-extension-point-mlbackend) already proved `libloading` works on Windows MSVC + CUDA 13.1. SP2 generalizes that pattern through `abi_stable`'s typed boundary.

---

## File Structure

### New crates and files

| Path                                                                | Responsibility                                                          |
| ------------------------------------------------------------------- | ----------------------------------------------------------------------- |
| `crates/vox-plugin-api/Cargo.toml`                                  | Crate manifest (depends on abi_stable, serde, thiserror).               |
| `crates/vox-plugin-api/src/lib.rs`                                  | Re-exports + ABI version constant + module wiring.                      |
| `crates/vox-plugin-api/src/manifest.rs`                             | `PluginManifest` discriminated union and TOML deserialization.          |
| `crates/vox-plugin-api/src/errors.rs`                               | `LogLevel` enum + plain error types.                                    |
| `crates/vox-plugin-api/src/abi.rs`                                  | `VoxPluginRoot` prefix struct + `VoxPlugin` `#[sabi_trait]`.            |
| `crates/vox-plugin-api/src/host.rs`                                 | `VoxHost` `#[sabi_trait]` for capability injection.                     |
| `crates/vox-plugin-api/src/skill.rs`                                | `SkillManifest`, `LoadedSkill`, `SkillPayloadConfig` plain-Rust types.  |
| `crates/vox-plugin-api/src/extensions/mod.rs`                       | Module declarations for the seven placeholder extension-point traits.   |
| `crates/vox-plugin-api/src/extensions/{ml_backend,tensor_backend,audio_capture,hardware_probe,cloud_sync,script_executor,mesh_driver}.rs` | One file per extension point. SP2 ships placeholders only — the actual trait methods land in SP3+. |
| `crates/vox-plugin-host/Cargo.toml`                                 | Crate manifest (depends on vox-plugin-api, libloading, walkdir).        |
| `crates/vox-plugin-host/src/lib.rs`                                 | Public API re-exports.                                                  |
| `crates/vox-plugin-host/src/registry.rs`                            | `Registry` (dual-kind: code + skill).                                   |
| `crates/vox-plugin-host/src/discover.rs`                            | `discover(root: &Path) -> Result<Registry>`.                            |
| `crates/vox-plugin-host/src/loader.rs`                              | `Loader` for code plugins (libloading + ABI check + init).              |
| `crates/vox-plugin-host/src/skill_registry.rs`                      | Skill loader and in-memory registry view.                               |
| `crates/vox-plugin-host/src/errors.rs`                              | `PluginMissingError`, `SkillNotInstalledError`, `LoadError`, `AbiMismatchError`. |
| `crates/vox-plugin-host/src/telemetry.rs`                           | Lifecycle event emission (tracing-based).                               |
| `crates/vox-plugin-host/src/host_impl.rs`                           | Default `VoxHost` impl wrapping `vox_data_dir`, log forwarder, telemetry sink. |
| `crates/vox-plugin-host/tests/load_noop_code.rs`                    | End-to-end: build noop-code, copy to tempdir, discover + load + invoke. |
| `crates/vox-plugin-host/tests/load_noop_skill.rs`                   | End-to-end: copy noop-skill dir to tempdir, discover + parse.           |
| `crates/vox-plugin-host/tests/abi_mismatch.rs`                      | Force a mismatch and assert clear error + telemetry event.              |
| `crates/vox-plugin-noop-code/Cargo.toml`                            | `[lib] crate-type = ["cdylib", "rlib"]`. Depends on vox-plugin-api.     |
| `crates/vox-plugin-noop-code/src/lib.rs`                            | Implements `VoxPlugin` with no extension points.                        |
| `crates/vox-plugin-noop-code/Plugin.toml`                           | Manifest with `payload-kind = "code"`, no extension points.             |
| `crates/vox-plugin-noop-skill/Plugin.toml`                          | Manifest with `payload-kind = "skill"`, references `noop.skill.md`.     |
| `crates/vox-plugin-noop-skill/noop.skill.md`                        | Tiny SKILL.md with one fake tool.                                       |
| `crates/vox-cli/src/commands/ci/plugin_abi_parity.rs`               | CI guard: rebuild every in-tree code plugin, assert ABI matches host.   |
| `crates/vox-cli/src/commands/ci/plugin_skill_parity.rs`             | CI guard: validate every in-tree skill `Plugin.toml` against schema.    |

### Modified files

| Path                                                       | Change                                                                  |
| ---------------------------------------------------------- | ----------------------------------------------------------------------- |
| `Cargo.toml` (workspace)                                   | Add `abi_stable`, `libloading`, `vox-plugin-api`, `vox-plugin-host` to `[workspace.dependencies]`. |
| `crates/vox-cli/Cargo.toml`                                | Add `vox-plugin-host` workspace dep + `walkdir` (if not already).       |
| `crates/vox-cli/src/commands/ci/{mod,cmd_enums,run_body}.rs` | Wire the two new CI subcommands following the SP1 Task 11 pattern.    |
| `crates/vox-plugin-catalog/catalog.toml`                   | Add entries for `noop-code` and `noop-skill` (test fixtures, marked with `bundled-in = []`). |

### Workspace member auto-inclusion

All four new crates (`vox-plugin-api`, `vox-plugin-host`, `vox-plugin-noop-code`, `vox-plugin-noop-skill` — note the last is a directory only, NOT a Rust crate) drop into `crates/`. The Rust crates are auto-included via `members = ["crates/*"]`. **Important:** `vox-plugin-noop-skill` has NO `Cargo.toml` because it has no Rust code — it's just `Plugin.toml` + `noop.skill.md`. To prevent cargo from trying to treat it as a member, either (a) name the dir without a `Cargo.toml` so cargo skips it, or (b) explicitly exclude it in the workspace `Cargo.toml`'s `[workspace] exclude = [...]` list. **Verify cargo's behavior in Task 14 below.**

---

## Tasks

### Task 1: Scaffold `vox-plugin-api` crate

**Files:** Create `crates/vox-plugin-api/{Cargo.toml,src/lib.rs}`, plus `tests/smoke.rs`.

Follow the same pattern as SP1 Task 1 ([`plugin-system-redesign-sp1-plan-2026.md`](plugin-system-redesign-sp1-plan-2026.md), Task 1).

- [ ] **Step 1:** Write `tests/smoke.rs` with a single `crate_compiles` test (per SP1 Task 1).
- [ ] **Step 2:** Verify it fails (crate doesn't exist).
- [ ] **Step 3:** Create `Cargo.toml`:

```toml
[package]
name = "vox-plugin-api"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
description = "Shared API surface for Vox plugins: ABI version, traits, manifest types, error types."

[dependencies]
abi_stable = { workspace = true }
serde = { workspace = true, features = ["derive"] }
thiserror = { workspace = true }
toml = { workspace = true }
workspace-hack = { workspace = true }

[lints]
workspace = true
```

(`abi_stable` is added to the workspace deps in Task 13 below — order matters; this Cargo.toml will not compile until Task 13 lands. Either land Task 13 first or stub `abi_stable` out of this Cargo.toml until Task 13. The cleanest order is Task 13 → Task 1, but TDD tradition writes the test first. Pragmatic: do Task 13 first.)

- [ ] **Step 4:** Create `src/lib.rs` with module declarations + ABI version:

```rust
//! Shared API surface for Vox plugins. Both host and code-payload plugin
//! crates depend on this crate.
//!
//! See: docs/src/architecture/plugin-system-redesign-2026.md

pub const VOX_PLUGIN_ABI_VERSION: u32 = 1;

pub mod abi;
pub mod errors;
pub mod extensions;
pub mod host;
pub mod manifest;
pub mod skill;
```

- [ ] **Step 5:** Run smoke test; expect PASS once Task 13 has landed.
- [ ] **Step 6:** Commit: `feat(plugin-api): scaffold vox-plugin-api crate with ABI version constant`.

### Task 2: `errors.rs` — `LogLevel` enum and basic error types

**Files:** Create `crates/vox-plugin-api/src/errors.rs`. Test in `tests/errors_basic.rs`.

- [ ] **Step 1:** Write `tests/errors_basic.rs`:

```rust
use vox_plugin_api::errors::LogLevel;

#[test]
fn log_levels_round_trip_through_serde() {
    let levels = [LogLevel::Trace, LogLevel::Debug, LogLevel::Info, LogLevel::Warn, LogLevel::Error];
    for l in levels {
        let json = serde_json::to_string(&l).unwrap();
        let back: LogLevel = serde_json::from_str(&json).unwrap();
        assert_eq!(l, back);
    }
}
```

- [ ] **Step 2:** Verify FAIL.
- [ ] **Step 3:** Implement `errors.rs`:

```rust
//! Plain-Rust error and log types shared across host and plugins.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}
```

Add `serde_json = { workspace = true }` to `[dev-dependencies]` for the test.

- [ ] **Step 4:** Verify PASS.
- [ ] **Step 5:** Commit: `feat(plugin-api): add LogLevel enum`.

### Task 3: `manifest.rs` — `PluginManifest` discriminated union

**Files:** Create `crates/vox-plugin-api/src/manifest.rs`. Test in `tests/manifest_parsing.rs`.

The manifest matches the schema documented in `docs/src/reference/plugin-manifest.md` (committed in SP1 Task 12). Three payload variants: code, skill, composite.

- [ ] **Step 1:** Write three roundtrip tests — one per payload kind — that parse a TOML literal and assert key fields. Pattern: SP1 Task 2's `schema_roundtrip.rs`.
- [ ] **Step 2:** Verify FAIL.
- [ ] **Step 3:** Implement `manifest.rs` with the types:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub plugin: PluginHeader,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PluginHeader {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    pub host: HostRequirement,
    pub payload: PluginPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct HostRequirement {
    pub min_vox_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum PluginPayload {
    Code(CodePayload),
    Skill(SkillPayload),
    Composite(CompositePayload),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CodePayload {
    pub abi_version: u32,
    #[serde(default)]
    pub provides: PayloadProvides,
    #[serde(default)]
    pub requires: PayloadRequires,
    pub artifacts: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PayloadProvides {
    #[serde(default)]
    pub extension_points: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PayloadRequires {
    #[serde(default)]
    pub os: Vec<String>,
    #[serde(default)]
    pub arch: Vec<String>,
    #[serde(default)]
    pub native_libs: Vec<NativeLib>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct NativeLib {
    pub name: String,
    #[serde(default)]
    pub min_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SkillPayload {
    pub format_version: u32,
    pub skill_md: String,
    #[serde(default)]
    pub tools: SkillTools,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SkillTools {
    #[serde(default)]
    pub exposes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CompositePayload {
    pub code: CodePayload,
    pub skill: SkillPayload,
}
```

- [ ] **Step 4:** Verify all three roundtrip tests PASS.
- [ ] **Step 5:** Commit: `feat(plugin-api): add PluginManifest discriminated union with code/skill/composite variants`.

### Task 4: `skill.rs` — plain-Rust skill registry types

**Files:** Create `crates/vox-plugin-api/src/skill.rs`. Test in `tests/skill_types.rs`.

These mirror what the previous `vox-skills::SkillManifest` looked like, but live in `vox-plugin-api` with the new schema.

- [ ] **Step 1:** Write a small test that constructs a `LoadedSkill` and reads back its fields.
- [ ] **Step 2:** Verify FAIL.
- [ ] **Step 3:** Implement `skill.rs`:

```rust
//! Plain-Rust types for the skill side of plugin loading.
//! Skill payloads do not cross a dylib boundary, so no abi_stable here.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SkillManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(default)]
    pub tools: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct LoadedSkill {
    pub plugin_id: String,
    pub format_version: u32,
    pub manifest: SkillManifest,
    pub body: String,
    pub exposed_tools: Vec<String>,
}
```

- [ ] **Step 4:** Verify PASS.
- [ ] **Step 5:** Commit: `feat(plugin-api): add LoadedSkill and SkillManifest plain-Rust types`.

### Task 5: `host.rs` — `VoxHost` `#[sabi_trait]`

**Files:** Create `crates/vox-plugin-api/src/host.rs`. No test in this task (test comes via the noop plugin in Task 16).

The `VoxHost` trait is the capability surface the host injects into every code plugin at `init()` time. Methods are stable-ABI-friendly: only `RStr<'_>`, `RString`, primitive types.

- [ ] **Step 1:** Implement `host.rs`:

```rust
//! VoxHost trait — the capability surface a code plugin receives at init.
//! Stable-ABI for the dylib boundary via abi_stable.

use abi_stable::{sabi_trait, std_types::*, StableAbi};
use crate::errors::LogLevel;

#[derive(Debug, Clone, Copy, StableAbi)]
#[repr(u8)]
pub enum SabiLogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl From<LogLevel> for SabiLogLevel {
    fn from(l: LogLevel) -> Self {
        match l {
            LogLevel::Trace => Self::Trace,
            LogLevel::Debug => Self::Debug,
            LogLevel::Info => Self::Info,
            LogLevel::Warn => Self::Warn,
            LogLevel::Error => Self::Error,
        }
    }
}

#[sabi_trait]
pub trait VoxHost: Send + Sync {
    fn data_dir(&self) -> RString;
    fn log(&self, level: SabiLogLevel, msg: RStr<'_>);
    fn telemetry_event(&self, kind: RStr<'_>, payload: RStr<'_>);
}
```

- [ ] **Step 2:** Verify it compiles.
- [ ] **Step 3:** Commit: `feat(plugin-api): add VoxHost #[sabi_trait] for plugin->host capability injection`.

### Task 6: `extensions/` — placeholder modules for each extension point

**Files:** `crates/vox-plugin-api/src/extensions/mod.rs` plus seven sibling files (`ml_backend.rs`, `tensor_backend.rs`, `audio_capture.rs`, `hardware_probe.rs`, `cloud_sync.rs`, `script_executor.rs`, `mesh_driver.rs`).

Each file declares an empty module with a placeholder type. Real trait methods land in SP3 (MlBackend) and SP7 (the others).

- [ ] **Step 1:** Write `extensions/mod.rs`:

```rust
//! Extension-point trait modules. SP2 ships placeholders only.
//! SP3 fills in MlBackend; SP7 fills in the rest.

pub mod audio_capture;
pub mod cloud_sync;
pub mod hardware_probe;
pub mod mesh_driver;
pub mod ml_backend;
pub mod script_executor;
pub mod tensor_backend;
```

- [ ] **Step 2:** Each sibling file gets one placeholder line:

```rust
//! Placeholder. Real trait lands in a later sub-project.
```

- [ ] **Step 3:** Verify the crate still compiles.
- [ ] **Step 4:** Commit: `feat(plugin-api): scaffold seven extension-point placeholder modules`.

### Task 7: `abi.rs` — `VoxPlugin` `#[sabi_trait]` and `VoxPluginRoot` prefix struct

**Files:** Create `crates/vox-plugin-api/src/abi.rs`.

This is the most architecturally important file. The `VoxPluginRoot` is the C-ABI struct each plugin dylib exports under symbol `_vox_plugin_root`; the `VoxPlugin` trait is the typed surface obtained via `init()`.

- [ ] **Step 1:** Implement `abi.rs`:

```rust
//! ABI surface for Vox code plugins. Each plugin dylib exports a single
//! root symbol (`_vox_plugin_root`) of type `VoxPluginRootRef`. The host
//! reads `abi_version`, calls `init` to obtain a `VoxPluginRef`, and
//! interacts with the trait object thereafter.
//!
//! Per spec: SP2 ships only the `VoxPlugin` root; per-extension-point
//! `as_*` accessors return RNone in this version. SP3 wires `as_ml_backend`.

use abi_stable::{
    library::RootModule,
    package_version_strings,
    sabi_trait,
    std_types::*,
    StableAbi,
};

use crate::host::VoxHost_TO;

#[repr(C)]
#[derive(StableAbi)]
#[sabi(kind(Prefix(prefix_ref = VoxPluginRootRef)))]
#[sabi(missing_field(panic))]
pub struct VoxPluginRoot {
    pub abi_version: u32,
    pub manifest_json: extern "C" fn() -> RString,
    pub init: extern "C" fn(host: VoxHost_TO<'static, RBox<()>>) -> RResult<VoxPluginRef, RBoxError>,
}

impl RootModule for VoxPluginRootRef {
    abi_stable::declare_root_module_statics! {VoxPluginRootRef}
    const BASE_NAME: &'static str = "vox_plugin";
    const NAME: &'static str = "vox_plugin";
    const VERSION_STRINGS: abi_stable::sabi_types::VersionStrings = package_version_strings!();
}

#[sabi_trait]
pub trait VoxPlugin: Send + Sync {
    fn id(&self) -> RString;
    fn shutdown(&self) -> RResult<(), RBoxError>;
    // SP3+ adds typed-extension accessors here. For SP2 the trait has no
    // extension methods; load tests just verify id + shutdown.
}

pub type VoxPluginRef = VoxPlugin_TO<'static, RBox<()>>;
```

- [ ] **Step 2:** Verify it compiles. (No runtime test yet — comes in Task 16 via the noop plugin.)
- [ ] **Step 3:** Commit: `feat(plugin-api): add VoxPluginRoot prefix struct and VoxPlugin sabi_trait`.

### Task 8: Scaffold `vox-plugin-host` crate

**Files:** `crates/vox-plugin-host/Cargo.toml`, `src/lib.rs`, `tests/smoke.rs`.

Same pattern as SP1 Task 1.

- [ ] **Step 1:** smoke test.
- [ ] **Step 2:** verify FAIL.
- [ ] **Step 3:** Cargo.toml:

```toml
[package]
name = "vox-plugin-host"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
description = "Vox host-side plugin discovery, loading, and registry."

[dependencies]
vox-plugin-api = { workspace = true }
abi_stable = { workspace = true }
libloading = { workspace = true }
serde = { workspace = true, features = ["derive"] }
toml = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
walkdir = { workspace = true }
dirs = { workspace = true }
workspace-hack = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }

[lints]
workspace = true
```

- [ ] **Step 4:** lib.rs with module declarations:

```rust
//! Vox plugin host: discovery, loading, registry.
//!
//! See: docs/src/architecture/plugin-system-redesign-2026.md

pub mod discover;
pub mod errors;
pub mod host_impl;
pub mod loader;
pub mod registry;
pub mod skill_registry;
pub mod telemetry;

pub use discover::discover;
pub use errors::{AbiMismatchError, LoadError, PluginMissingError, SkillNotInstalledError};
pub use host_impl::DefaultVoxHost;
pub use loader::Loader;
pub use registry::{PluginEntry, Registry};
pub use skill_registry::SkillRegistry;
```

- [ ] **Step 5:** Test PASS.
- [ ] **Step 6:** Commit.

### Task 9: `errors.rs` — host-side error types

**Files:** Create `crates/vox-plugin-host/src/errors.rs`.

- [ ] **Step 1:** Implement (paste this; no failing test needed since these are pure data types covered indirectly by Tasks 16–18):

```rust
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
#[error(
    "This Vox feature requires the '{plugin_id}' plugin (extension point '{extension_point}'), which is not installed.\n\nTo install it, run:\n\n  vox plugin install {plugin_id}\n\nSee: docs/src/reference/plugins.md"
)]
pub struct PluginMissingError {
    pub plugin_id: &'static str,
    pub extension_point: &'static str,
}

#[derive(Debug, Error)]
#[error(
    "Skill '{skill_id}' is not installed.\n\nTo install it, run:\n\n  vox plugin install {skill_id}"
)]
pub struct SkillNotInstalledError {
    pub skill_id: String,
}

#[derive(Debug, Error)]
pub enum LoadError {
    #[error("plugin manifest at {path:?} failed to parse: {source}")]
    ManifestParse { path: PathBuf, #[source] source: toml::de::Error },
    #[error("plugin dylib at {path:?} failed to dlopen: {source}")]
    DlopenFailed { path: PathBuf, #[source] source: libloading::Error },
    #[error("plugin '{id}' has ABI version {plugin_abi}, host expects {host_abi}")]
    AbiMismatch(AbiMismatchError),
    #[error("plugin init returned an error: {0}")]
    InitFailed(String),
}

#[derive(Debug, Error)]
#[error("plugin '{id}' has ABI version {plugin_abi}, host expects {host_abi}")]
pub struct AbiMismatchError {
    pub id: String,
    pub plugin_abi: u32,
    pub host_abi: u32,
}
```

- [ ] **Step 2:** Verify it compiles.
- [ ] **Step 3:** Commit: `feat(plugin-host): add host-side error types`.

### Task 10: `telemetry.rs` — lifecycle event emission

**Files:** Create `crates/vox-plugin-host/src/telemetry.rs`. Test in `tests/telemetry.rs`.

Per the parent spec's Cross-Cutting > Telemetry section, emit these events through `tracing::info!` with structured fields.

- [ ] **Step 1:** Write a test using `tracing-test` (already a workspace dep, see vox-populi):

```rust
use tracing_test::traced_test;
use vox_plugin_host::telemetry;

#[traced_test]
#[test]
fn discovered_event_includes_id_and_version() {
    telemetry::discovered("test-id", "1.2.3", "code", 1);
    assert!(logs_contain("plugin.discovered"));
    assert!(logs_contain("test-id"));
}
```

- [ ] **Step 2:** Verify FAIL.
- [ ] **Step 3:** Implement `telemetry.rs`:

```rust
//! Plugin lifecycle telemetry events. Per
//! docs/src/architecture/telemetry-trust-ssot.md, emitted via tracing.

use tracing::info;

pub fn discovered(id: &str, version: &str, payload_kind: &str, abi_or_format_version: u32) {
    info!(
        event = "plugin.discovered",
        id, version, payload_kind, abi_or_format_version,
    );
}

pub fn loaded(id: &str, version: &str, payload_kind: &str, load_ms: u128) {
    info!(event = "plugin.loaded", id, version, payload_kind, load_ms);
}

pub fn load_failed(id: &str, version: &str, error_kind: &str) {
    info!(event = "plugin.load_failed", id, version, error_kind);
}

pub fn abi_mismatch(id: &str, plugin_abi: u32, host_abi: u32) {
    info!(event = "plugin.abi_mismatch", id, plugin_abi, host_abi);
}
```

- [ ] **Step 4:** Add `tracing-test = { workspace = true }` to host's `[dev-dependencies]`. Verify PASS.
- [ ] **Step 5:** Commit: `feat(plugin-host): add lifecycle telemetry events`.

### Task 11: `host_impl.rs` — `DefaultVoxHost`

**Files:** Create `crates/vox-plugin-host/src/host_impl.rs`.

Implement the `VoxHost` trait from `vox-plugin-api` so plugins receive a working host capability bundle.

- [ ] **Step 1:** Implement (no separate test — exercised via Task 16):

```rust
use abi_stable::std_types::*;
use vox_plugin_api::host::{SabiLogLevel, VoxHost};
use crate::telemetry;

pub struct DefaultVoxHost {
    data_dir: String,
}

impl DefaultVoxHost {
    pub fn new() -> Self {
        let data_dir = dirs::data_local_dir()
            .map(|p| p.join("vox").join("plugins").to_string_lossy().to_string())
            .unwrap_or_else(|| "./vox-plugins".into());
        Self { data_dir }
    }

    pub fn with_data_dir(data_dir: impl Into<String>) -> Self {
        Self { data_dir: data_dir.into() }
    }
}

impl Default for DefaultVoxHost {
    fn default() -> Self { Self::new() }
}

impl VoxHost for DefaultVoxHost {
    fn data_dir(&self) -> RString {
        self.data_dir.clone().into()
    }
    fn log(&self, level: SabiLogLevel, msg: RStr<'_>) {
        match level {
            SabiLogLevel::Trace => tracing::trace!("{}", msg.as_str()),
            SabiLogLevel::Debug => tracing::debug!("{}", msg.as_str()),
            SabiLogLevel::Info  => tracing::info!("{}", msg.as_str()),
            SabiLogLevel::Warn  => tracing::warn!("{}", msg.as_str()),
            SabiLogLevel::Error => tracing::error!("{}", msg.as_str()),
        }
    }
    fn telemetry_event(&self, kind: RStr<'_>, payload: RStr<'_>) {
        telemetry::loaded("plugin", "?", "telemetry", 0); // placeholder routing
        let _ = (kind, payload); // until the telemetry sink is wired
    }
}
```

- [ ] **Step 2:** Compile.
- [ ] **Step 3:** Commit: `feat(plugin-host): add DefaultVoxHost VoxHost implementation`.

### Task 12: `skill_registry.rs` and `registry.rs` — in-memory dual registry

**Files:** Create both files. Test in `tests/registry_basics.rs`.

Per parent spec's "Discovery & load flow" — skills are eagerly parsed at discovery; code plugins are lazily loaded.

- [ ] **Step 1:** Write tests for: register a skill, look it up by id; register a code-plugin entry placeholder, query for ml_backend (returns None since no plugin actually loaded yet).
- [ ] **Step 2:** Verify FAIL.
- [ ] **Step 3:** Implement `skill_registry.rs`:

```rust
use std::collections::HashMap;
use std::sync::RwLock;
use vox_plugin_api::skill::LoadedSkill;
use crate::errors::SkillNotInstalledError;

#[derive(Default)]
pub struct SkillRegistry {
    skills: RwLock<HashMap<String, LoadedSkill>>,
}

impl SkillRegistry {
    pub fn new() -> Self { Self::default() }

    pub fn install(&self, skill: LoadedSkill) {
        let mut w = self.skills.write().unwrap();
        w.insert(skill.plugin_id.clone(), skill);
    }

    pub fn lookup(&self, id: &str) -> Result<LoadedSkill, SkillNotInstalledError> {
        let r = self.skills.read().unwrap();
        r.get(id).cloned().ok_or(SkillNotInstalledError { skill_id: id.to_string() })
    }

    pub fn list_ids(&self) -> Vec<String> {
        self.skills.read().unwrap().keys().cloned().collect()
    }
}
```

- [ ] **Step 4:** Implement `registry.rs`:

```rust
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;
use vox_plugin_api::manifest::PluginPayload;
use crate::skill_registry::SkillRegistry;

pub struct PluginEntry {
    pub id: String,
    pub version: String,
    pub install_dir: PathBuf,
    pub payload: PluginPayload,
}

pub struct Registry {
    entries: RwLock<HashMap<String, PluginEntry>>,
    pub skills: SkillRegistry,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            skills: SkillRegistry::new(),
        }
    }
    pub fn record(&self, entry: PluginEntry) {
        self.entries.write().unwrap().insert(entry.id.clone(), entry);
    }
    pub fn get(&self, id: &str) -> Option<PluginEntryHandle> {
        // Returns a clone-of-metadata handle. SP2 has no actual loaded code
        // dispatch surface — that comes in SP3 when MlBackend lands.
        self.entries.read().unwrap().get(id).map(|e| PluginEntryHandle {
            id: e.id.clone(),
            version: e.version.clone(),
        })
    }
    pub fn list_ids(&self) -> Vec<String> {
        self.entries.read().unwrap().keys().cloned().collect()
    }
}

impl Default for Registry { fn default() -> Self { Self::new() } }

pub struct PluginEntryHandle {
    pub id: String,
    pub version: String,
}
```

- [ ] **Step 5:** Verify PASS.
- [ ] **Step 6:** Commit: `feat(plugin-host): add in-memory Registry and SkillRegistry`.

### Task 13: Add `abi_stable`, `libloading`, `dirs`, `walkdir`, `tracing-test` (if missing) to workspace

**Files:** Modify root `Cargo.toml`.

- [ ] **Step 1:** `grep -nE "^(abi_stable|libloading|dirs|walkdir|tracing-test)" Cargo.toml` to see what's missing.
- [ ] **Step 2:** Add the missing ones to `[workspace.dependencies]`. Suggested versions:

```toml
abi_stable = "0.11"
libloading = "0.8"
# dirs and walkdir are likely already present — verify before adding.
```

- [ ] **Step 3:** `cargo check --workspace` to confirm green.
- [ ] **Step 4:** Commit: `chore(workspace): add abi_stable + libloading workspace deps for plugin-host`.

(In practice, do this BEFORE Task 1's Cargo.toml so the api crate compiles. Adjust ordering during execution.)

### Task 14: Discover function

**Files:** Create `crates/vox-plugin-host/src/discover.rs`. Test in `tests/discover.rs`.

Walks an install root, parses every `Plugin.toml`, and populates a `Registry`. For skill payloads, eagerly loads the SKILL.md. Defers code-plugin dlopen to first use.

- [ ] **Step 1:** Write a test that creates a tempdir with one fake skill plugin (`Plugin.toml` + `noop.skill.md`) and asserts `discover()` populates the registry's `skills.list_ids()` to include the fake id.
- [ ] **Step 2:** Verify FAIL.
- [ ] **Step 3:** Implement `discover.rs`:

```rust
use std::path::Path;
use vox_plugin_api::manifest::{PluginManifest, PluginPayload};
use vox_plugin_api::skill::{LoadedSkill, SkillManifest};
use crate::errors::LoadError;
use crate::registry::{PluginEntry, Registry};
use crate::telemetry;

pub fn discover(root: &Path) -> Result<Registry, LoadError> {
    let registry = Registry::new();
    if !root.is_dir() {
        return Ok(registry);
    }
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name() == "Plugin.toml")
    {
        let path = entry.path();
        let raw = std::fs::read_to_string(path).map_err(|e| LoadError::ManifestParse {
            path: path.to_path_buf(),
            source: toml::de::Error::custom(format!("io: {e}")),
        })?;
        let manifest: PluginManifest = toml::from_str(&raw).map_err(|source| LoadError::ManifestParse {
            path: path.to_path_buf(),
            source,
        })?;
        let install_dir = path.parent().unwrap().to_path_buf();

        // Skill side: eagerly parse and register.
        match &manifest.plugin.payload {
            PluginPayload::Skill(s) | PluginPayload::Composite(_) => {
                if let Some(skill_md_filename) = match &manifest.plugin.payload {
                    PluginPayload::Skill(s) => Some(&s.skill_md),
                    PluginPayload::Composite(c) => Some(&c.skill.skill_md),
                    _ => None,
                } {
                    let skill_md_path = install_dir.join(skill_md_filename);
                    let body = std::fs::read_to_string(&skill_md_path).unwrap_or_default();
                    let exposed_tools = match &manifest.plugin.payload {
                        PluginPayload::Skill(s) => s.tools.exposes.clone(),
                        PluginPayload::Composite(c) => c.skill.tools.exposes.clone(),
                        _ => vec![],
                    };
                    let format_version = match &manifest.plugin.payload {
                        PluginPayload::Skill(s) => s.format_version,
                        PluginPayload::Composite(c) => c.skill.format_version,
                        _ => 0,
                    };
                    let _ = s; // suppress unused warning when arm matches Composite
                    registry.skills.install(LoadedSkill {
                        plugin_id: manifest.plugin.id.clone(),
                        format_version,
                        manifest: SkillManifest {
                            id: manifest.plugin.id.clone(),
                            name: manifest.plugin.name.clone(),
                            version: manifest.plugin.version.clone(),
                            description: manifest.plugin.description.clone(),
                            tools: exposed_tools.clone(),
                        },
                        body,
                        exposed_tools,
                    });
                }
            }
            _ => {}
        }

        let payload_kind = match &manifest.plugin.payload {
            PluginPayload::Code(_) => "code",
            PluginPayload::Skill(_) => "skill",
            PluginPayload::Composite(_) => "composite",
        };
        let abi_or_format = match &manifest.plugin.payload {
            PluginPayload::Code(c) => c.abi_version,
            PluginPayload::Skill(s) => s.format_version,
            PluginPayload::Composite(c) => c.code.abi_version,
        };
        telemetry::discovered(&manifest.plugin.id, &manifest.plugin.version, payload_kind, abi_or_format);

        registry.record(PluginEntry {
            id: manifest.plugin.id.clone(),
            version: manifest.plugin.version.clone(),
            install_dir,
            payload: manifest.plugin.payload,
        });
    }
    Ok(registry)
}
```

- [ ] **Step 4:** Verify the test PASSes.
- [ ] **Step 5:** Verify the workspace member behavior: cargo should NOT try to build `vox-plugin-noop-skill` as a Rust crate. If it does, add `exclude = ["crates/vox-plugin-noop-skill"]` to root `Cargo.toml`'s `[workspace]` section.
- [ ] **Step 6:** Commit: `feat(plugin-host): add discover() walking install root for Plugin.toml manifests`.

### Task 15: Code-plugin Loader

**Files:** Create `crates/vox-plugin-host/src/loader.rs`. Test (with noop plugin) lives in Task 16.

Wraps `libloading::Library::new`, finds the `_vox_plugin_root` symbol, asserts ABI match, calls `init`, returns the trait object.

- [ ] **Step 1:** Implement (no separate test — exercised via Task 16):

```rust
use std::path::Path;
use std::time::Instant;
use abi_stable::library::RootModule;
use libloading::Library;
use vox_plugin_api::abi::{VoxPluginRef, VoxPluginRootRef};
use vox_plugin_api::host::VoxHost_TO;
use vox_plugin_api::VOX_PLUGIN_ABI_VERSION;
use crate::errors::{AbiMismatchError, LoadError};
use crate::host_impl::DefaultVoxHost;
use crate::telemetry;

pub struct Loader;

impl Loader {
    pub fn load(plugin_id: &str, version: &str, dylib_path: &Path) -> Result<LoadedCodePlugin, LoadError> {
        let started = Instant::now();
        let lib = unsafe { Library::new(dylib_path) }
            .map_err(|source| LoadError::DlopenFailed { path: dylib_path.to_path_buf(), source })?;
        // SAFETY: VoxPluginRootRef is a sabi prefix type.
        let root_ref: VoxPluginRootRef = unsafe {
            VoxPluginRootRef::load_module_with(|| {
                <VoxPluginRootRef as RootModule>::load_from_library(&lib).map_err(|e| {
                    LoadError::InitFailed(e.to_string())
                })
            })?
        };
        if root_ref.abi_version() != VOX_PLUGIN_ABI_VERSION {
            telemetry::abi_mismatch(plugin_id, root_ref.abi_version(), VOX_PLUGIN_ABI_VERSION);
            return Err(LoadError::AbiMismatch(AbiMismatchError {
                id: plugin_id.to_string(),
                plugin_abi: root_ref.abi_version(),
                host_abi: VOX_PLUGIN_ABI_VERSION,
            }));
        }
        let host = DefaultVoxHost::new();
        let host_to = VoxHost_TO::from_value(host, abi_stable::erased_types::TD_Opaque);
        let plugin_ref = (root_ref.init())(host_to)
            .into_result()
            .map_err(|e| LoadError::InitFailed(e.to_string()))?;
        telemetry::loaded(plugin_id, version, "code", started.elapsed().as_millis());
        Ok(LoadedCodePlugin {
            _lib: lib,
            plugin: plugin_ref,
        })
    }
}

pub struct LoadedCodePlugin {
    _lib: Library, // dropped last
    pub plugin: VoxPluginRef,
}
```

(The exact `RootModule::load_from_library` invocation may need tweaking against current `abi_stable` 0.11 API. If signatures differ, the canonical reference is `abi_stable`'s book / examples. The implementer may need to adjust.)

- [ ] **Step 2:** Verify it compiles.
- [ ] **Step 3:** Commit: `feat(plugin-host): add code-plugin Loader using libloading + abi_stable`.

### Task 16: `vox-plugin-noop-code` cdylib

**Files:** `crates/vox-plugin-noop-code/{Cargo.toml,src/lib.rs,Plugin.toml}`.

The smallest possible code plugin — proves the load path end-to-end.

- [ ] **Step 1:** `Cargo.toml`:

```toml
[package]
name = "vox-plugin-noop-code"
version = "0.1.0"
edition.workspace = true
publish = false

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
vox-plugin-api = { workspace = true }
abi_stable = { workspace = true }
```

- [ ] **Step 2:** `src/lib.rs`:

```rust
//! Noop code plugin for SP2 host loader tests.

use abi_stable::{
    export_root_module, prefix_type::PrefixTypeTrait, sabi_extern_fn,
    std_types::*,
};
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
    RString::from(r#"{"id":"noop-code","version":"0.1.0"}"#)
}

#[sabi_extern_fn]
fn init(_host: VoxHost_TO<'static, RBox<()>>) -> RResult<VoxPluginRef, RBoxError> {
    let plugin = NoopPlugin;
    let to = VoxPlugin_TO::from_value(plugin, abi_stable::erased_types::TD_Opaque);
    RResult::ROk(to)
}

struct NoopPlugin;

impl VoxPlugin for NoopPlugin {
    fn id(&self) -> RString { RString::from("noop-code") }
    fn shutdown(&self) -> RResult<(), RBoxError> { RResult::ROk(()) }
}
```

- [ ] **Step 3:** `Plugin.toml`:

```toml
[plugin]
id = "noop-code"
name = "Noop Code"
version = "0.1.0"
description = "Test fixture: a no-op code plugin used by vox-plugin-host loader tests."

[plugin.host]
min-vox-version = "0.5.0"

[plugin.payload]
kind = "code"
abi-version = 1

[plugin.payload.provides]
extension-points = []

[plugin.payload.artifacts]
"windows-x86_64" = "vox_plugin_noop_code.dll"
"linux-x86_64"   = "libvox_plugin_noop_code.so"
"macos-aarch64"  = "libvox_plugin_noop_code.dylib"
```

- [ ] **Step 4:** `cargo build -p vox-plugin-noop-code` — verify the dylib produces.
- [ ] **Step 5:** End-to-end test in `crates/vox-plugin-host/tests/load_noop_code.rs`:

```rust
use std::path::PathBuf;
use vox_plugin_host::{discover, Loader};

#[test]
fn end_to_end_load_noop_code() {
    let tmp = tempfile::tempdir().unwrap();
    let plugin_dir = tmp.path().join("noop-code").join("0.1.0");
    std::fs::create_dir_all(&plugin_dir).unwrap();

    // Copy the Plugin.toml.
    let manifest = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent().unwrap()
            .join("vox-plugin-noop-code")
            .join("Plugin.toml")
    ).unwrap();
    std::fs::write(plugin_dir.join("Plugin.toml"), manifest).unwrap();

    // Find the built dylib.
    let mut dylib = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dylib.pop(); dylib.pop();
    dylib.push("target");
    dylib.push("debug");
    if cfg!(target_os = "windows") { dylib.push("vox_plugin_noop_code.dll"); }
    else if cfg!(target_os = "macos") { dylib.push("libvox_plugin_noop_code.dylib"); }
    else { dylib.push("libvox_plugin_noop_code.so"); }
    assert!(dylib.exists(), "build noop-code first: cargo build -p vox-plugin-noop-code");
    std::fs::copy(&dylib, plugin_dir.join(dylib.file_name().unwrap())).unwrap();

    let registry = discover(tmp.path()).expect("discover");
    assert!(registry.list_ids().contains(&"noop-code".to_string()));

    let loaded = Loader::load("noop-code", "0.1.0", &plugin_dir.join(dylib.file_name().unwrap())).expect("load");
    assert_eq!(loaded.plugin.id().as_str(), "noop-code");
    loaded.plugin.shutdown().into_result().expect("shutdown");
}
```

- [ ] **Step 6:** Verify PASS.
- [ ] **Step 7:** Commit: `feat(plugin-noop-code): add noop code plugin + end-to-end load test`.

### Task 17: `vox-plugin-noop-skill` directory

**Files:** `crates/vox-plugin-noop-skill/{Plugin.toml,noop.skill.md}` (no Rust crate).

- [ ] **Step 1:** `Plugin.toml`:

```toml
[plugin]
id = "noop-skill"
name = "Noop Skill"
version = "0.1.0"
description = "Test fixture: a no-op skill plugin for SP2 discover tests."

[plugin.host]
min-vox-version = "0.5.0"

[plugin.payload]
kind = "skill"
format-version = 1
skill-md = "noop.skill.md"

[plugin.payload.tools]
exposes = ["noop_tool"]
```

- [ ] **Step 2:** `noop.skill.md`:

```markdown
---
id: noop-skill
name: Noop Skill
version: 0.1.0
---

# Noop Skill

Test fixture used by `vox-plugin-host` integration tests. Exposes one
fictional tool, `noop_tool`, which the host's skill registry parses and
records but does not invoke.
```

- [ ] **Step 3:** Verify cargo doesn't try to build this directory as a Rust crate. If it does, add `exclude = ["crates/vox-plugin-noop-skill"]` to root `Cargo.toml`.
- [ ] **Step 4:** End-to-end test in `crates/vox-plugin-host/tests/load_noop_skill.rs`:

```rust
use vox_plugin_host::discover;

#[test]
fn end_to_end_load_noop_skill() {
    let tmp = tempfile::tempdir().unwrap();
    let plugin_dir = tmp.path().join("noop-skill").join("0.1.0");
    std::fs::create_dir_all(&plugin_dir).unwrap();

    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .join("vox-plugin-noop-skill");
    for f in ["Plugin.toml", "noop.skill.md"] {
        std::fs::copy(src.join(f), plugin_dir.join(f)).unwrap();
    }

    let registry = discover(tmp.path()).expect("discover");
    let skill = registry.skills.lookup("noop-skill").expect("lookup");
    assert_eq!(skill.exposed_tools, vec!["noop_tool".to_string()]);
    assert!(skill.body.contains("Noop Skill"));
}
```

- [ ] **Step 5:** PASS.
- [ ] **Step 6:** Commit: `feat(plugin-noop-skill): add noop skill plugin + end-to-end load test`.

### Task 18: ABI mismatch test

**Files:** `crates/vox-plugin-host/tests/abi_mismatch.rs`.

Build a second noop dylib with a deliberately wrong ABI version, assert the loader rejects it with `AbiMismatch` and emits the telemetry event.

This requires either (a) a second sibling `vox-plugin-noop-code-bad-abi` crate, or (b) an env-var-keyed conditional in the noop crate. Pragmatic choice: option (a). Quick crate, ~10 lines different from the good noop.

- [ ] **Step 1:** Create `crates/vox-plugin-noop-code-bad-abi/` mirroring noop-code but with `abi_version: 999_999` in the `VoxPluginRoot`.
- [ ] **Step 2:** Write the test at `crates/vox-plugin-host/tests/abi_mismatch.rs`:

```rust
use vox_plugin_host::{Loader, errors::LoadError};

#[test]
fn rejects_mismatched_abi() {
    // Path to the built bad-abi dylib (mirrors load_noop_code logic).
    let dylib = /* same locator as noop-code, swapping name */;
    let result = Loader::load("noop-bad-abi", "0.1.0", &dylib);
    match result {
        Err(LoadError::AbiMismatch(e)) => {
            assert_eq!(e.plugin_abi, 999_999);
        }
        other => panic!("expected AbiMismatch, got {other:?}"),
    }
}
```

- [ ] **Step 3:** PASS.
- [ ] **Step 4:** Commit.

### Task 19: `vox ci plugin-abi-parity` CI guard

Mirror SP1 Task 16's pattern (`plugin_catalog_parity`).

- [ ] **Step 1:** Read `crates/vox-cli/src/commands/ci/plugin_catalog_parity.rs` for the reference.
- [ ] **Step 2:** Write `crates/vox-cli/src/commands/ci/plugin_abi_parity.rs`. Logic: walk `crates/` for any `Plugin.toml` declaring `payload.kind = "code"`. For each, assert the corresponding `crates/<name>` actually builds AND its dylib loads without ABI error.
   In SP2 with only `noop-code` and `noop-bad-abi` present, this guard passes for noop-code and rejects noop-bad-abi (which is intentional in-tree fixture for the abi_mismatch test). Special-case the bad-abi dylib in the guard, OR have the guard ignore plugins whose Plugin.toml has a flag like `[plugin.fixture] expect-abi-mismatch = true`. Pragmatic: special-case ids starting with `noop-` AND skip bad-abi explicitly.
- [ ] **Step 3:** Wire into `mod.rs` / `cmd_enums.rs` / `run_body.rs` per SP1 Task 11 pattern.
- [ ] **Step 4:** Smoke test as in SP1 Task 16.
- [ ] **Step 5:** Commit.

### Task 20: `vox ci plugin-skill-parity` CI guard

Same pattern as Task 19 but validates skill `Plugin.toml` files: parses each, asserts the referenced `skill-md` file exists and is readable, and validates `exposes` is non-empty.

Tasks identical structure to Task 19.

### Task 21: Update `vox-plugin-catalog/catalog.toml`

Add `noop-code` and `noop-skill` test fixture entries. They have `bundled-in = []` (never auto-installed) and `default-source = "local:crates/vox-plugin-noop-code"` / `local:crates/vox-plugin-noop-skill`.

This satisfies the `plugin-catalog-parity` guard from SP1 Task 16, which would otherwise complain about in-tree `Plugin.toml` files without catalog entries.

- [ ] **Step 1:** Append the two entries.
- [ ] **Step 2:** Run `cargo test -p vox-plugin-catalog` (existing 25 tests should still pass; catalog now has 19 plugins).
- [ ] **Step 3:** Run `cargo run -q -p vox-cli -- ci plugin-catalog-parity` — should pass with both new ids recognized.
- [ ] **Step 4:** Run `cargo run -q -p vox-cli -- ci generate-plugin-catalog-docs` to regenerate the .generated.md files; commit those too.
- [ ] **Step 5:** Commit: `feat(plugin-catalog): add noop test-fixture entries for SP2 plugins`.

### Task 22: Update `AGENTS.md`

Add `vox-plugin-noop-skill/` to a list of "directories that look like crates but aren't" if such a list exists, OR note in the workspace-conventions doc that a directory under `crates/` without a `Cargo.toml` is intentional. Likely not needed; skip if no natural home exists.

### Task 23: Final acceptance

Run the same battery as SP1 Task 18 plus:

- `cargo build -p vox-plugin-noop-code` — green.
- `cargo build -p vox-plugin-noop-code-bad-abi` — green.
- `cargo test -p vox-plugin-host` — all four integration tests (smoke, load_noop_code, load_noop_skill, abi_mismatch) green.
- `cargo run -q -p vox-cli -- ci plugin-abi-parity` — exits 0.
- `cargo run -q -p vox-cli -- ci plugin-skill-parity` — exits 0.

If green: SP2 done. SP3 and SP4 are unblocked.

---

## Spec coverage check (self-review)

| SP2 spec deliverable                                                             | Plan task |
| -------------------------------------------------------------------------------- | --------- |
| `vox-plugin-api` crate scaffolding                                               | 1         |
| `VOX_PLUGIN_ABI_VERSION: u32 = 1`                                                | 1         |
| `VoxPluginRoot`, `VoxPlugin`, `VoxHost` `#[sabi_trait]` definitions              | 5, 7      |
| Placeholder extension-point trait files                                          | 6         |
| `SkillManifest`, `LoadedSkill` plain-Rust types                                  | 4         |
| `PluginManifest` discriminated union (code/skill/composite)                      | 3         |
| `LogLevel` enum, error types                                                     | 2, 9      |
| `Registry` (in-memory, dual-kind)                                                | 12        |
| `SkillRegistry` view with same shape as old `vox-skills`                         | 12        |
| `discover(plugin_root: &Path)` parses every `Plugin.toml`                        | 14        |
| `Loader` (libloading + ABI check + init)                                         | 15        |
| Skill loader (parse Plugin.toml + SKILL.md, register)                            | 14        |
| `PluginMissingError`, `SkillNotInstalledError`                                   | 9         |
| Telemetry events                                                                 | 10        |
| `vox-plugin-noop-code` cdylib + integration test                                 | 16        |
| `vox-plugin-noop-skill` directory + integration test                             | 17        |
| Workspace deps: `abi_stable`, `libloading`                                       | 13        |
| `vox ci plugin-abi-parity`                                                       | 19        |
| `vox ci plugin-skill-parity`                                                     | 20        |
| Catalog updated for noop fixtures                                                | 21        |

All SP2 deliverables map to at least one task. The largest implementation risk is task 7 + 15 — the `abi_stable` API surface for `Prefix`/`#[sabi_trait]` may require minor signature adjustments; the implementer should consult `abi_stable`'s book and examples if Task 15's loader doesn't compile as written.
