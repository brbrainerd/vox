# vox-orchestrator-core Extraction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract `crates/vox-orchestrator/src/orchestrator/` (and its sibling-module dependencies) into a new `crates/vox-orchestrator-core` crate, lowering `vox-orchestrator` LoC back below `max_loc = 35_000` and isolating the dispatch kernel from the daemon/runtime shell.

**Architecture:** Move the `Orchestrator` struct and its inherent-impl files into `vox-orchestrator-core`. Re-export the struct from `vox-orchestrator/src/lib.rs` so external consumers (3 files in `vox-orchestrator-mcp/llm_bridge/model_route_policy/`) keep compiling without source edits. Sibling modules that hold `Orchestrator` struct fields or have `impl Orchestrator` blocks co-move; modules used only by the daemon/runtime stay. The exact co-move set is determined by D1 audit, not pre-decided.

**Tech Stack:** Rust 1.92, cargo-nextest 0.9.95, cargo-llvm-cov 0.8.5, `cargo metadata --no-deps`, `git mv` for blame preservation. No new dependencies introduced; the new crate inherits workspace dep declarations from the moved modules.

**Spec:** [`docs/superpowers/specs/2026-05-28-vox-orchestrator-core-extraction-design.md`](../specs/2026-05-28-vox-orchestrator-core-extraction-design.md)

**Posture:** Trigger-gated. **Today the plan exits at D0** because `vox-orchestrator` is at 56,200 LoC (19.7% headroom, gate not firing). D1 is safe to pre-execute as preparation work because it's read-only measurement. D2–D7 must wait for the gate to trip.

---

## Phase summary

| Phase | What | Estimate (when running) | Safe to run today? |
|---|---|---:|---|
| D0 | Gate check | 5 min | Yes — confirms the gate state |
| D1 | Co-move manifest (read-only audit) | 1–2 h | Yes — produces a useful artifact even if D2+ defers |
| D2 | Skeleton crate | 2 h | No — creates an empty crate |
| D3 | Move co-move modules | 1–2 days | No |
| D4 | Move `orchestrator/` + struct | 4–8 h | No |
| D5 | Integration glue | 2–4 h | No |
| D6 | Tests | 2 h | No |
| D7 | Cleanup | 1 h | No |

---

### Task D0: Gate check

**Files:**
- Read: `docs/src/architecture/layers.toml` (verify `vox-orchestrator.max_loc`)
- Read: `vox-arch-check` output

- [x] **Step 1: Run arch-check and capture vox-orchestrator findings**

Run:
```powershell
cargo run -p vox-arch-check 2>&1 | Select-String "vox-orchestrator"
```

Expected output: zero or more lines mentioning `vox-orchestrator`. Look specifically for:
- A Rule 13 (LoC delta) warning of the form `vox-orchestrator: current_loc N grew M% vs baseline …` — if present, the gate has FIRED.
- A Rule 3 (LoC budget) line of the form `vox-orchestrator: NNNNN / 70000 LoC` — capture the current LoC.

- [x] **Step 2: Compute headroom**

Headroom = (70000 − current_loc) / 70000 × 100.

- [x] **Step 3: Decide continue or exit**

Continue to D1 if EITHER:
- Rule 13 fired against `vox-orchestrator`, OR
- Headroom < 5% (current_loc > 66,500)

Otherwise: **STOP the plan here.** Record the current LoC in this plan doc below this checkbox, commit the doc update with message `docs(plan): D0 gate check 2026-MM-DD — not tripped (N LoC, M% headroom)`, and re-run D0 at the next release tag.

**D0 result (recorded 2026-05-28):** Gate NOT tripped. vox-orchestrator at 61061 LoC, 12.8% headroom against max_loc=70_000. Rule 13: not firing. Plan exits here; re-run D0 at the next release tag.

- [ ] **Step 4: (If continuing) Verify no active MENS / mesh sprint touches `vox-orchestrator/src/`**

Run:
```powershell
git log --since "2 weeks ago" --oneline -- crates/vox-orchestrator/src/ | Select-Object -First 20
```

If you see commits from a MENS or mesh feature sprint in the last two weeks, **defer** until that sprint finishes — merge conflicts on `src/orchestrator/` would be severe.

---

### Task D1: Co-move manifest audit

**Files:**
- Modify: `docs/superpowers/specs/2026-05-28-vox-orchestrator-core-extraction-design.md` (append D1 manifest table)
- Read: `crates/vox-orchestrator/src/**/*.rs`

