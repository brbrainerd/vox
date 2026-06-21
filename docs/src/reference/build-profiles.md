---
title: "Build profiles"
description: "Named cargo build profiles for Vox: lean CLI, full desktop, and planned targets."
category: "Language Reference"
status: "current"
last_updated: "2026-06-18"
---

# Build profiles

Vox supports named build profiles — distinct `cargo build` invocations targeting specific deployment contexts.

## Named profiles

| Profile | Cargo invocation | Notes |
|---------|-----------------|-------|
| **lean** | `cargo build -p vox-cli --no-default-features --features script-execution` | Minimal CLI: parsing, eval, LSP. No gamification, no GUI. |
| **full** | `cargo build -p vox-cli --features extras-ludus` | Full language + orchestrator. |
| **mobile** | _Planned_ — `cargo build -p vox-cli --target aarch64-linux-android` | No ARM CI target yet. |
| **raspberry-pi** | _Planned_ — `cargo build -p vox-cli --target aarch64-unknown-linux-gnu` | See cross-platform audit. |

## Lean profile exclusions

The lean profile explicitly forbids the following crates, enforced by Rule 20 in `vox-arch-check`
and by the `vox ci profile-parity` gate:

| Crate | Reason |
|-------|--------|
| `vox-gamify` | Game mechanics (quests/battles/XP) — future: loads dynamically via `vox-plugin-gamify` |
| `vox-gui` | Tauri desktop frontend — separate process, not a CLI dep |

## Enforcement

- **Rule 20** (`vox-arch-check`): reads `[profiles.lean]` from `docs/src/architecture/layers.toml`,
  runs `cargo tree -p vox-cli --no-default-features`, and errors if any `forbidden` crate appears.
- **`vox ci profile-parity`**: reads `contracts/reports/lean-cli-profile.v1.json` for the
  crate-count ceiling and forbidden list. Run as an advisory CI step
  (`continue-on-error: true`) until gamify/gui leave the lean graph.

## SSOT locations

| What | Where |
|------|-------|
| Forbidden-crate lists | `docs/src/architecture/layers.toml` `[profiles.*]` |
| Crate-count budget | `contracts/reports/lean-cli-profile.v1.json` |
| arch-check enforcement | `crates/vox-arch-check/src/main.rs` Rule 20 |
| CI gate | `.github/workflows/ci.yml` `Profile parity` step |
