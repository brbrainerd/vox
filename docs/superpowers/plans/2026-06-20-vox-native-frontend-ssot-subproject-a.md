# Vox-Native Frontend SSOT — Sub-project A (Backend-seam formalization + coverage ledger) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Route production frontend emission through the existing `Target` seam (instead of a hard-coded direct call to the TypeScript emitter) and establish a checked-in, drift-guarded coverage ledger that measures how much of the 173-file React GUI is `.vox`-expressible today.

**Architecture:** Two independent halves. (1) A new `frontend_backend::emit_frontend(target, hir, options)` dispatch in `vox-codegen` that `match`es exhaustively on `vox_compiler::target::Target` — one wired arm (`TypeScript` → the current `generate_with_options`), the rest typed errors — then re-point the CLI `vox build --target client` call site through it. Zero behavior change for the TS path; the value is the formalized seam (Model 3 commitment) and the parity-breaking exhaustive match. (2) A markdown coverage ledger enumerating every GUI surface with its `.vox`-expressibility status, plus a Rust currency test in `vox-gui` that fails if a surface is added/removed without updating the ledger.

**Tech Stack:** Rust (`vox-codegen`, `vox-cli`, `vox-gui`), `vox_compiler::target::Target`, `vox_codegen::codegen_ts` (`CodegenOptions`/`CodegenOutput`/`generate_with_options`), the `parse(lex(src))` → `lower_module` test pattern, markdown ledger.

**Spec:** `docs/superpowers/specs/2026-06-20-vox-native-frontend-ssot-design.md` (this plan implements Sub-project A only; B–G are sequenced follow-on plans).

**Execution model (Claude Sonnet 4.6 in Claude Code):** TDD mandatory — failing test first, observed-output verification before any "done". Parallelism map below; an orchestrator may fan out the `[PARALLEL-SAFE]` tasks as concurrent sub-agents.

**Parallelism map:**
- Task 1 `[SEQUENTIAL base]` — the seam; everything in Half 1/2 depends on it.
- Task 2 `[SEQUENTIAL after 1]` — CLI wiring; needs Task 1's function.
- Task 3 `[PARALLEL-SAFE]` — ledger doc; touches only `docs/`, independent of 1/2.
- Task 4 `[PARALLEL-SAFE after 3]` — ledger currency test; needs the ledger file from Task 3 but nothing from 1/2.

So an orchestrator runs {Task 1 → Task 2} and {Task 3 → Task 4} as two concurrent chains.

---

## File Structure

| File | New/Modify | Responsibility |
|---|---|---|
| `crates/vox-codegen/src/frontend_backend.rs` | **New** | The formalized emission seam: `emit_frontend(target, hir, options)` dispatching on `Target`. |
| `crates/vox-codegen/src/lib.rs` | Modify (add `pub mod`) | Register the new module. |
| `crates/vox-codegen/tests/frontend_backend.rs` | **New** | Parity + dispatch tests for the seam. |
| `crates/vox-cli/src/commands/build.rs` | Modify (`:217`) | Route `--target client` emission through `emit_frontend`. |
| `docs/superpowers/ledgers/frontend-coverage-ledger.md` | **New** | Checked-in coverage ledger: every GUI surface + `.vox`-expressibility status. |
| `crates/vox-gui/tests/frontend_coverage_ledger.rs` | **New** | Currency test: every `components/surfaces/` directory has a ledger row, and vice-versa. |

---

## Task 1: Formalize the frontend emission seam `[SEQUENTIAL base]`

**Files:**
- Create: `crates/vox-codegen/src/frontend_backend.rs`
- Modify: `crates/vox-codegen/src/lib.rs` (module list, near line 17–18 where `codegen_ts` / `emission_profile` are declared)
- Test: `crates/vox-codegen/tests/frontend_backend.rs`

- [ ] **Step 1: Write the failing parity + dispatch test**

Create `crates/vox-codegen/tests/frontend_backend.rs`:

