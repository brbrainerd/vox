# Plugin Codebase — Complete Single-Source-of-Truth Scoping (2026-06-07)

## Goal

The SDK work (PRs #184–#199) established **one** SSOT: the extension-point *surface* is
derived from the `#[sabi_trait]` traits into `contracts/plugin/extension-points.v1.yaml`,
gated by `plugin-surface-sync`. This document scopes extending that discipline across the
**entire** plugin codebase so that **every plugin datum has exactly one authoring site**,
and everything else either derives from it or is gate-validated against it.

The plugin system today has **strong semantic gates** (ABI load check, dep-boundary,
cdylib isolation, surface parity) but **weak identity SSOT**: the same fact is hand-written
in three or four places, mostly with no cross-check.

---

## The duplication map (verified)

Per first-party plugin, each datum is independently declared at these sites:

| Datum | Plugin.toml | crate `Cargo.toml` | `declare_plugin!` (lib.rs) | `impl VoxPlugin` (lib.rs) | `catalog.toml` | Cross-checked today? |
|-------|:-:|:-:|:-:|:-:|:-:|---|
| **id** | ✓ `[plugin] id` | — | ✓ `id:` | ✓ `fn id()` | ✓ entry id | only id↔catalog membership (`plugin-catalog-parity`) |
| **version** | ✓ `[plugin] version` (`0.1.0`) | ✓ `version.workspace` (`0.6.0`) | ✓ `version:` (`0.1.0`) | — | — | **no** — and the two namespaces *disagree* |
| **name** | ✓ `[plugin] name` | — (crate name) | — | — | — | n/a |
| **description** | ✓ `[plugin] description` | ✓ `[package] description` (*different text*) | — | — | ✓ `description` | **no** |
| **status** | ✓ `[plugin] status` | — | — | — | ✓ `status` | **no** |
| **abi-version** | ✓ literal `12` | — | implicit (`VOX_PLUGIN_ABI_VERSION`) | — | — | **no** (load check compares plugin-vs-host, not manifest-literal-vs-const) |
| **extension-points** | ✓ `provides.extension-points` | — | — | ✓ `as_*` accessors | ✓ `extension-points` | manifest↔SSOT (`plugin-surface-sync`/SP-6); **not** manifest↔impl, **not** manifest↔catalog |
| **exposes-tools** | ✓ `tools.exposes` | — | — | — | ✓ `exposes-tools` (+ `SKILL.md` frontmatter `vox-tools`) | **no** (3-way) |
| **artifacts** (lib filenames) | ✓ per-platform literals | — (Cargo `cdylib` default) | — | — | — | **no** — mechanically derivable, hand-written |
| **target-triple keys** | ✓ artifact map keys | — | — | — | — | re-hardcoded in host `current_target_triple_key()` *and* `plugin-abi-parity` |

**Canonical constants that are re-typed instead of referenced:**
- `VOX_PLUGIN_ABI_VERSION = 12` (`crates/vox-plugin-api/src/lib.rs:13`) — duplicated as a literal
  in every code/composite `Plugin.toml` `abi-version`, with **no gate** verifying they agree.
- Target-triple key list (`windows-x86_64`, `linux-x86_64`, `macos-aarch64`, …) — defined in
  `vox-plugin-host::current_target_triple_key()` (`src/lib.rs:112`), re-implemented in
  `plugin_abi_parity.rs:54`, and re-typed in every `Plugin.toml` artifact map.
- Artifact lib-name rule `<prefix>vox_plugin_<id_underscored><ext>` — implicit in Cargo's
  cdylib output, re-stated by hand in every manifest, never centrally defined.

---

## Design principle

Assign **one authoring site** per datum, by *who is authoritative at runtime*:

- The **host reads `Plugin.toml` from disk** at load/discovery time and never consults the
  catalog or the crate's `Cargo.toml`. So for per-plugin identity, **`Plugin.toml` is the
  runtime SSOT** — everything else (the `declare_plugin!` args, the crate description, the
  catalog row, the artifact filenames) should *derive from* or be *validated against* it.
- For workspace-wide facts (ABI version, the legal triple set, the lib-name rule), the
  **Rust constant / one shared function is the SSOT**, and `Plugin.toml` literals must be
  *generated* or *gate-validated* against it.
- The **catalog** keeps only what is genuinely catalog-unique (bundle membership,
  `default-source`, `requires-tag`); its per-plugin echo fields become *derived*.

Then: a datum with two hand-edited copies is a bug waiting to happen → either collapse to
one copy, generate the second, or add a parity gate. Prefer **generate** > **gate** >
**leave duplicated**, in that order.

---

## Per-datum target state

### 1. id — collapse code copies into the manifest
- **SSOT:** `Plugin.toml [plugin] id`.
- **Change:** `declare_plugin!` reads the manifest at compile time
  (`include_str!("../Plugin.toml")`, parse `id`/`version`) instead of taking them as args —
  so `manifest_json` and the macro stop re-stating id/version. The `impl VoxPlugin::id()`
  can return the embedded id too (or stay, gate-checked).
- **Gate:** extend `plugin-catalog-parity` (already checks id membership) to also assert the
  `declare_plugin!`/`impl id()` strings == the manifest id (a cheap regex parity, same shape
  as `plugin-surface`'s accessor check).
- **Net:** id authored once (manifest); macro + impl derive or are gate-pinned.

### 2. version — **needs a maintainer decision** (see Decisions)
- Today `Plugin.toml` says `0.1.0` while the crate is workspace `0.6.0`. Pick a model:
  - **(a) Plugin product version, independent of the monorepo** → keep `0.1.0` in the
    manifest as SSOT; `declare_plugin!` reads it; add a gate that the crate `Cargo.toml`
    does *not* claim to be the plugin version (or set `version = "0.1.0"` explicitly).
  - **(b) Tie plugin version to the workspace** → generate `Plugin.toml version` from
    `workspace.package.version` (drop the `0.1.0` literal), like `gui-version-sync` does.
- Either way: **one** authoring site, the other generated.

### 3. description / name / status — manifest-authoritative, catalog + Cargo derived
- **SSOT:** `Plugin.toml`.
- **Change:** generate the crate `Cargo.toml [package] description` and the `catalog.toml`
  per-plugin `description`/`status` from the manifest (or gate them equal). Today they are
  three independent strings.

### 4. abi-version — const-authoritative, manifest generated/gated
- **SSOT:** `VOX_PLUGIN_ABI_VERSION`.
- **Change (cheap, high-value):** new `plugin-abi-version-parity` check (or fold into
  `plugin-surface-sync`): every code/composite `Plugin.toml` `abi-version` == the const.
  Closes a silent gap — a stale manifest literal would only be caught at load time, per
  platform, today.

### 5. extension-points — close the manifest↔impl gap
- **SSOT:** the extension traits (already → `extension-points.v1.yaml`).
- **Existing:** manifest `provides` ⊆ SSOT names (SP-6); accessor↔module parity (SP-3).
- **Missing:** **manifest `provides` ↔ the plugin's actual `impl VoxPlugin` `as_*` set.** A
  plugin can declare `provides=["HardwareProbe"]` yet not implement `as_hardware_probe` (or
  vice-versa) — undetected. Add: for each plugin crate, parse its `impl VoxPlugin` `as_*`
  methods, map to extension-point names via the SSOT `accessor`→`name` table, and assert
  equality with its manifest `provides`. Also assert catalog `extension-points` == manifest.

### 6. exposes-tools — one list, referenced 3×
- Today: `catalog.toml exposes-tools`, `Plugin.toml tools.exposes`, and `SKILL.md`
  frontmatter `vox-tools`. **SSOT:** `Plugin.toml tools.exposes`.
- **Change:** `plugin-skill-parity` (exists) gains a parity assertion that all three lists
  are equal; or generate the catalog + SKILL frontmatter list from the manifest.

### 7. target-triple keys — one canonical set
- **SSOT:** a single `pub const PLUGIN_TARGET_TRIPLES: &[&str]` (+ the `current` resolver)
  in `vox-plugin-types` (the no-FFI types crate).
- **Change:** `vox-plugin-host` and `plugin-abi-parity` both import it (delete the duplicate
  `cfg!` ladder in the gate). Add a gate: every `Plugin.toml` artifact-map key ∈ the set.

### 8. artifacts — derive, don't hand-write
- **SSOT:** id + the triple set + the lib-name rule.
- **Change:** one `artifact_filename(id, triple)` function (in `vox-plugin-types`), used by
  the host discovery, release packaging, and a generator. `vox plugin scaffold` already
  needs this; today it hand-writes literals. Either **generate** the `[artifacts]` table
  into `Plugin.toml` (a sync command) or **gate** that the literals match the rule.

### 9. catalog — derive per-plugin rows from manifests
- **SSOT split:** the per-plugin echo fields (id, description, status, extension-points,
  exposes-tools, payload-kind) derive from the manifests; catalog keeps only
  `default-source`, `requires-tag`, `bundled-in`, and the **bundle** definitions
  (which have no per-plugin equivalent).
- **Change:** a `vox ci plugin-catalog-sync [--write]` that regenerates the per-plugin block
  of `catalog.toml` by scanning `crates/vox-plugin-*/Plugin.toml`, merging the hand-authored
  bundle/source block. Verify-mode into `ssot-drift`. (`generate-plugin-catalog-docs`
  already proves the catalog→docs direction; this adds manifests→catalog.)

---

## Phasing (each a small, independently-mergeable PR)

- **PS-1 — abi-version parity** *(cheapest, highest safety)*: gate every `Plugin.toml`
  `abi-version` == `VOX_PLUGIN_ABI_VERSION`; fold into `plugin-surface-sync`. (~1 gate fn)
- **PS-2 — canonical triple set + artifact rule** in `vox-plugin-types`; host + parity gate
  import it; new gate: manifest artifact keys ∈ set and filenames == rule. Deletes 2
  duplicated triple ladders.
- **PS-3 — manifest↔impl extension-point parity**: parse each plugin's `impl VoxPlugin`
  `as_*`, assert == its `provides` (+ catalog `extension-points`). Closes the last
  extension-point drift seam.
- **PS-4 — id/version into the manifest**: `declare_plugin!` reads `include_str!(Plugin.toml)`
  for id/version (stop taking them as args); resolves the **version-model decision** first.
- **PS-5 — catalog derived from manifests**: `plugin-catalog-sync [--write]` for the
  per-plugin block; description/status/extension-points/exposes-tools become generated;
  verify in `ssot-drift`.
- **PS-6 — exposes-tools 3-way parity** (catalog ↔ manifest ↔ SKILL frontmatter), or
  generate the two echoes from the manifest.

Recommended order: **PS-1 → PS-2 → PS-3** (pure gates, no behavior change, immediate drift
protection) → **PS-4** (needs the version decision; touches the macro) → **PS-5 → PS-6**
(catalog/skill generation). PS-1..PS-3 are each ≈ the size of SP-3.

---

## What stays hand-authored (the irreducible SSOTs)

- The extension **traits** (`extensions/*.rs`) and the `VoxPlugin` accessors (`abi.rs`).
- `VOX_PLUGIN_ABI_VERSION` / `_MIN_SUPPORTED` (one const pair).
- Each plugin's **`Plugin.toml`** identity block + the actual trait impls (the real code).
- The catalog's **bundle** definitions + per-plugin `default-source` / `requires-tag`.
- `SKILL.md` bodies (prose).

Everything else derives from or is gated against those.

## Net effect

After PS-1..PS-6, every plugin datum has exactly one authoring site, and adding/renaming a
plugin, an extension point, a tool, or an ABI bump is a **regenerate-or-red-build**
operation — never a "remember to update five files" operation. This is the same guarantee
the SDK plan delivered for the extension surface, extended to identity, constants, the
catalog, and the skill surface.

---

## Decisions needed before PS-4 / PS-5

1. **Plugin versioning model** — independent product version (`Plugin.toml` is SSOT,
   crate `Cargo.toml` decoupled) **vs** tied to the workspace version (generate the manifest
   version from `workspace.package.version`). This blocks PS-4.
2. **Catalog derivation direction** — generate the per-plugin catalog rows *from* manifests
   (PS-5 as written) **vs** keep the catalog hand-authored and only add strict parity gates.
   Generation removes the duplication entirely; gating is less invasive but keeps two copies.
3. **Scope of `declare_plugin!` change** — have it embed/parse `Plugin.toml`
   (`include_str!`) to kill the id/version macro args, **vs** leave the macro args and just
   add a parity gate. Embedding is the true SSOT; the gate is lower-risk.
