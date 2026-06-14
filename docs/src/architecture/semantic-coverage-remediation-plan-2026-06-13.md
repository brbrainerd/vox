---
title: "Semantic Coverage Remediation Plan (2026-06-13, v2 audited)"
description: "Audited, self-contained implementation plan to close the REAL semantic test-coverage gap. Includes a fidelity audit of the coverage map (measured false-positive/negative rates), the corrected symbol universe, a mandatory per-symbol verification protocol, worked test examples with real code, analyzer fixes, and a CI ratchet. Written to be executed by Claude Sonnet 4.6 task-by-task and to survive context compression."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---

# Semantic Coverage Remediation Plan v2 (audited)

> **For agentic workers:** REQUIRED SUB-SKILL — use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task, and `superpowers:test-driven-development` for every individual test. Steps use checkbox (`- [ ]`) syntax.
>
> **This document is self-contained on purpose.** If your context was compressed and you are resuming: read §A (what the data means), §C (how to verify a symbol before testing it), and §D (how to write the test), then continue at the first unchecked box in §F. Do **not** trust the raw coverage counts without §A.

**Goal:** Raise the share of the codebase whose behavior is *proven by an assertion* (not merely executed), focusing on error/edge/invariant proofs, while writing **zero** redundant or trivial tests — and install a CI ratchet so it cannot regress.

**Architecture:** A regenerated graphify semantic-coverage map (graph + 3 overlays) is the *prioritization heat-map*. Because the static overlay has measured false-positive/negative rates (§A), every candidate symbol passes a 3-gate verification protocol (§C) before a test is written. Tests follow three behavior-kind patterns with worked examples (§D). Wave 0 fixes the analyzer + builds the gate; Waves 1–5 work the corrected per-crate backlog.

**Tech Stack:** Rust workspace (118 crates), `cargo nextest`, `cargo llvm-cov`, the Python coverage-graph toolchain in [`scripts/coverage-graph/`](../../../scripts/coverage-graph/), graphify graph at `graphify-out/graph.json`.

---

## §A. Fidelity audit — READ THIS FIRST (why the raw numbers lie)

The map was regenerated on `main` tip `223558adf7` (2026-06-13). A fidelity audit (3 independent verification agents over a 30-unproven + 15-proven sample across `vox-db`, `vox-orchestrator`, `vox-gamify`) found the raw signal is **not** a worklist. Both error directions are real and large:

### A.1 The raw counts are inflated ~6× by non-symbols

The graph node set is "every identifier token seen," not "this crate's symbols." The raw Phase-1 report (`COVERAGE_MAP.md`) counts **46,622 "symbols" / 37,912 "unproven"**. After removing nodes that are not production-symbol definitions:

| Removed class | How to detect | Count |
|---|---|---|
| File nodes | `label` ends in `.rs` | 3,376 |
| Type references / std-use (`Option`, `Duration`, `Box`, external types) | node `id` starts with `crates_` (full-path form), not `src_` | 34,120 |
| In-`src/` `#[cfg(test)]` **test functions** (counted as prod symbols!) | label matches a detected `_origin:test` fn in the same file | 1,952 |
| `benches/`, `examples/`, `build.rs` defs | `source_file` not under `/src/` | ~600 |

**Corrected universe: 7,174 production symbol definitions in `src/`; 882 proven (12.3%); 6,292 unproven.** This is the real denominator. The persisted, cleaned worklist is **[`graphify-out/CANDIDATE_GAPS.md`](../../../graphify-out/CANDIDATE_GAPS.md)** (regenerate: `python scripts/coverage-graph/candidate_gaps.py`).

### A.2 The ranking flips — the v1 priorities were wrong

Ranking by raw unproven sent effort to the wrong crates. Corrected (cleaned production defs):

