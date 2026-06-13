# Semantic behavior map — `vox-plugin-types`

The `vox-plugin-types` crate's `target.rs` is the single source of truth for plugin target-triple keys and cdylib artifact-filename derivation. Six extracted Behavior claims cover its two public functions. After dedup they collapse to **2 symbols**: `current_target_triple()` (1 happy + 1 invariant claim) and `plugin_artifact_filename()` (3 happy claims across the three OS families + 1 error claim). Coverage is solid for the artifact deriver — it has both a multi-platform happy path and an explicit rejection path. The thinner surface is `current_target_triple()`, whose `None` branch is documented but unproven.

## `current_target_triple()`
File: `crates/vox-plugin-types/src/target.rs`

Distinct proven behaviors:
- **Happy:** returns `Some(_)` on supported CI platforms (win/linux/mac × x64/arm64).
- **Invariant:** the returned value, when `Some`, is always a member of `PLUGIN_TARGET_TRIPLES`.

Proof coverage: happy = yes, invariant = yes, error/None branch = **no**.

The `None` arm (unsupported platform, e.g. `solaris`/`sparc`) is documented in the contract but cannot be exercised from a single CI host — an inherent, not negligent, gap.

## `plugin_artifact_filename()`
File: `crates/vox-plugin-types/src/target.rs`

Distinct proven behaviors:
- **Happy (Windows):** `("vox-plugin-nvml-probe", "windows-x86_64")` → `vox_plugin_nvml_probe.dll` (no prefix, `.dll`).
- **Happy (Linux):** `("vox-plugin-nvml-probe", "linux-x86_64")` → `libvox_plugin_nvml_probe.so` (`lib` prefix, `.so`).
- **Happy (macOS):** `("vox-plugin-speech", "macos-aarch64")` → `libvox_plugin_speech.dylib` (`lib` prefix, `.dylib`).
- **Error/reject:** returns `None` for a triple outside `PLUGIN_TARGET_TRIPLES` (proven with `solaris-sparc`).

Proof coverage: happy = yes (all 3 OS families), error/reject = yes, edge/invariant = **partial**.

The function performs `crate_name.replace('-', '_')` normalization. Hyphenated inputs exercise this incidentally, but no test isolates the normalization rule (multiple hyphens, already-underscored names). The aarch64 vs x86_64 variants within each OS map to the same `(prefix, ext)`; only one arch per OS is asserted, so the win-aarch64 / linux-aarch64 arms are reasoned-but-untested.

## Semantic gaps

The crate is in good shape: the validator-like `plugin_artifact_filename()` *does* have a rejection test, which is the usual blind spot. Remaining, lower-severity gaps:

1. **`current_target_triple()` — no `None`/unsupported-platform proof.** The documented failure mode (platform Vox plugins don't target) is unverified. Inherently hard on CI; consider extracting a pure, triple-string-input helper that the public `cfg!`-based wrapper delegates to, so the `None` arm becomes unit-testable.
2. **`plugin_artifact_filename()` — hyphen normalization not asserted in isolation.** `replace('-', '_')` is load-bearing for correct cdylib names but only tested via incidental hyphenated inputs. Add an edge test for multi-hyphen / already-underscored crate names.
3. **Untested arch arms.** `windows-aarch64`, `linux-aarch64`, and `macos-x86_64` share match arms with their tested siblings but have no direct assertion — fine for now, worth a cheap parametric sweep over all six `PLUGIN_TARGET_TRIPLES` entries.

None of these are integrity/security surfaces; the most actionable is gap #1 (refactor for testability of the documented `None` contract).