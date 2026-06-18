# vox-plugin-api

The stable ABI contract for [Vox](https://github.com/vox-foundation/vox) plugins.

> **Plugin authors:** use [`vox-plugin-sdk`](https://crates.io/crates/vox-plugin-sdk) instead —
> it re-exports everything here and adds the `declare_plugin!` macro. Depend on `vox-plugin-api`
> directly only if you need raw ABI control without the macro layer.

## ABI versioning

```rust
/// Newest plugin ABI this host speaks.
pub const VOX_PLUGIN_ABI_VERSION: u32 = 12;

/// Oldest plugin ABI this host still accepts.
pub const VOX_PLUGIN_ABI_MIN_SUPPORTED: u32 = 12;

/// Returns true when a plugin built against `plugin_abi` is loadable by this host.
pub fn abi_compatible(plugin_abi: u32) -> bool { … }
```

Additive ABI changes (new optional `as_*` accessors) raise only `VOX_PLUGIN_ABI_VERSION`.
Breaking changes raise both constants. Plugins outside `[min, max]` are rejected at load with
a clear error message.

## Extension points

Implement `VoxPlugin` and return your extension from the corresponding `as_*` accessor.
All `as_*` methods default to `ROption::RNone` — implement only what your plugin provides.

| Accessor | Extension trait | Use case |
|---|---|---|
| `as_ml_backend()` | `MlBackend` | CUDA / Metal training backends |
| `as_hardware_probe()` | `HardwareProbe` | GPU introspection (e.g. NVML) |
| `as_mesh_driver()` | `MeshDriver` | Distributed agent mesh transport |
| `as_skill_runtime()` | `SkillRuntime` | WASM / container skill sandboxes |
| `as_speech_to_text()` | `SpeechToText` | Whisper or other STT inference |
| `as_audio_capture()` | `AudioCapture` | Microphone recording |
| `as_browser_automation()` | `BrowserAutomation` | Chrome DevTools Protocol |
| `as_http_listener()` | `HttpListener` | Inbound webhook server |
| `as_publication()` | `Publication` | RSS ingest / social platform publish |
| `as_cloud_sync()` | `CloudSync` | Cloud artifact sync *(reserved)* |
| `as_tensor_backend()` | `TensorBackend` | Custom tensor ops *(reserved)* |
| `as_script_executor()` | `ScriptExecutor` | Script evaluation *(reserved)* |

## The host interface

Your plugin receives a `VoxHost` at `init`:

```rust
host.data_dir() -> RString            // writable directory scoped to this plugin
host.log(level, msg)                  // routes to the host tracing subscriber
host.telemetry_event(kind, payload)   // emits a structured telemetry event
```

## FFI types (abi_stable)

| Std type | ABI-stable replacement |
|---|---|
| `String` | `RString` |
| `&str` | `RStr<'_>` |
| `Box<T>` | `RBox<T>` |
| `Option<T>` | `ROption<T>` |
| `Result<T, E>` | `RResult<T, E>` |

## License

Apache-2.0 — see [LICENSE](../../LICENSE).