```rust
//! The `frontend_backend` seam must (a) route `Target::TypeScript` to the exact
//! same output as calling `codegen_ts::generate_with_options` directly (no
//! behavior change — this is the Model 3 seam, not a rewrite), and (b) reject
//! non-frontend targets with a typed error rather than a silent fallthrough.

use vox_codegen::codegen_ts::CodegenOptions;
use vox_codegen::frontend_backend::emit_frontend;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::lex;
use vox_compiler::parser::parse;
use vox_compiler::target::Target;

// Mirrors the verified-working shape in apps/vox-mental-tracker/src/main.vox
// (state + controlled input via `bind`), so it is known to parse and lower.
const SRC: &str = r#"
component Hello() {
    state name: str = ""
    view: input(type="text", bind={name})
}
"#;

fn hir_for(src: &str) -> vox_compiler::hir::HirModule {
    lower_module(&parse(lex(src)).expect("parse"))
}

#[test]
fn typescript_target_matches_direct_emitter_output() {
    let hir = hir_for(SRC);
    let opts = CodegenOptions::default();

    let direct = vox_codegen::codegen_ts::generate_with_options(&hir, opts.clone())
        .expect("direct emit ok");
    let via_seam = emit_frontend(Target::TypeScript, &hir, opts).expect("seam emit ok");

    // Same emitted file set, byte-for-byte — proves zero behavior change.
    assert_eq!(
        via_seam.files, direct.files,
        "seam output must equal direct generate_with_options output"
    );
}

#[test]
fn backend_targets_are_rejected_with_typed_error() {
    let hir = hir_for(SRC);
    for t in [Target::RustAxum, Target::Interpreter, Target::RustTauri] {
        let err = emit_frontend(t, &hir, CodegenOptions::default())
            .expect_err("non-frontend target must error");
        assert!(
            err.contains("not a frontend emission target"),
            "expected typed seam error for {t:?}, got: {err}"
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-codegen --test frontend_backend`
Expected: FAIL — `unresolved import vox_codegen::frontend_backend` (module does not exist yet).

- [ ] **Step 3: Create the seam module**

Create `crates/vox-codegen/src/frontend_backend.rs`:

```rust
//! Formalized frontend-emission seam (Model 3 spine).
//!
//! Production emission is selected by [`vox_compiler::target::Target`] rather
//! than calling the TypeScript emitter directly. Today there is exactly one
//! concrete frontend backend — `Target::TypeScript` → React/TSX via
//! [`crate::codegen_ts::generate_with_options`]. Other targets are not frontend
//! emitters and return a typed error.
//!
//! `Target` is deliberately NOT `#[non_exhaustive]` (see
//! `vox_compiler::target`): adding a variant must break this `match`, forcing a
//! decision about how the new target participates in frontend emission. A future
//! leaner backend (Web Components / WASM) becomes a new arm here and touches
//! neither `web_ir::lower` nor `web_ir::validate`.

use vox_compiler::hir::HirModule;
use vox_compiler::target::Target;

use crate::codegen_ts::{generate_with_options, CodegenOptions, CodegenOutput};

/// Emit the frontend for `target` from a lowered `hir`.
///
/// # Errors
/// Returns `Err` if `target` is not a frontend emission target, or if the
/// underlying emitter fails.
pub fn emit_frontend(
    target: Target,
    hir: &HirModule,
    options: CodegenOptions,
) -> Result<CodegenOutput, String> {
    match target {
        Target::TypeScript => generate_with_options(hir, options),
        Target::RustTauri | Target::RustAxum | Target::Interpreter => Err(format!(
            "{} is not a frontend emission target (only Target::TypeScript emits the web frontend)",
            target.id()
        )),
    }
}
```

- [ ] **Step 4: Register the module**

In `crates/vox-codegen/src/lib.rs`, add alongside the existing `pub mod` declarations (e.g. directly after `pub mod emission_profile;` on line 18):

```rust
pub mod frontend_backend;
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p vox-codegen --test frontend_backend`
Expected: PASS (2 tests: `typescript_target_matches_direct_emitter_output`, `backend_targets_are_rejected_with_typed_error`).

- [ ] **Step 6: Confirm the error string matches the test assertion**

The test asserts the error contains `"not a frontend emission target"`. The module's `Err(format!(...))` contains exactly that substring. If you changed either, reconcile them now.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-codegen/src/frontend_backend.rs crates/vox-codegen/src/lib.rs crates/vox-codegen/tests/frontend_backend.rs
git commit -m "feat(codegen): formalize frontend emission seam on Target (Model 3 spine)"
```