This task is **read-only**: it produces a markdown table classifying each candidate module. No code changes. Safe to run today.

- [x] **Step 1: Enumerate sibling modules of `src/orchestrator/`**

Run:
```powershell
Get-ChildItem crates/vox-orchestrator/src -Directory | Where-Object { $_.Name -ne "orchestrator" } | Select-Object -ExpandProperty Name
```

Expected: ~24 directory names. Record this set as `CANDIDATES`.

- [x] **Step 2: For each candidate, count `crate::<mod>` imports from `src/orchestrator/`**

Run (PowerShell):
```powershell
$candidates = Get-ChildItem crates/vox-orchestrator/src -Directory | Where-Object { $_.Name -ne "orchestrator" } | Select-Object -ExpandProperty Name
foreach ($mod in $candidates) {
  $count = (Select-String -Path "crates/vox-orchestrator/src/orchestrator/*.rs", "crates/vox-orchestrator/src/orchestrator/**/*.rs" -Pattern "crate::$mod\b" -ErrorAction SilentlyContinue | Measure-Object).Count
  "{0,4}  {1}" -f $count, $mod
}
```

Expected output: one line per candidate with its import-frequency from inside `src/orchestrator/`. Sort by descending count.

- [x] **Step 3: For each candidate, count `impl Orchestrator` blocks in its files**

Run:
```powershell
foreach ($mod in $candidates) {
  $impls = (Select-String -Path "crates/vox-orchestrator/src/$mod/*.rs", "crates/vox-orchestrator/src/$mod/**/*.rs" -Pattern "^impl Orchestrator\s*\{|^impl crate::orchestrator::Orchestrator\s*\{" -ErrorAction SilentlyContinue | Measure-Object).Count
  "{0,3}  {1}" -f $impls, $mod
}
```

Modules with `impls > 0` are forced-move (Rust coherence).

- [x] **Step 4: For each candidate, check whether it's an `Orchestrator` struct field**

Run:
```powershell
$structDef = Get-Content crates/vox-orchestrator/src/orchestrator.rs -Raw
$fieldRange = [regex]::Match($structDef, '(?s)pub struct Orchestrator\s*\{(.*?)^\}').Groups[1].Value
foreach ($mod in $candidates) {
  $isField = $fieldRange -match "\b$mod::\w+\b" -or $fieldRange -match "\b$($mod -replace '_','')\w*\b"
  if ($isField) { "FIELD  $mod" }
}
```

Modules whose types appear in the struct field list are forced-move.

- [x] **Step 5: Classify each candidate**

Apply the rules from the spec (§4-D1):

- **Move** if: has `impl Orchestrator` blocks (Step 3 > 0) OR is an Orchestrator struct field (Step 4 hit) OR imported by >5 files in `src/orchestrator/` (Step 2 > 5)
- **Trait-inject** if: imported by 1–5 files, no `impl Orchestrator`, not a struct field, and the public interface is ≤5 methods (count with `grep -c "^\s*pub fn" crates/vox-orchestrator/src/<mod>/mod.rs`)
- **Stay** if: imported by 0 files in `src/orchestrator/`

- [x] **Step 6: Append the manifest to the spec doc**

Edit `docs/superpowers/specs/2026-05-28-vox-orchestrator-core-extraction-design.md`. After §4-D1, append a new section:

```markdown
## D1 manifest (audited YYYY-MM-DD)

| Module | LoC | crate:: imports from src/orchestrator/ | impl Orchestrator blocks | Struct field? | Decision |
|---|---:|---:|---:|:---:|:---:|
| types     | NNNN |   N | N | yes/no | Move |
| config    | NNNN |   N | N | yes/no | Move |
| …         | …    |   … | … | …      | …    |
```

Fill the table with measurements from Steps 2–5. One row per candidate.

- [x] **Step 7: Commit the manifest**

```powershell
cd C:\Users\Owner\vox
git add docs/superpowers/specs/2026-05-28-vox-orchestrator-core-extraction-design.md
git commit -m @'
docs(spec): vox-orchestrator-core D1 manifest (YYYY-MM-DD)

Per-module classification for the co-move set: Move / Trait-inject / Stay.
Measured by crate:: import count, impl Orchestrator presence, and
Orchestrator struct-field membership.

Produced read-only; no source edits. Drives D3 in the corresponding plan.
'@
```

The manifest is now the source of truth for D3.

---

### Task D2: Skeleton crate

