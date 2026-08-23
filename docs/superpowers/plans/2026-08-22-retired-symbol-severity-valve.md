# Retired-Symbol Severity Valve Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give `contracts/documentation/retired-symbols.v1.yaml` a per-symbol `warn`/`error` severity, so new contract entries can be added ahead of a corpus repair without making the tree unmergeable.

**Architecture:** `retired_symbol_check.rs` currently accumulates every hit into one `Vec<String>` and fails the whole run if it's non-empty. Add an optional `severity` field to each contract entry (default `error`, so every existing entry is unaffected), thread it through `scan_source_lines`'s return type, and partition at the top of `run()`: warnings print to stdout and never fail the build, errors behave exactly as today. This is the exact shape `crate_edges.rs` already uses in this crate for its own stale-baseline warnings — same partition, same `warning:` stdout prefix, same untouched `Result<()>` signature.

**Tech Stack:** Rust (`vox-cli-ci`), YAML contract.

**Spec:** `docs/superpowers/specs/2026-08-22-docs-corpus-repair-design.md` (revision 3), workstream W3.6 — "the sequencing rule from revision 1 is INVERTED": adding contract entries before the corpus is repaired produces an estimated 460–620 hard CI failures today (`crates/vox-dashboard` alone is ~200+), because the detector has no severity tier. This plan is the prerequisite W3.6 names.

## Global Constraints

- **No new crate, no new dependency.** `regex` and `serde` are already dependencies of `vox-cli-ci`.
- **`run()` keeps its exact `Result<()>` signature.** It is called with `?` from three sites, one of them (`run_body.rs:126`, inside `harness-trust-guard`) a security-relevant path — a warning must never be able to fail it, and the signature staying the same means none of the three call sites need to change.
- **Do not add any `warn`-severity entries to the contract in this plan.** This plan builds the mechanism only. Populating it (W3.1: `vox-dashboard`, `vox-oratio`, `vox-dei-shim`, `@endpoint`, the decorator class) is separate work, sequenced strictly after this lands — see spec W3.6.
- **Verification tier:** `--full`, not `--complete` (`--complete` runs no tests).
- **Line endings LF** for `rs` and `yaml`.
- **One agent per worktree; no checker enters this plan until it has been run against the real tree and its actual output pasted into the step** — the standing rule from the two sibling plans, restated here because it is what caught three of the four defects those plans shipped with.

---

## File Structure

| File | Responsibility |
| --- | --- |
| `crates/vox-cli-ci/src/retired_symbol_check.rs` | `SymbolSeverity` enum (contract gains an optional `severity: warn \| error` key, default `error` -- no contract file is edited by this plan); `RetiredSymbol` gains the field; `scan_source_lines` returns `(SymbolSeverity, String)` pairs instead of bare `String`; `run()` partitions and reports |

No GUI, no CLI flag, no schema file (`contracts/documentation/` has exactly one `.schema.json` today, for a different contract, and nothing validates it — adding an unread schema file here would be the same pattern this whole program exists to stop building).

---

### Task 1: Add `severity` to the contract, the enum, and thread it through scanning

**Files:**
- Modify: `crates/vox-cli-ci/src/retired_symbol_check.rs`

The contract YAML is deliberately **not** touched in this plan. `severity` is
`#[serde(default)]`, so writing `severity: error` onto an entry exercises the
identical code path as omitting it -- it would prove nothing the Step 2 unit
test doesn't already prove, while putting a no-op diff in a production
contract. The first real contract edit belongs to W3.1, which adds actual
`warn` entries.

**Interfaces:**
- Consumes: nothing from outside this plan.
- Produces: `enum SymbolSeverity { Warn, Error }` (default `Error`), `RetiredSymbol.severity: SymbolSeverity`, `fn scan_source_lines(...) -> Vec<(SymbolSeverity, String)>` (signature change from `Vec<String>`), consumed by Task 2.

- [ ] **Step 1: Read the exact current shapes before changing them**

