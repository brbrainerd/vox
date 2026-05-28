---
title: "Migrating from Cargo Feature Flags"
description: "Mapping from `cargo --features` invocations to the runtime plugin system."
category: "How-To Guides"
---

# Migrating from Cargo Feature Flags

Vox no longer uses Cargo `--features` for optional capabilities. Each
optional feature is now a runtime-installable plugin.

## Old vs new mapping

| Old invocation | New invocation |
|---|---|
| `cargo build --features gpu,mens-candle-cuda` | `vox plugin install tensor-burn-wgpu mens-candle-cuda` |
| `cargo build --features oratio,oratio-mic` | `vox plugin install oratio oratio-mic` |
| `cargo build --features cloud` | `vox plugin install cloud` |
| `cargo build --features populi` | `vox plugin install populi-mesh` |
| Or all at once: | `vox bundle apply vox-dev` |

## Per-skill migration

The 8 built-in agent skills moved out of the `vox-skills` crate into
standalone plugins. Most users do not need to do anything —
`vox bundle apply vox-fullstack` installs them all.

| Old builtin (vox-skills) | New standalone plugin |
|---|---|
| `vox.compiler` (removed SP4) | `vox plugin install skill-compiler` |
| `vox.git` | `vox plugin install skill-git` |
| `vox.memory` | `vox plugin install skill-memory` |
| `vox.orchestrator` | `vox plugin install skill-orchestrator` |
| `vox.rag` | `vox plugin install skill-rag` |
| `vox.testing` | `vox plugin install skill-testing` |
| `vox.testing.validate` | `vox plugin install skill-testing-validate` |
| `vox.v0` | `vox plugin install skill-v0` |
| `vox.populi` (part of composite) | `vox plugin install populi-mesh` |

The full list of available plugins is shown by `vox plugin list`.

## Code consumers of vox-skills

If your code imported from `vox-skills` directly, the crate remains as a
thin shim for the current release cycle. Full removal is deferred.
When you are ready to migrate, switch to `vox-plugin-host`:

```rust
// Old:
use vox_skills::SkillRegistry;

// New:
use vox_plugin_host::SkillRegistry;
```

## Code consumers of vox-build-meta

`vox-build-meta` has been deleted (SP6). Replace any remaining calls:

```rust
// Old:
vox_build_meta::require("gpu", "vox plugin install tensor-burn-wgpu")?;

// New (inline the error message):
anyhow::bail!(
    "This capability requires the 'gpu' plugin.\n\
     Run: vox plugin install tensor-burn-wgpu"
);
```

## Cargo alias changes

The following `.cargo/config.toml` aliases were removed because the
`--features gpu` flag they relied on no longer gates anything at build time:

- `vox-cuda-release` → use `vox bundle apply vox-ml` at runtime
- `vox-mens-dev` → use `vox plugin install tensor-burn-wgpu` then `vox mens`
- `vox-mens-release` → same as above
- `vox-schola-cuda` → `vox-schola-dev` alias (schola binary) still available
