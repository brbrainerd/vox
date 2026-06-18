# vox-plugin-types

Pure manifest and skill types for the [Vox](https://github.com/vox-foundation/vox) plugin system.

## What this crate is

A **zero-dependency types leaf**: no async runtime, no ABI machinery (`abi_stable`), no database client.
If you need only the shape of a `Plugin.toml` manifest or a skill manifest without pulling in the
full plugin host, depend here.

Re-exported by `vox-plugin-api` (manifest types) and `vox-plugin-host` (skill manifest + state-backend
trait) for backwards compatibility — you usually reach these types through those crates rather than
adding `vox-plugin-types` directly.

## Key types

| Type | Description |
|---|---|
| `PluginManifest` | Typed deserialization of `Plugin.toml` files |
| `PluginHeader` | Top-level metadata: id, name, version, license, status, … |
| `PluginPayload` | `Code`, `Skill`, or `Composite` payload variant |
| `CodePayload` | Binary plugin: ABI version, extension points, native lib requirements |
| `SkillPayload` | Skill plugin: format version, `.skill.md` path, exposed tools |
| `CompositePayload` | Both code and skill payloads in one plugin |
| `SkillManifest` | Parsed skill manifest: id, permissions, tools, dependencies |
| `SkillCategory` | Domain enum: Compiler, Testing, Git, Database, Security, … |
| `SkillPermission` | Declared permission: ReadFiles, WriteFiles, ShellExec, Network, … |
| `PluginStateBackend` | Async trait for plugin-owned persistent state storage |
| `current_target_triple()` | Returns the host's plugin artifact filename suffix |
| `plugin_artifact_filename(id)` | Constructs the OS-appropriate dylib name for a plugin id |

## When to use this vs. the other plugin crates

| Need | Crate |
|---|---|
| Parse or inspect a `Plugin.toml` | **`vox-plugin-types`** ← this crate |
| Implement a Vox plugin (write extension code) | [`vox-plugin-sdk`](https://crates.io/crates/vox-plugin-sdk) |
| Load and dispatch plugins at runtime | `vox-plugin-host` (internal to Vox) |
| ABI traits and version constants only | [`vox-plugin-api`](https://crates.io/crates/vox-plugin-api) |

## Versioning

This crate follows the Vox workspace version (`0.6.x`). A major version bump signals a breaking
change to the manifest or skill schema.

## License

Apache-2.0 — see [LICENSE](../../LICENSE).