```bash
sed -n '1,45p' crates/vox-cli-ci/src/retired_symbol_check.rs
sed -n '136,150p' crates/vox-cli-ci/src/retired_symbol_check.rs
sed -n '265,290p' crates/vox-cli-ci/src/retired_symbol_check.rs
```

Confirm: `RetiredSymbol` at line ~14 has `id`, `pattern`, `replacement`, `rationale`, `scan_rust_source` (with `#[serde(default)]`). Line numbers below `scan_source_lines` have likely shifted from a concurrent, unrelated change (Task 2 of the sibling gate-and-policy-honesty plan adds a `first_cell_only` function above it) -- **do not trust the numbers in this brief; re-read the file fresh** with:

```bash
grep -n 'fn scan_source_lines\|let mut failures = Vec::new();\|failures.push(format!' crates/vox-cli-ci/src/retired_symbol_check.rs
```

and use the real line numbers you get back. Confirm `scan_source_lines`'s real signature -- it takes **five** parameters, and the fourth is a **slice of `(&RetiredSymbol, Regex)` pairs**, not a single symbol/regex pair:

```rust
fn scan_source_lines(
    path: &Path,
    root: &Path,
    body: &str,
    regexes: &[(&RetiredSymbol, Regex)],
    cfg: ScanCfg,
) -> Vec<String>
```

The `sym` variable (the matching `RetiredSymbol`) is in scope at the push site inside this function's loop body.

- [ ] **Step 2: Add the imports the new tests need, then write the failing tests**

The existing `#[cfg(test)] mod tests` (near the end of the file) currently imports only `use super::{first_cell_only, should_skip_rust_line};` -- extend it, since both new tests below need more:

```rust
use super::{
    RetiredSymbol, ScanCfg, SymbolPolicy, SymbolSeverity, first_cell_only, run,
    scan_source_lines, should_skip_rust_line,
};
use regex::Regex;
use std::path::Path;
```