| Crate | Raw unproven (v1) | **Cleaned unproven (v2)** | Note |
|---|---|---|---|
| vox-cli | 4,605 (#1) | **212 (#8)** | raw count was clap command/catalog data, not behavior |
| vox-compiler | 2,524 (#3) | **91 (#16)** | raw count was HIR type-references |
| vox-orchestrator | 4,088 | **789 (#1)** | genuinely the largest |
| vox-orchestrator-mcp | 2,064 | **547 (#2)** | |
| vox-research-events | 551 | **402 (#3)** | was #15 raw; surfaces as real |
| vox-db | 1,949 | **361 (#4)** | |
| vox-publisher | 1,461 | **357 (#5)** | |

### A.3 Even the cleaned "unproven" list is ~2/3 false candidates

Measured on the 30-symbol sample (treat as directional, wide CI, but the mechanism is structural and certain):

| Verdict | Share | Meaning |
|---|---|---|
| **GENUINELY_UNPROVEN** | **~33%** | real, actionable gap — write a test |
| **COVERED_ELSEWHERE** (false negative) | **~47%** | already tested; writing a test is redundant — the wasted effort the maintainer rejects |
| **NOT_A_BEHAVIOR** | **~20%** | trivial getter/DTO/clock/re-export — a test would be line-touching |

**Implication:** the ~6,292 candidates contain only **~2,000 genuine gaps**. Per-symbol verification (§C) is mandatory; skipping it reproduces exactly the "fragile tests that touch a lot of code uselessly" the maintainer is trying to eliminate.

### A.4 Root causes (so the analyzer can be fixed in Wave 0)

1. **Methods can never be "proven."** `overlay_tests.py` `build_name_index` drops every label starting with `.` (leading-dot, i.e. `.foo()` method labels) from edge targets ([`overlay_tests.py:127`](../../../scripts/coverage-graph/overlay_tests.py)). So **every method symbol is structurally false-negative.** Confirmed: `.redact()`, `.with_obo_token()`, `.new()` (circuit_breaker), `.to_markdown()` all have passing tests yet show unproven.
2. **Only same-crate assertions are credited.** Cross-crate proof requires the symbol name to be globally unique ([`overlay_tests.py:503-510`](../../../scripts/coverage-graph/overlay_tests.py)). Integration tests in `tests/` dirs and sibling crates that assert on a symbol are dropped → constructors/builders tested via integration flows show unproven. Confirmed: `SnapshotStore`, `RiskScoreEvent`, `get_research_artifact`, `scan_workspace`.
3. **`impl Type {` markers count as the type.** An `impl VoxDb {` line creates a `VoxDb` node; an assertion naming `VoxDb` elsewhere marks it "proven" though it is an impl marker → ~20% false-positive-proven.
4. **`proves` = "named inside/near an assertion,"** not "the asserted value originates from the symbol" ([`overlay_tests.py:520-556`](../../../scripts/coverage-graph/overlay_tests.py)). Setup values and constructors inside an `assert!(...)`/within 80 chars of `.unwrap()` get spurious proof. This *over*-counts proven (safe direction: real gap ≥ reported).
5. **`.vox` goldens are never parsed.** Only `crates/**/*.rs` is scanned; behaviors proven by the 600+ goldens produce no edge.

> **Net:** "unproven" is **biased toward false-negatives** (methods, integration, goldens), and the symbol universe is noise-dominated. The map is a heat-map for *where to look*, never a list of *guaranteed-missing tests*.

---

## §B. The corrected backlog (the real prioritization)

Source of truth: **[`graphify-out/CANDIDATE_GAPS.md`](../../../graphify-out/CANDIDATE_GAPS.md)** — 6,292 cleaned candidates as `- [ ]` lines grouped by crate, each with `symbol @ file:line`. Per-crate genuine-gap estimate ≈ `cleaned_unproven × ~0.33`.

| # | Crate | Cleaned defs | Proven | Cleaned unproven | ~Genuine (×0.33) |
|---|---|---|---|---|---|
| 1 | vox-orchestrator | 867 | 78 | 789 | ~260 |
| 2 | vox-orchestrator-mcp | 588 | 41 | 547 | ~180 |
| 3 | vox-research-events | 420 | 18 | 402 | ~130 |
| 4 | vox-db | 412 | 51 | 361 | ~120 |
| 5 | vox-publisher | 388 | 31 | 357 | ~120 |
| 6 | vox-gamify | 307 | 32 | 275 | ~90 |
| 7 | vox-actor-runtime | 275 | 33 | 242 | ~80 |
| 8 | vox-cli | 252 | 40 | 212 | ~70 |
| 9 | vox-codegen-ts | 165 | 24 | 141 | ~47 |
| 10 | vox-search | 149 | 11 | 138 | ~46 |
| 11 | vox-code-audit | 151 | 25 | 126 | ~42 |
| 12 | vox-audit | 139 | 28 | 111 | ~37 |
| 13 | vox-config | 144 | 34 | 110 | ~36 |
| 14 | vox-speech | 124 | 15 | 109 | ~36 |
| 15 | vox-sql | 111 | 15 | 96 | ~32 |
| 16 | vox-compiler | 118 | 27 | 91 | ~30 |

Full 104-crate table is the regenerable `CANDIDATE_GAPS.md`. Frozen-core crates (`vox-compiler`, `vox-db`, `vox-orchestrator`, `vox-actor-runtime` — see [`layers.toml`](layers.toml)) keep priority within their tier even when their count is mid-table, because a wrong behavior there has the largest blast radius.

**Caveats that further shrink the real target (apply judgment in §C Gate 3):** the cleaned list still includes private helpers (lower priority — often covered transitively) and plain data structs (need only a serde round-trip or no test). Prefer `pub` symbols with real branching/error/invariant logic.

---

## §C. The per-symbol verification protocol (MANDATORY before any test)

Run this for **every** candidate before writing a test. It is the entire defense against false positives/negatives. Budget ~2–4 minutes per symbol.

```
For symbol S at crates/<crate>/src/<file>.rs:Lnn :

GATE 1 — Is S a real, testable production symbol?
  - Read the definition at file:Lnn.
  - REJECT (skip, mark n/a) if S is:
      * an `impl Type {` line (not a fn/method/real type),
      * a re-export / `use` / `super::X`,
      * a derive-only `.default()` / `.clone()`,
      * a one-line pure getter or `.as_str()`/`.from_str()` with no branching,
      * a plain data struct whose only logic is `#[derive(...)]`
        (UNLESS it has a custom Serialize/Deserialize or invariants — then a
         round-trip test IS warranted).
  - If S has real branching, error handling, ordering, arithmetic, or state
    transition → continue.

GATE 2 — Is S already proven somewhere the static map missed? (catches the ~47% false-negatives)
  Search ALL surfaces for an EXISTING assertion on S's behavior:
    rg -n "\bS\b" crates/<crate>/src        # in-crate #[cfg(test)] (esp. methods!)
    rg -n "\bS\b" crates/<crate>/tests       # this crate's integration tests
    rg -n "\bS\b" crates --glob '*/tests/**' # OTHER crates' integration tests
    rg -n "<behavior-or-sql-or-string-S-produces>" examples/golden  # .vox goldens
  Open each hit. If a test constructs S (or its receiver) and ASSERTS on the
  value/effect S produces → S is COVERED_ELSEWHERE. Mark it proven-elsewhere,
  do NOT write a test. (This is the redundant-test trap.)

GATE 3 — Is the missing proof worth writing? (catches the ~20% trivial)
  - Identify which behavior kinds are UNPROVEN: happy / error / edge / invariant.
  - If S already has a happy-path test (check §D Phase-2 map COVERAGE_BEHAVIORS_<crate>.md
    "Semantic gaps" list) but no error/edge → write ONLY the missing kind.
  - If S is genuinely untested and non-trivial → write happy + the error/edge/
    invariant that matters.

ONLY symbols that pass Gates 1–3 get a test, written per §D.
```

> Cross-check shortcut: `COVERAGE_BEHAVIORS_<crate>.md` (Phase-2 LLM map) lists symbols that already have a *happy-path* proof and which kind is missing — use it to jump straight to the right gap for symbols it covered. But it is breadth-first/shallow on giant crates, so absence from it ≠ untested; Gate 2 is still required.

---

## §D. How to write the tests (three behavior kinds, with real worked examples)

A *meaningful* test asserts an **observable** result — a return value or an effect — including failure and boundary paths. It is NOT `assert!(true)`, NOT "call it and check it didn't panic," NOT a snapshot of an entire struct that breaks on any field change. Match the crate's existing test style (look at a neighboring `#[cfg(test)]` module first).

The three kinds, ranked by value:

- **error** — an `Err`/panic/failure path is asserted (highest value; most often missing).
- **edge** — empty/boundary/duplicate/overflow input produces the right result.
- **invariant** — a property that must always hold (ordering, idempotence, round-trip, monotonicity).
- (happy — nominal success; write it only if absent.)

### Worked example 1 — invariant + edge (pure function)

Target: `merge_tags` — [`crates/vox-orchestrator/src/context_lifecycle.rs:404`](../../../crates/vox-orchestrator/src/context_lifecycle.rs). Private `fn merge_tags(prev: &[String], incoming: &[String]) -> Vec<String>` that dedups, trims, skips empty, preserves first-seen order. Gate verdict: GENUINELY_UNPROVEN (internal helper, no test asserts it). Tests live in the same file's `#[cfg(test)] mod` (private fn).

- [ ] **Step 1 — write the failing tests** (add to the `#[cfg(test)] mod tests` in `context_lifecycle.rs`; create the module if absent, following a neighboring crate file's pattern):

```rust
#[test]
fn merge_tags_dedupes_preserving_first_seen_order() {
    let prev = vec!["a".to_string(), "b".to_string()];
    let incoming = vec!["b".to_string(), "c".to_string()];
    // invariant: each tag once, in first-seen order
    assert_eq!(merge_tags(&prev, &incoming), vec!["a", "b", "c"]);
}

#[test]
fn merge_tags_trims_and_skips_empty_and_whitespace() {
    let prev = vec!["  a  ".to_string(), String::new()];
    let incoming = vec!["   ".to_string(), "a".to_string(), "b".to_string()];
    // edge: "  a  " -> "a"; empty + whitespace-only dropped; "a" dedup'd
    assert_eq!(merge_tags(&prev, &incoming), vec!["a", "b"]);
}
```

- [ ] **Step 2 — run, expect FAIL→PASS** (the fn already exists, so these should PASS immediately if your understanding is right; if a test FAILS, you found a real bug — stop and report it, do not "fix" the test to match buggy behavior):

Run: `cargo nextest run -p vox-orchestrator merge_tags`
Expected: 2 passed.

- [ ] **Step 3 — commit:**

```bash
git add crates/vox-orchestrator/src/context_lifecycle.rs
git commit -m "test(orchestrator): prove merge_tags dedup/order/trim invariants"
```

### Worked example 2 — error path + edge (fallible async fn)

Target: `import_orchestrator_memory_dir` — [`crates/vox-db/src/legacy_import_extras.rs:18`](../../../crates/vox-db/src/legacy_import_extras.rs). `pub async fn (...) -> Result<u64, StoreError>`: returns `Err(StoreError::Db("not a directory: ..."))` for a non-dir; otherwise ingests `*.md` files and returns the inserted count, skipping non-`.md`/non-file entries. Gate verdict: GENUINELY_UNPROVEN.

- [ ] **Step 1 — find the crate's VoxDb test fixture (do NOT invent it).** Read an existing `vox-db` test to see how a test `VoxDb` is built:

Run: `rg -n "VoxDb::(connect|open|new)|in_memory|tempdir" crates/vox-db/tests | head`
Use whatever constructor the existing tests use (e.g. an in-memory or tempdir-backed `VoxDb`). Confirm `StoreError`'s variant name by reading its definition: `rg -n "enum StoreError" crates/vox-db/src`.

- [ ] **Step 2 — write the failing tests** in `crates/vox-db/tests/legacy_import_extras_test.rs` (new file; mirror the imports/setup of a neighboring `crates/vox-db/tests/*.rs`):

```rust
// Construct `db` with the SAME fixture pattern the other vox-db tests use (Step 1).
#[tokio::test]
async fn import_orchestrator_memory_dir_errors_on_non_directory() {
    let db = /* crate's standard test VoxDb fixture from Step 1 */;
    let missing = std::path::Path::new("definitely/not/a/dir");
    let err = vox_db::legacy_import_extras::import_orchestrator_memory_dir(
        &db, missing, "agent", "session",
    ).await.unwrap_err();
    // error path: must be the Db variant and name the failure
    assert!(matches!(err, vox_db::StoreError::Db(_)));
    assert!(err.to_string().contains("not a directory"));
}

#[tokio::test]
async fn import_orchestrator_memory_dir_counts_only_markdown() {
    let db = /* same fixture */;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.md"), "# hello").unwrap();
    std::fs::write(dir.path().join("b.txt"), "ignored").unwrap();
    let n = vox_db::legacy_import_extras::import_orchestrator_memory_dir(
        &db, dir.path(), "agent", "session",
    ).await.unwrap();
    // edge: non-.md entries are skipped
    assert_eq!(n, 1, "only the .md file should be ingested");
}
```

- [ ] **Step 3 — run, expect PASS** (fix only compile errors from fixture/variant names, never weaken an assertion):

Run: `cargo nextest run -p vox-db import_orchestrator_memory_dir`
Expected: 2 passed. If `tempfile` is not a dev-dependency, add it: `cargo add --dev tempfile -p vox-db`.

- [ ] **Step 4 — commit:**

```bash
git add crates/vox-db/tests/legacy_import_extras_test.rs crates/vox-db/Cargo.toml
git commit -m "test(db): prove import_orchestrator_memory_dir non-dir error + md-only edge"
```

### Worked example 3 — branch coverage (override vs fallback)

Target: `EventConfigOverrides::resolve` — [`crates/vox-gamify/src/reward_policy.rs:418`](../../../crates/vox-gamify/src/reward_policy.rs). `pub fn resolve(&self, event_type: &str) -> BaseReward`: returns the stored override if present, else `base_reward(event_type)`. Both branches unproven. `BaseReward` has fields `xp`, `crystals` (from `BaseReward::new(xp, crystals)`).

- [ ] **Step 1 — write the failing tests** in the `#[cfg(test)] mod` of `reward_policy.rs`:

```rust
#[test]
fn resolve_returns_override_when_present() {
    let mut o = EventConfigOverrides::default();
    o.set("quest.done", 999, 42);
    let r = o.resolve("quest.done");
    assert_eq!(r.xp, 999);          // override branch
    assert_eq!(r.crystals, 42);
}

#[test]
fn resolve_falls_back_to_policy_base_when_absent() {
    let o = EventConfigOverrides::default();
    // fallback branch: identical to the policy base for that event
    assert_eq!(o.resolve("quest.done"), base_reward("quest.done"));
}
```

- [ ] **Step 2 — run, expect PASS** (confirm `BaseReward` derives `PartialEq`; if not, assert the two fields instead of the whole struct):

Run: `cargo nextest run -p vox-gamify reward_policy::`
Expected: 2 passed.

- [ ] **Step 3 — commit:**

```bash
git add crates/vox-gamify/src/reward_policy.rs
git commit -m "test(gamify): prove EventConfigOverrides::resolve override + fallback branches"
```

> **General rules distilled from the examples:** (1) one behavior per test, named for the behavior; (2) assert the *value/effect*, not "no panic"; (3) for `Result`, assert the `Err` variant AND a message substring; (4) for collections, assert order/contents, not just length; (5) if a test you expect to pass FAILS, you've found a bug — report it, never weaken the assertion to green.

---

## §E. Wave 0 — fix the analyzer, then build the ratchet (DO FIRST)

The map cannot be a CI gate until it stops mislabeling methods, integration tests, and impl-markers (§A.4). And remediation is pointless if new code lands unproven. Wave 0 is the prerequisite for everything.

### Task 0.1 — exclude non-symbols from the report denominator

**File:** [`scripts/coverage-graph/overlay_tests.py`](../../../scripts/coverage-graph/overlay_tests.py) `_write_report` (around L598–612).

- [ ] **Step 1 — write the failing test.** Add `scripts/coverage-graph/test_overlay_report.py` asserting that, given a tiny graph containing a file node (`label="x.rs"`), an `impl`-marker node, an in-`src` test fn, and one real `src_` fn, the per-crate `Symbols` count includes only the real fn.
- [ ] **Step 2 — run:** `python scripts/coverage-graph/test_overlay_report.py` → FAIL.
- [ ] **Step 3 — implement:** in `_write_report`, filter `crate_symbols` to nodes where `node["id"].startswith("src_")`, `not label.endswith(".rs")`, `"/src/" in source_file`, and label-not-in the detected test-fn set (pass the test-fn set in from `run_overlay`). This makes the report match `candidate_gaps.py`.
- [ ] **Step 4 — run → PASS; regenerate** `COVERAGE_MAP.md` and confirm totals ≈ 7,174 defs / 6,292 unproven (not 46,622 / 37,912).
- [ ] **Step 5 — commit.**

### Task 0.2 — credit method (`.foo()`) assertions

**File:** `overlay_tests.py` `build_name_index` (L121–143) + resolution (L477–510).

- [ ] **Step 1 — failing test:** a test body `assert_eq!(x.redact(s), "***")` where `redact` is a method def in the same crate must produce a `proves` edge to the method symbol. Currently it does not (leading-dot labels are dropped).
- [ ] **Step 2 — run → FAIL.**
- [ ] **Step 3 — implement:** stop discarding leading-dot labels from the index; instead index method symbols under their bare method name (`strip(".redact()")=="redact"`) in a separate `method_index`. When an identifier in an assertion is immediately preceded by `.` in the body, resolve it against `method_index` for the test's crate. Keep the same-crate precedence. (Receiver-type resolution is out of scope; same-crate method-name match is sufficient and matches the existing same-crate heuristic.)
- [ ] **Step 4 — run → PASS;** regenerate; confirm method-heavy crates' proven counts rise and the false-negative method bias drops.
- [ ] **Step 5 — commit.**

### Task 0.3 — credit cross-crate integration-test assertions

**File:** `overlay_tests.py` resolution path (L502–510).

- [ ] **Step 1 — failing test:** an assertion in `crates/A/tests/it.rs` on a `pub` symbol defined in crate `B` must produce a `proves` edge even when the name is not globally unique, IF the test file `use`s `B`'s path for it. 
- [ ] **Step 2 — run → FAIL.**
- [ ] **Step 3 — implement:** when a candidate is cross-crate and not unique, parse the test file's `use ::B::...` / `B::Symbol` paths to disambiguate to the crate actually imported; credit that one. Fall back to current behavior if no import resolves.
- [ ] **Step 4 — run → PASS;** regenerate; confirm integration-tested constructors (e.g. `SnapshotStore`, `RiskScoreEvent`) flip to proven.
- [ ] **Step 5 — commit.**

> After 0.1–0.3, re-run the §A audit sample to re-measure the false-negative rate; it should fall well below the ~47% baseline. Record the new rate in this section.

### Task 0.4 — `vox ci semantic-coverage` gate (the ratchet)

**Files:** new subcommand mirroring [`crates/vox-effort-audit`](../../../crates/vox-effort-audit); wire into [`.github/workflows/ci.yml`](../../../.github/workflows/ci.yml) next to `coverage-gates`.

- [ ] **Step 1 — committed baseline:** generate `contracts/reports/semantic-coverage.v1.json` = per-crate `{cleaned_defs, proven}` from the *fixed* overlay.
- [ ] **Step 2 — failing test:** the gate fails when a crate's proven ratio drops below its committed floor.
- [ ] **Step 3 — implement** the gate to (a) load the overlay, (b) compare per-crate proven ratio to the floor, (c) fail on any drop. Run on `push:main` and PRs.
- [ ] **Step 4 — assertion-free-test warning (warn-only first):** list test fns with `targets` but zero `proves` edges (candidate line-touching tests) — the anti-pattern this initiative rejects.
- [ ] **Step 5 — commit + wire into CI.**

### Task 0.5 — refresh Phase 0 (reached) and document the local route

Phase 0 (reached-but-unproven) is carried from 2026-06-07 (`REACHED_VS_PROVEN.md`); fresh lcov needs CI instrumentation (local `cargo llvm-cov export` is Windows-blocked, `os error 206`).

- [ ] **Step 1 — refresh** from the next green `main` CI `llvm-cov` artifact: `gh run download <id> -n llvm-cov -D target/` then `python scripts/coverage-graph/ingest_reaches.py --lcov target/llvm-cov-lcov.info --graph graphify-out/graph.json --out graphify-out/graph.json --report graphify-out/REACHED_VS_PROVEN.md`.
- [ ] **Local alternative:** if a fresh `cargo llvm-cov nextest --workspace --no-fail-fast` profile exists (`target/llvm-cov-target/vox.profdata`), export with [`scripts/coverage-graph/export_lcov_chunked.py`](../../../scripts/coverage-graph/export_lcov_chunked.py) (chunks the 600+ `-object` list past the Windows arg limit), then ingest. Reached-but-unproven is the highest-value subset to drain (line coverage's blind spot).

---

## §F. Waves 1–5 — work the corrected backlog

**The per-crate loop (identical for every crate `C`):**

- [ ] **F-loop Step 1** — open the crate's section in [`CANDIDATE_GAPS.md`](../../../graphify-out/CANDIDATE_GAPS.md) and its [`COVERAGE_BEHAVIORS_C.md`](../../../graphify-out/) (happy-only gaps).
- [ ] **F-loop Step 2** — for each candidate, run the §C protocol (Gates 1–3). Expect to reject ~2/3. Tick the candidate's box in `CANDIDATE_GAPS.md` as `n/a (covered)`, `n/a (trivial)`, or keep it for testing.
- [ ] **F-loop Step 3** — for each surviving symbol, write the missing-kind test(s) per §D (error/edge/invariant first). Commit per symbol or per file.
- [ ] **F-loop Step 4** — `cargo nextest run -p C` + `cargo clippy -p C -- -D warnings` green; regenerate the overlay for `C` and confirm proven rose; raise `C`'s floor in `semantic-coverage.v1.json` (Task 0.4).

Process crates in this order (criticality × corrected gap size):

### Wave 1 — frozen core
- [ ] **vox-compiler** (cleaned unproven 91) — parser/HIR/typeck **error paths** (malformed source → specific diagnostic), evaluator **invariants**. Note: many raw "gaps" were type-refs; the real list is small and high-value.
- [ ] **vox-db** (361) — migration safety error paths, query-validation `Err`, round-trip **invariants**. Worked example 2 lives here.
- [ ] **vox-orchestrator** (789, #1) — start from the silent-drop sites in [`semantic-gap-audit-2026.md`](semantic-gap-audit-2026.md) F2–F6 (reliability/lineage/budget persistence) — prove the **failure** paths; then `merge_tags` (worked example 1), a2a bus, dispatch, budget enforcement.
- [ ] **vox-actor-runtime** (242) — activity timeout/retry/backoff **error** paths.

### Wave 2 — data/agent spine
- [ ] **vox-orchestrator-mcp** (547, #2) — tool-call parse/validation `Err`, a2a surfacing rules, idempotency-key invariants.
- [ ] **vox-research-events** (402, #3) — event payload validation + serde round-trip invariants (many are event structs — apply Gate 1: round-trip test only if custom (de)serialize or invariants).
- [ ] **vox-publisher** (357) — adapter **error** paths (auth refresh, rate-limit, malformed response).
- [ ] **vox-code-audit** (126) — detector outcomes on positive AND negative fixtures.
- [ ] **vox-scientia** — claim decompose/extract edge cases; novelty invariants.

### Wave 3 — happy-only quick wins (add the missing error/edge to already-proven symbols)
Drive purely from each `COVERAGE_BEHAVIORS_<crate>.md` "Semantic gaps (proven happy-path only)" list — these are pre-verified to have a happy test, so Gate 2 is mostly satisfied; add the missing failure/boundary assertion.
- [ ] **vox-gamify** (worked example 3), **vox-config**, **vox-audit**, **vox-speech**, **vox-actor-runtime** residual, **vox-orchestrator-queue**, **vox-research-shim**.

### Wave 4 — user-facing surface
- [ ] **vox-cli** (212, not 4,605 — mostly dispatch/catalog), **vox-gui**, **vox-codegen-ts** (141), **vox-rn-codegen**.

### Wave 5 — long tail
- [ ] Remaining crates from `CANDIDATE_GAPS.md` top-down. Skip confirmed-dead code (graph in-degree + audit method). `*-types` crates: serde round-trip / ordering invariants only.

---

## §G. Definition of done, regeneration, persistence

**Done =** (1) Wave 0 analyzer fixes merged and the re-measured false-negative rate recorded in §E; (2) `vox ci semantic-coverage` enforcing on CI; (3) every frozen-core + spine crate's `pub` symbols either proven or marked `n/a` with reason; (4) workspace proven ratio (cleaned denominator) climbs from **12.3%** past agreed milestones (25% → 40%), measured by the fixed overlay; (5) `REACHED_VS_PROVEN.md` refreshed and its reached-but-unproven set being drained.

**Regenerate the whole map** (when `main` moves):
```bash
PYG=$(cat graphify-out/.graphify_python)
$PYG scripts/coverage-graph/rebuild_full_graph.py . graphify-out/graph.full.json          # base AST graph
$PYG scripts/coverage-graph/overlay_tests.py --graph graphify-out/graph.full.json --repo-root . --out graphify-out/graph.coverage.json --report graphify-out/COVERAGE_MAP.md   # Phase 1
$PYG scripts/coverage-graph/candidate_gaps.py --graph graphify-out/graph.coverage.json --out graphify-out/CANDIDATE_GAPS.md                                                    # cleaned worklist
# Phase 2 (LLM, THROTTLED — chunks of 8; never one 16-wide burst over all crates):
#   Workflow scripts/coverage-graph/phase2_extract_v2.js (args = JSON array of crate names)
$PYG scripts/coverage-graph/recover_and_synth.py --journal <journal.jsonl> --out-dir graphify-out
$PYG scripts/coverage-graph/build_index.py --journal <journal.jsonl> --out-dir graphify-out
$PYG scripts/coverage-graph/merge_behaviors_to_graph.py --journals-list graphify-out/_our_journals.txt --graph graphify-out/graph.coverage.json --out graphify-out/graph.semantic.json
cp graphify-out/graph.semantic.json graphify-out/graph.json
# Phase 0: §E Task 0.5.
```

**Persistence map (what to re-read after compression):**
- This plan (§A audit, §C protocol, §D examples) — the methodology.
- `graphify-out/CANDIDATE_GAPS.md` — the cleaned per-symbol worklist (regenerable).
- `graphify-out/COVERAGE_BEHAVIORS_<crate>.md` — per-crate happy-only gaps (Phase 2).
- `graphify-out/COVERAGE_MAP.md` / `REACHED_VS_PROVEN.md` — Phase 1 / Phase 0 counts.
- Memory: `[[project_semantic_test_coverage_graph_2026]]`, `[[feedback_graphify_large_extraction_throttle]]`.

## §H. Related
- [`semantic-test-coverage-graph-strategy-2026-06-07.md`](semantic-test-coverage-graph-strategy-2026-06-07.md) — strategy SSOT (reached<targeted<proven).
- [`semantic-gap-audit-2026.md`](semantic-gap-audit-2026.md) + [`semantic-gap-implementation-plan-2026.md`](semantic-gap-implementation-plan-2026.md) — the earlier 8-finding audit (F1–F6 are concrete Wave 1/2 error-path targets).
- [`scripts/coverage-graph/README.md`](../../../scripts/coverage-graph/README.md) — toolchain runbook.
- [`.config/coverage-gates.toml`](../../../.config/coverage-gates.toml) — the line-coverage gate this complements (the "useless touch" signal this plan deliberately does not optimize for).
