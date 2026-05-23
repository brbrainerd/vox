---
title: "Plugin System Deep Audit (2026-05-08)"
description: "Second-pass audit covering ABI completeness, discovery, sandbox model, cross-cutting concerns, and distribution."
category: "Architecture SSOTs"
status: "research"
training_eligible: true
training_rationale: "Comprehensive snapshot of plugin system invariants and gaps useful for plugin authors and future architecture work."
---

# Plugin System Deep Audit (2026-05-08)

> This is the second-pass audit. First-pass: `plugin-system-audit-2026-05-08.md`. ABI = 11.

---

## ABI Surface Inventory

### VoxPlugin (root trait — `vox-plugin-api/src/abi.rs`)

Every code plugin must implement `VoxPlugin`. The trait also provides optional extension-point accessors (default = `RNone`).

| Method | Required | Default | Implemented in N plugins |
|---|---|---|---|
| `id() -> RString` | yes | none | all 9 code plugins |
| `shutdown() -> RResult<(), RBoxError>` | yes | none | all 9 code plugins |
| `as_ml_backend()` | no | `RNone` | 1 (`mens-candle-cuda`) |
| `as_hardware_probe()` | no | `RNone` | 1 (`nvml-probe`) |
| `as_mesh_driver()` | no | `RNone` | 1 (`populi-mesh`) |
| `as_tensor_backend()` | no | `RNone` | 1 (`tensor-burn-wgpu`) |
| `as_audio_capture()` | no | `RNone` | 2 (`oratio`, `oratio-mic`) |
| `as_cloud_sync()` | no | `RNone` | 1 (`cloud`) |
| `as_script_executor()` | no | `RNone` | 1 (`script-execution`) |
| `as_browser_automation()` | no | `RNone` | 1 (`browser`) |
| `as_speech_to_text()` | no | `RNone` | 1 (`oratio`) |

**Gap**: `noop-code` implements `VoxPlugin` with a stub `shutdown` that returns `ROk(())`. It does not implement any extension points — correct for a test fixture.

### VoxHost (host capability — `vox-plugin-api/src/host.rs`)

The host provides three capabilities to each plugin at `init` time:

| Method | Status |
|---|---|
| `data_dir() -> RString` | Implemented (returns plugin install root) |
| `log(level, msg)` | Implemented (routes to `tracing::*!`) |
| `telemetry_event(kind, payload)` | Implemented (logs at INFO via tracing; no metrics pipeline) |

**Gap**: `telemetry_event` routes to the same tracing sink as `log`. There is no structured metrics export (no OpenTelemetry export, no Prometheus endpoint). The event kind and payload are logged but not aggregated.

### MlBackend (revision 3 — `extensions/ml_backend.rs`)

| Method | Candle-CUDA | tensor-burn-wgpu | Expected |
|---|---|---|---|
| `revision()` | default (3) | N/A (not impl) | all MlBackend plugins |
| `load_model(path)` | real impl | — | — |
| `train_step(model, batch)` | real impl | — | — |
| `eval_step(model, batch)` | real impl | — | — |
| `save_checkpoint(model, dest)` | real impl | — | — |
| `run_full_training(config)` | real impl | — | — |
| `run_inference(model, prompt)` | real impl | — | — |
| `merge_adapter(base, adapter, dest)` | real impl | — | — |

**Note**: `tensor-burn-wgpu` implements `TensorBackend`, not `MlBackend`. The two extension points are separate.

### HardwareProbe (revision 1)

| Method | nvml-probe |
|---|---|
| `revision()` | default (1) |
| `probe_summary_json()` | real impl (NVML) |
| `device_metrics_json()` | real impl (NVML) |

### MeshDriver (revision 2)

| Method | populi-mesh |
|---|---|
| `revision()` | default (2) |
| `start_transport(config)` | real impl |
| `stop_transport()` | real impl |
| `dispatch(request)` | real impl |
| `node_join(record)` | real impl |
| `list_nodes()` | real impl |
| `relay_message(peer, request)` | real impl (exists in trait) |

### TensorBackend (revision 1)