---

## Task 2: Route `vox build --target client` through the seam `[SEQUENTIAL after 1]`

**Files:**
- Modify: `crates/vox-cli/src/commands/build.rs:217`

This is a pure call-site swap. `generate_with_options(&hir, ts_opts)` becomes `frontend_backend::emit_frontend(Target::TypeScript, &hir, ts_opts)`. Output is identical (Task 1 proved byte-equality), so existing build tests are the regression guard.

- [ ] **Step 1: Identify the current call site**

Run: `cargo test -p vox-cli` first to capture a green baseline (record the count of passing tests).
Expected: PASS baseline.

- [ ] **Step 2: Swap the call site**

In `crates/vox-cli/src/commands/build.rs`, replace the line at `:217`:

```rust
        let ts_output = vox_codegen::codegen_ts::generate_with_options(&hir, ts_opts)
            .map_err(|e| anyhow::anyhow!("TypeScript codegen error: {}", e))?;
```

with:

```rust
        let ts_output = vox_codegen::frontend_backend::emit_frontend(
            vox_compiler::target::Target::TypeScript,
            &hir,
            ts_opts,
        )
        .map_err(|e| anyhow::anyhow!("TypeScript codegen error: {}", e))?;
```

(`vox_compiler::target::Target` is already used in this file at line 135 — no new `use` needed.)

- [ ] **Step 3: Build to verify it compiles**

Run: `cargo build -p vox-cli`
Expected: success, no warnings about the changed lines.

- [ ] **Step 4: Run the CLI build tests to verify no regression**

Run: `cargo test -p vox-cli`
Expected: PASS — same count as the Step 1 baseline.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/build.rs
git commit -m "refactor(cli): route client build through frontend_backend seam"
```

---

## Task 3: Create the coverage ledger `[PARALLEL-SAFE]`

**Files:**
- Create: `docs/superpowers/ledgers/frontend-coverage-ledger.md`

The ledger is the measured baseline for the spec's "95–99%". It enumerates every top-level GUI surface directory under `crates/vox-gui/ui/src/components/surfaces/` and records whether it is `.vox`-expressible today, and if not, the blocking gap (referencing spec §3.2). Status values are exactly one of: `expressible`, `blocked:reactive-streams`, `blocked:interop`, `blocked:mobile`, `blocked:other`.

> The 25 surface directories are: Approvals, Browser, Catalog, Chat, Console, Coverage, Dashboard, Flow, Gamify, Harness, Loquela, Matrix, Memory, Mesh, Models, Policies, Publications, Repository, Research, Runs, Scientia, Search, Settings, SkillsPlugins, Tasks.

- [ ] **Step 1: Write the ledger file**

Create `docs/superpowers/ledgers/frontend-coverage-ledger.md`:

```markdown
---
title: "Frontend coverage ledger — .vox-expressibility of GUI surfaces"
category: "Architecture SSOTs"
status: living
date: 2026-06-20
---

# Frontend Coverage Ledger

Measures Sub-project A's baseline for the Vox-Native Frontend SSOT spec
(`docs/superpowers/specs/2026-06-20-vox-native-frontend-ssot-design.md`). One row
per top-level surface directory under `crates/vox-gui/ui/src/components/surfaces/`.
The `crates/vox-gui/tests/frontend_coverage_ledger.rs` currency test fails if a
surface is added or removed without updating this table.

**Status legend** (exactly one per row):
- `expressible` — renderable from `.vox` with today's authoring surface.
- `blocked:reactive-streams` — depends on `vox://*` stream subscription / effect
  deps / cleanup (spec §3.2 critical gap).
- `blocked:interop` — depends on an unfinished ecosystem-import slice (spec §3.3).
- `blocked:mobile` — depends on the mobile-first rule / PWA scaffold (spec §3).
- `blocked:other` — blocked on something else; note it in the Notes column.

