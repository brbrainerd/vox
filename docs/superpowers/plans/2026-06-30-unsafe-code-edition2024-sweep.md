# Edition-2024 `unsafe_code` Sweep — Runbook

**Goal:** Make `cargo clippy --workspace --all-targets --exclude vox-gui -- -D warnings <allow-list>` go green, which is currently blocked across the whole workspace.

**Root cause:** Edition 2024 made `std::env::{set_var,remove_var}` `unsafe`. The workspace pins `unsafe_code = "warn"` (`Cargo.toml:38`), so under `-D warnings` every env-mutation `unsafe {}` block that lacks an `#[allow(unsafe_code)]` is a hard error. Scope: **376 unsafe-env blocks across 93 files / ~40 crates**.

**Decision (keep the guardrail):** Do NOT set `unsafe_code = "allow"` workspace-wide — the lint also guards **~88 genuine FFI/pointer/`libc`/Windows-API unsafe sites** (`model.opaque as *const`, `mmap` safetensors, `libc::execve/setpriority`, `SetPriorityClass`) that must stay flagged. Instead, scope an allow to each env-mutating test module/file, matching the conventions **already in this codebase** (`vox-telemetry/src/config.rs:494`, `vox-test-harness/src/env_scratch.rs:4`). No new helper crate — `EnvScratch` already covers the scoped/RAII case.

## The convention (apply per file)

1. **Integration-test files** (`crates/*/tests/*.rs`) that mutate env: add a file-level inner attribute at the very top (after the leading `//!`/`//` comments, before the first `use`):
   ```rust
   // Rust 2024 made std::env::{set_var,remove_var} unsafe; mutated single-threaded.
   #![allow(unsafe_code)]
   ```
2. **`#[cfg(test)] mod tests` blocks** in `src/*.rs`: add `#[allow(unsafe_code)]` on the module — **but FIRST check** the module doesn't already have an inner `#![allow(... unsafe_code ...)]` (several do). Adding a duplicate triggers `clippy::duplicated_attributes` + `mixed_attributes_style` errors. Prefer extending an existing inner `#![allow(...)]` over adding an outer one.
3. **Production `src` sites** (rare — most are test-only): per-site `#[allow(unsafe_code)] // SAFETY: serialized with <lock>` on the `unsafe {}` block, matching `vox-config/src/env_parse.rs:276`. Do not file-level-allow a production file that also contains real FFI unsafe.

## The gotcha (learned the hard way)

Allows are placed **inconsistently** — inner `#![...]` inside a module, outer `#[...]`, or per-block. **Read the whole module/file before adding an allow**, or you'll create `duplicated_attributes`. This is why it is per-file work, not a `sed` pass.

## Per-crate loop

For each crate in the list below:
```bash
# 1. find env-mutation unsafe blocks lacking a nearby allow (read each hit's surrounding module)
rg -Un --multiline "unsafe\s*\{\s*(std::)?env::(set_var|remove_var)" crates/<crate>/
# 2. apply the convention above
# 3. verify with the EXACT gate command (must be green, 0 warnings):
cargo clippy -p <crate> --all-targets -- -D warnings \
  -A clippy::items_after_test_module -A clippy::collapsible_match \
  -A clippy::collapsible_if -A clippy::should_implement_trait \
  -A clippy::doc_overindented_list_items -A clippy::doc_lazy_continuation
# 4. commit per crate (small, reviewable, resumable)
```
Finally, run the full `--workspace --all-targets --exclude vox-gui` clippy to confirm green.

## Crates to sweep (~40; vox-telemetry DONE `fe34a70c37`)

vox-config, vox-cli, vox-ml-cli, vox-compiler, vox-actor-runtime, vox-gamify,
vox-config-derive, vox-plugin-speech, vox-lsp, vox-telemetry-otlp,
vox-plugin-populi-mesh, vox-integration-tests, vox-plugin-host, vox-speech,
vox-populi, vox-orchestrator, vox-repository, vox-db, vox-secrets,
vox-orchestrator-mcp, vox-publisher, vox-search, vox-scientia, vox-cli-tests,
vox-gui, vox-plugin-webhook, vox-test-harness (env_scratch.rs already has the
allow — verify only), vox-compiler tests, vox-lsp tests, vox-populi tests, …

(Full file list: `rg -Ul --multiline "unsafe\s*\{\s*(std::)?env::(set_var|remove_var)" crates/`.)

## Notes
- This is independent of the CI-runner-remediation work (commits `172605a408..5ca00d4af9` + `fe34a70c37`).
- Tracker chip: `task_4045e2f1`.
- Keep `unsafe_code = "warn"` — the sweep is additive allows, never a workspace-wide relaxation.