| Method | tensor-burn-wgpu |
|---|---|
| `revision()` | default (1) |
| `name()` | stub ("burn-wgpu") |
| `supports_cuda()` | stub (false) |
| `supports_wgpu()` | stub (true) |
| `allocate_tensor_json(spec)` | stub (returns TODO error) |

**Gap**: `tensor-burn-wgpu` is a SP7 scaffold. All methods except `name` / `supports_wgpu` are stubs. Actual tensor extraction from `vox-tensor/src/` is deferred.

### AudioCapture (revision 1)

| Method | oratio | oratio-mic |
|---|---|---|
| `list_devices_json()` | real impl | real impl |
| `start_capture(device, config)` | real impl | real impl |
| `stop_capture()` | real impl | real impl |
| `read_chunk()` | real impl | real impl |

### CloudSync (revision 1)

| Method | cloud |
|---|---|
| `provider_id()` | stub (returns "cloud") |
| `upload(local, remote)` | stub (returns TODO error) |
| `download(remote, local)` | stub (returns TODO error) |
| `list_remote_json(prefix)` | stub (returns TODO error) |

**Gap**: `cloud` is a SP7 scaffold. All methods are stubs. Extraction from `vox-schola` deferred.

### SpeechToText (revision 2)

| Method | oratio |
|---|---|
| `transcribe(audio, config)` | real impl |
| `begin_stream(config)` | real impl |
| `push_audio(session, audio)` | real impl |
| `end_stream(session)` | real impl |
| `transcribe_path(path, config)` | real impl |

### BrowserAutomation (revision 1)

| Method | browser |
|---|---|
| `open(url, headless)` | real impl (chromiumoxide) |
| `goto(page, url)` | real impl |
| `click(page, target)` | real impl |
| `fill(page, target, value)` | real impl |
| `eval_js(page, script)` | real impl |
| `screenshot_png(page)` | real impl |
| `close(page)` | real impl |

### ScriptExecutor (revision 1)

| Method | script-execution |
|---|---|
| `execute(path, args)` | stub (TODO) |
| `validate(path)` | stub (TODO) |

**Gap**: `script-execution` is a SP7 scaffold. Extraction from `vox-eval` deferred.

---

## Plugin Lifecycle

### Discovery

Path: `vox-plugin-host/src/discover.rs` → `discover(root: &Path) -> Result<Registry, LoadError>`

1. Walks `root` (or `$VOX_PLUGINS_DIR` / `~/.local/share/vox/plugins`) recursively with `walkdir`.
2. Finds all `Plugin.toml` files.
3. Parses each with `toml::from_str::<PluginManifest>`.
4. For skill or composite plugins: eagerly reads and registers the `SKILL.md` body into `SkillRegistry`.
5. For code plugins: records the `PluginEntry` (dylib path only) without loading.
6. Emits a `plugin.discovered` tracing event per plugin.

**Failure modes during discovery**:
- `Plugin.toml` unreadable → warning logged, plugin skipped (soft failure)
- `Plugin.toml` parse error → warning logged, plugin skipped (soft failure)
- `SKILL.md` missing → warning logged, skill plugin skipped (soft failure)

Discovery never fails hard — the registry may be partial. Callers must check `registry.has(id)` before dispatching.

### Loading

Path: `vox-plugin-host/src/loader.rs` → `Loader::load(id, version, dylib_path)`

1. Calls `VoxPluginRootRef::load_from_file(dylib_path)` — this is `abi_stable`'s `RootModule` machinery, which calls `dlopen` (Linux/macOS) or `LoadLibraryW` (Windows).
2. Reads `root_ref.abi_version()` and compares against `VOX_PLUGIN_ABI_VERSION` (11). Mismatch → `LoadError::AbiMismatch` (hard failure, no loading).
3. Constructs a `DefaultVoxHost`, wraps it in an `abi_stable::erased_types::TD_Opaque` RBox, and calls `root_ref.init(host)`.
4. Returns `LoadedCodePlugin { plugin: VoxPluginRef }`.

`abi_stable` intentionally leaks the underlying `libloading::Library` handle. The dylib can never be unloaded at runtime.