(`run` is needed for Task 2's test, added later; include it now so this import block only needs writing once.)

Add to the same module:

```rust
    #[test]
    fn symbol_severity_defaults_to_error_when_absent() {
        // Every symbol in the real contract omits `severity:` today. Confirm
        // that absence still deserializes to Error, so this change is a
        // strict no-op for the existing 14 entries until someone opts in.
        let yaml = r#"
symbols:
  - id: test-symbol
    pattern: "\\btest-symbol\\b"
    replacement: "replacement"
    rationale: "test fixture"
"#;
        let parsed: SymbolPolicy = serde_yaml::from_str(yaml).expect("parse fixture");
        assert_eq!(parsed.symbols[0].severity, SymbolSeverity::Error);
    }

    #[test]
    fn symbol_severity_warn_partitions_out_of_failures() {
        // A `severity: warn` symbol's hits must not appear in the Vec that
        // makes run() bail — they are reported separately.
        let sym = RetiredSymbol {
            id: "test-warn-symbol".to_string(),
            pattern: r"\btest-warn-symbol\b".to_string(),
            replacement: "replacement".to_string(),
            rationale: "test fixture".to_string(),
            scan_rust_source: false,
            severity: SymbolSeverity::Warn,
        };
        let cfg = ScanCfg {
            is_md: true,
            skip_md_table_rows: false,
            is_rust: false,
        };
        let path = Path::new("fixture.md");
        let root = Path::new("");
        let content = "this line mentions test-warn-symbol directly\n";
        let re = Regex::new(&sym.pattern).expect("compile fixture pattern");
        let hits = scan_source_lines(path, root, content, &[(&sym, re)], cfg);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, SymbolSeverity::Warn);
    }
```

Both tests need `serde_yaml` already in `Cargo.toml` (it is — used by `canonical_docs.rs`, `check_links.rs`, and others in this crate) and need to construct a `RetiredSymbol` and call `scan_source_lines` directly — read its exact current parameter list at line 142 before writing the call in the second test, and match it exactly (do not guess the parameter order or the `cfg` field names — the read in Step 1 has them).

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p vox-cli-ci retired_symbol_check::tests::symbol_severity -- --nocapture`

Expected: FAIL to compile — `SymbolSeverity` does not exist, `RetiredSymbol` has no `severity` field, `scan_source_lines`'s return type does not match `hits[0].0`.

- [ ] **Step 4: Add the enum and the field**

Add just above `struct RetiredSymbol`:

```rust
/// Whether a hit on this symbol fails the build or only warns.
///
/// Defaults to `Error` so every one of the 14 entries in the contract today
/// is unaffected by this field's existence — `severity:` is opt-in per entry.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
enum SymbolSeverity {
    Warn,
    #[default]
    Error,
}
```

In `RetiredSymbol`, add after `scan_rust_source`:

```rust
    /// `warn`: hits are reported but never fail the build. `error` (default,
    /// and the implicit value for every entry that omits this key): hits
    /// fail the build exactly as before this field existed. Use `warn` when
    /// adding a new retired-symbol entry ahead of repairing the references
    /// it will find — landing the entry as `error` immediately makes the
    /// tree unmergeable until every existing reference is fixed in the same
    /// commit.
    #[serde(default)]
    severity: SymbolSeverity,
```

- [ ] **Step 5: Change `scan_source_lines`'s return type**

At line 142, change the signature's return type from `Vec<String>` to
`Vec<(SymbolSeverity, String)>`. At the push site (find it fresh: `grep -n 'failures.push(format!' crates/vox-cli-ci/src/retired_symbol_check.rs`),
change `failures.push(format!(...))` to `failures.push((sym.severity, format!(...)))`
— `sym` is already in scope there. The `let mut failures = Vec::new();` binding
needs no type annotation (Rust infers it from the push), or annotate explicitly
as `Vec<(SymbolSeverity, String)>` if the compiler asks for one.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p vox-cli-ci retired_symbol_check::tests::symbol_severity`

Expected: PASS.

- [ ] **Step 7: Confirm the crate still compiles and the real detector still passes**

```bash
cargo build -p vox-cli-ci
cargo run -q -p vox-cli -- ci retired-symbol-check
```

Expected: clean build; the second command exits 0, identical to before this
task (all 14 entries are still `error`, so behavior is unchanged).

- [ ] **Step 8: Commit**

```bash
git add crates/vox-cli-ci/src/retired_symbol_check.rs
git commit -m "feat(ci): add per-symbol severity to retired-symbol-check, default error"
```

---

### Task 2: Partition warnings out of `run()`'s failure path

**Files:**
- Modify: `crates/vox-cli-ci/src/retired_symbol_check.rs` (`run()`, starting line 325; `failures` accumulation through the `if !failures.is_empty()` block at ~511)

**Interfaces:**
- Consumes: `SymbolSeverity`, the `Vec<(SymbolSeverity, String)>` return of `scan_source_lines` (Task 1).
- Produces: `run(root: &Path) -> Result<()>` — signature unchanged. Stdout gains `warning: ` lines for `Warn`-severity hits; the error path and its message format are otherwise unchanged for `Error`-severity hits.

- [ ] **Step 1: Read `run()`'s current accumulation and failure-reporting exactly**

```bash
sed -n '325,360p' crates/vox-cli-ci/src/retired_symbol_check.rs
sed -n '505,528p' crates/vox-cli-ci/src/retired_symbol_check.rs
```

Confirm: `let mut failures = Vec::new();` near 349; five `failures.extend(scan_source_lines(...))` call sites between there and ~500 (each now yields `(SymbolSeverity, String)` pairs, from Task 1); the failure block starting `if !failures.is_empty() {` around 511 prints each with `eprintln!` and returns `Err` via `anyhow!("Found {} retired symbol violations...", failures.len())`.

For comparison, read the existing warn-and-continue precedent already in this crate:

```bash
sed -n '325,345p' crates/vox-cli-ci/src/crate_edges.rs
```

Note the exact shape: `warning: ` prefix, printed with `println!` (stdout, not stderr — warnings are not failures), before the pass/fail branch.

- [ ] **Step 2: Write the failing test**

```rust
    #[test]
    fn run_does_not_fail_on_warn_only_hits() {
        // Build a scratch repo tree with one doc referencing a warn-severity
        // symbol and nothing else, and confirm run() returns Ok.
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        std::fs::create_dir_all(root.join("docs")).unwrap();
        std::fs::write(
            root.join("docs/test.md"),
            "this doc mentions test-warn-symbol in prose\n",
        )
        .unwrap();
        std::fs::create_dir_all(root.join("contracts/documentation")).unwrap();
        std::fs::write(
            root.join("contracts/documentation/retired-symbols.v1.yaml"),
            "symbols:\n  - id: test-warn-symbol\n    pattern: \"\\\\btest-warn-symbol\\\\b\"\n    replacement: \"replacement\"\n    rationale: \"fixture\"\n    severity: warn\n",
        )
        .unwrap();
        // AGENTS.md / CLAUDE.md / GEMINI.md and .cursor/rules/ are read
        // unconditionally by run() — read the exact scan list at the top of
        // run() (Step 1) and create empty stand-ins for whichever of those
        // run() requires to exist before it will proceed past setup.
        let result = run(root);
        assert!(result.is_ok(), "warn-only hits must not fail run(): {result:?}");
    }
```

**`run()`'s first call is `crate::docs_deprecated_command_guard::run(root)?`** --
easy to miss if you only read the scanning code. Traced: it only acts when
`docs/` is a directory (the fixture creates it) and optionally reads
`docs/agents/script-registry.json` / `scripts/README.md`, both skipped via
`.is_file()` checks when absent. AGENTS.md/GEMINI.md/CLAUDE.md, `.cursor/rules/`,
and `crates/` are all similarly existence-gated elsewhere in `run()`. **The
fixture tree specified above (`docs/test.md` + the contract YAML) is
sufficient** -- verified against the real guard chain, not guessed. `tempfile`
is already a `[dev-dependencies]` entry in `crates/vox-cli-ci/Cargo.toml` --
no addition needed.

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p vox-cli-ci retired_symbol_check::tests::run_does_not_fail_on_warn_only -- --nocapture`

Expected: FAIL — `run()` still bails on any hit regardless of severity, so this
returns `Err`.

- [ ] **Step 4: Partition in `run()`**

Read the real current failure block fresh before editing (line numbers below
are illustrative, not exact -- confirm with
`grep -n 'if !failures.is_empty()\|let suffix\|retired-symbol-check OK' crates/vox-cli-ci/src/retired_symbol_check.rs`).
The real block computes a `suffix` string describing scan scope
(`"docs/, policy roots, .cursor/rules, and crates/**/*.rs"` when
`scan_crates` is set, else `"docs/, policy roots, and .cursor/rules"`) and
includes it in the error message -- **preserve this**, it is not decorative,
it tells the reader what was actually scanned:

```rust
if !failures.is_empty() {
    for f in &failures { eprintln!("{}", f); }
    let suffix = if scan_crates {
        "docs/, policy roots, .cursor/rules, and crates/**/*.rs"
    } else {
        "docs/, policy roots, and .cursor/rules"
    };
    return Err(anyhow!("Found {} retired symbol violations in {}", failures.len(), suffix));
}
println!("retired-symbol-check OK");
Ok(())
```

Immediately before this block, insert the partition:

```rust
    let (errors, warnings): (Vec<_>, Vec<_>) = failures
        .into_iter()
        .partition(|(sev, _)| *sev == SymbolSeverity::Error);
    for (_, w) in &warnings {
        println!("warning: {w}");
    }
```

Then change the failure block to operate on `errors` instead of `failures`,
**keeping `suffix` and its two branches exactly as they are**, and fold the
warning count into the same message rather than replacing it:

```rust
    if !errors.is_empty() {
        for (_, e) in &errors {
            eprintln!("{e}");
        }
        let suffix = if scan_crates {
            "docs/, policy roots, .cursor/rules, and crates/**/*.rs"
        } else {
            "docs/, policy roots, and .cursor/rules"
        };
        return Err(anyhow!(
            "Found {} retired symbol violations in {} ({} warning(s))",
            errors.len(),
            suffix,
            warnings.len()
        ));
    }
    println!("retired-symbol-check OK ({} warning(s))", warnings.len());
    Ok(())
```

`scan_crates` is the existing local variable this block already reads (from
the `VOX_CI_RETIRED_SYMBOL_SCAN_CRATES` env check earlier in `run()`) --
confirm its exact name in your fresh read rather than assuming it matches
here.

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p vox-cli-ci retired_symbol_check::tests::run_does_not_fail_on_warn_only`

Expected: PASS.

- [ ] **Step 6: Run the full test module and the real detector**

```bash
cargo test -p vox-cli-ci retired_symbol_check
cargo run -q -p vox-cli -- ci retired-symbol-check
```

Expected: all tests PASS; the real command still exits 0 with the same
behavior as before this plan (every real entry is still `error`-severity).

- [ ] **Step 7: Confirm the three call sites still compile untouched**

```bash
cargo check -p vox-cli
```

Expected: clean. `run()`'s signature did not change, so
`crates/vox-cli/src/commands/ci/pre_push.rs:1104`,
`run_body.rs:126` (the `harness-trust-guard` path), and `run_body.rs:679`
all still compile with no edits.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-cli-ci/src/retired_symbol_check.rs crates/vox-cli-ci/Cargo.toml
git commit -m "feat(ci): warn-severity retired-symbol hits no longer fail the build"
```

---

### Task 3: Full gate and push

- [ ] **Step 1: Format**

Run: `vox run scripts/fmt.vox`

- [ ] **Step 2: Full test suite for the touched crate**

```bash
cargo test -p vox-cli-ci
cargo clippy -p vox-cli-ci --all-targets -- -D warnings
```

Expected: clean.

- [ ] **Step 3: Regenerate the doc inventory**

`contracts/documentation/retired-symbols.v1.yaml` changed, which is read by the
inventory walker's evidence hints in at least one docs-reality-audit claim —
regenerate to be safe:

```bash
cargo run -q -p vox-cli -- ci doc-inventory generate --output docs/agents/doc-inventory.json
git add docs/agents/doc-inventory.json
```

- [ ] **Step 4: Run the full pre-push tier**

Run: `vox ci pre-push --full`

- [ ] **Step 5: Push once**

```bash
git push -u origin HEAD
```

---

## Self-Review

**1. Spec coverage.** This plan implements exactly W3.6's named prerequisite:
a severity valve, so that W3.1 (populating `warn`-severity entries for
`vox-dashboard`, `vox-oratio`, `vox-dei-shim`, `@endpoint`, the decorator
class) can land without the ~460–620 estimated hard failures the spec computed
against the current all-error contract. Populating those entries is explicitly
out of scope here (Global Constraints) and is the next plan in sequence.

**2. Placeholder scan.** No TBDs. Two steps (Task 2 Steps 2 and 4) instruct the
implementer to read the exact current code before writing new code, rather
than presenting invented signatures — this is deliberate, following the
sibling plans' lesson that hand-guessed Rust shapes shipped compile errors
twice. It is not a placeholder; it is a read-before-write instruction with an
exact command to run.

**3. Type consistency.** `SymbolSeverity` is defined in Task 1 and consumed
unchanged in Task 2. `scan_source_lines`'s new return type
(`Vec<(SymbolSeverity, String)>`) is produced in Task 1 and consumed by the
partition in Task 2. `run()`'s signature (`Result<()>`) is asserted unchanged
in both tasks' verification steps.

**Ordering:** Task 1 before Task 2 (Task 2's partition needs the tuple return
type). Task 3 last.
