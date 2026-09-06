---
title: "Plugin System Redesign (2026)"
description: "Design spec for unifying Vox's modularity surfaces — Cargo features, vox-skills, and capability registries — into a single runtime-loadable plugin system that supports lean per-deployment distribution bundles."
category: "Architecture SSOTs"
status: "research"
training_eligible: true
training_rationale: "Architecture spec defining the unified plugin model, ABI, loader, CLI, and distribution-bundle story."
---

# Plugin System Redesign (2026)

> **Implementation status (2026-05-03):** Foundation landed on `claude/infallible-lalande-baf300` (~70 commits). **Fully implemented:** SP1, SP2, SP3 mens training extraction (sub-batches A–D), SP4, SP5 CLI surface, SP6 vox-build-meta deletion + skill migration, SP8 bundle build/verify + CI workflow, plus NVML extraction. **Trait surface only (zero code-motion):** SP3 residual mens/tensor (~7,852 LOC still in vox-populi), SP7 four extension scaffolds (TensorBackend / AudioCapture / CloudSync / ScriptExecutor — ~5,329 LOC across vox-tensor and vox-oratio not yet moved), populi-mesh composite (4,990 LOC of transport not yet moved). **Deferred:** vox-skills full retirement (ARS shim still consumed by 15+ vox-cli call sites), vox-browser plugin extraction. See [`vox-populi-extraction-followup-plan-2026.md`](vox-populi-extraction-followup-plan-2026.md) for the honest scope of what remains.

## Summary

Vox's modularity is currently spread across three uncoordinated surfaces:

1. **Cargo feature flags** — 21 crates with `[features]` blocks, plus [`vox-build-meta`](../../../crates/vox-build-meta/Cargo.toml)'s nine stubs (`gpu`, `oratio`, `mens-candle-cuda`, …). Heavy stacks (Burn, wgpu, NVML, candle-cuda) are pulled in by [`vox-populi`'s defaults](../../../crates/vox-populi/Cargo.toml).
2. **Skills** — nine built-in agent-facing capability docs in [`vox-skills`](../../../crates/vox-skills/), embedded at compile time via `include_str!`, with their own registry separate from the MCP capability catalog.
3. **Capability/MCP registries** — [`vox-capability-registry`](../../../crates/vox-capability-registry/) and `vox-mcp-registry` are two more catalogs that overlap with the skill registry.

This spec collapses all three into a single **Vox Plugin** concept: a unit of optional functionality with a `Plugin.toml` manifest, a typed payload (native code, agent skill, or both), a single install location, a single CLI (`vox plugin`), and a single in-memory registry inside a new `vox-plugin-host` crate. Plugins are runtime-loaded so users don't rebuild from source to add or remove capabilities.

On top of plugins, **distribution bundles** make the user's leanness goal concrete: `vox bundle` packages a host binary with a curated plugin set, producing artifacts like `vox-base`, `vox-fullstack`, `vox-ml`, `vox-mesh`, `vox-server`. Each is the same host binary with a different default plugin layout.

The work is decomposed into **eight sub-projects** sequenced in order. Each ships independently with its own implementation plan.

## Goals

1. Users can add or remove Vox capabilities (GPU, ML backends, audio, cloud, mesh, agent skills) without recompiling, via `vox plugin install <id>` / `vox plugin remove <id>`.
2. There is **one** definition of "what a Vox plugin is" — schema, ABI, lifecycle, install location — covering both native code and agent skills.
3. The base `vox` host binary pulls in zero optional dependencies. Every capability lives in a plugin.
4. Distributors can ship purpose-built bundles (`vox-server` without GUI/ML, `vox-ml` with CUDA stack, `vox-fullstack` with default skills) by selecting plugin sets, not by maintaining parallel binary builds.
5. The plugin ABI is versioned and stable enough that a future third-party plugin published outside the Vox repo can target it without modifying host code.
6. When a feature is unavailable because its plugin is not installed, the user gets a precise, actionable error preserving the UX shape of today's [`FeatureMissingError`](../../../crates/vox-build-meta/src/lib.rs).
7. CUDA and other essential native paths continue to work end-to-end on every system that supports them today; the plugin model imposes no regressions on supported hardware.

## Non-Goals

- A plugin marketplace, plugin signing, or any plugin-source registry beyond a flat list of GitHub-release URLs in the catalog. (Aspirational; deferred.)
- WASM / sandboxed plugins. Native dylibs only. GPU plugins (CUDA, NVML) demand native code.
- Hot-reload of plugins inside a running host. Restart is acceptable.
- Migrating in-tree build-tooling (lints, doc-pipeline, CI command-compliance) into plugins. Plugins are for *runtime* and *agent-facing* capabilities only.
- Replacing the SKILL.md format. The markdown body and TOML frontmatter that today's skills use stay as the on-disk shape of skill payloads.

## Key Decisions

### D1. Native dylibs over WASM

Required by the GPU motivating use case (CUDA, wgpu, NVML). Trade-off: plugins are platform-specific and run in-process with no sandboxing. Mitigated by treating plugin install as an explicit, manifest-checked user action.

### D2. `abi_stable` for the typed code-payload boundary