### Caching

There is no in-process cache. Each call to `load_code_plugin_by_id` (or `load_code_plugin`) re-walks the install root and re-calls `Loader::load`. The `Registry` is rebuilt from disk every time.

**Gap**: No in-process plugin cache. For hot paths (e.g., repeated `mens-candle-cuda` dispatch during training), callers must hold onto the `LoadedCodePlugin` across calls. There is no global singleton registry.

### Unload

Plugins are never unloaded. `abi_stable` leaks the library. `VoxPlugin::shutdown()` exists in the trait but is never called by the host today. Hot reload is not supported.

---

## Sandbox Model

### Current State

The plugin host provides **no process isolation or syscall filtering**. A loaded code plugin runs in the host process with full OS privileges. The sandbox is purely by convention (the ABI surface restricts what the host *offers* to the plugin, not what the plugin can *do*).

| Mechanism | Status |
|---|---|
| Process isolation (separate process / Wasmtime) | Not implemented |
| Syscall filtering (seccomp, Landlock) | Not implemented |
| Filesystem jail | Not implemented |
| Network restriction | Not implemented |
| Memory limits | Not implemented |
| Signature / code-signing check | Not implemented |
| Capability tokens (plugin can only use declared extension points) | Not implemented |

### Capability Declaration (Informational Only)

`Plugin.toml` declares `extension-points` and `requires` (OS, arch, native libs). These are **informational** — the host does not enforce that a plugin only calls back through its declared extension points. A malicious plugin could:

- Read or write any file
- Spawn processes
- Open network connections
- Access other plugins' data directories
- Escalate to root if the host process has elevated privileges

### Trust Model

All installed plugins are implicitly trusted. The install path (`$VOX_PLUGINS_DIR` or `~/.local/share/vox/plugins`) is the only access control — whoever can write to that directory can install arbitrary native code.

### Gaps and Recommendations

| Gap | Severity | Recommended Fix |
|---|---|---|
| No process isolation | High | Wasmtime component model for sandboxed plugins (long-term) |
| No signature check | High | Require plugins to be signed with a known key; reject unsigned |
| No capability enforcement | Medium | Map declared extension points to a capability token; restrict host callbacks |
| No filesystem jail | Medium | Use Landlock (Linux) / Win32Job (Windows) to restrict plugin I/O |
| No hot reload | Low | Track library handles; add `VoxPlugin::reload()` + `shutdown()` lifecycle |

---

## Cross-Cutting Concerns

### Logging

Plugins receive a `VoxHost::log(level, msg)` callback. The `DefaultVoxHost` implementation routes this to `tracing::trace!/debug!/info!/warn!/error!`.

- **Format**: unstructured string. Plugin log messages do not carry a `plugin_id` field automatically.
- **Gap**: No structured per-plugin log context. A plugin calling `host.log(Info, "training started")` is indistinguishable in logs from a host-side tracing event.
- **Recommendation**: Wrap `DefaultVoxHost` in a per-plugin context that prepends `plugin_id = "..."` as a tracing span field.

### Telemetry

`VoxHost::telemetry_event(kind, payload)` is implemented as a `tracing::info!` call in `host_impl.rs`. There is no OpenTelemetry, Prometheus, or structured metrics pipeline.

- Plugin-emitted telemetry goes to the same log sink as all other tracing output.
- There is no aggregation, no histogram, no counter.
- **Recommendation**: Wire `telemetry_event` to an in-process metrics store (e.g., `metrics` crate or a channel-based aggregator) in addition to the tracing sink.

### Versioning Beyond ABI

- **ABI version** (integer, currently 11): enforced at load time via `VoxPluginRoot::abi_version`.
- **Plugin semver** (`version` field in `Plugin.toml`): recorded but not enforced by the host.
- **Extension-point revision** (each trait has a `revision()` method): not checked by the host at runtime. The host calls methods without verifying the plugin's declared revision matches expectations.
- **Manifest format version** (skill plugins): format-version 1 is the only version; no migration path.
- **Gap**: No host-side check of `MlBackend::revision()` etc. If a trait gains a new required method, older plugins will panic at the vtable boundary.