**Files:**
- Create: `crates/vox-orchestrator-core/Cargo.toml`
- Create: `crates/vox-orchestrator-core/src/lib.rs`
- Modify: `docs/src/architecture/layers.toml`
- Modify: `docs/src/architecture/where-things-live.md`
- Modify: `.config/coverage-gates.toml`

- [ ] **Step 1: Verify the workspace `Cargo.toml` uses a glob members pattern**

Run:
```powershell
Select-String -Path Cargo.toml -Pattern "members" -Context 0,5 | Select-Object -First 10
```

Expected: a `members` list. If it contains `"crates/*"`, the new crate will be picked up automatically — proceed to Step 2. If it lists crates explicitly (e.g. `"crates/vox-orchestrator"`, `"crates/vox-actor-runtime"`, …), edit the root `Cargo.toml` now to add `"crates/vox-orchestrator-core"` in alphabetical order with the others. Either way, after this step, the new crate is in the workspace member set.

- [ ] **Step 2: Create the new crate directory and Cargo.toml**

Create `crates/vox-orchestrator-core/Cargo.toml`:

```toml
[package]
name = "vox-orchestrator-core"
description = "Dispatch / routing / config / models kernel extracted from vox-orchestrator. Holds the Orchestrator struct and its inherent impls. See docs/superpowers/specs/2026-05-28-vox-orchestrator-core-extraction-design.md."
version.workspace = true
edition.workspace = true

[features]
# D3 will populate features as modules move. Start empty.

[dependencies]
# D3 will populate dependencies as modules move. Start empty.

[lints]
workspace = true
```

- [ ] **Step 3: Create the empty lib.rs**

Create `crates/vox-orchestrator-core/src/lib.rs`:

```rust
//! Dispatch / routing / config / models kernel extracted from `vox-orchestrator`.
//!
//! This crate holds the `Orchestrator` struct and its inherent `impl` blocks
//! along with the sibling modules that the struct depends on directly. The
//! parent `vox-orchestrator` crate re-exports this crate's public surface
//! for backward compatibility with `vox-cli`, `vox-orchestrator-mcp`, and
//! other external consumers.
//!
//! See `docs/superpowers/specs/2026-05-28-vox-orchestrator-core-extraction-design.md`.

// D3 will add `pub mod <mod>;` for each moved module.
// D4 will add `pub mod orchestrator;` and the `Orchestrator` struct definition.
```

- [ ] **Step 4: Verify the new crate compiles**

Run:
```powershell
cargo check -p vox-orchestrator-core
```

Expected: `Finished 'dev' profile` with no errors.

- [ ] **Step 5: Add the new entry to layers.toml**

Edit `docs/src/architecture/layers.toml`. Find the `[crates.vox-orchestrator]` block. After it, add:

```toml
[crates.vox-orchestrator-core]
layer = 3
max_loc = 40_000
max_dependents = 30
```

Keep the existing `[crates.vox-orchestrator]` entry unchanged for now (D7 will lower its `max_loc`).

- [ ] **Step 6: Add the new entry to where-things-live.md**

Edit `docs/src/architecture/where-things-live.md`. Find the row for `crates/vox-orchestrator/`. Immediately after it, add a new row:

```markdown
| `crates/vox-orchestrator-core/` | L3 | Dispatch / routing / config / models kernel. Holds the `Orchestrator` struct and inherent impls extracted from `vox-orchestrator`. See `docs/superpowers/specs/2026-05-28-vox-orchestrator-core-extraction-design.md`. |
```

Adjust columns to match the table's existing format.

- [ ] **Step 7: Add the coverage floor**

Edit `.config/coverage-gates.toml`. In the `[crates]` table, after `vox-orchestrator = 40.0` add:

```toml
vox-orchestrator-core = 40.0  # New crate from 2026-05-28 extraction; floor to match parent.
```

- [ ] **Step 8: Verify arch-check still passes**

Run:
```powershell
cargo run -p vox-arch-check
```

Expected: clean (no Rule 12 WTL-parity warning, since both the new crate dir and the new WTL row exist).

- [ ] **Step 9: Commit the skeleton**

```powershell
cd C:\Users\Owner\vox
git add crates/vox-orchestrator-core/ docs/src/architecture/layers.toml docs/src/architecture/where-things-live.md .config/coverage-gates.toml
git commit -m @'
feat(arch): skeleton crates/vox-orchestrator-core (D2)

Empty crate skeleton with Cargo.toml, src/lib.rs, layers.toml entry
(layer=3, max_loc=40_000), where-things-live row, and coverage-gates
floor (40.0). D3 will start populating it via git mv of co-move
modules per the D1 manifest.
'@
```

