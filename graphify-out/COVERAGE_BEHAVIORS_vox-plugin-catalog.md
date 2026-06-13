# Semantic Behavior Map — `vox-plugin-catalog`

Deterministically synthesized from 34 distinct proven-behavior claims (of 34 extracted) across 18 symbols. 0 symbols have an explicit error-path proof; **10 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `bundle_resolved()`  (happy, invariant; EXTRACTED)
- [happy] bundle_resolved('vox-base') returns an empty list of plugins  (crates/vox-plugin-catalog/tests/bundle_resolution.rs)
- [happy] bundle_resolved('vox-fullstack') resolves to exactly 9 plugins  (crates/vox-plugin-catalog/tests/bundle_resolution.rs)
- [happy] bundle_resolved('vox-fullstack') includes plugin with id 'skill-compiler'  (crates/vox-plugin-catalog/tests/bundle_resolution.rs)
- [happy] bundle_resolved('vox-fullstack') includes plugin with id 'runtime-wasm'  (crates/vox-plugin-catalog/tests/bundle_resolution.rs)
- [happy] bundle_resolved('vox-ml') resolves to exactly 11 plugins through extends chain  (crates/vox-plugin-catalog/tests/bundle_resolution.rs)
- [happy] bundle_resolved('vox-ml') includes plugin with id 'mens-candle-cuda'  (crates/vox-plugin-catalog/tests/bundle_resolution.rs)
- [happy] bundle_resolved('vox-ml') includes plugin with id 'nvml-probe'  (crates/vox-plugin-catalog/tests/bundle_resolution.rs)
- [invariant] bundle_resolved('vox-dev') deduplicates plugins appearing in multiple extends chains, ensuring skill-orchestrator appears exactly once  (crates/vox-plugin-catalog/tests/bundle_resolution.rs)

### `all_plugins()`  (happy, invariant; EXTRACTED)
- [happy] all_plugins() returns a non-empty list  (crates/vox-plugin-catalog/tests/catalog_load.rs)
- [happy] all_plugins() includes a plugin with id 'mens-candle-cuda'  (crates/vox-plugin-catalog/tests/catalog_load.rs)
- [happy] all_plugins() contains all expected code and composite plugins: nvml-probe, mens-candle-cuda, mens-candle-metal, oratio, cloud, populi-mesh, webhook, browser, runtime-wasm, runtime-container, publication  (crates/vox-plugin-catalog/tests/catalog_load.rs)
- [happy] all_plugins() contains all expected skill plugins: skill-compiler, skill-git, skill-memory, skill-orchestrator, skill-rag, skill-testing, skill-testing-validate, skill-v0  (crates/vox-plugin-catalog/tests/catalog_load.rs)
- [invariant] all_plugins() returns a set where every plugin id is unique with no duplicates  (crates/vox-plugin-catalog/tests/catalog_validation.rs)

### `PluginCatalogEntry.status`  (happy; EXTRACTED)
- [happy] Plugin 'webhook' has status field set to CatalogStatus::Stable  (crates/vox-plugin-catalog/tests/catalog_load.rs)
- [happy] Plugin 'cloud' has status field set to CatalogStatus::Alpha  (crates/vox-plugin-catalog/tests/catalog_load.rs)
- [happy] Plugin 'skill-v0' has status field set to CatalogStatus::Deprecated  (crates/vox-plugin-catalog/tests/catalog_load.rs)

### `all_bundles()`  (happy; EXTRACTED)
- [happy] all_bundles() returns a non-empty list  (crates/vox-plugin-catalog/tests/catalog_load.rs)
- [happy] all_bundles() includes a bundle with id 'vox-base'  (crates/vox-plugin-catalog/tests/catalog_load.rs)
- [happy] all_bundles() contains all expected bundles: vox-base, vox-fullstack, vox-ml, vox-ml-metal, vox-mesh, vox-server, vox-edge, vox-cloud-only, vox-dev, vox-mobile  (crates/vox-plugin-catalog/tests/catalog_load.rs)

### `PluginCatalogEntry`  (happy; EXTRACTED)
- [happy] PluginCatalogEntry deserializes from TOML with correct id, PayloadKind::Code payload_kind, and extension_points  (crates/vox-plugin-catalog/tests/schema_roundtrip.rs)
- [happy] PluginCatalogEntry deserializes from TOML with PayloadKind::Skill payload_kind and exposes_tools  (crates/vox-plugin-catalog/tests/schema_roundtrip.rs)