Rust has no stable ABI; raw `extern "C"` requires C-compatible types and is painful for trait-rich extension points. [`abi_stable`](https://github.com/rodrimati1992/abi_stable_crates) is the well-trodden Rust-on-Rust dylib path (Servo lineage). [`stabby`](https://github.com/ZettaScaleLabs/stabby) is rejected here for ecosystem maturity.

### D3. One catalog, one schema, one install layout — for both code and skills

A new crate `vox-plugin-catalog` holds the SSOT. [`vox-build-meta`](../../../crates/vox-build-meta/)'s feature stubs are deleted. [`vox-skills`](../../../crates/vox-skills/)'s `SkillRegistry` and `install_builtins()` are deleted. Plugin install location is `${vox_data_dir}/plugins/<plugin-id>/<version>/`, derived via the `dirs` crate.

### D4. Two-crate API split: `vox-plugin-api` (shared) and `vox-plugin-host` (host-only)

- `vox-plugin-api` declares the extension-point traits, the root `VoxPlugin` interface, the skill payload format, and the manifest types. Both host and code-payload plugins depend on it.
- `vox-plugin-host` knows how to discover, version-check, load, and dispatch to plugins (both kinds). Only the host depends on it.
- Skill-payload plugins do not need a Rust crate at all — they are just a directory with `Plugin.toml` + `SKILL.md`.

### D5. ABI version is a single integer; trait semver is separate

`vox-plugin-api` exports `pub const VOX_PLUGIN_ABI_VERSION: u32 = N;`. Code plugins record this in their manifest. Loader refuses anything that doesn't match. Within an ABI version, individual extension-point traits use semver (`MlBackend@1.x.y`); host can refuse a too-old trait revision while accepting newer ones.

Skill plugins declare `payload-format-version` instead of an ABI version (currently `1`).

### D6. Breaking changes are accepted

- Cargo feature aliases (`vox-cuda-release`, `vox-mens-dev`, `vox-mens-release` in [`.cargo/config.toml`](../../../.cargo/config.toml)) and `--features gpu,mens-candle-cuda` invocations stop working.
- The [`vox-skills`](../../../crates/vox-skills/) crate is deleted. Direct importers ([`vox-orchestrator`](../../../crates/vox-orchestrator/Cargo.toml), [`vox-plugin-host`](../../../crates/vox-plugin-host/Cargo.toml), [`vox-cli`](../../../crates/vox-cli/Cargo.toml) under feature `ars`, and [`vox-integration-tests`](../../../crates/vox-integration-tests/Cargo.toml)) migrate to `vox-plugin-host`.
- The `vox_skill_install` / `vox_skill_uninstall` / `vox_skill_list` / `vox_skill_search` / `vox_skill_info` MCP tools become `vox_plugin_*` aliases with the same shapes.
- A migration doc maps every old workflow to its new equivalent.

### D7. Naming

- Plugin crate name (code-payload): `vox-plugin-<short-id>` (e.g. `vox-plugin-mens-candle-cuda`).
- Manifest `id`: short hyphenated, globally unique (e.g. `mens-candle-cuda`, `skill-compiler`).
- User-facing CLI: short id (`vox plugin install mens-candle-cuda`).
- Extension-point trait: PascalCase, no prefix (`MlBackend`, `TensorBackend`).
- Bundle name: `vox-<flavor>` (e.g. `vox-base`, `vox-ml`, `vox-mesh`).

### D8. No host-side default plugins; bundles supply defaults

The `vox` binary alone ships with zero plugins. Defaults are supplied by the **bundle** the user installs. `vox-base` is a bare host. `vox-fullstack` ships the eight current skill plugins pre-installed. `vox-ml` adds the GPU/ML code plugins. Distributors pick.

### D9. Plugins have a typed payload: `code`, `skill`, or `composite`

A single Plugin.toml schema with a `[plugin.payload]` block whose `kind` field discriminates:

- **`code`** — ships a `cdylib` per OS/arch; provides one or more code extension points (`MlBackend`, `TensorBackend`, …). What today's Cargo features become.
- **`skill`** — ships a `SKILL.md` (TOML frontmatter + markdown body); registers itself with the agent-facing MCP surface. What today's `vox-skills` builtins become.
- **`composite`** — ships both. A future `mens-candle-cuda` plugin can ship the dylib AND a SKILL.md that documents to agents how to use it.

This gives both kinds the same install/list/remove/doctor experience while keeping their loaders distinct (no point in dragging `abi_stable` into skill loading).

### D10. Distribution bundles are derived, not parallel builds

A bundle is *the same host binary* + a curated `plugins/` directory. `vox bundle build vox-ml` produces a tarball; bundle definitions are TOML files in [`vox-plugin-catalog`](../../../crates/vox-plugin-catalog/). No per-bundle compile flags. CI matrix shrinks to "build the host once + build each plugin once per OS/arch".

## Architecture

### Crate layout (new, changed, retired)

| Crate                       | Status   | Purpose                                                                              |
| --------------------------- | -------- | ------------------------------------------------------------------------------------ |
| `vox-plugin-api`            | NEW      | Shared traits, types, ABI version constant, manifest types, skill payload format.    |
| `vox-plugin-host`           | NEW      | Discovery, manifest parsing, dual-payload loader (code + skill), registry, errors.   |
| `vox-plugin-catalog`        | NEW      | SSOT TOML listing all first-party plugins and bundle definitions.                    |
| `vox-plugin-<id>` × N       | NEW      | One `cdylib` per first-party code plugin.                                            |
| `vox-build-meta`            | RETIRED  | Feature stubs deleted. `has()`/`require()` removed. Whole crate deleted in SP6.      |
| `vox-skills`                | RETIRED  | `SkillRegistry`, `install_builtins`, `SkillPlugin`, ARS shim deleted in SP6.         |
| `vox-capability-registry`   | CHANGED  | MCP tool catalog merged into plugin registry; crate kept as the contract surface for now, callers point at `vox-plugin-host`. Full retirement deferred. |
| `vox-populi`                | CHANGED  | `mens-*` features extracted to plugin crates. `default = []`.                        |
| `vox-tensor`                | CHANGED  | `gpu`/`train` features extracted into a plugin crate. `default = []`.                |
| `vox-orchestrator`          | CHANGED  | Skill consumption migrates from `vox-skills` to `vox-plugin-host`.                   |
| `vox-actor-runtime`               | CHANGED  | Same migration.                                                                      |
| `vox-cli`                   | CHANGED  | New `plugin` and `bundle` subcommands. Removes `--features` plumbing.                |

### Plugin manifest schema (`Plugin.toml`)

Common header (all kinds):

```toml
[plugin]
id = "mens-candle-cuda"
name = "Mens (Candle + CUDA)"
version = "0.1.0"
description = "ML training backend using Candle with CUDA acceleration."
authors = ["Vox Project"]
license = "Apache-2.0"
homepage = "https://github.com/vox-foundation/vox"

[plugin.host]
min-vox-version = "1.0.0"
```

**Code payload variant.** Adds the ABI version, the artifacts table, and the extension points provided. Native-lib hints are advisory.

```toml
[plugin.payload]
kind = "code"
abi-version = 1

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
"macos-aarch64"  = "libvox_plugin_mens_candle_cuda.dylib"
```

**Skill payload variant.** Tiny — just points at the SKILL.md and declares the MCP tools the skill registers.

```toml
[plugin.payload]
kind = "skill"
format-version = 1
skill-md = "compiler.skill.md"

[plugin.payload.tools]
exposes = ["vox_validate_file", "vox_run_tests", "vox_check_workspace"]
```

**Composite variant.** Combines both blocks under one `[plugin.payload]` with `kind = "composite"`:

```toml
[plugin.payload]
kind = "composite"

[plugin.payload.code]
abi-version = 1
provides.extension-points = ["MeshDriver"]
artifacts."linux-x86_64" = "libvox_plugin_populi_mesh.so"
# …same shape as a code-only payload

[plugin.payload.skill]
format-version = 1
skill-md = "populi.skill.md"
tools.exposes = ["vox_populi_join", "vox_populi_dispatch"]
```

### Catalog schema (`vox-plugin-catalog/catalog.toml`)

The catalog lists every plugin the host *knows about*, plus the bundle definitions. Hand-edited; doc-pipeline generates the reference doc from it.

```toml
[[plugin]]
id = "mens-candle-cuda"
payload-kind = "code"
description = "ML training backend using Candle with CUDA acceleration."
extension-points = ["MlBackend"]
requires-tag = "nvidia-gpu"
default-source = "github:vox-foundation/vox-plugin-mens-candle-cuda"

[[plugin]]
id = "skill-compiler"
payload-kind = "skill"
description = "Agent-facing skill describing the Vox compiler tools."
exposes-tools = ["vox_validate_file", "vox_run_tests", "vox_check_workspace"]
default-source = "github:vox-foundation/vox-plugin-skill-compiler"
# Advisory: which first-party bundles preinstall this plugin.
bundled-in = ["vox-fullstack", "vox-ml", "vox-dev"]

[[bundle]]
id = "vox-base"
description = "Bare host binary, no plugins."
plugins = []

[[bundle]]
id = "vox-fullstack"
description = "Default developer experience with all built-in skill plugins."
plugins = [
    "skill-compiler", "skill-testing", "skill-testing-validate", "skill-memory",
    "skill-git", "skill-orchestrator", "skill-rag", "skill-v0",
    "runtime-wasm",
]

[[bundle]]
id = "vox-ml"
description = "Fullstack plus ML/GPU code plugins (NVIDIA CUDA stack)."
extends = "vox-fullstack"
plugins = ["mens-candle-cuda", "nvml-probe"]

[[bundle]]
id = "vox-mesh"
description = "Server-side mesh deployment with cloud sync."
extends = "vox-base"
plugins = ["populi-mesh", "cloud", "skill-orchestrator"]

[[bundle]]
id = "vox-server"
description = "Headless backend deployment: orchestrator + mesh, no GUI/ML."
extends = "vox-base"
plugins = ["populi-mesh", "cloud", "skill-orchestrator", "skill-memory", "webhook"]

[[bundle]]
id = "vox-edge"
description = "Edge / on-device deployment: lightweight runtime + local skills, no cloud or mesh."
extends = "vox-base"
plugins = ["skill-compiler", "skill-memory", "skill-v0", "runtime-wasm"]

[[bundle]]
id = "vox-cloud-only"
description = "Cloud-managed deployment: cloud sync only, no local ML or mesh transport."
extends = "vox-base"
plugins = ["cloud", "skill-orchestrator", "skill-memory"]

[[bundle]]
id = "vox-dev"
description = "Contributor / power-user development environment: fullstack + ML + mesh + audio."
extends = "vox-fullstack"
plugins = [
    "mens-candle-cuda", "nvml-probe",
    "populi-mesh", "cloud", "oratio", "oratio-mic",
    "script-execution", "browser", "webhook",
    "grammar-export",
]
```

### Host ABI surface (`vox-plugin-api`)

**Code-payload boundary** uses `abi_stable` (unchanged from the v1 sketch):

```rust
use abi_stable::{StableAbi, library::RootModule, sabi_trait, std_types::*};

pub const VOX_PLUGIN_ABI_VERSION: u32 = 1;

#[repr(C)]
#[derive(StableAbi)]
#[sabi(kind(Prefix(prefix_ref = VoxPluginRootRef)))]
pub struct VoxPluginRoot {
    pub abi_version: u32,
    pub manifest_json: extern "C" fn() -> RString,
    pub init: extern "C" fn(host: VoxHostRef) -> RResult<VoxPluginRef, RBoxError>,
}

#[sabi_trait]
pub trait VoxPlugin: Send + Sync {
    fn id(&self) -> RString;
    fn shutdown(&self) -> RResult<(), RBoxError>;

    #[sabi(last_prefix_field)]
    fn as_ml_backend(&self) -> ROption<MlBackend_TO<'static, RBox<()>>> { RNone }
    fn as_tensor_backend(&self) -> ROption<TensorBackend_TO<'static, RBox<()>>> { RNone }
    fn as_audio_capture(&self) -> ROption<AudioCapture_TO<'static, RBox<()>>> { RNone }
    fn as_hardware_probe(&self) -> ROption<HardwareProbe_TO<'static, RBox<()>>> { RNone }
    fn as_cloud_sync(&self) -> ROption<CloudSync_TO<'static, RBox<()>>> { RNone }
    fn as_script_executor(&self) -> ROption<ScriptExecutor_TO<'static, RBox<()>>> { RNone }
    fn as_mesh_driver(&self) -> ROption<MeshDriver_TO<'static, RBox<()>>> { RNone }
}

#[sabi_trait]
pub trait VoxHost: Send + Sync {
    fn data_dir(&self) -> RString;
    fn log(&self, level: LogLevel, msg: RStr<'_>);
    fn telemetry_event(&self, kind: RStr<'_>, payload: RStr<'_>);
}
```

**Skill-payload boundary** is plain Rust — no `abi_stable` needed because no dylib boundary is crossed. The host parses the SKILL.md off disk and constructs an in-memory `LoadedSkill`:

```rust
pub struct LoadedSkill {
    pub plugin_id: String,
    pub format_version: u32,
    pub manifest: SkillManifest,    // parsed TOML frontmatter
    pub body: String,               // markdown body
    pub exposed_tools: Vec<String>, // from Plugin.toml
}

impl SkillRegistry {
    pub fn install(&self, skill: LoadedSkill) -> Result<(), SkillError>;
    pub fn list(&self) -> Vec<SkillSummary>;
    pub fn search(&self, query: &str) -> Vec<SkillSummary>;
    pub fn info(&self, id: &str) -> Option<&LoadedSkill>;
}
```

The skill registry is owned by `vox-plugin-host` and exposed alongside the code-extension registry. MCP tools that today live on `vox-skills` (`vox_skill_install`, etc.) are renamed to `vox_plugin_*` and dispatch through the unified registry.

### Discovery & load flow

```
1. Host startup
   └─> scan ${vox_data_dir}/plugins/*/*/Plugin.toml
       └─> for each manifest:
           - validate schema
           - check os/arch match
           - record (id, version, payload-kind, abi/format version, dir) in registry
           - if payload kind is `skill` or `composite`'s skill side:
                eagerly parse SKILL.md and register with skill registry (cheap)
           - if payload kind is `code` or `composite`'s code side:
                record dylib path; do NOT load yet (lazy)

2. On first use of a code extension point
   └─> registry.get_implementor(ExtensionPoint::MlBackend)
       └─> dlopen the dylib via libloading
           - find symbol `_vox_plugin_root` -> VoxPluginRootRef
           - assert root.abi_version == VOX_PLUGIN_ABI_VERSION
           - call root.init(host_handle) -> VoxPluginRef
           - call plugin.as_ml_backend() -> MlBackend trait object
       └─> cache for process lifetime

3. On host shutdown
   └─> for each loaded code plugin: plugin.shutdown(); drop dylib handle
   └─> skill registry drops in-memory state
```

When two installed versions of the same plugin exist, the loader picks the highest semver whose `abi-version` (or `format-version`) matches the host. Mismatched plugins are silently skipped during selection but reported by `vox plugin doctor`.

### Install layout

```
${vox_data_dir}/plugins/
├── mens-candle-cuda/
│   ├── 0.1.0/
│   │   ├── Plugin.toml
│   │   ├── vox_plugin_mens_candle_cuda.dll
│   │   └── third-party-licenses/
│   └── 0.1.1/                                     ← multiple versions OK
├── tensor-burn-wgpu/
│   └── 0.2.0/
│       ├── Plugin.toml
│       └── vox_plugin_tensor_burn_wgpu.dll
└── skill-compiler/
    └── 0.1.0/
        ├── Plugin.toml
        └── compiler.skill.md
```

`${vox_data_dir}` resolves via the `dirs` crate:

- Windows: `%LOCALAPPDATA%\vox\plugins`
- Linux: `${XDG_DATA_HOME:-~/.local/share}/vox/plugins`
- macOS: `~/Library/Application Support/vox/plugins`

### Distribution bundles

A bundle is a tarball with this layout:

```
vox-ml-1.0.0-linux-x86_64.tar.gz
├── bin/vox                          ← the same host binary every bundle ships
├── plugins/
│   ├── mens-candle-cuda/0.1.0/
│   ├── tensor-burn-wgpu/0.2.0/
│   ├── nvml-probe/0.1.0/
│   ├── skill-compiler/0.1.0/
│   └── …everything from vox-fullstack via `extends`
├── BUNDLE.toml                      ← which bundle definition this came from
└── LICENSES/
```

`vox bundle build <bundle-id>` consults the catalog, verifies all listed plugins have artifacts available for the target OS/arch, and emits the tarball. The tarball is the deployment unit; an installer just unpacks it. The host binary inside is identical across every bundle of the same Vox version.

This is how the user's "deploy different versions easily" goal becomes concrete: distributors maintain a list of plugin ids per use-case, not a parallel set of build commands.

### Error model

Replace [`vox-build-meta::FeatureMissingError`](../../../crates/vox-build-meta/src/lib.rs) with `PluginMissingError`:

```rust
#[derive(Debug, thiserror::Error)]
#[error(
    "This Vox feature requires the '{plugin_id}' plugin, which is not installed.\n\n\
     To install it, run:\n\n  vox plugin install {plugin_id}\n\n\
     See: docs/src/reference/plugins.md"
)]
pub struct PluginMissingError {
    pub plugin_id: &'static str,
    pub extension_point: &'static str,
}
```

For skill plugins, the dispatch path on a missing skill returns `SkillNotInstalledError` with the same shape.

## Sub-Project 1: Plugin Manifest, Catalog & Schemas

**Scope.** Define and document the manifest and catalog schemas for both code and skill payloads. Create `vox-plugin-catalog` with the SSOT TOML (initial entries: 9 code-plugin placeholders for retired Cargo features + 8 skill-plugin entries for the current built-in skills + 5 bundle definitions). Delete `vox-build-meta`'s feature stubs (the crate stays as a stub until SP6). No loader, no ABI, no CLI work yet.

**Deliverables.**

1. Crate `vox-plugin-catalog`:
   - `catalog.toml` with all entries.
   - `build.rs` that validates the catalog and emits `OUT_DIR/catalog_generated.rs`.
   - `lib.rs` exposing `pub fn all_plugins()`, `pub fn all_bundles()`, `pub fn bundle_resolved(id: &str) -> Vec<&'static PluginCatalogEntry>` (with `extends` resolved).
2. Schema docs:
   - `docs/src/reference/plugin-manifest.md` — hand-rolled, covers all three payload kinds.
   - `docs/src/reference/plugin-catalog.md` — hand-rolled prose.
   - `docs/src/reference/plugin-catalog.generated.md` — rolled from the catalog by the doc pipeline.
   - `docs/src/reference/distribution-bundles.generated.md` — rolled from bundle definitions.
3. Empty stub of feature-stub fields in [`vox-build-meta/Cargo.toml`](../../../crates/vox-build-meta/Cargo.toml) and [`vox-build-meta/src/lib.rs`](../../../crates/vox-build-meta/src/lib.rs); `has`/`require`/`active_features` become deprecation shims that always return "this feature is now plugin `X`; install via `vox plugin install X`".
4. New CI guard `vox ci plugin-catalog-parity`: fails if a `Plugin.toml` exists in-tree for a plugin id not in the catalog, or vice versa.

**Acceptance.**

- `cargo build --workspace` succeeds with zero `--features` flags.
- `vox-plugin-catalog::all_plugins()` returns ≥18 entries (9 code + 9 skill).
- `vox-plugin-catalog::bundle_resolved("vox-ml")` resolves through `extends "vox-fullstack" extends "vox-base"` and returns the union.
- Generated reference docs list every plugin and every bundle.

## Sub-Project 2: Host ABI, Loader & Dual-Payload Registry

**Scope.** Build `vox-plugin-api` (shared types — code and skill — and the ABI constant) and `vox-plugin-host` (loader for both payload kinds, unified registry, errors). Prove end-to-end with two test plugins: `vox-plugin-noop-code` and `vox-plugin-noop-skill`.

**Deliverables.**

1. `vox-plugin-api`:
   - `VOX_PLUGIN_ABI_VERSION: u32 = 1`.
   - `VoxPluginRoot`, `VoxPlugin`, `VoxHost` `#[sabi_trait]` definitions (placeholder extension-point trait files exist; real traits land in SP3 and SP7).
   - `SkillManifest`, `SkillPayloadConfig`, `LoadedSkill` plain-Rust types.
   - `PluginManifest` discriminated union covering all three payload kinds.
   - `LogLevel` enum, error types.
2. `vox-plugin-host`:
   - `Registry` (in-memory; tracks both code and skill plugins).
   - `SkillRegistry` view (subset for MCP-tool dispatch, mirrors today's `vox-skills` API shape so callers migrate cheaply).
   - `discover(plugin_root: &Path) -> Result<Registry>`.
   - `Loader` for code plugins (wraps `libloading`, performs ABI check, calls `init`).
   - Skill loader (parses `Plugin.toml` + `SKILL.md`, validates against schema, registers).
   - `PluginMissingError`, `SkillNotInstalledError`.
   - Telemetry events (see Cross-Cutting).
3. Test plugins (in-tree, under `crates/`):
   - `vox-plugin-noop-code` — `cdylib`, implements `VoxPlugin` with no extension points.
   - `vox-plugin-noop-skill` — directory only (no Rust crate); a fixture `Plugin.toml` + tiny `SKILL.md`.
4. Workspace: add `abi_stable` and `libloading` to `[workspace.dependencies]`.
5. CI: `vox ci plugin-abi-parity` recompiles every in-tree code plugin against current `vox-plugin-api` and asserts ABI match. `vox ci plugin-skill-parity` validates every skill plugin's TOML against the schema.

**Acceptance.**

- Integration test in `vox-plugin-host` builds `vox-plugin-noop-code`, copies the artifact + manifest to a tempdir, runs `discover` → `Loader::load` → asserts the plugin reports its id and shuts down cleanly.
- Same test installs `vox-plugin-noop-skill` and asserts the skill registry returns the parsed body and exposed-tools list.
- Forcing an ABI mismatch returns a clear `AbiMismatch` error and a telemetry event.

**Risk.** `abi_stable`'s `#[sabi_trait]` ergonomics require care; the noop code plugin is the test bed for getting patterns right.

<a id="sub-project-3-first-code-extension-point-mlbackend"></a>

## Sub-Project 3: First Code Extension Point — `MlBackend`

**Scope.** Define `MlBackend` in `vox-plugin-api`. Extract `mens-candle-qlora` + `mens-candle-qlora-cuda` from [`vox-populi`](../../../crates/vox-populi/Cargo.toml) into a new `vox-plugin-mens-candle-cuda` crate. Wire `vox-populi` to consume `MlBackend` through `vox-plugin-host`. Demonstrate behavioral parity.

**Why MlBackend first.** Most-tangled feature today. Also the CUDA proof point — if a `cdylib` can host candle-cuda's nvcc kernels cleanly, every other GPU plugin is mechanical.

**Deliverables.**

1. `MlBackend` trait in `vox-plugin-api::extensions::ml_backend` (revision `1.0`); methods derived from current `mens-candle-qlora` callsites (load model, train step, eval step, save checkpoint).
2. New crate `vox-plugin-mens-candle-cuda` (`cdylib`):
   - Owns candle-core, candle-nn, qlora-rs, peft-rs, safetensors, tokenizers, memmap2 deps.
   - Implements `MlBackend`.
   - Ships its own `Plugin.toml` and integration test that loads it and runs a one-step training pass against a tiny fixture.
3. `vox-populi` changes:
   - Delete `mens-candle-qlora`, `mens-candle-qlora-cuda` features.
   - Replace direct candle calls with `host.ml_backend().ok_or(PluginMissingError { plugin_id: "mens-candle-cuda", … })?`.
4. Update [`mens-training-ssot.md`](mens-training-ssot.md) with the plugin-based invocation.
5. **CUDA-specific spike** — ✅ **completed 2026-05-03 on Windows MSVC + CUDA 13.1.** See [`crates/vox-plugin-cuda-spike/`](plugin-system-redesign-2026.md#sub-project-3-first-code-extension-point-mlbackend) and the "CUDA cdylib spike result" section below. Result: candle-core 0.9 with `features = ["cuda"]` builds cleanly inside a `cdylib`; resulting 193 KB `.dll` loads via `libloading::Library::new()`; exported `extern "C"` symbol calls into candle, initializes CUDA, opens device 0, returns success. nvcc-built kernels in the patched `candle-kernels` crate link correctly into the cdylib output. **Linux x86_64 verification deferred to SP3 implementation; design proceeds with the direct-cdylib pattern, no `staticlib` adapter needed.**

**Acceptance.**

- `vox-populi` builds without any candle-cuda features.
- With `vox-plugin-mens-candle-cuda` installed, the existing mens training integration test produces checkpoints byte-identical to pre-migration baseline (within hardware tolerance).
- Without the plugin installed, `vox train` returns `PluginMissingError` with the install command.
- The CUDA spike result is documented and the chosen pattern is the one used by the real plugin.

**Risk.** Candle-CUDA in a `cdylib` is the dominant risk; the spike is gating.

## Sub-Project 4: First Skill Plugin Migration — `skill-compiler`

**Scope.** Migrate one of today's eight built-in skills from [`vox-skills`](../../../crates/vox-skills/) into a standalone skill plugin, end-to-end. Pick `vox.compiler` because it's the most-used and exercises all the MCP-tool surfaces (`vox_validate_file`, `vox_run_tests`, `vox_check_workspace`).

**Deliverables.**

1. New plugin directory `crates/vox-plugin-skill-compiler/` (no Rust crate; just a plugin source dir):
   - `Plugin.toml` declaring skill payload, exposed tools, format version.
   - `compiler.skill.md` — the same content as today's `crates/vox-skills/skills/compiler.skill.md`.
   - `tests/` — fixture-based test that the plugin loader parses the skill correctly.
2. `vox-orchestrator` changes:
   - Replace `vox_skills::SkillRegistry` use with `vox_plugin_host::SkillRegistry`.
   - Bootstrap path no longer calls `install_builtins()`; instead it scans the install dir.
3. `vox-actor-runtime` changes: same migration.
4. MCP tool aliasing: `vox_skill_install` etc. start emitting deprecation warnings; new `vox_plugin_install` surfaces the unified path.
5. End-to-end test: orchestrator + the migrated skill plugin installed → `vox_validate_file` works through the new plugin host registry.

**Acceptance.**

- `vox.compiler` skill is no longer in `vox-skills/skills/`.
- `vox-skills` crate's `compiler.skill.md` reference is removed from `builtins.rs`.
- Orchestrator and runtime build and test green with `vox-skills` still present (other 7 skills not yet migrated).
- An agent calling `vox_validate_file` via MCP gets the same response shape as before.

**Risk.** The orchestrator's skill bootstrap path is intertwined with several call sites; map them in the implementation plan before starting.

## Sub-Project 5: `vox plugin` and `vox bundle` CLI

**Scope.** Add `plugin` and `bundle` as top-level subcommands in [`vox-cli`](../../../crates/vox-cli/) following the pattern of [`commands/add.rs`](../../../crates/vox-cli/src/commands/add.rs).

**Subcommands.**

| Command                           | Behavior                                                                  |
| --------------------------------- | ------------------------------------------------------------------------- |
| `vox plugin list`                 | All catalog entries with status: installed / available / incompatible.    |
| `vox plugin info <id>`            | Show manifest, install path, native-lib resolution status.                |
| `vox plugin install <id>`         | Look up `default-source` in catalog, fetch artifact, validate, install.   |
| `vox plugin install --path <dir>` | Install from a local unpacked plugin dir (developer mode).                |
| `vox plugin install --url <url>`  | Install from an arbitrary HTTPS URL (GitHub release zipball).             |
| `vox plugin remove <id>`          | Delete the install dir for `<id>`.                                        |
| `vox plugin doctor`               | Walk the registry: ABI check, native-lib presence, version drift report.  |
| `vox bundle list`                 | All bundle definitions from the catalog.                                  |
| `vox bundle build <id>`           | Produce `<bundle-id>-<version>-<os>-<arch>.tar.gz` containing host + plugins. |
| `vox bundle apply <id>`           | Install every plugin in `<id>` (resolved through `extends`) into the current host. |

**Source resolution.** v1 supports two source kinds, with every catalog entry guaranteed to have a `default-source` resolvable for standalone install (so `vox plugin install <id>` always works without forcing a bundle):

- `local:` — a path on disk (`vox plugin install --path …`).
- `github:owner/repo[@tag]` — uses GitHub release API to find the asset matching `<id>-<version>-<os>-<arch>.zip`. Default for first-party plugins.

The catalog's `bundled-in = […]` field is advisory: it tells `vox plugin info <id>` which first-party bundles include the plugin so the user can choose between "install one plugin" and "apply a bundle". It does not gate standalone install.

**Deliverables.**

1. `crates/vox-cli/src/commands/plugin/{mod,list,info,install,remove,doctor}.rs`.
2. `crates/vox-cli/src/commands/bundle/{mod,list,build,apply}.rs`.
3. Wire into [`cli_dispatch`](../../../crates/vox-cli/src/cli_dispatch/mod.rs) and the command catalog.
4. Generated CLI reference appears in `docs/src/reference/cli-command-surface.generated.md` automatically.
5. Integration tests: install/remove/doctor against a fixture plugin dir; stubbed HTTP server for the URL flow; bundle build + bundle apply round-trip.

**Acceptance.**

- `vox plugin list` after a fresh install shows every catalog entry with status `available`.
- `vox plugin install --path crates/vox-plugin-mens-candle-cuda/dist/` installs the dev-built plugin and `vox train` then works.
- `vox bundle build vox-base` produces a tarball whose `plugins/` is empty.
- `vox bundle build vox-fullstack` produces a tarball whose `plugins/` contains all 8 skill plugin dirs.
- `vox plugin doctor` flags a corrupted plugin without crashing the host.

## Sub-Project 6: Slim Defaults, Crate Retirement & Migration

**Scope.** Flip every `default = [...]` Cargo feature to `default = []`. Delete [`.cargo/config.toml`](../../../.cargo/config.toml) aliases. Delete [`vox-build-meta`](../../../crates/vox-build-meta/) and [`vox-skills`](../../../crates/vox-skills/) entirely. Document the migration.

**Deliverables.**

1. `vox-populi`, `vox-tensor`, `vox-ml-cli` (if still present), and any other identified crate: `default = []`.
2. Delete `vox-cuda-release`, `vox-mens-dev`, `vox-mens-release`, `vox-schola-cuda` aliases from [`.cargo/config.toml`](../../../.cargo/config.toml). Replace the comment block with a pointer to the new plugin docs.
3. Delete `vox-build-meta` entirely. Remove from `[workspace.members]`. Audit no dependent remains.
4. Delete `vox-skills` entirely. The remaining 8 skills (`testing`, `testing.validate`, `memory`, `git`, `orchestrator`, `populi`, `v0`, `rag`) get migrated to standalone skill plugins as part of this sub-project (mechanical — same shape as SP4's `skill-compiler`).
5. Migration doc `docs/src/how-to/how-to-migrate-from-cargo-features.md`:
   - Old `--features` invocation → new `vox plugin install` mapping table.
   - Old `vox-skills` import → new `vox-plugin-host` import recipe.
   - Worked example: `--features gpu,mens-candle-cuda` becomes `vox plugin install tensor-burn-wgpu mens-candle-cuda` (or `vox bundle apply vox-ml`).
6. CI:
   - `vox ci frozen-crates` (existing) updated with the removed crates.
   - `vox ci plugin-catalog-parity` (from SP1) graduates from optional to required.
   - CI scripts under `crates/vox-cli/src/commands/ci/run_body_helpers/` (`cuda.rs`, `cuda_release_build.rs`, `mens.rs`) migrated to building the relevant plugin crates.

**Acceptance.**

- Fresh `cargo install --path crates/vox-cli` produces a binary that pulls zero optional ML/audio/cloud dependencies.
- Cold `cargo build --release -p vox-cli` time on a clean target/ measurably improves; capture before/after numbers in the migration doc (target ≥ 30% reduction).
- All references to `--features gpu`, `--features mens-*`, etc. in docs are replaced with `vox plugin install …` equivalents.
- All 9 former built-in skills exist as standalone skill plugins in the catalog.

**Risk.** The MCP-tool dispatch surface in `vox-orchestrator` is touched in many places by the skill migration; the implementation plan must enumerate every callsite.

## Sub-Project 7: Remaining Code Extension Points

**Scope.** Define one extension-point trait per iteration, extract the corresponding crate, port consumers. Each iteration is its own implementation plan.

| Order | Extension point   | Replaces today                                | First plugin                              |
| ----- | ----------------- | --------------------------------------------- | ----------------------------------------- |
| 7a    | `TensorBackend`   | `vox-tensor/gpu`, `vox-tensor/train`          | `vox-plugin-tensor-burn-wgpu`             |
| 7b    | `HardwareProbe`   | `vox-populi/nvml-gpu-probe`                   | `vox-plugin-nvml-probe`                   |
| 7c    | `AudioCapture`    | `oratio`, `oratio-mic` features               | `vox-plugin-oratio`, `vox-plugin-oratio-mic` |
| 7d    | `CloudSync`       | `cloud`, `vox-populi/mens-cloud`              | `vox-plugin-cloud`                        |
| 7e    | `MeshDriver`      | `vox-populi/transport`, `populi` build flag   | `vox-plugin-populi-mesh` (composite — also exposes the `populi` skill) |
| 7f    | `ScriptExecutor`  | `script-execution` feature                    | `vox-plugin-script-execution`             |

Each iteration mirrors SP3's pattern: define trait → extract crate → wire host consumer → integration test → docs → catalog entry update.

**Acceptance per iteration.** Old Cargo feature(s) gone; new plugin's integration test passes; no host code references the old feature path; bundle definitions in the catalog updated where relevant.

## Sub-Project 8: Distribution Bundles in CI & Releases

> **Status: SHIPPED 2026-05-03** — `vox bundle build` now produces real
> tar.gz tarballs (replacing the SP5 dry-run stub); `vox bundle verify`
> extracts to a tempdir and runs `vox plugin doctor`; the
> `.github/workflows/bundle-release.yml` matrix builds all 8 bundles per
> (OS, arch) on release tags.  Deferred from initial delivery: release-notes
> delta generator (deliverable 4) — non-blocking, planned as a follow-up.

**Scope.** Make bundle production a first-class CI artifact. Each `vox` release produces `vox-base`, `vox-fullstack`, `vox-ml`, `vox-mesh`, `vox-server` tarballs per supported (OS, arch).

**Deliverables.**

1. `vox bundle build` integration into the release workflow. ✓
2. CI matrix: for each (OS, arch, bundle-id) tuple, build the bundle and upload. ✓
3. Bundle integrity test: extract each bundle, run `<bundle>/bin/vox plugin list` and assert all listed plugins load (`vox plugin doctor` exits 0). ✓ (`vox bundle verify`)
4. Release notes generator updated to enumerate which bundle gained / lost which plugins between versions. — *Deferred; non-blocking.*
5. Documentation: `docs/src/how-to/how-to-pick-a-vox-bundle.md` — short decision-tree for users. ✓

**Acceptance.**

- A release tag produces ≥ 8 bundle tarballs per supported platform (`vox-base`, `vox-fullstack`, `vox-ml`, `vox-mesh`, `vox-server`, `vox-edge`, `vox-cloud-only`, `vox-dev`).
- Each bundle's `vox plugin doctor` exits 0 in CI immediately after extraction.
- Bundle sizes are reported in CI output and tracked over time (regression budget TBD in implementation plan).

## Cross-Cutting Concerns

### Testing

- Every code plugin crate ships an in-process integration test (`tests/load_and_call.rs`) that builds itself as `cdylib`, copies the artifact + `Plugin.toml` to a tempdir, and exercises the host loader.
- Every skill plugin ships a parser test that validates `Plugin.toml` + `SKILL.md` against the schema.
- Host-level integration tests in `vox-plugin-host` only depend on `vox-plugin-noop-code` and `vox-plugin-noop-skill` — never on real-functionality plugins (keeps host build fast).
- A shared `vox-plugin-test-harness` crate provides utilities for tempdir setup, manifest fixtures, and ABI assertions.

### Documentation

- Reference: `plugins.md` (concept), `plugin-manifest.md` (schema), `plugin-catalog.generated.md`, `distribution-bundles.generated.md`.
- How-to: `how-to-write-a-plugin.md`, `how-to-write-a-skill-plugin.md`, `how-to-migrate-from-cargo-features.md`, `how-to-pick-a-vox-bundle.md`.
- Architecture: this file.
- Update [`AGENTS.md`](../../../AGENTS.md) "Auto-generated documentation files" list with `plugin-catalog.generated.md` and `distribution-bundles.generated.md`.
- Set valid frontmatter (`title`, `description`, `category`, `status`). Do not recreate `research-index.md`.
- Architecture discovery: Starlight sidebar; run `cargo run -p vox-doc-pipeline -- --lint-only` after doc edits — do not hand-edit generated stubs listed in root `AGENTS.md`.

### Telemetry

Every plugin lifecycle event emits a telemetry record per [`telemetry-trust-ssot.md`](telemetry-trust-ssot.md):

- `plugin.discovered` { id, version, payload_kind, abi_or_format_version }
- `plugin.loaded` { id, version, payload_kind, load_ms }
- `plugin.load_failed` { id, version, error_kind }
- `plugin.abi_mismatch` { id, plugin_abi, host_abi }
- `plugin.installed` { id, version, source_kind }
- `plugin.removed` { id, version }
- `bundle.applied` { bundle_id, plugin_count }
- `bundle.built` { bundle_id, target_triple, byte_size }

### Security

- **Code plugins** are native code running in-process with full host privileges. The install action is the trust boundary: `vox plugin install` shows source URL and SHA-256, prompts for confirmation unless `--yes` is passed.
- **Skill plugins** are markdown documents. They cannot execute code directly — they describe MCP tools that already exist in the host or in code plugins. Install confirmation is still required (the skill body becomes part of agent context and could prompt-inject), but the threat model is much weaker.
- The loader refuses to load any plugin whose path is outside `${vox_data_dir}/plugins/` unless `VOX_PLUGIN_DEV_PATHS` is set (set automatically by `vox plugin install --path`).
- Plugin signing is out of scope (deferred); the catalog's `default-source` is the only weak provenance v1 offers.
- Bundles inherit the trust posture of their constituent plugins. `vox bundle apply` shows the full plugin list and SHA-256 of each artifact before installing.

## What This Will Break

Honest enumeration. Every item has a documented migration in SP6's `how-to-migrate-from-cargo-features.md`.

**Build / packaging:**

1. `cargo build --features gpu,mens-candle-cuda,…` — every existing `--features` invocation that touches a now-pluginized capability stops compiling cleanly because the feature flags are removed. Replacement: `vox plugin install <id>` post-install (or `vox bundle apply vox-ml`).
2. `.cargo/config.toml` aliases `vox-cuda-release`, `vox-mens-dev`, `vox-mens-release`, `vox-schola-cuda` — deleted. Replacement: `cargo build -p vox-cli` produces the slim host; bundles produce the rest.
3. CI scripts that invoke `--features` directly: enumerated in SP6.

**Code consumers:**

4. Any code importing `vox_skills::*` — four known importers (`vox-orchestrator`, `vox-actor-runtime`, `vox-cli` under feature `ars`, `vox-integration-tests`); migrated in SP4 and SP6.
5. Any code calling `vox_build_meta::has()` / `require()` / `active_features()` — replaced by host registry queries via `vox-plugin-host`.
6. `vox-capability-registry` consumers — unchanged in SP1–SP6 (registry stays as the contract surface); a follow-up sub-project may collapse it once the plugin registry is proven.
7. The ARS shim in `vox-skills/src/ars_shim/` — bridges OpenClaw to the skill registry. Either ports to `vox-plugin-host`'s skill registry or is retired with the OpenClaw integration. Decided in SP4 implementation plan.

**Agent surface:**

8. MCP tools `vox_skill_install` / `vox_skill_uninstall` / `vox_skill_list` / `vox_skill_search` / `vox_skill_info` — kept as deprecation aliases that warn in their response and forward to `vox_plugin_*`. Removed in a Vox 2.x release.
9. The eight built-in skills' compile-time embedding (`include_str!` in `vox-skills/src/builtins.rs`) — replaced by skill plugins installed at first run by the bundle the user chose. Skills are still available; they're just sourced from disk under the install dir, not from a const.

**End-user UX:**

10. First-run behavior changes: today, `vox` ships with all 9 skills baked in. After the change, `vox-base` has none and `vox-fullstack` has all 9. The default download recommended by the website becomes `vox-fullstack`, preserving the prior experience.

### CUDA-specific risk and mitigation

CUDA is essential on the systems that need it; this spec must not break those paths. The risks specifically:

| Risk                                                                                            | Likelihood | Mitigation                                                                                                                                                                       |
| ----------------------------------------------------------------------------------------------- | ---------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Candle-CUDA's `nvcc`-built kernels may not link cleanly inside a `cdylib`.                      | **Resolved (2026-05-03)** | Spike at [`crates/vox-plugin-cuda-spike/`](plugin-system-redesign-2026.md#sub-project-3-first-code-extension-point-mlbackend) verified the direct-cdylib pattern works on Windows MSVC + CUDA 13.1. nvcc kernel build succeeds, dll is 193 KB, `libloading` opens it, candle initializes CUDA from inside the loaded dylib. Linux x86_64 still to verify in SP3 but no design pivot expected. Fallback (`staticlib` + thin cdylib, or IPC child process) deleted from the plan. |
| CUDA toolkit (cudart, cublas, nvcc) presence at user runtime varies wildly.                     | High       | No regression — same problem today. `Plugin.toml` `requires.native-libs` makes the requirement explicit. `vox plugin doctor` reports missing libs with install guidance. Loader does not block: failure surfaces at first `MlBackend` call, not at process start. |
| Two GPU plugins (e.g. `tensor-burn-wgpu` and `mens-candle-cuda`) might link incompatible versions of cudart or cublas. | Medium     | Plugins built in this repo pin native-lib versions in their `[plugin.payload.requires.native-libs]`. CI guard `vox ci plugin-native-lib-coherence` flags conflicting requirements across the catalog. Cross-version coexistence is not guaranteed; documented limitation. |
| CUDA initializes process-wide context; multiple plugins sharing context could race.             | Low        | No regression — same in today's monolith. Document that CUDA-using plugins must not assume exclusive context. Add a `HardwareProbe::cuda_context_init()` host-mediated hook in SP7b for plugins that need explicit init ordering. |
| nvcc-built kernel object size inflates the dylib past Windows MAX_PATH/loader limits.           | Low        | Caught by SP3 spike. Kernel-heavy plugins ship the kernels as a sibling resource file alongside the dylib; loader-side resolution. |
| Plugin authors of GPU plugins still need a CUDA dev environment.                                | n/a        | No regression. Documented in `how-to-write-a-plugin.md`.                                                                                                                         |
| Released CUDA-linked dylibs need to dynamically link the user's system cudart.                  | n/a        | Standard CUDA practice — same as today. Bundle artifacts list cudart as advisory, never bundled.                                                                                  |

The **gating commitment** for CUDA was met on 2026-05-03 for Windows MSVC + CUDA 13.1. SP3 proceeds with the direct-cdylib pattern. Linux x86_64 verification is now a routine SP3 step rather than a design-blocking risk.

### CUDA cdylib spike result (2026-05-03)

The spike crate at [`crates/vox-plugin-cuda-spike/`](plugin-system-redesign-2026.md#sub-project-3-first-code-extension-point-mlbackend) is the smallest reproduction of the SP3 plugin pattern: a `cdylib` (+ `rlib` for in-process tests) depending on `candle-core` with `features = ["cuda"]`, exporting two `extern "C"` symbols.

**Environment.** Windows 11, MSVC 2022 Community via `vcvars64.bat`, NVCC 13.1, Rust edition 2024.

**Build.** `cargo build --release -p vox-plugin-cuda-spike` finished in 1m 06s, producing `target/release/vox_plugin_cuda_spike.dll` (193 KB, 2.2 KB import library). The patched `candle-kernels` build script ran `bindgen-cuda` and nvcc successfully; kernel objects linked into the dylib.

**Load test.** [`tests/load_via_dylib.rs`](plugin-system-redesign-2026.md#sub-project-3-first-code-extension-point-mlbackend) opens the dylib via `libloading::Library::new()`, resolves both exported symbols, and calls them. Result:

```
test dlopen_resolves_smoke_symbol ... ok
vox_spike_cuda_available returned: 1
test dlopen_calls_cuda_path ... ok
test result: ok. 2 passed; 0 failed
```

The `1` return from `vox_spike_cuda_available` confirms candle-core successfully opened CUDA device 0 from inside the dlopen'd dylib — the full code path is exercised, not just a static link check.

**Implications for SP3.**

1. The direct-cdylib pattern is the SP3 design. The `staticlib`-adapter and IPC-child-process fallbacks are deleted from the plan.
2. The 193 KB dylib size confirms candle defers cudart loading and kernel JIT to runtime — plugins do not statically link a particular cudart version, so coexistence of multiple GPU plugins is not constrained at link time. This eases the "two plugins with conflicting native-lib versions" risk.
3. Edition 2024's `#[unsafe(no_mangle)]` syntax works. The two `unsafe-code` lint warnings the spike emits should be expected on real plugins; the SP2 `vox-plugin-api` design uses `abi_stable`'s single-root-symbol pattern, which avoids manual `no_mangle` on the trait surface entirely (only the `_vox_plugin_root` symbol is exported by name).
4. CUDA 13.1 works with candle 0.9 — no need to pin to an older toolkit for plugins.

**What this spike did not verify.** Linux x86_64 build (deferred to SP3 implementation; same pattern expected to work). Multi-plugin coexistence (two cdylibs both pulling in candle loaded simultaneously). Full candle training workflow (only device init was exercised). The `abi_stable` boundary itself (orthogonal; well-established Rust pattern).

The spike crate is kept in the tree as a reference for SP3 plugin authors.

## Risks & Open Questions

| Risk                                                                                                | Mitigation                                                                                     |
| --------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| Candle-CUDA `cdylib` build (see CUDA section above).                                                 | Gating spike in SP3.                                                                           |
| `abi_stable` adds compile-time overhead and trait-method limitations.                                | Constrain extension-point traits to method shapes `abi_stable` supports cleanly; document the rules in `vox-plugin-api/CONTRIBUTING.md`. |
| Host ABI churn during SP3–SP7 invalidates installed plugins.                                        | Bump `VOX_PLUGIN_ABI_VERSION` deliberately; clear `AbiMismatch` error tells users to reinstall. Pre-1.0 era is open season. |
| Per-OS dylib distribution (especially Windows MSVC vs GNU) multiplies CI matrix.                    | First-party plugins target only OS/arch combos already in CI.                                  |
| Native-lib presence varies wildly across user machines.                                              | `Plugin.toml`'s `requires.native-libs` is advisory; `vox plugin doctor` reports failures but loader doesn't block. |
| Skill-payload schema diverges from existing SKILL.md format used by `vox-skills`.                   | Format-version field allows evolution; SP4's migration is a verbatim port (no schema change).  |
| Open: should `vox-capability-registry` be folded into `vox-plugin-host`?                            | Deferred. Spec retains it as the contract surface; collapse decision after SP7 lands.          |
| Open: `vox plugin upgrade <id>` — install layout supports multiple versions, but the upgrade verb isn't speced. | Deferred to a follow-up; non-breaking add later.                                               |
| Open: should bundles be cryptographically signed?                                                   | Deferred. Bundle integrity = SHA-256 of constituent plugins for v1.                            |

## Sequencing Summary

```
SP1  (Manifest, Catalog & Schemas)         — foundation, no runtime change
SP2  (Host ABI, Loader & Dual-Payload Reg) — depends on SP1
SP3  (MlBackend + first code plugin)       — depends on SP2; CUDA spike gates the rest
SP4  (skill-compiler migration)            — depends on SP2; can land in parallel with SP3
SP5  (vox plugin + vox bundle CLI)         — depends on SP2; can land alongside SP3/SP4
SP6  (Slim defaults, retire vox-skills + vox-build-meta) — depends on SP3, SP4, SP5
SP7  (Remaining code extension points)     — depends on SP6; iterative ~6 sub-iterations
SP8  (Bundles in CI & releases)            — depends on SP6; can run alongside SP7
```

SP3, SP4, SP5 can all be developed in parallel by separate plans once SP2 lands. None of them ship in a single PR.

## Definition of Done (whole spec)

- All nine current `vox-build-meta` features and all nine current built-in skills are reborn as installed plugins (code or skill payload as appropriate).
- `vox-build-meta` and `vox-skills` are deleted from the workspace.
- `cargo build --workspace` requires zero `--features` flags and produces a slim host binary.
- Cold `cargo build --release -p vox-cli` time on clean target/ has measurably dropped (target ≥ 30% reduction; actual captured in SP6 migration doc).
- A user with only the built `vox` binary and the docs can install GPU support without rebuilding from source.
- A future contributor adding a capability writes one plugin (with `Plugin.toml` + either a skill markdown or a `cdylib`) and one catalog entry — no host code changes required.
- CUDA workflows on supported hardware (Windows MSVC + Linux x86_64 with NVIDIA + cudart 12.x) produce checkpoints behaviorally identical to pre-migration baseline.
- Each release ships `vox-base`, `vox-fullstack`, `vox-ml`, `vox-mesh`, `vox-server`, `vox-edge`, `vox-cloud-only`, `vox-dev` bundles per supported platform, each passing `vox plugin doctor` immediately after extraction.