---

### Task D3: Move co-move modules

This task is a loop body. Execute it **once per module** in the D1 manifest where `Decision = Move`. Track progress by checking off the per-module rows in the D1 manifest table as you finish each module.

**Files (per iteration; substitute `<mod>` with the module name):**
- `git mv`: `crates/vox-orchestrator/src/<mod>/` → `crates/vox-orchestrator-core/src/<mod>/`
- Modify: `crates/vox-orchestrator-core/src/lib.rs` (add `pub mod <mod>;`)
- Modify: `crates/vox-orchestrator-core/Cargo.toml` (add any workspace deps the module needs)
- Modify: `crates/vox-orchestrator/src/lib.rs` (replace `pub mod <mod>;` with `pub use vox_orchestrator_core::<mod>;`)
- Modify: `crates/vox-orchestrator/Cargo.toml` (on first iteration, add `vox-orchestrator-core = { path = "../vox-orchestrator-core" }`)

- [ ] **Step 1: Record baseline test pass count**

Run:
```powershell
cargo nextest run -p vox-orchestrator --no-fail-fast --retries 0 2>&1 | Select-String "Summary"
```

Record the line for use in Step 8's check. Example baseline: `Summary [Xs] N tests run: N passed, M skipped`.

- [ ] **Step 2: Move the module**

```powershell
git mv crates/vox-orchestrator/src/<mod> crates/vox-orchestrator-core/src/<mod>
```

The `git mv` preserves blame. If the module is a single file (e.g., `<mod>.rs` not `<mod>/mod.rs`), adjust accordingly.

- [ ] **Step 3: Wire the module into the new crate**

Edit `crates/vox-orchestrator-core/src/lib.rs`. Add (in alphabetical order with other `pub mod` lines):

```rust
pub mod <mod>;
```

- [ ] **Step 4: Add any deps the module needs to the new crate's Cargo.toml**

Determine deps by:
```powershell
Select-String -Path "crates/vox-orchestrator-core/src/<mod>/**/*.rs", "crates/vox-orchestrator-core/src/<mod>.rs" -Pattern "^use ([a-z_][a-z0-9_]*)::" -ErrorAction SilentlyContinue | ForEach-Object { $_.Matches.Groups[1].Value } | Sort-Object -Unique
```

For each crate name in the output that's a workspace dep (check `crates/vox-orchestrator/Cargo.toml` for the dep declaration), copy the dep line into `crates/vox-orchestrator-core/Cargo.toml`'s `[dependencies]`.

- [ ] **Step 5: On the FIRST iteration only — add vox-orchestrator-core as a dep of vox-orchestrator**

Edit `crates/vox-orchestrator/Cargo.toml`. In `[dependencies]`, add (in alphabetical order):

```toml
vox-orchestrator-core = { path = "../vox-orchestrator-core" }
```

Skip this step on subsequent D3 iterations.

- [ ] **Step 6: Rewire the parent crate's lib.rs**

Edit `crates/vox-orchestrator/src/lib.rs`. Find `pub mod <mod>;` and replace with:

```rust
pub use vox_orchestrator_core::<mod>;
```

- [ ] **Step 7: Verify both crates compile**

```powershell
cargo check -p vox-orchestrator-core
cargo check -p vox-orchestrator
```

If either fails: do NOT continue to Step 8. Read the error; common causes:
- A type in the moved module is referenced as `crate::<mod>::X` from elsewhere in `vox-orchestrator/src/` → fix to `vox_orchestrator_core::<mod>::X` (or use the re-export from `crate::<mod>::X` which now goes through the `pub use` added in Step 6).
- A workspace dep wasn't copied → re-run Step 4.
- The module re-exported sibling items via `pub use crate::<other>` → those references now break; fix by changing to `pub use crate::<other>` if `<other>` is also in vox-orchestrator-core, or by leaving a re-export shim in the parent.

- [ ] **Step 8: Verify test pass count is unchanged**

```powershell
cargo nextest run -p vox-orchestrator --no-fail-fast --retries 0 2>&1 | Select-String "Summary"
cargo nextest run -p vox-orchestrator-core --no-fail-fast --retries 0 2>&1 | Select-String "Summary"
```

The sum of passed tests across the two crates must equal the baseline from Step 1. If not, a test got lost — investigate which test file moved with `<mod>` and verify it's discoverable by nextest in the new crate.

- [ ] **Step 9: Commit**