### `BundleEntry`  (happy; EXTRACTED)
- [happy] BundleEntry deserializes from TOML with extends field and plugin list preserved  (crates/vox-plugin-catalog/tests/schema_roundtrip.rs)

### `BundleEntry::extends`  (happy; EXTRACTED)
- [happy] BundleEntry deserializes from TOML with an extends field containing the referenced bundle name  (crates/vox-plugin-catalog/tests/schema_roundtrip.rs)

### `BundleEntry::plugins`  (happy; EXTRACTED)
- [happy] BundleEntry deserializes from TOML with a plugins field containing the correct number of plugin entries  (crates/vox-plugin-catalog/tests/schema_roundtrip.rs)

### `Plugin::default_source`  (invariant; EXTRACTED)
- [invariant] Every plugin has a non-empty default_source field  (crates/vox-plugin-catalog/tests/catalog_validation.rs)

### `Plugin::exposes_tools (Skill/Composite)`  (invariant; EXTRACTED)
- [invariant] Skill and Composite plugins have non-empty exposes_tools field  (crates/vox-plugin-catalog/tests/catalog_validation.rs)

### `Plugin::extension_points (Code/Composite)`  (invariant; EXTRACTED)
- [invariant] Code and Composite plugins have non-empty extension_points field  (crates/vox-plugin-catalog/tests/catalog_validation.rs)

### `PluginCatalogEntry.bundled_in`  (invariant; EXTRACTED)
- [invariant] Every plugin's bundled_in field references only bundle ids that exist in all_bundles()  (crates/vox-plugin-catalog/tests/catalog_validation.rs)

### `all_components`  (happy; EXTRACTED)
- [happy] all_components contains a 'gui' component with binary='vox-gui', non-empty description, and requires.os covering windows, macos, and linux  (crates/vox-plugin-catalog/tests/component_load.rs)

### `all_plugins`  (invariant; EXTRACTED)
- [invariant] all_plugins() returns no plugin with id='gui'  (crates/vox-plugin-catalog/tests/component_load.rs)

### `bundle_resolved`  (invariant; EXTRACTED)
- [invariant] For each plugin's bundled_in reference, that plugin appears in the resolved bundle's plugins  (crates/vox-plugin-catalog/tests/catalog_validation.rs)

### `render_bundles_md`  (happy; EXTRACTED)
- [happy] render_bundles_md output contains every bundle id and contains 'plugins' string  (crates/vox-plugin-catalog/tests/docs_generation.rs)

### `render_catalog_md`  (happy; EXTRACTED)
- [happy] render_catalog_md output contains every plugin id, and contains 'payload-kind' and 'default-source' strings  (crates/vox-plugin-catalog/tests/docs_generation.rs)

### `render_catalog_md and render_bundles_md`  (happy; EXTRACTED)
- [happy] Both render_catalog_md and render_bundles_md output start with '---\n' frontmatter and contain '<!-- AUTOGENERATED' marker  (crates/vox-plugin-catalog/tests/docs_generation.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`BundleEntry`** — only: _BundleEntry deserializes from TOML with extends field and plugin list preserved_
- **`BundleEntry::extends`** — only: _BundleEntry deserializes from TOML with an extends field containing the referenced bundle name_
- **`BundleEntry::plugins`** — only: _BundleEntry deserializes from TOML with a plugins field containing the correct number of plugin entries_
- **`PluginCatalogEntry`** — only: _PluginCatalogEntry deserializes from TOML with correct id, PayloadKind::Code payload_kind, and extension_points_
- **`PluginCatalogEntry.status`** — only: _Plugin 'webhook' has status field set to CatalogStatus::Stable_
- **`all_bundles()`** — only: _all_bundles() returns a non-empty list_
- **`all_components`** — only: _all_components contains a 'gui' component with binary='vox-gui', non-empty description, and requires.os covering windows, macos, and linux_
- **`render_bundles_md`** — only: _render_bundles_md output contains every bundle id and contains 'plugins' string_
- **`render_catalog_md`** — only: _render_catalog_md output contains every plugin id, and contains 'payload-kind' and 'default-source' strings_
- **`render_catalog_md and render_bundles_md`** — only: _Both render_catalog_md and render_bundles_md output start with '---\n' frontmatter and contain '<!-- AUTOGENERATED' marker_
