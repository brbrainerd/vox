# Plugin SDK + ABI Sync — Implementation Plan (2026-06-06)

## Thesis

The hard question is not "make a `#[vox_plugin]` macro." It is: **how does the plugin
SDK stay correct as Vox the language and compiler evolve, without anyone hand-rewriting
it?** The answer is that Vox already has the two ingredients that make this tractable, and
the SDK's job is to *sit entirely on top of them* and add a parity gate:

1. **A stable C-ABI boundary** (`abi_stable`, `VOX_PLUGIN_ABI_VERSION = 12`) — plugins
   link against `repr(C)` trait objects, **never** against compiler internals, the AST,
   the type checker, or codegen. Language changes that don't touch the *extension traits*
   are invisible to plugins by construction.
2. **An SSOT → generate → CI-gate pipeline** (`contracts/operations/catalog.v1.yaml` →
   `vox ci operations-sync` → `ssot-drift`) — the project's proven pattern for keeping
   derived artifacts from drifting from a single source.

So the design rule is: **the SDK is generated/derived, never the source of truth; the
source of truth is the extension-trait set in `vox-plugin-api`; a CI gate fails the build
the moment the SDK, the host, the manifests, and the docs disagree.** When Vox changes, a
contributor runs one regenerate command (or CI tells them to), rather than re-deriving the
plugin surface by hand.

---

## Current state (verified)

| Piece | Location | Status |
|-------|----------|--------|
| ABI version constant | `crates/vox-plugin-api/src/lib.rs:11` (`VOX_PLUGIN_ABI_VERSION: u32 = 12`) | exists |
| Root module / FFI | `crates/vox-plugin-api/src/abi.rs` (`VoxPluginRoot`, `#[sabi_trait] VoxPlugin`) | exists |
| Extension points (12) | `crates/vox-plugin-api/src/extensions/*.rs`, each with a `*_REVISION: u32` | exists |
| Host callback | `crates/vox-plugin-api/src/host.rs` (`#[sabi_trait] VoxHost`) | exists |
| Pure-types surface | `crates/vox-plugin-types` (manifests, no abi_stable) | exists |
| Host loader + ABI check | `crates/vox-plugin-host/src/loader.rs:46` (exact-equality, fatal on mismatch) | exists |
| Manifest schema | `crates/vox-plugin-types/src/plugin_manifest.rs`; `Plugin.toml` (`abi-version`, `artifacts`, `provides`) | exists |
| First-party catalog | `crates/vox-plugin-catalog` (SSOT `catalog.toml` + build.rs validation) | exists |
| **Per-plugin boilerplate** | `#[export_root_module]` + `manifest_json()` + `init()` + `_TO::from_value(..)` accessors (~40-50 lines/plugin) | **hand-written** |
| `plugin-abi-parity` gate | `crates/vox-cli/src/commands/ci/plugin_abi_parity.rs` | **exists but NOT in `.github/workflows/ci.yml`** |
| ABI compatibility | exact integer match only | **no ranges / negotiation** |
| SDK crate | — | **does not exist** |
| `vox plugin new` scaffold | — | **does not exist** |

Key gaps for the sync story: (a) no SDK to absorb boilerplate, (b) the parity gate isn't
enforced in CI, (c) ABI compatibility is brittle exact-match (every host ABI bump forces a
rebuild of every plugin even when nothing relevant changed), (d) nothing asserts the SDK's
generated surface matches `vox-plugin-api`.

---

## Design principles (the "no rewrite" contract)

1. **Source of truth = the extension traits in `vox-plugin-api`.** The SDK, the
   `Plugin.toml` `provides.extension-points` vocabulary, the host's accessor set, and the
   docs are all *derived from or gated against* that set. Add an extension point in one
   place; everything else either regenerates or fails the gate with a clear fix command.
2. **The SDK is a thin, macro-driven crate that re-exports `vox-plugin-api`.** It adds
   zero new ABI surface. A plugin's only Vox dependency is `vox-plugin-sdk`, which
   re-exports `abi_stable` + the API. Because the `#[vox_plugin]` macro expands *against
   whatever `vox-plugin-api` is in scope at compile time*, new extension points and trait
   methods are picked up on recompile — no hand-edits.
3. **Version the SDK to the ABI, not the language.** `vox-plugin-sdk` carries its own
   semver tied to `VOX_PLUGIN_ABI_VERSION` (SDK `12.x` ⇄ ABI 12). Workspace `0.6.0` and
   language features churn freely underneath without forcing SDK changes.
4. **Make ABI compatibility a *range*, not a point.** Hosts advertise `[min_abi, max_abi]`;
   plugins record the ABI they were built against. A plugin loads if its ABI ∈ host range.
   Most language changes never touch the ABI, so the range stays wide and plugins keep
   working across many Vox releases.