```powershell
cd C:\Users\Owner\vox
git add crates/vox-orchestrator-core/ crates/vox-orchestrator/src/lib.rs crates/vox-orchestrator/Cargo.toml
git commit -m "refactor(arch): move <mod> to vox-orchestrator-core (D3)"
```

Repeat Steps 1–9 for the next module in the D1 manifest. When all Move-decision modules are processed, mark D3 complete and proceed to D4.

---

### Task D4: Move `orchestrator/` subdir + `Orchestrator` struct

**Files:**
- `git mv`: `crates/vox-orchestrator/src/orchestrator/` → `crates/vox-orchestrator-core/src/orchestrator/`
- `git mv`: `crates/vox-orchestrator/src/orchestrator.rs` → `crates/vox-orchestrator-core/src/orchestrator.rs`
- Modify: `crates/vox-orchestrator-core/src/lib.rs` (add `pub mod orchestrator;`)
- Modify: `crates/vox-orchestrator/src/lib.rs` (add re-exports for the struct and the orchestrator module)
- Modify: source files inside the moved `orchestrator/` subdir (fix `crate::` paths)

- [ ] **Step 1: Record baseline test count for the workspace**

```powershell
cargo nextest run --workspace --no-fail-fast --retries 0 --run-ignored default 2>&1 | Select-String "Summary"
```

Record the full counts: `N tests run: P passed, F failed, S skipped`.

- [ ] **Step 2: Move the struct definition file**

```powershell
git mv crates/vox-orchestrator/src/orchestrator.rs crates/vox-orchestrator-core/src/orchestrator.rs
```

- [ ] **Step 3: Move the inherent-impl subdir**

```powershell
git mv crates/vox-orchestrator/src/orchestrator crates/vox-orchestrator-core/src/orchestrator
```

- [ ] **Step 4: Wire into vox-orchestrator-core/src/lib.rs**

Edit `crates/vox-orchestrator-core/src/lib.rs`. Add (in alphabetical order with other `pub mod` lines):

```rust
pub mod orchestrator;
```

Note that `orchestrator.rs` (the file) and `orchestrator/mod.rs` (the subdir module) are siblings under Rust's module system; `pub mod orchestrator;` discovers either form depending on which exists. After the moves above, the file form (`orchestrator.rs`) is the struct definition and the subdir form has its own contents. Verify with `cargo check` after the next step.

- [ ] **Step 5: Re-export from vox-orchestrator/src/lib.rs**

Edit `crates/vox-orchestrator/src/lib.rs`. Find the existing `pub mod orchestrator;` line (or `mod orchestrator;` plus a `pub use orchestrator::Orchestrator;`). Replace with:

```rust
pub use vox_orchestrator_core::orchestrator;
pub use vox_orchestrator_core::Orchestrator;
```

If there were additional re-exports from the old `orchestrator` module (e.g. `pub use orchestrator::Foo;`), replicate them via the new path:

```rust
pub use vox_orchestrator_core::orchestrator::Foo;
```

- [ ] **Step 6: Fix `crate::` paths inside the moved subdir**

The moved files now live in `crates/vox-orchestrator-core/src/orchestrator/` and their `crate::` refers to `vox_orchestrator_core` (not `vox_orchestrator`). For modules moved in D3, `crate::<mod>` still resolves correctly (both subdirs are now in the same crate). For modules that stayed in `vox-orchestrator`, `crate::<mod>` must become `vox_orchestrator::<mod>`.

Run:
```powershell
cargo check -p vox-orchestrator-core 2>&1 | Select-String "could not find|unresolved import" | Select-Object -First 20
```

For each error of the form `could not find <name> in the crate root`, change the corresponding `use crate::<name>::…` to `use vox_orchestrator::<name>::…` — but be careful: this introduces a circular dep (`vox-orchestrator-core` → `vox-orchestrator` → `vox-orchestrator-core`). If the error is for a module that should have moved in D3 but didn't, **revert this step and go back to D3 to move that module**.

Realistically, D1 should have caught these; if Step 6 surfaces unexpected dependencies, the D1 manifest was incomplete. Append a note to the D1 manifest documenting the additional moves and proceed.

- [ ] **Step 7: Verify both crates compile**

```powershell
cargo check -p vox-orchestrator-core
cargo check -p vox-orchestrator
```

Both must succeed. If `vox-orchestrator-core` compiles but `vox-orchestrator` doesn't, the issue is in `lib.rs` re-exports — fix per Step 5's pattern.

- [ ] **Step 8: Verify mcp consumer compiles via the re-export**

