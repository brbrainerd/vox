# Architectural Governance (TOESTUB)

The Vox codebase enforces architectural health automatically using the TOESTUB engine.

## Running TOESTUB

```bash
vox toestub self               # scan the entire Vox workspace
vox stub-check --path .        # equivalent
vox stub-check --severity error # only show errors/criticals
vox stub-check --fix           # generate AI fix suggestions
```

In CI, `vox stub-check` exits 1 if any error-level findings are present.

## Enforced Rules

| Rule ID | Description | Severity |
|---|---|---|
| `no-todo` | `todo!()`, `unimplemented!()`, `fixme` in production code | Error |
| `no-magic-values` | Raw numeric literals > 3 digits inline | Warning |
| `no-empty-body` | Empty function bodies without `todo!` | Error |
| `god-object` | Files > 500 lines or structs > 12 methods | Warning |
| `sprawl-guard` | Directories > 20 files | Warning |
| `no-generic-names` | Files named `utils.rs`, `helpers.ts`, `misc.*` | Warning |
| `dry-violation` | Near-duplicate blocks (≥ 5 lines, 90% similar) | Warning |
| `victory-claim` | Comments like "// solved", "// done", "// fixed" | Info |
| `schema-compliance` | New crate paths not in `vox-schema.json` | Error |

## God Object Lock

Files exceeding **500 lines** or structs with **> 12 methods** are locked for new features.
Before adding to them, refactor into traits and sub-modules first.

Affected files as of March 2026:
- `crates/vox-orchestrator/src/orchestrator.rs` (70 KB) → See ORCH-01 in plan
- `crates/vox-orchestrator/src/memory.rs` (31 KB) → tracked

## Sprawl Guard

No directory may contain more than **20 files**. When exceeded, sub-slice into
feature modules. Example:
```
# From:
crates/vox-mcp/src/tool_a.rs
crates/vox-mcp/src/tool_b.rs  (20+ files)

# To:
crates/vox-mcp/src/tools/
  tool_a.rs
  tool_b.rs
  mod.rs
```

## Naming Enforcement

Generic names are **strictly forbidden**:
- `utils.rs`, `helpers.rs`, `misc.rs`, `common.rs`
- `utils.ts`, `helpers.ts`, `types.ts` (unless it is the canonical types file)

All files must have a specific, meaningful name tied to their domain.

## Schema Compliance

All new crate definitions and path conventions must be registered in `vox-schema.json`
at the workspace root before the file is created. The `schema-compliance` TOESTUB rule
enforces this.

## Vox Quality Rules (Code Review Checklist)

Before marking any PR or task complete, verify:

- [ ] No `.unwrap()` or `.expect("TODO")` in production codepaths
- [ ] No `todo!()` macros outside tests
- [ ] All `match` arms are exhaustive (no wildcard `_ => panic!()` unless explicitly justified)
- [ ] New public APIs have doc comments
- [ ] `cargo check --workspace` passes with zero errors
- [ ] `vox stub-check` finds no Error/Critical severity issues
- [ ] `cargo clippy` (or equivalent) shows no denies

## Agent Scope Rules

- **File Affinity**: An agent must hold a lock via `vox_claim_file` before editing.
  Overlapping edits are blocked by the Orchestrator's `scope.rs` guard.
- **Scope Violation**: Writing outside assigned scope emits a `ScopeViolation` event
  which is logged and surfaced in the VS Code extension status bar.

## Build Environment Notes (Windows)

```powershell
# Always use full path in agent shell sessions where PATH may not be set:
& "$env:USERPROFILE\.cargo\bin\cargo.exe" check --workspace

# Prefer check over build for agent sessions (faster, no linker lock):
cargo check -p vox-cli

# Transient Windows linker errors (LNK1104) → retry or use check:
$env:CARGO_TARGET_DIR = "target_alt"; cargo check -p vox-orchestrator
```
