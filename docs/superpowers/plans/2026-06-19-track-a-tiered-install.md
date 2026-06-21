---
title: "Track A — Tiered Install + Enable"
description: "Wire voxup install <tier> to the distribution SSOT, add vox doctor per-tier dep surfacing, and commit the vox-langtool minimal binary seam."
category: "Architecture SSOTs"
status: "in_progress"
---

# Track A — Tiered Install + Enable

> **Executor:** Claude Sonnet 4.6  
> **Plan date:** 2026-06-19  
> **Depends on:** Track 0 (`contracts/distribution/profiles.v1.yaml` + `crates/voxup/src/profiles.rs` present)  
> **Operates on branch:** `claude/crate-build-spine-hardening`  
> **Ledger entry:** AGH-0018 (append when done)

## Scope

Track A makes the distribution SSOT *load-bearing* for the install + diagnostics path:

1. `voxup install <tier>` validates the requested tier against the SSOT, prints the tier description + what will be installed, and errors with the valid tier list on unknown input.
2. `vox doctor` gains a per-tier runtime-optional dep check: reports which optional deps for the active tier are present/missing, with actionable install hints.
3. `vox-langtool` minimal binary seam: investigate the stray-branch prototype and commit the Cargo.toml seam (not the full implementation — just the manifest so the minimal tier is well-defined).

PATH automation is **already complete** in `shell.rs` (bash/zsh/fish/PowerShell + Windows registry). No changes needed there.

## Design notes

### Embedding the SSOT in voxup (compile-time)

`run_install` runs on a user machine without the repo. Embed at build time:
```rust
const PROFILES_YAML: &str = include_str!("../../../contracts/distribution/profiles.v1.yaml");
```
Path from `crates/voxup/src/`: up 3 levels = workspace root, then down.

### vox doctor tier-deps check

`vox-cli` can use the same embed approach for its doctor check. The doctor check module
will embed the SSOT and check for each `runtime_optional` dep in the requested tier:
- `agy` → `which agy` / `where agy`
- `model-weights` → check `~/.vox/models/` has at least one model file
- `plugins` → check `~/.vox/plugins/` directory exists

The `--tier <name>` flag selects which tier's optional deps to report. Defaults to
reporting full-tier deps (surface the widest set).

### vox-langtool seam

The worktree at `.claude/worktrees/agent-a209e4350e8b85c16/crates/vox-langtool/`
has the prototype. For Track A, we cherry-pick only the `Cargo.toml` seam — a minimal-binary
placeholder crate that compiles clean and is registered in the workspace. Full
implementation (the actual lean binary) is Track A follow-up.

## Tasks

### Task 1 — Test: unknown tier errors with valid tier list (RED)

**File:** `crates/voxup/tests/tier_validation.rs`

Write a test that calls a new `install::validate_tier` helper with "bogus" and asserts the
returned error contains "minimal", "default", and "full" (the valid tiers from the SSOT).

Commit message: `test(voxup): RED — unknown tier validation against SSOT`

### Task 2 — Implement: embed SSOT + `validate_tier` helper (GREEN)

**Files:** `crates/voxup/src/install.rs`, `crates/voxup/src/profiles.rs`

- Add `pub const PROFILES_YAML: &str = include_str!("../../../contracts/distribution/profiles.v1.yaml");` to `profiles.rs`.
- Add `pub fn validate_tier(profiles_yaml: &str, tier: &str) -> Result<(), String>` to `install.rs` (returns `Err` with valid tier list on unknown tier; `Ok` on known).
- Call `validate_tier(PROFILES_YAML, profile)` early in `run_install`, before the network call.
- Print tier description on success: `"Installing Vox ({tier}) — {description}"`.

Commit message: `feat(voxup): embed distribution SSOT + tier validation in run_install`

### Task 3 — Test: tier description printed (RED + GREEN inline)

**File:** `crates/voxup/src/install.rs` (unit test)

Add a test that `validate_tier(PROFILES_YAML, "minimal")` → `Ok(())` and
`validate_tier(PROFILES_YAML, "full")` → `Ok(())`.
These should pass with Task 2's implementation.

Commit message: `test(voxup): tier validation happy-path tests`

### Task 4 — Test: doctor tier-deps check (RED)

**File:** `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/tier_deps.rs` (new)

Write a unit test that calls a new `check_runtime_optional_deps(tier: &Tier) -> Vec<DepStatus>`
function (where `DepStatus` is a new type with `{name, present, hint}`). The test verifies:
- For a `Tier` with `runtime_optional: []` → returns empty vec.
- For a `Tier` with `runtime_optional: ["fake-tool-that-does-not-exist-9999"]` → returns one entry
  with `present: false`.

Commit message: `test(doctor): RED — tier runtime-optional dep surfacing`

### Task 5 — Implement: `tier_deps` check module (GREEN)

**Files:** `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/tier_deps.rs` (new),
`crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/mod.rs`

Implement `check_runtime_optional_deps`:
- For each dep name in `tier.runtime_optional`, check if a binary named `dep` is on PATH
  (`which`/`where`). Special case `model-weights` (not a binary — check `~/.vox/models/`).
  Special case `plugins` (check `~/.vox/plugins/` exists).
- Return `Vec<DepStatus>` with `{name: String, present: bool, hint: String}`.

Wire into `run_checks` with a new embed for the SSOT:
```rust
const PROFILES_YAML: &str = include_str!("../../../../../../contracts/distribution/profiles.v1.yaml");
```

The check defaults to the "full" tier (surface the widest set); with `--tier` wired in Task 6
it becomes dynamic.

Register the check as a new `Check` entry in `run_checks`. Show as "tier deps (full)" or similar.

Commit message: `feat(doctor): tier runtime-optional dep surfacing from distribution SSOT`

### Task 6 — Wire `--tier` into `vox doctor` CLI

**Files:** `crates/vox-cli/src/cli_args.rs`, `crates/vox-cli/src/commands/diagnostics/doctor/mod.rs`

Add `--tier <name>` flag to `vox doctor` (optional, default "full"). Pass it through
`run()` → `run_checks()` → the `tier_deps` check.

Commit message: `feat(doctor): --tier flag selects which tier's runtime deps to surface`

### Task 7 — Update distribution-parity CI to capture Track A

The existing `.github/workflows/distribution-parity.yml` already tests voxup.
No new workflow needed — just add the tier_deps path trigger pattern to the existing workflow's
`paths:` list and verify the workflow still passes.

Commit message: `ci(dist): add vox-cli tier-deps path to distribution-parity trigger`

## Verification commands

```bash
cargo test -p voxup > test-out.txt 2>&1
cargo test -p vox-cli --lib -- doctor::checks_standard::tier_deps > test-out.txt 2>&1
cargo fmt -p voxup -- --check
cargo fmt -p vox-cli -- --check
cargo clippy -p voxup -- -D warnings
cargo clippy -p vox-cli --exclude vox-gui -- -D warnings
```

## Definition of done

- [ ] All Task 1–7 committed with exact messages above.
- [ ] `cargo test -p voxup` green (≥ 10 tests including new tier validation).
- [ ] `voxup install bogus` would error with valid tier list (manual trace or integration test).
- [ ] `cargo test -p vox-cli --lib -- doctor` green.
- [ ] Clippy and fmt clean on both crates.
- [ ] Ledger entry AGH-0018 filled.