```powershell
cargo check -p vox-orchestrator-mcp
```

This consumes `vox_orchestrator::Orchestrator` from two files (`llm_bridge/model_route_policy/resolve.rs`, `tests.rs`). Must compile without source edits.

- [ ] **Step 9: Run the full workspace test suite**

```powershell
cargo nextest run --workspace --no-fail-fast --retries 0 --run-ignored default 2>&1 | Select-String "Summary"
```

Pass / fail / skip counts must match the Step 1 baseline (modulo pre-existing flakes like `vox-oratio peak_normalize_scales_quiet_signal`). If a test count regressed, find the missing test:

```powershell
cargo nextest list --workspace 2>&1 > current-tests.txt
# Compare against a list captured before D4 began.
```

- [ ] **Step 10: Commit**

```powershell
cd C:\Users\Owner\vox
git add crates/vox-orchestrator-core/ crates/vox-orchestrator/src/lib.rs
git commit -m @'
refactor(arch): move Orchestrator struct + orchestrator/ subdir to
vox-orchestrator-core (D4)

The struct definition (src/orchestrator.rs) and its inherent-impl
subdir (src/orchestrator/) move together to satisfy Rust coherence.
vox-orchestrator now re-exports Orchestrator via pub use, preserving
the public API for vox-orchestrator-mcp (3 consumer files in
llm_bridge/model_route_policy/) and vox-cli without source edits.
'@
```

---

### Task D5: Integration glue

**Files:**
- Modify (audit only): `crates/vox-orchestrator/src/runtime.rs`
- Modify (audit only): `crates/vox-orchestrator/src/orch_daemon/`
- Modify (audit only): `crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/resolve.rs`
- Modify (audit only): `crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/tests.rs`
- Verify-only: `docs/src/architecture/layers.toml` `[[known_inversions]]` entries

- [ ] **Step 1: Verify `runtime.rs` still constructs Orchestrator successfully**

Run:
```powershell
Select-String -Path crates/vox-orchestrator/src/runtime.rs -Pattern "Orchestrator::new" | Select-Object -First 5
```

For each match, verify the surrounding code can still see `Orchestrator` — it should, because Step D4-5 added `pub use vox_orchestrator_core::Orchestrator;` to `vox-orchestrator/src/lib.rs`, making `crate::Orchestrator` resolve.

If `runtime.rs` uses `use crate::orchestrator::Orchestrator`, that still works via the `pub use vox_orchestrator_core::orchestrator;` re-export. If it uses `use crate::orchestrator::Orchestrator;` and that path fails, change to `use crate::Orchestrator;`.

- [ ] **Step 2: Verify `orch_daemon/` still constructs Orchestrator**

```powershell
cargo check -p vox-orchestrator --features=""
cargo check -p vox-orchestrator --all-features
```

Run with and without features to catch feature-gated import sites.

- [ ] **Step 3: Run the mcp consumer's targeted tests**

```powershell
cargo nextest run -p vox-orchestrator-mcp model_route_policy --no-fail-fast 2>&1 | Select-String "Summary"
```

All tests in that module must pass. If any fail with `unresolved import vox_orchestrator::Orchestrator`, return to Step D4-5 and verify the re-export is in place.

- [ ] **Step 4: Verify known_inversions in layers.toml still cover the dep edges**

```powershell
Select-String -Path docs/src/architecture/layers.toml -Pattern "vox-orchestrator|vox-orchestrator-core" -Context 2 | Select-Object -First 20
```

