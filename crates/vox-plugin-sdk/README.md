# vox-plugin-sdk

Authoring SDK for [Vox](https://github.com/vox-foundation/vox) plugins.
If you are writing a plugin, start here.

## Quick start

**1. `Cargo.toml` for your plugin:**

```toml
[package]
name = "my-vox-plugin"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
vox-plugin-sdk = "0.6"
```

**2. `Plugin.toml` (next to `Cargo.toml`):**

```toml
[plugin]
id = "my-plugin"
name = "My Plugin"
version = "0.1.0"
description = "Does something useful."
license = "Apache-2.0"

[plugin.host]
min-vox-version = "0.6.0"

[plugin.payload]
kind = "code"
abi-version = 12

[plugin.payload.provides]
extension-points = ["HardwareProbe"]
```

Replace `"HardwareProbe"` with your extension point. Valid values: `MlBackend`,
`HardwareProbe`, `MeshDriver`, `SkillRuntime`, `SpeechToText`, `AudioCapture`,
`BrowserAutomation`, `HttpListener`, `Publication`.

**3. `src/lib.rs`:**

```rust
use vox_plugin_sdk::prelude::*;

#[derive(Clone)]
struct MyPlugin;

impl VoxPlugin for MyPlugin {
    fn id(&self) -> RString { RString::from("my-plugin") }
    fn shutdown(&self) -> RResult<(), RBoxError> { RResult::ROk(()) }

    fn as_hardware_probe(&self) -> ROption<HardwareProbe_TO<'static, RBox<()>>> {
        ROption::RSome(HardwareProbe_TO::from_value(
            self.clone(),
            abi_stable::erased_types::TD_Opaque,
        ))
    }
}

impl HardwareProbe for MyPlugin {
    fn probe_summary_json(&self) -> RResult<RString, RBoxError> {
        RResult::ROk(RString::from(r#"{"devices":[]}"#))
    }
    fn device_metrics_json(&self, _index: u32) -> RResult<RString, RBoxError> {
        RResult::ROk(RString::from("{}"))
    }
}

declare_plugin! {
    init: |_host| ROk(wrap(MyPlugin))
}
```

**4. Build:**

```sh
cargo build --release
```

The `.dll` / `.so` / `.dylib` in `target/release/` is your plugin artifact.
Install: `vox plugin install --path ./target/release/libmy_vox_plugin.so`.

## What this crate provides

| Export | Description |
|---|---|
| `declare_plugin! { init: \|host\| ... }` | Emits the three required dylib export symbols |
| `wrap(plugin)` | Erases your plugin type into a `VoxPlugin_TO` trait object |
| `prelude::*` | All `abi_stable` stable types, `VoxPlugin`, `VoxHost_TO`, all 12 extension traits and their `_TO` wrappers |

## How `declare_plugin!` works

The macro reads your `Plugin.toml` at compile time via `include_str!` and embeds it into the
`__vox_plugin_manifest_json` export. The host validates it at load time.

## ABI compatibility

Plugins built with SDK 0.6 implement ABI v12. Additive ABI changes (new optional accessors)
do not require rebuilding existing plugins. Breaking changes are announced in the changelog
and raise the minimum supported ABI version.

## License

Apache-2.0 — see [LICENSE](../../LICENSE).