> These statuses are the **initial audited estimate**. They are refined as
> Sub-projects B–G land. Counts here are the denominator for the 95–99% target.

| Surface | Status | Notes |
|---|---|---|
| Approvals | blocked:reactive-streams | live approval queue via agent-events stream |
| Browser | blocked:other | CDP frame mirror + native session commands |
| Catalog | expressible | mostly static command catalog rendering |
| Chat | blocked:reactive-streams | streamed tokens + secretary-proposed events |
| Console | blocked:other | PTY streams + xterm.js terminal emulation |
| Coverage | expressible | tabular report rendering |
| Dashboard | blocked:reactive-streams | live orch-status / widget streams |
| Flow | blocked:reactive-streams | live pipeline timeline events |
| Gamify | blocked:reactive-streams | ludus notifications stream |
| Harness | expressible | diff + repo file listing (request/response) |
| Loquela | blocked:reactive-streams | live agent conversation stream |
| Matrix | expressible | static matrix/grid rendering |
| Memory | expressible | recall/reindex request-response |
| Mesh | expressible | trusted-node list CRUD |
| Models | expressible | model cards + routing request-response |
| Policies | expressible | policy list/show request-response |
| Publications | blocked:reactive-streams | scientia-queue / discovery-surfaced events |
| Repository | expressible | repo file/branch listing |
| Research | blocked:reactive-streams | async research start + live progress |
| Runs | expressible | run list/detail request-response |
| Scientia | blocked:reactive-streams | review queue change pings |
| Search | expressible | query/result request-response |
| Settings | expressible | config get/set forms |
| SkillsPlugins | expressible | skill/plugin list rendering |
| Tasks | blocked:reactive-streams | tasks-changed live mutations |

## Summary

- Total surfaces: 25
- `expressible` today: 13
- `blocked:reactive-streams`: 10
- `blocked:other`: 2 (Browser, Console)

The dominant blocker is `reactive-streams` (Sub-project B), confirming the spec's
§8 risk call: closing the `vox://*` authoring gap is the make-or-break for 99%.
```

- [ ] **Step 2: Sanity-check the surface list against the filesystem**

Run: `ls crates/vox-gui/ui/src/components/surfaces/ | grep -E '/$|^[A-Z]' `
Expected: the 25 directory names listed in the ledger appear (loose `.ts`/`.tsx` files like `CommandCardsView.tsx`, `decoratorRegistry.ts` are NOT surfaces and are excluded). If the directory set differs from the 25 rows, update the table before committing.

- [ ] **Step 3: Commit**

```bash
git add docs/superpowers/ledgers/frontend-coverage-ledger.md
git commit -m "docs(ledger): seed frontend coverage ledger (25 surfaces, 13 expressible)"
```

---

## Task 4: Coverage-ledger currency test `[PARALLEL-SAFE after 3]`

**Files:**
- Create: `crates/vox-gui/tests/frontend_coverage_ledger.rs`

The test enforces that the ledger and the filesystem never drift: every directory under `components/surfaces/` has a ledger row, and every ledger row names a real directory. It reads files only (no Tauri/GUI feature needed), using `CARGO_MANIFEST_DIR` to locate both paths relative to the `vox-gui` crate root.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-gui/tests/frontend_coverage_ledger.rs`:

```rust
//! Drift guard: the frontend coverage ledger must list exactly the surface
//! directories present under `ui/src/components/surfaces/`. Adding or removing a
//! surface without updating the ledger fails CI — this is what keeps the
//! 95-99% denominator honest (spec Sub-project A / F).

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

fn surfaces_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("ui/src/components/surfaces")
}

fn ledger_path() -> PathBuf {
    // vox-gui crate root → workspace root (two levels up) → docs/...
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/superpowers/ledgers/frontend-coverage-ledger.md")
}

/// Directory names directly under `components/surfaces/`.
fn filesystem_surfaces() -> BTreeSet<String> {
    fs::read_dir(surfaces_dir())
        .expect("read surfaces dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect()
}

/// Surface names parsed from the first column of the ledger's markdown table.
/// A row is `| Name | status | notes |`; header/separator rows and the summary
/// section are skipped by requiring the second column to be a known status.
fn ledger_surfaces() -> BTreeSet<String> {
    const STATUSES: [&str; 5] = [
        "expressible",
        "blocked:reactive-streams",
        "blocked:interop",
        "blocked:mobile",
        "blocked:other",
    ];
    let text = fs::read_to_string(ledger_path()).expect("read ledger");
    let mut out = BTreeSet::new();
    for line in text.lines() {
        let cols: Vec<&str> = line.split('|').map(str::trim).collect();
        // Leading/trailing '|' produce empty first/last cells → cols[1], cols[2].
        if cols.len() >= 4 {
            let name = cols[1];
            let status = cols[2];
            if STATUSES.contains(&status) && !name.is_empty() {
                out.insert(name.to_string());
            }
        }
    }
    out
}

#[test]
fn ledger_matches_filesystem_surfaces() {
    let fs_set = filesystem_surfaces();
    let ledger_set = ledger_surfaces();

    let missing_from_ledger: Vec<_> = fs_set.difference(&ledger_set).collect();
    let stale_in_ledger: Vec<_> = ledger_set.difference(&fs_set).collect();

    assert!(
        missing_from_ledger.is_empty(),
        "surfaces present on disk but missing a ledger row: {missing_from_ledger:?} \
         — add rows to docs/superpowers/ledgers/frontend-coverage-ledger.md"
    );
    assert!(
        stale_in_ledger.is_empty(),
        "ledger rows naming non-existent surface dirs: {stale_in_ledger:?} \
         — remove or rename them in the ledger"
    );
}
```

- [ ] **Step 2: Run test to verify it passes (ledger already current from Task 3)**

Run: `cargo test -p vox-gui --test frontend_coverage_ledger`
Expected: PASS — `ledger_matches_filesystem_surfaces`. (Task 3 seeded all 25 rows, so the sets match on first run. This is the intended steady state; the test exists to fail on *future* drift.)

- [ ] **Step 3: Prove the drift guard actually fires (negative check)**

Temporarily create an empty surface dir, confirm the test fails, then remove it:

```bash
mkdir crates/vox-gui/ui/src/components/surfaces/ZzTemp
cargo test -p vox-gui --test frontend_coverage_ledger 2>&1 | grep -q "ZzTemp" && echo "DRIFT GUARD OK"
rmdir crates/vox-gui/ui/src/components/surfaces/ZzTemp
```

Expected: prints `DRIFT GUARD OK` (the test failed and named `ZzTemp`), then the dir is removed. Re-run `cargo test -p vox-gui --test frontend_coverage_ledger` → PASS again.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/tests/frontend_coverage_ledger.rs
git commit -m "test(gui): coverage-ledger currency drift guard"
```

---

## Definition of Done (Sub-project A)

- [ ] `emit_frontend` exists, dispatches on `Target` exhaustively, and the TS arm is byte-identical to the prior direct call (Task 1 parity test green).
- [ ] `vox build --target client` flows through the seam; `cargo test -p vox-cli` unchanged (Task 2).
- [ ] The coverage ledger exists with all 25 surfaces classified and a summary (Task 3).
- [ ] The currency test passes and provably fires on drift (Task 4).
- [ ] Full check before handoff:
  ```bash
  cargo test -p vox-codegen --test frontend_backend
  cargo test -p vox-cli
  cargo test -p vox-gui --test frontend_coverage_ledger
  cargo clippy -p vox-codegen -p vox-cli -- -D warnings
  ```
  Expected: all green. (Per repo guidance, exclude `vox-gui` from `--all-targets` clippy; the lib-only / single-test checks above are sufficient here.)

## What this deliberately does NOT do (deferred to B–G)

- No new authoring capability — the `vox://*` reactive-stream primitive is **Sub-project B**.
- No ecosystem-import changes — **Sub-project C**.
- No mobile-first rule / PWA scaffold — **Sub-project D**.
- No `voxup` toolchain automation — **Sub-project E**.
- No convergence CI gate forbidding off-SSOT `.tsx` — **Sub-project F**.
- No surface migration — **Sub-project G**.