The existing `vox-cli -> vox-orchestrator` known inversion (if any) does not need to change — the inversion direction is unchanged. If you see a `vox-orchestrator-core -> vox-cli` inversion appear in `vox-arch-check` output (it shouldn't), that's a sign D6's tests should catch a layering bug; flag and investigate.

- [ ] **Step 5: Run vox-arch-check**

```powershell
cargo run -p vox-arch-check
```

Expected: clean. If a new layer inversion appears, the extraction has introduced an arch violation; review the D1 manifest decisions before continuing.

- [ ] **Step 6: Commit integration verifications (no source edits expected — empty commit if nothing changed)**

If Steps 1–5 surfaced any source edits, commit them:
```powershell
cd C:\Users\Owner\vox
git add -A
git commit -m "fix(arch): rewire vox-orchestrator integration callsites after D4 (D5)"
```

If no edits were needed, skip the commit and proceed to D6 (this is the expected outcome — the re-export in D4-5 should cover everything).

---

### Task D6: Tests

**Files:**
- Run: nextest across workspace
- Modify (potentially): tests in `crates/vox-orchestrator/tests/` whose imports reference internals that moved to `vox-orchestrator-core`

- [ ] **Step 1: List test inventory before**

```powershell
cargo nextest list --workspace 2>&1 | Out-File -FilePath target/tests-pre-d6.txt -Encoding utf8
(Get-Content target/tests-pre-d6.txt | Measure-Object -Line).Lines
```

Record the line count (≈ test count + headers).

- [ ] **Step 2: Run workspace tests with full coverage of both crates**

```powershell
cargo nextest run -p vox-orchestrator -p vox-orchestrator-core --no-fail-fast --retries 0 2>&1 | Select-String "Summary"
```

Both crates' suites must pass.

- [ ] **Step 3: Run full workspace nextest**

```powershell
cargo nextest run --workspace --no-fail-fast --retries 0 --run-ignored default 2>&1 | Select-String "Summary"
```

Counts must match the D4 Step 1 baseline.

- [ ] **Step 4: For each integration test in `crates/vox-orchestrator/tests/` — check imports**

```powershell
Select-String -Path "crates/vox-orchestrator/tests/*.rs" -Pattern "use vox_orchestrator::" | Select-Object -First 30
```

For each match using an internal path (e.g., `use vox_orchestrator::orchestrator::internal::X`), check whether `internal::X` still resolves via the re-export. If not, change to `use vox_orchestrator_core::orchestrator::internal::X`.

- [ ] **Step 5: Verify `serial_test`-annotated tests still serialize correctly**

```powershell
cargo nextest run -p vox-orchestrator-core --test-threads=1 --no-fail-fast 2>&1 | Select-String "Summary"
```

Compare to the parallel run from Step 2 — both must pass.

- [ ] **Step 6: Run vox-arch-check and coverage-gates**

```powershell
cargo run -p vox-arch-check
cargo llvm-cov nextest --workspace --no-fail-fast --no-report --retries 0
cargo run -p vox-cli --quiet -- run scripts/perf/coverage-report.vox
cargo run -p vox-cli --quiet -- ci coverage-gates --summary-json=target/coverage-summary.json
```

The coverage-gates command must report `OK` for both `vox-orchestrator` (still ≥ floor) and `vox-orchestrator-core` (≥ 40.0 floor set in D2 Step 7).

- [ ] **Step 7: Commit any test rewires**

If Step 4 produced edits:
```powershell
cd C:\Users\Owner\vox
git add crates/vox-orchestrator/tests/
git commit -m "test(vox-orchestrator): rewire integration test imports for post-D4 paths (D6)"
```

Otherwise skip; D6 was verify-only.

---

### Task D7: Cleanup

**Files:**
- Modify: `docs/src/architecture/layers.toml` (lower `vox-orchestrator.max_loc`)
- Modify: `docs/src/architecture/where-things-live.md` (update `vox-orchestrator` row description)
- Modify: `docs/superpowers/specs/2026-05-28-vox-orchestrator-core-extraction-design.md` (append post-split LoC; mark complete)
- Modify: `docs/src/architecture/2026-05-15-orchestrator-tier-d-plan.md` (mark superseded)
- Modify: `.config/coverage-gates.toml` (potentially re-baseline `vox-orchestrator` floor if it shifted)

- [ ] **Step 1: Measure post-split LoC**

```powershell
$orch = (Get-ChildItem crates/vox-orchestrator/src -Recurse -Filter *.rs | Get-Content | Measure-Object -Line).Lines
$core = (Get-ChildItem crates/vox-orchestrator-core/src -Recurse -Filter *.rs | Get-Content | Measure-Object -Line).Lines
"vox-orchestrator     : $orch LoC"
"vox-orchestrator-core: $core LoC"
```

Record both numbers. The success criterion (spec §7) is `vox-orchestrator < 35,000` AND `vox-orchestrator-core < 40,000`.

- [ ] **Step 2: Lower vox-orchestrator's max_loc in layers.toml**

Edit `docs/src/architecture/layers.toml`. Find `[crates.vox-orchestrator]`. Change `max_loc = 70_000` to a value 20% above the measured post-split LoC, rounded up to the nearest 1_000. Example: if `vox-orchestrator` is now 28,400 LoC, set `max_loc = 35_000`.

- [ ] **Step 3: Update where-things-live.md description for vox-orchestrator**

Edit the existing `vox-orchestrator` row to reflect its new scope ("Daemon entry point, runtime, a2a transport, session, hopper, routing, integration glue. Core dispatch/routing/config kernel lives in `vox-orchestrator-core`.").

- [ ] **Step 4: Append post-split LoC to the design spec**

Edit `docs/superpowers/specs/2026-05-28-vox-orchestrator-core-extraction-design.md`. After §8, add:

```markdown
## 9. Post-split outcome (YYYY-MM-DD)

| Crate | Pre-split LoC | Post-split LoC | Budget |
|---|---:|---:|---:|
| vox-orchestrator      | 56,200 | NNNNN | NN_000 |
| vox-orchestrator-core | —      | NNNNN | 40_000 |

D1 manifest moved N modules; M trait-injected; K stayed.
Workspace nextest counts pre/post: P/P passed, F/F failed, S/S skipped.
```

Change the spec's frontmatter `status: "current"` to `status: "completed"`.

- [ ] **Step 5: Mark the 2026-05-15 plan as superseded**

Edit `docs/src/architecture/2026-05-15-orchestrator-tier-d-plan.md`. Change frontmatter `status: "current"` to `status: "superseded"`. After the title, add:

```markdown
> **Superseded by:** [`../../superpowers/specs/2026-05-28-vox-orchestrator-core-extraction-design.md`](../../superpowers/specs/2026-05-28-vox-orchestrator-core-extraction-design.md) which carried this through to execution on YYYY-MM-DD.
```

- [ ] **Step 6: Re-baseline coverage floor for vox-orchestrator if it shifted significantly**

```powershell
cargo run -p vox-cli --quiet -- run scripts/perf/coverage-report.vox
cargo run -p vox-cli --quiet -- ci coverage-gates --summary-json=target/coverage-summary.json
```

If `vox-orchestrator` line coverage shifted by >5 percentage points, update `.config/coverage-gates.toml` to a floor 2 points below the new measured value (matches the convention from the vox-cli floor adjustment in commit `7249712014`).

- [ ] **Step 7: Final verification**

Run all the success criteria from spec §7:

```powershell
cargo nextest run --workspace --no-fail-fast --retries 0 --run-ignored default 2>&1 | Select-String "Summary"
cargo run -p vox-arch-check
cargo run -p vox-cli --quiet -- run scripts/perf/coverage-report.vox
cargo run -p vox-cli --quiet -- ci coverage-gates --summary-json=target/coverage-summary.json
cargo build -p vox-orchestrator-mcp
```

All must pass / be clean.

- [ ] **Step 8: Commit cleanup**

```powershell
cd C:\Users\Owner\vox
git add docs/src/architecture/layers.toml docs/src/architecture/where-things-live.md docs/superpowers/specs/2026-05-28-vox-orchestrator-core-extraction-design.md docs/src/architecture/2026-05-15-orchestrator-tier-d-plan.md .config/coverage-gates.toml
git commit -m @'
chore(arch): finalize vox-orchestrator-core extraction (D7)

- layers.toml: lower vox-orchestrator max_loc from 70_000 -> 35_000
- where-things-live.md: update vox-orchestrator description
- spec doc: append post-split LoC, mark status: completed
- 2026-05-15 tier-d plan: mark superseded
- coverage-gates: (if applicable) re-baseline vox-orchestrator floor

Extraction complete. vox-orchestrator-core holds the Orchestrator
struct + dispatch kernel; vox-orchestrator is now a thinner daemon /
runtime / a2a shell that re-exports the kernel for backward compat.
'@
```

- [ ] **Step 9: Push**

```powershell
git push origin main
```

---

## Self-review notes

The plan covers every spec requirement:

- Spec §1 trigger gate → Task D0
- Spec §4 D1 manifest → Task D1
- Spec §4 D2–D7 → Tasks D2–D7
- Spec §5 test strategy → Task D6 (count check in Step 1+3, integration test rewire in Step 4, serial_test verification in Step 5)
- Spec §6 risk register mitigations → distributed across tasks (cargo-check iteration in D3/D4, smoke build in D5, test-count check in D6, etc.)
- Spec §7 success criteria → Task D7 Step 7
- Spec §8 open items → resolved by D3's per-module template (no pre-decided order; D1 manifest drives) and D7's commit message format (`refactor(arch):` matches dei_shim precedent commit `94ce7d5b5f`)

Open item from the spec (active MENS sprint check) is wired into D0 Step 4.

The plan does NOT cover: trait-inject implementations for borderline modules. Those are deferred to a follow-on (the D1 manifest can mark them; the actual `dyn Trait` wiring is a separate small effort once the bulk move lands).