### Hot Reload

Not supported. `abi_stable` leaks library handles. There is no `reload()` lifecycle method and no file-watcher integration.

---

## Distribution and Installation Flow

### `vox plugin install <id>`

1. CLI resolves the plugin ID against `vox-plugin-catalog` (SSOT in `catalog.toml`).
2. Reads `default-source`:
   - `local:crates/<name>` → build from workspace source (dev / CI workflow)
   - `github:vox-foundation/<repo>` → download pre-built artifact from GitHub Releases
3. Determines the current target triple via `vox_plugin_host::current_target_triple_key()`.
4. Downloads or copies the `.dll` / `.so` / `.dylib` to `$VOX_PLUGINS_DIR/<id>/`.
5. Copies `Plugin.toml` to the same directory.
6. For composite or skill plugins: copies `SKILL.md`.

> Implementation detail: the actual download loop lives in `vox-cli/src/commands/plugin/install.rs`. It uses `reqwest` with `self_update`-style archive extraction (tar.gz / zip).

### Registry/Marketplace

There is no public plugin registry or marketplace. All first-party plugins are listed in `catalog.toml`. Third-party plugins are not currently supported at the install-command level (no `vox plugin search`, no registry API).

### Bundle Install

`vox bundle apply <flavor>` resolves the bundle ID via `vox_plugin_catalog::bundle_resolved`, which walks the `extends` chain and deduplicates plugin IDs. Each resolved plugin is then passed through the same install flow as `vox plugin install`.

Bundle flavors as of this audit:

| Bundle | Plugins included |
|---|---|
| `vox-base` | (none) |
| `vox-fullstack` | skill-compiler, skill-testing, skill-testing-validate, skill-memory, skill-git, skill-orchestrator, skill-rag, skill-v0 |
| `vox-ml` | vox-fullstack + tensor-burn-wgpu, mens-candle-cuda, nvml-probe |
| `vox-mesh` | populi-mesh, cloud, skill-orchestrator |
| `vox-server` | populi-mesh, cloud, skill-orchestrator, skill-memory |
| `vox-edge` | skill-compiler, skill-memory, skill-v0 |
| `vox-cloud-only` | cloud, skill-orchestrator, skill-memory |
| `vox-dev` | vox-fullstack + tensor-burn-wgpu, mens-candle-cuda, nvml-probe, populi-mesh, cloud, oratio, oratio-mic, script-execution, browser |

### Local Build Flow (dev)

For `local:crates/<name>` sources, `vox plugin install` invokes `cargo build --profile dist -p <crate-name>` and copies the resulting dylib. The `dist` profile is defined in the workspace `Cargo.toml`.

---

## Recommendations (Priority + Effort)

| # | Recommendation | Priority | Effort |
|---|---|---|---|
| 1 | Add per-plugin `tracing::Span` context to `DefaultVoxHost::log` | P1 | Low (1h) |
| 2 | Enforce extension-point revision at load time — call `backend.revision()` and compare to host constant | P1 | Low (2h) |
| 3 | Wire `telemetry_event` to an in-process metrics aggregator (even a simple `AtomicU64` counter map) | P2 | Medium (1d) |
| 4 | Add in-process `LoadedCodePlugin` cache (Arc<RwLock<HashMap<id, LoadedCodePlugin>>>) | P2 | Low (4h) |
| 5 | Remove `execution-api` and `stub-check` ghost entries from `catalog.toml` or create placeholder crates | P2 | Trivial (30m) |
| 6 | Add plugin code-signing check at install time (verify SHA256 against a manifest signature) | P2 | Medium (2d) |
| 7 | Add `unload_model(handle)` to `MlBackend` trait (currently models are leaked — ABI rev bump needed) | P3 | Medium (1d) |
| 8 | Sandbox plugins with Wasmtime component model (long-term, requires redesign) | P4 | High (weeks) |
| 9 | Add hot-reload support: file watcher + `VoxPlugin::reload()` lifecycle | P4 | High (3-5d) |
| 10 | Create third-party plugin registry API and `vox plugin search` command | P4 | High (weeks) |