5. **Gate everything in CI.** Wire `plugin-abi-parity` into `ci.yml`, and add an
   `sdk-abi-parity` check that fails if the SDK's generated accessor list ≠ the API's
   extension-point set. Drift becomes a red build with a one-line remedy, not a latent bug.

---

## The `vox-plugin-sdk` crate

New crate `crates/vox-plugin-sdk` (L1, depends only on `vox-plugin-api` + `abi_stable`;
publish-clean per the [leaf-publish work](../../../crates/vox-plugin-api/Cargo.toml)).

```rust
// What a plugin author writes — the ENTIRE glue, replacing ~45 lines:
use vox_plugin_sdk::prelude::*;

#[vox_plugin(id = "oratio", manifest = "Plugin.toml")]
struct SpeechPlugin { /* state */ }

#[vox_plugin_impl]
impl SpeechPlugin {
    // Implement only the extension traits you provide; the macro discovers them,
    // generates the `as_speech_to_text()` / `as_audio_capture()` accessors, the
    // root-module export, the `manifest_json()` extern fn, and the ABI stamping.
    fn speech_to_text(&self) -> impl SpeechToText { /* ... */ }
    fn audio_capture(&self) -> impl AudioCapture { /* ... */ }
}
```

The macro emits exactly today's hand-written pattern (`#[export_root_module]`,
`VoxPluginRoot { abi_version: VOX_PLUGIN_ABI_VERSION, .. }.leak_into_prefix()`,
`#[sabi_extern_fn]` wrappers, `VoxPlugin_TO::from_value(.., TD_Opaque)`), so it is **purely
additive and ABI-neutral** — a hand-written plugin and a macro plugin produce byte-identical
exports. Crucially, the macro reads the available `as_*` accessors and `*_TO` types from
`vox-plugin-api` *at expansion time*, so when the API gains a 13th extension point, existing
plugins keep compiling and new ones can provide it without the macro changing.

Helper surface (also re-exported): `RResult`/`RString`/`RSlice` conversion helpers
(`into_rresult`, `rstr`, `pcm`), a `host()` accessor, and `manifest_json!()` that embeds
`Plugin.toml` at compile time via `include_str!` + validates it against the schema in a
`const`-eval/`build.rs` check.

---

## ABI compatibility ranges (kills most forced rebuilds)

Today: `loader.rs:46` does `plugin_abi != VOX_PLUGIN_ABI_VERSION → fatal`. Replace the point
check with a range:

- `vox-plugin-api` adds `VOX_PLUGIN_ABI_MIN_SUPPORTED: u32` alongside the current (treated as
  `MAX`). Host accepts `MIN ..= MAX`.
- Bumping the ABI is reclassified into two kinds:
  - **Additive** (new extension point, new *optional* trait method behind a revision bump):
    raise `MAX`, keep `MIN` — old plugins still load.
  - **Breaking** (changed/removed trait method signature, struct layout): raise *both* `MIN`
    and `MAX` — old plugins are rejected with a clear message. These should be rare and
    batched.
- The existing per-extension `*_REVISION` constants become the fine-grained signal: the host
  records which revision each accessor expects; a plugin built against an older revision of a
  *still-supported* ABI is still loadable (the host treats absent newer methods via the
  `#[sabi(missing_field(...))]` prefix mechanism abi_stable already provides).

Net: a Vox release that doesn't touch extension traits never moves `MIN`, so every existing
plugin binary keeps loading. That is the concrete mechanism by which the SDK/plugins "don't
get rewritten as the language changes."

---

## Sync pipeline (reuse, don't reinvent)

Mirror the `operations-sync` pattern. Introduce a generated descriptor of the extension
surface and gate it:

1. **Source**: `crates/vox-plugin-api/src/extensions/*.rs` (the traits + `*_REVISION`).
2. **Generator**: `vox ci plugin-surface-sync --write` emits
   `contracts/plugin/extension-points.v1.yaml` (one row per extension point: name, revision,
   accessor ident, method signatures hash). Built by a small extractor that parses the
   `extensions/mod.rs` module list + each `#[sabi_trait]` (syn-based, same approach as
   `vox-drift-check`'s TS extractor).
3. **Derived/gated**:
   - the `Plugin.toml` `provides.extension-points` enum vocabulary,
   - the SDK's generated accessor table (`sdk-abi-parity`),
   - the docs page `docs/src/reference/plugin-extension-points.generated.md`.
4. **Gate**: extend `ssot-drift` with `plugin-surface-verify` (fails if the YAML is stale vs
   the traits) and add `plugin-abi-parity` + `sdk-abi-parity` to `.github/workflows/ci.yml`.

This makes the extension-point set a first-class SSOT with the same "edit one file →
regenerate → CI enforces" guarantee the rest of the contracts already enjoy.

---

## `vox plugin new <id>` scaffold

A `commands::plugin::new` subcommand that scaffolds a publish-ready plugin crate:
`Cargo.toml` (`crate-type = ["cdylib","rlib"]`, `vox-plugin-sdk` dep, no `workspace-hack`),
a `Plugin.toml` pre-filled with the current `abi-version` + chosen extension points, and a
`src/lib.rs` using `#[vox_plugin]`. The chosen extension points are validated against
`contracts/plugin/extension-points.v1.yaml`, so the scaffold can never reference a
nonexistent point.

---

## Phased delivery

- **SP-1 — SDK crate + macro (the boilerplate win).** New `vox-plugin-sdk` with
  `#[vox_plugin]` / `#[vox_plugin_impl]` + conversion helpers + `manifest_json!`. Port the
  two real plugins (`vox-plugin-speech`, `vox-plugin-nvml-probe`) and the noop fixture to it;
  assert byte-identical exports (a `nm`/symbol diff test). *Done when both plugins build via
  the macro and `plugin-abi-parity` passes locally.* (difficulty 3)
- **SP-2 — Wire the parity gates into CI.** Add `plugin-abi-parity` to `ci.yml` (a job that
  builds the cdylibs then runs the gate). Requires deciding the runner story for the
  CUDA/Metal plugins (build CPU-only variants in CI; GPU plugins stay
  `target`-gated/skip-on-platform as the gate already supports). (difficulty 2, but unblocks
  the whole "drift = red build" guarantee)
- **SP-3 — Extension-surface SSOT + `plugin-surface-sync`.** The syn extractor →
  `extension-points.v1.yaml` → `sdk-abi-parity` + generated docs; fold `plugin-surface-verify`
  into `ssot-drift`. (difficulty 3)
- **SP-4 — ABI compatibility ranges.** Add `VOX_PLUGIN_ABI_MIN_SUPPORTED`; convert
  `loader.rs` to a range check; document the additive-vs-breaking bump policy; add a test
  that an "ABI = MIN" fixture still loads. (difficulty 3 — the actual resilience payoff)
- **SP-5 — `vox plugin new` scaffold.** (difficulty 2)
- **SP-6 (optional) — typed manifest codegen.** Generate the `Plugin.toml` deserializer +
  `provides` enum from the surface SSOT via the existing `typify` pipeline, so even the
  manifest types track the trait set automatically. (difficulty 2)

Recommended order: SP-1 → SP-2 → SP-4 → SP-3 → SP-5 → SP-6. (SP-4 before SP-3 because the
range check is the highest-leverage resilience item; SP-3 is the belt-and-suspenders gate.)

---

## The maintenance playbook (what happens when X changes)

| Change | Who does what | Plugin rebuild? |
|--------|---------------|-----------------|
| Language/compiler feature, no ABI-trait change | nothing — ABI traits untouched, `MIN` unmoved | **No** |
| Add a new extension point | edit `extensions/`, run `plugin-surface-sync --write`; `MAX`++ (additive) | No (old plugins load; new ones opt in) |
| Add an optional method to an existing trait (revision bump) | bump `*_REVISION`, `MAX`++ | No (missing-field prefix handles it) |
| Breaking change to a trait signature | `MIN`++ and `MAX`++; documented in a migration note | Yes (rare, batched) |
| SDK ergonomics improvement | edit `vox-plugin-sdk` only; macro re-expands | No |

The gate (`ssot-drift` + `plugin-abi-parity` + `sdk-abi-parity`) guarantees that any of these
that is done *incompletely* turns into a red build with the exact `vox ci ... --write` remedy,
so the surfaces cannot silently diverge.

---

## Risks

- **Macro expansion vs `abi_stable` internals.** `#[sabi_trait]` generates the `_TO` types the
  macro must reference; the macro must use the public generated idents only. Mitigation: the
  byte-identical-export test in SP-1 catches any divergence immediately.
- **CI build cost for cdylibs** (SP-2): building plugin cdylibs adds compile time. Mitigation:
  a dedicated `plugin-checks` job, CPU-only features, sccache (already in the runner image).
- **GPU plugins remain un-CI-able** (CUDA/Metal): the parity gate already skips
  no-artifact-for-platform; document that GPU plugin ABI is validated on the same Win+MSVC+CUDA
  box used for candle, not in cloud CI.
- **Over-coupling the SDK to the catalog**: keep `vox-plugin-sdk` dependency-free of
  `vox-plugin-catalog`/`vox-cli`; the SDK must be buildable by a third party off crates.io
  (the publish-clean leaf rule).

---

## Out of scope (separate efforts)

- Plugin marketplace / remote distribution beyond the existing `vox-plugin-catalog` +
  `default_source` (`github:` / `local:`).
- WASM-target plugins (today's ABI is native cdylib; a WASM ABI is a distinct boundary).
- Signing/sandboxing of third-party plugin binaries (security track).
