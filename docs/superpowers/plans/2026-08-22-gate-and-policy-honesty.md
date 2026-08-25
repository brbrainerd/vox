# Gate and Policy Honesty Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop this repository from teaching coding agents syntax the compiler rejects, and stop the audit machinery from reporting numbers nobody can trust.

**Architecture:** Three defect groups, one root cause each. (A) Prose and machine-readable artifacts prescribe at-prefixed decorator forms that became hard parse errors on 2026-06-30 — fixed at the source, then locked by narrowing one predicate in the existing `retired_symbol_check` detector rather than by adding a bespoke markdown parser. (B) Policy documents contradict the constants that enforce them — fixed by relocating the assertion into the crate that owns the constant. (C) `docs-reality-audit` carries a metric nobody reads and a verifier that cannot detect its own staleness — fixed by deleting the former and extending the latter.

**Tech Stack:** Rust (`vox-cli`, `vox-cli-ci`, `vox-doc-pipeline`), Markdown, JSON contracts.

**Spec:** `docs/superpowers/specs/2026-08-22-docs-corpus-repair-design.md` (revision 2)

## Revision note

Revision 1 of this plan was audited by eight parallel critique tracks. It
contained **two guards that shipped dead**, **three compile errors**, **three
false justifications**, a **wrong ADR renumber target**, and the **wrong
verification tier**. All are corrected here; the cut rationale is in each task.
Every code block below was compiled and run against the real tree during the
audit unless explicitly marked otherwise.

## Global Constraints

- **Covers spec workstreams W1, W6, W7, and W3.5 only.** W2, W4, W5, W8 are separate efforts. Per spec §8, W2/W3/W5 should update the existing ledgers (`legacy-tombstone-remediation-ledger-2026.md`, `pr92-handoff.md` §5.3) rather than spawn new plan documents.
- **No new crate, GUI surface, findings schema, scheduler, or crate edge.** Do not add entries to `contracts/ci/crate-edges.allow.v1.json`.
- **Do NOT add contract entries to `contracts/documentation/retired-symbols.v1.yaml` in this plan.** `retired_symbol_check.rs` has no severity tier — adding entries while ~680 live references exist makes the tree unmergeable. That is W3.1 and requires the severity valve first.
- **Test-first.** Every new `pub fn` needs a test in the same file first. This plan adds no new `pub fn`.
- **Never run `cargo fmt --all`** — `os error 206` on Windows. Use `vox run scripts/fmt.vox` or `cargo fmt -p <crate>`.
- **Line endings are LF** for `md`/`rs`/`json`/`yaml`.
- **Verification tier is `--full`, not `--complete`.** `--complete` runs **no tests**; only `--full` adds `cargo nextest run --workspace`.
- **`doc-inventory.json` drifts on nearly every task here** and is verified in `--complete` and CI. Regenerate and commit it (Task 14).
- **One agent per worktree.** During the audit, parallel agents editing this tree deleted each other's files mid-build.
- **No checker enters this plan until it has been RUN against the real tree and its actual output pasted into the step.** Across two plans, five guards were written to catch drift and **five could not fire** -- two read the wrong markdown column, one skipped the very rows it protected, one was permanently red, one asserted a hardcoded string against a file it never read. Every one was reasoned about instead of executed. "Expected: FAIL" is a transcript, not a prediction.
- **PR discipline:** CodeRabbit reviews once on open. Batch commits; push once; re-request via a `@coderabbitai review` comment.

---

## File Structure

| File | Responsibility | Task |
| --- | --- | --- |
| `AGENTS.md` | Retired-surfaces row fix; three prescriptive lines outside the table; three new crate rows | 1, 3 |
| `crates/vox-cli-ci/src/retired_symbol_check.rs` | Narrow `skip_md_table_rows` to the first cell — the root-cause guard replacing revision 1's broken bespoke parser | 2 |
| `crates/vox-doc-pipeline/src/pipeline/lint.rs` | Governance-parity assertion (relocated here so it can see `VALID_CATEGORIES`); false comment fix | 4 |
| `docs/src/contributors/documentation-governance.md`, `docs/src/adr/002-diataxis-doc-architecture.md` | Category vocabulary corrections | 4 |
| `docs/src/adr/**`, `docs/src/architecture/adr-*.md` | ADR 037 collision, index gaps, prose citations, `NNN` placeholder | 5 |
| `crates/vox-doc-pipeline/tests/policy_docs_guard.rs` | **New.** Two guards only: NUL bytes and ADR uniqueness. No markdown-table parsing. | 5, 6 |
| `docs/src/architecture/data-storage-lint-and-ci-spec-2026.md` | Strip the NUL byte | 6 |
| `docs-astro/astro.config.mjs`, `crates/vox-actor-runtime/src/llm/chat.rs`, `docs/src/contributors/docs-reality-audit-program.md` | Three false comments | 7 |
| `crates/vox-cli-ci/src/check_links.rs` | Add `CLAUDE.md` / `GEMINI.md` to the scanned policy roots | 8 |
| `crates/vox-cli/src/commands/llm.rs` | Stop printing hard-parse-error syntax as a "Golden Example" | 9 |
| `docs/agents/vox-language-surface.v1.json` | Remove five retired decorators and one that never existed | 10 |
| `crates/vox-cli-ci/src/docs_reality_audit.rs` | Delete the unread metric; recompute metrics in verify; short-circuit glob | 11, 12, 13 |

**Why only two guards.** Revision 1 proposed five. Two were verified inert: the
`AGENTS.md` decorator guard mis-columned the one row it existed for (the row's
`\|` escapes split as delimiters, so `cols[2]` read `"query\\"`), and the
crate-existence guard skipped every `crates/`-prefixed token — i.e. both rows it
was written to protect. The two that survive parse no markdown at all.

---

### Task 1: Fix every place `AGENTS.md` prescribes parse-error syntax

`AGENTS.md` is loaded into every agent session. §Retired Surfaces (`:454`) gives
the replacement for `@endpoint` as the at-prefixed forms, while §Grammar
Unification (`:245-248`) states those became hard parse errors on 2026-06-30.

Verified from the parser, not the doc: `crates/vox-compiler/src/parser/descent/mod.rs:822`
calls `reject_retired_decorator(...)` which pushes `ParseSeverity::Error`, and
`parse_module` returns `Err` on any error-severity entry. Commit `cd7cc96874`
exists, dated Tue Jun 30 2026, "Hard-error flip". All 79 `examples/golden/**/*.vox`
use bare forms exclusively.

**Files:**
- Modify: `AGENTS.md` (four locations: `:454`, `:494`, `:499`, `:507`)

**Interfaces:**
- Consumes: nothing.
- Produces: an `AGENTS.md` whose replacement column and prescriptive prose both name bare-keyword forms. Task 2's detector change enforces it.

- [ ] **Step 1: Fix the §Retired Surfaces row**

Change the replacement cell of the `@endpoint` row so it reads:

```
| `@endpoint(kind: server\|query\|mutation) fn` (removed v0.6.0) | `server fn` / `query fn` / `mutation fn` (bare-keyword; the at-prefixed forms became hard parse errors 2026-06-30, `cd7cc96874`) |
```

Leave the left column alone — naming the retired form is that column's job.

- [ ] **Step 2: Fix the three prescriptive lines outside the table**

These are in §Vox Language Enforcement Rules and are the most-read prescriptive
text in the file. A fix scoped to the table alone leaves all three wrong.

`AGENTS.md:494` — replace the at-prefixed forms:

```
- Any `pub fn`, `query fn`, `mutation fn`, or `server fn` that calls `http.*`, `net.*`, `fetch(`, `populi.*`, or `std.http.*` MUST carry `@uses(net)` in the preceding decorator list.
```

`AGENTS.md:499` — replace the at-prefixed forms **and delete `@activity`**,
which has never existed (no `AtActivity` token in
`crates/vox-compiler/src/lexer/token.rs`, no parser arm; `activity` is a bare
keyword):

```
- ID parameters on `query`, `mutation`, `server`, or `activity` functions, or actor-message functions, MUST use `Id[T]` (e.g., `Id[User]`) rather than bare `str`. Lint: `vox/types/id-required-at-boundary`.
```

`AGENTS.md:507`:

```
- Every `query fn`, `mutation fn`, or `server fn` should carry either `@auth(...)` for authenticated routes or an explicit open-access annotation.
```

- [ ] **Step 3: Confirm no at-prefixed data-layer form remains as a prescription**

```bash
grep -n '@server fn\|@query fn\|@mutation fn\|@activity' AGENTS.md
```

Expected: **no output**. Any remaining hit is either a new prescription (fix it)
or a left-column retirement notice (move it into the left column).

- [ ] **Step 4: Commit**

```bash
git add AGENTS.md
git commit -m "fix(agents): AGENTS.md prescribed decorator forms that are parse errors"
```

---

### Task 2: Narrow `skip_md_table_rows` so the detector can see replacement columns

Revision 1 proposed a bespoke markdown guard for Task 1. It was verified inert.
The root cause is that `retired_symbol_check.rs` deliberately cannot see the
row: `ScanCfg::skip_md_table_rows` skips the **entire** table row for policy
files. Narrowing it to the **first cell** covers the replacement column of
`AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, and all nine `.cursor/rules/*.mdc`, for
**every** contract entry present and future — one predicate change instead of a
hand-maintained list that drifts.

**Files:**
- Modify: `crates/vox-cli-ci/src/retired_symbol_check.rs` (the `skip_md_table_rows` branch in `scan_source_lines`, around `:146`)
- Test: same file's `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `fn first_cell_only(line: &str) -> Option<&str>` — private; returns the remainder of a markdown table row after its first data cell, or `None` if the line is not a table row.

- [ ] **Step 1: Read the current skip and confirm its shape**

```bash
sed -n '140,175p' crates/vox-cli-ci/src/retired_symbol_check.rs
```

Confirm the branch skips the whole line when `cfg.skip_md_table_rows` is set and
the trimmed line starts with `|`. Note the exact variable names before editing.

- [ ] **Step 2: Write the failing test**

Add to `#[cfg(test)] mod tests` in `crates/vox-cli-ci/src/retired_symbol_check.rs`:

```rust
    #[test]
    fn first_cell_only_exposes_the_replacement_column() {
        // The real AGENTS.md row: escaped pipes inside the first cell must not
        // be treated as column separators.
        let row = r"| `@endpoint(kind: server\|query\|mutation) fn` (removed v0.6.0) | `server fn` / `query fn` / `mutation fn` |";
        let rest = first_cell_only(row).expect("table row");
        assert!(
            !rest.contains("@endpoint"),
            "the retired form lives in the first cell and must be skipped, got: {rest}"
        );
        assert!(
            rest.contains("server fn"),
            "the replacement column must remain scannable, got: {rest}"
        );
    }

    #[test]
    fn first_cell_only_returns_none_for_non_table_lines() {
        assert!(first_cell_only("plain prose line").is_none());
        assert!(first_cell_only("").is_none());
    }
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p vox-cli-ci retired_symbol_check::tests::first_cell_only -- --nocapture`

Expected: FAIL to compile — `cannot find function 'first_cell_only' in this scope`.

- [ ] **Step 4: Implement `first_cell_only`**

Add near the other helpers in `crates/vox-cli-ci/src/retired_symbol_check.rs`:

```rust
/// For a markdown table row, return everything after the first data cell.
///
/// Policy files list the retired symbol in the first column on purpose, so that
/// cell is skipped — but the replacement column must stay scannable, because a
/// replacement that names a retired form is exactly the defect we are hunting.
///
/// `\|` inside a cell is escaped content, not a delimiter (the AGENTS.md
/// `@endpoint` row contains two), so it is masked before splitting.
fn first_cell_only(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    let mut rest = trimmed.strip_prefix('|')?;
    let mut consumed = trimmed.len() - rest.len();
    loop {
        let i = rest.find('|')?;
        if rest[..i].ends_with('\\') {
            // An escaped pipe inside the cell (e.g. AGENTS.md's `@endpoint`
            // row) -- not a column delimiter. Keep scanning past it.
            consumed += i + 1;
            rest = &rest[i + 1..];
            continue;
        }
        return Some(&trimmed[consumed + i..]);
    }
}
```



- [ ] **Step 5: Wire it into the skip branch**

In `scan_source_lines`, replace the whole-row skip with a first-cell skip: when
`cfg.skip_md_table_rows` is set and `first_cell_only(line)` returns `Some(rest)`,
scan `rest` instead of `line` rather than `continue`-ing. When it returns `None`,
behaviour is unchanged.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p vox-cli-ci retired_symbol_check`

Expected: all PASS, including the crate's pre-existing `retired_symbol_check` tests.

- [ ] **Step 7: Run the real detector — this is the acceptance check**

Run: `cargo run -q -p vox-cli -- ci retired-symbol-check`

Expected: **exit 0.** Task 1 already removed the offending prescriptions. If it
fails, the failure names a policy-file replacement column still prescribing a
retired form — fix that, do not widen the skip back.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-cli-ci/src/retired_symbol_check.rs
git commit -m "fix(ci): retired-symbol-check now scans policy-table replacement columns"
```

---

### Task 3: Add three missing crates to the retired-surfaces table

Three renamed or deleted crates are absent from the table, and the live corpus
carries references to all three: `crates/vox-dashboard` (273 live references,
named as the canonical implementation target in five ADRs and two reference
docs), `vox-dei-shim` (27), `crates/vox-oratio` (6).

Adding rows only. Fixing the references is W2 and requires the W3.6 severity
valve first.

**Files:**
- Modify: `AGENTS.md` (§Retired Surfaces table)

**Interfaces:**
- Consumes: Task 1's corrected table.
- Produces: nothing consumed by later tasks.

- [ ] **Step 1: Verify the three replacements before writing them**

```bash
ls -d crates/vox-gui crates/vox-speech crates/vox-research-shim
ls -d crates/vox-dashboard crates/vox-oratio crates/vox-dei-shim 2>&1 | head -3
grep -n 'visible_alias = "oratio"' crates/vox-cli/src/lib.rs
```

Expected: the first three exist, the second three do not, and the `oratio` alias
is present. Each replacement is documented, not inferred: ADR-037 decommissions
`vox-dashboard` in favour of `vox-gui`; `81681e81b` is the oratio→speech rename;
`5463bc16c` is the dei-shim→research-shim rename.

- [ ] **Step 2: Add the three rows**

```
| `crates/vox-dashboard` (deleted 2026-05-12, `af5f26278`; Axum dashboard retired per ADR-037) | `crates/vox-gui` (Tauri 2) |
| `crates/vox-oratio` (crate renamed `81681e81b`; the `vox speech` command keeps `oratio` as a visible alias) | `crates/vox-speech` |
| `vox-dei-shim` (renamed `5463bc16c`) | `vox-research-shim` |
```

Two precision points that revision 1 got wrong and that matter in an
always-loaded file: `vox-dashboard` **did exist** — writing "never existed"
replaces one falsehood with another. And the `oratio` **CLI alias still works**;
an agent over-applying the row would delete a working command.

- [ ] **Step 3: Confirm the detector still passes**

Run: `cargo run -q -p vox-cli -- ci retired-symbol-check`

Expected: exit 0. The new rows name retired crates only in the **first** cell,
which Task 2's narrowed skip still skips.

- [ ] **Step 4: Commit**

```bash
git add AGENTS.md
git commit -m "docs(agents): add vox-dashboard, vox-oratio, vox-dei-shim to retired surfaces"
```

---

### Task 4: Fix the category vocabulary, in the crate that owns it

`documentation-governance.md:41-52` advertises 9 slugs. `VALID_CATEGORIES`
(`lint.rs:18`) holds 13 display labels, matched exactly at `lint.rs:395` with no
alias map. All 918 docs use display labels; **zero** use the slugs. A contributor
following the governance doc fails the lint.

Revision 1 put this assertion in a new integration-test file with a hardcoded
copy of `VALID_CATEGORIES` — a **third** copy of the same SSOT. It belongs in
`lint.rs`'s own test module, where it can reference the constant directly.

**Files:**
- Modify: `crates/vox-doc-pipeline/src/pipeline/lint.rs` (test module; and the false comment at `:16-17`)
- Modify: `docs/src/contributors/documentation-governance.md:41-53`
- Modify: `docs/src/adr/002-diataxis-doc-architecture.md:58`

**Interfaces:**
- Consumes: `VALID_CATEGORIES` (already in scope inside `lint.rs`).
- Produces: nothing consumed by later tasks.

- [ ] **Step 1: Write the failing test inside `lint.rs`**

Add to the existing `#[cfg(test)] mod tests` in
`crates/vox-doc-pipeline/src/pipeline/lint.rs`:

```rust
    /// The governance doc is the SSOT AGENTS.md points contributors at. Every
    /// category it advertises must be one `VALID_CATEGORIES` accepts, because
    /// validation at lint.rs:395 is an exact match with no alias map.
    #[test]
    fn governance_doc_advertises_only_enforced_categories() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("repo root")
            .join("docs/src/contributors/documentation-governance.md");
        let doc = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));

        let start = doc
            .find("### Category vocabulary")
            .expect("governance doc must have a '### Category vocabulary' section");
        let rest = &doc[start..];
        // Stop at the next heading of any level, not just "### ".
        let end = ["\n### ", "\n## "]
            .iter()
            .filter_map(|h| rest[3..].find(h).map(|i| i + 3))
            .min()
            .unwrap_or(rest.len());
        let table = &rest[..end];

        let mut advertised = Vec::new();
        for line in table.lines() {
            let trimmed = line.trim();
            if !trimmed.starts_with('|') {
                continue;
            }
            let cols: Vec<&str> = trimmed.split('|').collect();
            if cols.len() < 3 {
                continue;
            }
            let raw = cols[1].trim();
            // Take the first backtick-delimited span so "`X` (note)" yields "X".
            let value = raw.split('`').nth(1).unwrap_or(raw).trim();
            if value.is_empty()
                || value == "category"
                || value.chars().all(|c| c == '-' || c == ':')
            {
                continue;
            }
            advertised.push(value.to_string());
        }

        assert!(
            !advertised.is_empty(),
            "parsed zero categories — the table shape changed and this guard is inert"
        );

        let unknown: Vec<&String> = advertised
            .iter()
            .filter(|v| !VALID_CATEGORIES.contains(&v.as_str()))
            .collect();
        assert!(
            unknown.is_empty(),
            "documentation-governance.md advertises categories the lint rejects: {unknown:?}"
        );
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p vox-doc-pipeline pipeline::lint::tests::governance_doc_advertises -- --nocapture`

Expected: FAIL listing `["getting-started", "tutorial", "how-to", "explanation", "reference", "adr", "architecture", "ci", "contributor"]`.

- [ ] **Step 3: Replace the governance table**

At `docs/src/contributors/documentation-governance.md:41-52`:

```markdown
| `category` | Meaning |
| --- | --- |
| `Getting Started` | first-stop pages and front-door onboarding |
| `Tutorials` | guided learning |
| `How-To Guides` | goal-oriented instructions |
| `Concepts` | conceptual understanding |
| `Language Reference` | stable lookup information |
| `API Reference — Crates` | per-crate API surface |
| `Examples` | worked examples |
| `Architecture Decisions (ADRs)` | architecture decisions |
| `Architecture SSOTs` | current architecture, authority maps, research indexes, roadmaps |
| `CI & Quality` | CI and quality-specific references |
| `Contributors` | contributor-facing governance and process docs |
| `Operations` | runbooks and operational catalogs |
| `archive` | tombstoned pages (excluded from the sidebar) |
```

Replace line 53 with:

```markdown
These display labels are the enforced vocabulary — `VALID_CATEGORIES` in
`crates/vox-doc-pipeline/src/pipeline/lint.rs` matches them exactly, with no
alias normalisation. They must stay in sync with the `sections` array in
`contracts/documentation/docs-sidebar-section-order.v1.json`.
```

The old table also **omitted** `Examples`, `Concepts`, `Operations`, and
`archive` — four live categories. The replacement adds them.

- [ ] **Step 4: Fix the false comment in `lint.rs`**

`lint.rs:16-17` reads "Display-label format … is canonical; slug aliases are kept
for grep-safety". **The array contains no slug aliases.** Replace with:

```rust
// These must match the `sections` array in contracts/documentation/docs-sidebar-section-order.v1.json.
// Display labels are the only accepted form — validation at `collect_lint_errors_*`
// is an exact `contains` check with no alias normalisation. `suggest()` maps a
// wrong value to the nearest label for the error message only.
```

- [ ] **Step 5: Fix the second source of the dead vocabulary**

`docs/src/adr/002-diataxis-doc-architecture.md:58` ships the same slug list
inside a yaml fence. Update it to the display labels, or mark the fence as
illustrative of the historical scheme with a one-line note saying the current
vocabulary lives in the governance doc.

- [ ] **Step 6: Run the test and the lint**

```bash
cargo test -p vox-doc-pipeline pipeline::lint::tests::governance_doc_advertises
cargo run -p vox-doc-pipeline -- --lint-only --paths contributors/documentation-governance.md
cargo run -p vox-doc-pipeline -- --lint-only --paths adr/002-diataxis-doc-architecture.md
```

Expected: test PASS, both lints clean.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-doc-pipeline/src/pipeline/lint.rs docs/src/contributors/documentation-governance.md docs/src/adr/002-diataxis-doc-architecture.md
git commit -m "fix(docs): governance category vocabulary did not match the enforced list"
```

---

### Task 5: Resolve the ADR numbering damage

Four separate defects. Three files claim 037; the index is missing five rows;
bare-prose citations will silently repoint after a rename; and one ADR filename
is still a literal placeholder.

**Critical correction from revision 1:** the next free numbers are **044 and
045**, not 042/043. `docs/src/architecture/adr-042-vox-populi-types.md` and
`adr-043-quantized-safetensors-ondisk-format.md` already exist **outside**
`docs/src/adr/`, and ADR-042 is cited from `layers.toml:158`,
`crate-audit-and-plan-2026.md:552`, and two Rust source files.

**Files:**
- Create: `crates/vox-doc-pipeline/tests/policy_docs_guard.rs`
- Rename: two `037-*.md` in `docs/src/adr/`
- Modify: `docs/src/adr/index.md`, plus inbound links and prose citations

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `fn repo_root() -> PathBuf` in the new test file, reused by Task 6.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-doc-pipeline/tests/policy_docs_guard.rs`:

```rust
//! Guards on repo policy documents that need no markdown-table parsing.
//!
//! Two only, deliberately. Table-parsing guards were tried and shipped inert:
//! escaped pipes shifted the columns and a prefix filter skipped the rows the
//! guard existed for. Retired-symbol coverage lives in `retired_symbol_check`
//! instead, which already owns the contract.

use std::path::{Path, PathBuf};

/// Workspace root, two levels up from this crate's manifest.
/// Matches the convention used by the other tests in this workspace
/// (`.ancestors().nth(2)`) — deliberately no `canonicalize`, which would put
/// `\\?\` verbatim prefixes into every assertion message on Windows.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repo root")
        .to_path_buf()
}

/// An ADR number is a citation key. Two documents sharing one makes every
/// "ADR-0NN" reference ambiguous. ADRs live in two directories today, so both
/// are scanned — checking only `docs/src/adr/` is how 042 and 043 came to be
/// occupied without anyone noticing.
#[test]
fn adr_numbers_are_unique() {
    use std::collections::BTreeMap;

    let root = repo_root();
    let mut by_number: BTreeMap<u32, Vec<String>> = BTreeMap::new();

    // `architecture/` REQUIRES the `adr-` prefix. Without that guard the 15
    // date-prefixed files there (`2026-05-08-workspace-reorg-design.md`, ...)
    // all parse as ADR number 2026 and this assertion is permanently red.
    // Exactly 3 digits, for the same reason.
    let dirs = [
        (root.join("docs").join("src").join("adr"), false),
        (root.join("docs").join("src").join("architecture"), true),
    ];

    for (dir, require_prefix) in &dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.ends_with(".md") {
                continue;
            }
            let stem = match name.strip_prefix("adr-") {
                Some(s) => s,
                None if *require_prefix => continue,
                None => name.as_str(),
            };
            let digits: String = stem.chars().take_while(|c| c.is_ascii_digit()).collect();
            if digits.len() != 3 {
                continue;
            }
            let Ok(number) = digits.parse::<u32>() else {
                continue;
            };
            by_number.entry(number).or_default().push(name);
        }
    }

    let collisions: Vec<_> = by_number.iter().filter(|(_, f)| f.len() > 1).collect();
    assert!(
        collisions.is_empty(),
        "ADR numbers must be unique across docs/src/adr and docs/src/architecture; \
         collisions: {collisions:?}"
    );
}

/// Every numbered ADR must appear in the index, or it is undiscoverable.
#[test]
fn adr_index_lists_every_numbered_adr() {
    let root = repo_root();
    let adr_dir = root.join("docs").join("src").join("adr");
    let index = std::fs::read_to_string(adr_dir.join("index.md")).expect("read adr/index.md");

    let mut missing = Vec::new();
    for entry in std::fs::read_dir(&adr_dir).expect("read docs/src/adr").flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.ends_with(".md") || name == "index.md" || name == "README.md" {
            continue;
        }
        let digits: String = name.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.len() < 2 {
            continue;
        }
        if !index.contains(&name) {
            missing.push(name);
        }
    }

    assert!(
        missing.is_empty(),
        "docs/src/adr/index.md is missing rows for: {missing:?}"
    );
}
```

- [ ] **Step 2: Run both tests to verify they fail**

Run: `cargo test -p vox-doc-pipeline --test policy_docs_guard adr -- --nocapture`

Expected: `adr_numbers_are_unique` FAILS on `037 -> [3 files]`;
`adr_index_lists_every_numbered_adr` FAILS listing five files (both 037
duplicates plus `038-ai-fixture-prompt-decorator.md`,
`039-ai-fixture-hole-decorator.md`, `040-ai-fixture-search-decorator.md`).

- [ ] **Step 3: Confirm 044/045 are free**

```bash
ls docs/src/adr/ docs/src/architecture/ | grep -E '^(adr-)?04[0-9]'
```

Expected: `040-*`, `041-*`, `adr-042-*`, `adr-043-*` and nothing at 044 or 045.

- [ ] **Step 4: Renumber**

Keep `037-tauri-convergence.md` at 037 — it is the one already cited in
`index.md:51`, so leaving it put minimises citation churn.

```bash
git mv docs/src/adr/037-ai-fixture-subagent-decorator.md docs/src/adr/044-ai-fixture-subagent-decorator.md
git mv docs/src/adr/037-tauri-gui-replaces-axum-dashboard.md docs/src/adr/045-tauri-gui-replaces-axum-dashboard.md
```

- [ ] **Step 5: Fix headings and self-references inside both renamed files**

```bash
grep -n '037' docs/src/adr/044-ai-fixture-subagent-decorator.md docs/src/adr/045-tauri-gui-replaces-axum-dashboard.md
```

Update the frontmatter `title:` and the H1 in each (lines 2 and 9 in the first,
2 and 8 in the second).

- [ ] **Step 6: Rewrite filename links**

```bash
grep -rn '037-ai-fixture-subagent-decorator\|037-tauri-gui-replaces-axum-dashboard' \
  docs contracts crates AGENTS.md CLAUDE.md GEMINI.md 2>/dev/null | grep -v doc-inventory.json
```

Known hits: `docs/src/architecture/vox-gui-harness-buildout-plan-2026.md:15`
and `contracts/frontend/surface-ownership.v1.yaml:26`. **Do not hand-edit
`docs/agents/doc-inventory.json`** — it is generated; Task 14 regenerates it.

- [ ] **Step 7: Sweep bare-prose citations — the step revision 1 omitted**

Filename links are not the whole problem. Bare `ADR-037` / `ADR 037` prose
citations point at three different decisions today, and after the rename the
ones meaning the two renamed ADRs would silently resolve to the wrong document.

```bash
grep -rn 'ADR-037\|ADR 037' crates docs contracts 2>/dev/null | grep -v '/archive/'
```

For each hit, determine which decision is meant and rewrite to the new number:
- `crates/vox-cli-ci/src/no_tauri_in_core.rs:1,28` — means tauri-convergence → **stays 037**
- `crates/vox-codegen/tests/tauri_convergence_snapshots.rs:1` — tauri-convergence → **stays 037**
- `crates/vox-codegen/tests/tauri_endpoint_client_parity_test.rs:4` — tauri-convergence → **stays 037**
- `docs/src/architecture/vox-gui-harness-buildout-plan-2026.md:28,34,167,399` — means tauri-gui-replaces-axum-dashboard → **becomes 045**
- `docs/src/architecture/rust-warning-audit-backlog-2026.md:105,106,108` — means ai-fixture-subagent-decorator → **becomes 044**

- [ ] **Step 8: Add the five missing index rows**

In `docs/src/adr/index.md`, following the existing format at line 51:

```markdown
| [038](038-ai-fixture-prompt-decorator.md) | **AI fixture `@prompt` decorator** |
| [039](039-ai-fixture-hole-decorator.md) | **AI fixture `@hole` decorator** |
| [040](040-ai-fixture-search-decorator.md) | **AI fixture `@search` decorator** |
| [044](044-ai-fixture-subagent-decorator.md) | **AI fixture `@subagent` decorator** |
| [045](045-tauri-gui-replaces-axum-dashboard.md) | **Tauri GUI replaces the Axum dashboard** |
```

- [ ] **Step 9: Fix the placeholder ADR filename and its false claim**

`docs/src/architecture/adr-NNN-scope-tauri-desktop-only.md` has a literal `NNN`
in its filename and states at `:30` that "ADR-037 (2026-05-11, not yet filed as a
doc)" — false; it is filed at `docs/src/adr/037-tauri-convergence.md`. Give the
file a real number (046, after Step 3 confirms it free), fix the claim, and add
an index row. If the decision is not actually accepted, move it out of an
`adr-` filename instead.

- [ ] **Step 10: Run the tests and the link gate**

```bash
cargo test -p vox-doc-pipeline --test policy_docs_guard adr
cargo run -q -p vox-cli -- ci check-links
```

Expected: both tests PASS, `check-links` exits 0.

- [ ] **Step 11: Commit — one commit, not two**

The rename and the link rewrite must land together or `check-links` fails in
between.

```bash
git add docs/src/adr docs/src/architecture contracts/frontend/surface-ownership.v1.yaml crates/vox-doc-pipeline/tests/policy_docs_guard.rs crates
git commit -m "fix(docs): resolve ADR 037 collision, index gaps, and prose citations"
```

---

### Task 6: Strip the NUL byte that hides a live spec from grep

`docs/src/architecture/data-storage-lint-and-ci-spec-2026.md` contains **exactly
one** NUL byte, at offset 37976 — the final byte, after the last newline. Enough
for `grep` to classify the file binary and skip it silently, so every grep-based
audit has an invisible blind spot there. Rust detectors read it fine, so CI never
noticed. It is the only such file among 1,906 markdown files in the repo.

**Files:**
- Modify: `crates/vox-doc-pipeline/tests/policy_docs_guard.rs` (add one test)
- Modify: `docs/src/architecture/data-storage-lint-and-ci-spec-2026.md`

**Interfaces:**
- Consumes: `repo_root` from Task 5.
- Produces: nothing.

- [ ] **Step 1: Write the failing test**

Append to `crates/vox-doc-pipeline/tests/policy_docs_guard.rs`:

```rust
/// A single NUL byte makes grep treat a markdown file as binary and skip it
/// silently, so every grep-based audit gains an invisible blind spot.
///
/// Scans the whole repo, not just docs/src: the policy files agents read most
/// (AGENTS.md, CLAUDE.md, docs/agents/**) live outside docs/src, and the repo
/// is already clean, so the wider scope costs nothing.
#[test]
fn no_markdown_contains_nul_bytes() {
    const SKIP: &[&str] = &["target", "node_modules", ".git"];

    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if path.is_dir() {
                if !SKIP.contains(&name.as_str()) {
                    walk(&path, out);
                }
            } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
                out.push(path);
            }
        }
    }

    let mut files = Vec::new();
    walk(&repo_root(), &mut files);
    assert!(
        !files.is_empty(),
        "walked zero markdown files — the guard is not checking anything"
    );

    let mut offenders = Vec::new();
    for path in &files {
        let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let count = bytes.iter().filter(|b| **b == 0).count();
        if count > 0 {
            offenders.push(format!("{} ({count} NUL bytes)", path.display()));
        }
    }

    assert!(
        offenders.is_empty(),
        "markdown files must not contain NUL bytes (grep skips them as binary):\n{}",
        offenders.join("\n")
    );
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p vox-doc-pipeline --test policy_docs_guard no_markdown_contains_nul -- --nocapture`

Expected: FAIL naming `data-storage-lint-and-ci-spec-2026.md (1 NUL bytes)`.

- [ ] **Step 3: Strip the byte**

```bash
f=docs/src/architecture/data-storage-lint-and-ci-spec-2026.md
tr -d '\000' < "$f" > "$f.tmp" && mv "$f.tmp" "$f"
```

- [ ] **Step 4: Confirm the size dropped by exactly one and grep sees text**

```bash
wc -c < docs/src/architecture/data-storage-lint-and-ci-spec-2026.md
grep -qI . docs/src/architecture/data-storage-lint-and-ci-spec-2026.md && echo text || echo binary
```

Expected: `37976`, and `text`.

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p vox-doc-pipeline --test policy_docs_guard no_markdown_contains_nul`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add docs/src/architecture/data-storage-lint-and-ci-spec-2026.md crates/vox-doc-pipeline/tests/policy_docs_guard.rs
git commit -m "fix(docs): strip NUL byte hiding data-storage spec from grep"
```

---

### Task 7: Correct three comments that describe behavior the code lacks

No tests — a test asserting a comment's prose would be worse than the defect.
Grouped into one commit because they share a cause: the code moved, the prose
did not.

**Files:**
- Modify: `docs-astro/astro.config.mjs:27`
- Modify: `crates/vox-actor-runtime/src/llm/chat.rs:359-361`
- Modify: `docs/src/contributors/docs-reality-audit-program.md` (§Operating cadence)

**Interfaces:** none.

- [ ] **Step 1: Fix the Astro sidebar comment**

Line 27 claims the sidebar is generated from `SUMMARY.md`. It comes from
`getSidebar()` in `docs-astro/src/utils/sidebar.mjs`, which walks `docs/src`,
parses frontmatter with `gray-matter`, and groups by `category` ordered by
`sort_order` then `title`. `SUMMARY.md` is gitignored and is only ever
*excluded* (`content.config.ts:10`, `routeData.ts:15`). Replace with:

```js
      // Sidebar is generated from each page's frontmatter (category / sort_order /
      // title) by src/utils/sidebar.mjs; section order comes from
      // contracts/documentation/docs-sidebar-section-order.v1.json.
```

- [ ] **Step 2: Fix the `infer_with_retry` doc comment**

The existing comment claims 401-skip and 429-continue. The body does no error
classification: every failure class takes the same branch and advances.

**Revision 1's proposed replacement was also wrong.** It said "a 429's
`retry_after` is not honoured — nothing sleeps". At the system level it *is*
honoured: `crates/vox-llm-egress/src/wire.rs:178-182` passes it to
`throttle::on_rate_limited`, which sets `cooldown_until = now + retry_after`,
and `throttle.rs:57-66` sleeps in `acquire`. Use this instead:

```rust
/// Sequential fallback over multiple candidate LLM configurations.
///
/// Tries each candidate exactly once, in order, and returns the first success.
/// There is deliberately no per-candidate retry and no error classification
/// here: 401, 429, 5xx, timeout, and transport failures all take the same
/// branch and advance to the next candidate. Cancellation is the one exception
/// — it returns immediately rather than advancing.
///
/// Rate-limit backoff is not this function's job. `EgressError::RateLimited`
/// carries `retry_after` to `vox_llm_egress::throttle`, which halves the
/// provider's concurrency window and sets a cooldown that the next
/// `acquire_permit` awaits.
///
/// Callers needing genuine provider fallback must pass a multi-candidate
/// vector; `vec![cfg]` yields exactly one attempt with no fallback.
```

Do not change the behavior — that is an LLM-runtime change, out of scope.

- [ ] **Step 3: Correct the audit program's cadence claim**

Replace the §Operating cadence body:

```markdown
## Operating cadence

**Status: dormant.** The cadence below is the intended operating model, not
current practice. As of 2026-08-22 the backlog holds zero findings and
`findings.v1.json` has one commit in its history (`3295a3bee`, 2026-05-12).
Treat any metric derived from it as "not started" rather than "healthy".

Intended cycles, when the program is resumed:

- **Weekly:** extend inventory / findings for files touched in the branch; re-run `verify`
- **Monthly:** full pass over `docs/src/` claims and hygiene of closed vs open findings
- **Release:** focus on `docs/src/reference/cli.md`, env vars, and operations catalog parity
```

- [ ] **Step 4: Verify**

```bash
node --check docs-astro/astro.config.mjs
cargo check -p vox-actor-runtime
cargo run -p vox-doc-pipeline -- --lint-only --paths contributors/docs-reality-audit-program.md
```

Expected: all three succeed.

- [ ] **Step 5: Commit**

```bash
git add docs-astro/astro.config.mjs crates/vox-actor-runtime/src/llm/chat.rs docs/src/contributors/docs-reality-audit-program.md
git commit -m "docs: correct three comments describing behavior the code lacks"
```

---

### Task 8: Make `check-links` scan the two policy roots it misses

`check_links.rs:337` reads `for rel in ["README.md", "AGENTS.md", "CONTRIBUTING.md"]`.
`CLAUDE.md` and `GEMINI.md` both link into `docs/` and are not scanned, so a
stale link there passes the merge gate and surfaces only in the nightly lychee
run. One-line fix.

**Files:**
- Modify: `crates/vox-cli-ci/src/check_links.rs:337`

**Interfaces:** none.

- [ ] **Step 1: Add the two roots**

```rust
    for rel in [
        "README.md",
        "AGENTS.md",
        "CONTRIBUTING.md",
        "CLAUDE.md",
        "GEMINI.md",
    ] {
```

- [ ] **Step 2: Run the gate — this is the test**

Run: `cargo run -q -p vox-cli -- ci check-links`

Expected: exit 0. If it fails, the failures are **real pre-existing broken links
in `CLAUDE.md` or `GEMINI.md`** that nothing has ever checked. Fix them in this
commit and note them in the message.

- [ ] **Step 3: Commit**

```bash
git add crates/vox-cli-ci/src/check_links.rs CLAUDE.md GEMINI.md
git commit -m "fix(ci): check-links now scans CLAUDE.md and GEMINI.md"
```

---

### Task 9: Stop `vox llm prompt` printing parse errors as golden examples

`crates/vox-cli/src/commands/llm.rs` is the subcommand whose entire purpose is
telling an LLM how to write Vox. It prints the **correct** bare form, then four
lines later prints the retired at-prefixed form labelled "Golden Example", then
an "MCP Schema Excerpt" declaring `"decorator": "@query"`. One invocation,
contradictory syntax. It also prints `pub fn`, which is not Vox.

This is higher-value than any prose fix in this plan: it is machine-consumed
output presented as canonical.

**Files:**
- Modify: `crates/vox-cli/src/commands/llm.rs:24-52`
- Test: same file

**Interfaces:** none.

- [ ] **Step 1: Read the two branches**

```bash
sed -n '20,60p' crates/vox-cli/src/commands/llm.rs
```

Note both the `web-route` branch and the `server-fn`/`mutation` branch, and that
each prints three sections: syntax, golden example, schema excerpt.

- [ ] **Step 2: Write the failing test**

Add a `#[cfg(test)] mod tests` block to `crates/vox-cli/src/commands/llm.rs` (or
extend the existing one). Because the current code `println!`s directly, assert
on the string constants rather than captured stdout — extract each golden
example into a `const` first so it is testable:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// The at-prefixed data-layer decorator spellings became hard parse errors
    /// on 2026-06-30 (cd7cc96874). This command exists to teach an LLM correct
    /// Vox, so emitting them is worse than emitting nothing.
    #[test]
    fn golden_examples_contain_no_retired_decorators() {
        for (label, text) in [
            ("route syntax", SYNTAX_ROUTE),
            ("route", GOLDEN_ROUTE),
            ("route schema", SCHEMA_ROUTE),
            ("mutation syntax", SYNTAX_MUTATION),
            ("mutation", GOLDEN_MUTATION),
            ("mutation schema", SCHEMA_MUTATION),
        ] {
            for retired in ["@query", "@mutation", "@server", "@table", "@tool"] {
                assert!(
                    !text.contains(retired),
                    "{label} golden example contains retired decorator {retired}: {text}"
                );
            }
            for non_vox in ["pub fn", "->", "u64", "String", "Result<"] {
                assert!(
                    !text.contains(non_vox),
                    "{label} contains {non_vox:?}, which is not Vox syntax: {text}"
                );
            }
        }
    }
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p vox-cli commands::llm::tests::golden_examples -- --nocapture`

Expected: FAIL to compile until Step 4 introduces the constants, then FAIL on
the assertions.

- [ ] **Step 4: Extract and correct the constants**

Above the handler in `crates/vox-cli/src/commands/llm.rs`:

```rust
// Copied VERBATIM from examples/golden/crud_api.vox:19-32, which the
// compiler verifies. Do NOT hand-write Vox here: a previous revision of
// this task invented `Ok(unit)`, a `Profile`/`Error` pair nothing
// declares, and a named-struct literal with zero precedent across all 79
// golden files -- shipping a new falsehood into the exact command this
// task exists to de-falsify.
const SYNTAX_ROUTE: &str = "query user_count() to int {\n    // ...\n}";
const GOLDEN_ROUTE: &str =
    "query user_count() to int {\n    return len(db.User.all())\n}";
const SCHEMA_ROUTE: &str = "{ \"type\": \"route\", \"keyword\": \"query\" }";
const SYNTAX_MUTATION: &str = "mutation seed_user(name: str) to str {\n    // ...\n}";
const GOLDEN_MUTATION: &str =
    "mutation seed_user(name: str) to str {\n    db.User.insert({ name: name, active: true })\n    return \"created\"\n}";
const SCHEMA_MUTATION: &str = "{ \"type\": \"mutation\", \"keyword\": \"mutation\" }";
```

Then replace **all three** content `println!` calls in each branch with these
constants.

**The syntax line is NOT already correct** -- a previous revision of this task
claimed it was. `llm.rs:26` prints `query get_user(id: u64) -> User`: `u64` is
not a Vox type (`int` is) and `->` is not the return arrow (`to` is). The
mutation branch at `:41` is worse: `String`, and `Result<(), Error>` with angle
brackets. That is why `SYNTAX_ROUTE` / `SYNTAX_MUTATION` exist above, and why
the test below iterates them too.

Before committing, confirm the snippets parse. The constants above are copied
from a compiler-verified golden file, so this should pass on the first run --
if it does not, the golden file moved and you must re-copy from it rather than
edit the snippet.

Do not write to `/tmp`: a Git-Bash `/tmp` path is not resolvable by the
`cargo run` child process on Windows. Use a repo-relative scratch file and
delete it.

```bash
printf 'query user_count() to int {\n    return len(db.User.all())\n}\n' > target/golden_check.vox
cargo run -q -p vox-cli -- check target/golden_check.vox
rm target/golden_check.vox
```

Never hand-write Vox into this task. Copy from `examples/golden/**/*.vox` --
those 79 files are compiler-verified, and the previous revision of this step
invented syntax that does not compile.

- [ ] **Step 5: Fix the cache-miss pointer**

`llm.rs:57` directs users to `docs/agents/vox-language-surface.v1.json` by name.
Task 10 fixes that file; no change needed here, but do not remove the pointer.

- [ ] **Step 6: Run the test and the command**

```bash
cargo test -p vox-cli commands::llm::tests::golden_examples
cargo run -q -p vox-cli -- llm prompt web-route
cargo run -q -p vox-cli -- llm prompt mutation
```

Expected: test PASS, and neither invocation prints an `@`-prefixed data-layer
decorator or `pub fn`.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-cli/src/commands/llm.rs
git commit -m "fix(cli): vox llm prompt printed parse-error syntax as a golden example"
```

---

### Task 10: Purge retired decorators from the machine-readable language surface

`docs/agents/vox-language-surface.v1.json` lists six decorators. **Five are hard
parse errors** (`@server`, `@table`, `@query`, `@mutation`, `@tool`) and the
sixth, **`@island`, has never existed** in the compiler. The `@table` example
reads `@table struct User` — `struct` is not a Vox declaration keyword. Stamped
`updated_at: 2026-04-19`, two months before the retirement.

The code SSOT already exists: `crates/vox-language-surface/src/lib.rs:336-348`
holds `LEXER_DEPRECATED_DECORATORS`. Rather than build a generator now, correct
the JSON by hand and add a test that pins it to that constant — the generator
becomes a later, optional step and the guard is what prevents recurrence.

**Files:**
- Modify: `docs/agents/vox-language-surface.v1.json`
- Test: `crates/vox-language-surface/src/lib.rs` (its own test module)

**Interfaces:**
- Consumes: `LEXER_DEPRECATED_DECORATORS` (already in scope in that crate).
- Produces: nothing.

- [ ] **Step 1: Read both sides**

```bash
sed -n '330,350p' crates/vox-language-surface/src/lib.rs
sed -n '20,50p' docs/agents/vox-language-surface.v1.json
```

- [ ] **Step 2: Write the failing test**

Add to `#[cfg(test)] mod tests` in `crates/vox-language-surface/src/lib.rs`:

**Ruling: `vox-language-surface` has exactly one dependency
(`workspace-hack`) and no dev-dependencies** -- a `serde_json`-based test
cannot compile there, and the "move it to vox-cli-ci" fallback would
duplicate `LEXER_DEPRECATED_DECORATORS` into a second SSOT (the exact
anti-pattern this task exists to reject in the JSON it is fixing) and needs
a crate edge that does not exist. Use a substring check against the file's
own JSON text instead -- no new dependency, stays next to the constant it
verifies.

```rust
    /// The agent-facing JSON is what `vox llm prompt` points models at on a
    /// cache miss. It must never advertise a decorator this crate already
    /// classifies as retired.
    ///
    /// A substring check, not a JSON parse: this crate has no serde_json
    /// dependency and none should be added just for this test. The file
    /// spells every decorator as `"name": "@foo"`, so that literal is
    /// specific enough to avoid false hits on prose mentions elsewhere in
    /// the document.
    #[test]
    fn agent_language_surface_json_has_no_retired_decorators() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("repo root")
            .join("docs/agents/vox-language-surface.v1.json");
        let raw = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));

        assert!(
            raw.contains("\"decorators\""),
            "shape changed -- this guard is inert"
        );

        let offenders: Vec<&&str> = LEXER_DEPRECATED_DECORATORS
            .iter()
            .filter(|d| raw.contains(&format!("\"name\": \"{d}\"")))
            .collect();

        assert!(
            offenders.is_empty(),
            "vox-language-surface.v1.json advertises retired decorators: {offenders:?}"
        );
    }
```

Verified `LEXER_DEPRECATED_DECORATORS` (`crates/vox-language-surface/src/lib.rs:336-355`,
14 entries, all `@`-prefixed) contains `@mcp.resource` -- which AGENTS.md and
Step 4 below both call fully valid, non-deprecated syntax. **Do not add
`@mcp.resource` to the JSON's `decorators` array** in Step 4; doing so trips
this guard. If a future need arises to document it, that is a change to
`LEXER_DEPRECATED_DECORATORS`'s own meaning, not to this JSON.

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p vox-language-surface agent_language_surface_json -- --nocapture`

Expected: FAIL listing the retired names present in the JSON.

- [ ] **Step 4: Correct the JSON**

Replace the `decorators` array with only decorators that currently parse. Per
AGENTS.md §Grammar Unification, the valid decorators are `@pure`, `@deprecated`,
`@require`, `@auth`, `@uses`, `@test`, `@durable`, `@scheduled`, and
`@mcp.resource`; `@mcp.tool` parses with a warning. The retired data-layer
keywords belong under a `keywords` entry, not `decorators`:

```json
  "decorators": [
    { "name": "@pure", "example": "@pure fn checksum(payload: bytes) { ... }" },
    { "name": "@uses", "example": "@uses(net) fn fetch_remote() { ... }" },
    { "name": "@auth", "example": "@auth(scheme: bearer) table Task { ... }" },
    { "name": "@durable", "example": "@durable fn run_pipeline() { ... }" },
    { "name": "@scheduled", "example": "@scheduled(\"0 9 * * *\") fn nightly() { ... }" },
    { "name": "@test", "example": "@test fn adds_two_numbers() { ... }" }
  ],
```

Delete `@island` entirely — it has no lexer token and no parser arm. Bump
`updated_at` to the current date. Verify each example against
`examples/golden/**/*.vox` rather than inventing syntax.

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p vox-language-surface agent_language_surface_json`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add docs/agents/vox-language-surface.v1.json crates/vox-language-surface/src/lib.rs
git commit -m "fix(agents): language-surface JSON taught five retired decorators and one that never existed"
```

---

### Task 11: Delete `rollout_milestone_pct`

It returns 25 for an empty backlog and 25 for eighty open findings — the one
headline number the program exposes cannot distinguish "not started" from
"nothing finished". Revision 1 proposed a floor constant to fix it.

**Verified: nothing consumes it.** Every reference in the workspace is its own
definition, its own output JSON, its own schema, and its own tests. The four
remaining fields (`inventory_claim_count`, `findings_total`, `findings_open`,
`findings_closed`) let any consumer compute any ratio. Deleting is a smaller diff
than a floor plus three tests, and a number nobody read going unquestioned for
three months argues against its existence.

**Files:**
- Modify: `crates/vox-cli-ci/src/docs_reality_audit.rs` (delete the fn at `:264-279`, its call at `:335`, the two JSON fields, and the existing test at `:397`)
- Modify: `contracts/reports/docs-reality-audit/metrics.v1.schema.json`
- Modify: `contracts/reports/docs-reality-audit/metrics.v1.json`

**Interfaces:**
- Produces: a metrics object with four count fields and `generated_at`, no `rollout_milestone_pct`, no `rollout_notes`.

- [ ] **Step 1: Confirm there are still no consumers**

```bash
grep -rn 'rollout_milestone_pct\|rollout_notes' --include='*.rs' --include='*.json' --include='*.yaml' --include='*.yml' --include='*.vox' . | grep -v '/target/' | grep -v docs/superpowers
```

Expected: hits only in `docs_reality_audit.rs`, `metrics.v1.json`,
`metrics.v1.schema.json`. If anything else appears, **stop** — a consumer exists
and this task becomes the revision-1 floor fix instead.

- [ ] **Step 2: Delete the function and its test**

Remove `fn rollout_milestone_pct` (`:264-279`) and the existing test
`rollout_milestone_empty_findings_is_25_when_inventory_nonempty` (`:397`).

- [ ] **Step 3: Delete the call and the two emitted fields**

At `:335` remove `let milestone = rollout_milestone_pct(...);`, and remove the
`"rollout_milestone_pct"` and `"rollout_notes"` entries from the `json!` literal.

- [ ] **Step 4: Update the schema**

In `contracts/reports/docs-reality-audit/metrics.v1.schema.json`, remove
`rollout_milestone_pct` from both the `required` array (`:19`) and the
`properties` object (`:66`), and remove `rollout_notes` if present in either.

- [ ] **Step 5: Regenerate and verify**

```bash
cargo test -p vox-cli-ci docs_reality_audit
cargo run -q -p vox-cli -- ci docs-reality-audit metrics --write
cargo run -q -p vox-cli -- ci docs-reality-audit verify
git diff contracts/reports/docs-reality-audit/metrics.v1.json
```

Expected: tests PASS; `verify` exits 0; the diff shows `rollout_milestone_pct`
and `rollout_notes` removed and `generated_at` updated.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-cli-ci/src/docs_reality_audit.rs contracts/reports/docs-reality-audit/
git commit -m "refactor(ci): delete unread rollout_milestone_pct metric"
```

---

### Task 12: Make `verify` recompute metrics instead of only schema-checking them

`run_verify` validates `metrics.v1.json` against its schema and stops. Nothing in
CI or `lefthook.yml` runs `metrics --write`. A 150-claim inventory beside
`inventory_claim_count: 10` would be a green build.

**Honest scoping correction:** this would **not** have caught the 2026-05-12
staleness, because `generated_at` is excluded from comparison by design, and the
committed file is numerically correct today. It guards a real future hazard, not
the incident revision 1 cited. It is still the highest-value W6 item, because
`run_verify` is wired into `ssot-drift` (`run_body_helpers/docs.rs:634-637`) and
therefore runs on every push.

**Files:**
- Modify: `crates/vox-cli-ci/src/docs_reality_audit.rs`
- Test: same file's `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `InventoryFile`, `FindingsFile` (this file). Depends on Task 11 having removed the two rollout fields.
- Produces: `fn compute_metrics(inv: &InventoryFile, findings: &FindingsFile) -> Value` — private; returns the metrics object **without** `generated_at`.

- [ ] **Step 1: Add the test helper and the failing test**

Add to `#[cfg(test)] mod tests`:

```rust
    fn repo_root_for_tests() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("repo root")
            .to_path_buf()
    }

    fn test_finding(id: &str, status: &str) -> FindingRow {
        FindingRow {
            id: id.to_string(),
            claim_ids: vec![],
            classification: "DocDeficit".to_string(),
            scores: FindingScores {
                impact: 1,
                blast_radius: 1,
                staleness: 0,
                enforcement_gap: 0,
                tractability: 0,
            },
            priority_score: 4,
            priority_band: "P2".to_string(),
            status: status.to_string(),
            recommended_action: "test".to_string(),
        }
    }

    #[test]
    fn compute_metrics_excludes_generated_at_and_counts_findings() {
        let inv = InventoryFile {
            schema_version: 1,
            claims: vec![],
        };
        let findings = FindingsFile {
            schema_version: 1,
            findings: vec![test_finding("f.1", "new"), test_finding("f.2", "closed")],
        };
        let m = compute_metrics(&inv, &findings);

        assert!(
            m.get("generated_at").is_none(),
            "compute_metrics must not stamp a timestamp; comparison would never match"
        );
        assert_eq!(m["findings_total"], 2);
        assert_eq!(m["findings_open"], 1);
        assert_eq!(m["findings_closed"], 1);
    }
```

`FindingRow` has exactly eight fields and `test_finding` sets all eight; the
scores give `priority_score = 4` / band `"P2"`, which is self-consistent, so the
formula and band checks in `verify_findings_consistency` do not fire first.
`claim_ids` is empty so the unknown-claim bail cannot preempt later tests.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p vox-cli-ci docs_reality_audit::tests::compute_metrics_excludes -- --nocapture`

Expected: FAIL to compile — `cannot find function 'compute_metrics'`.

- [ ] **Step 3: Extract `compute_metrics`**

Add above `run_metrics` in `crates/vox-cli-ci/src/docs_reality_audit.rs`:

```rust
/// Metrics object derived purely from inventory + findings.
///
/// Deliberately omits `generated_at` so the value is a pure function of the
/// inputs and can be compared against the committed file.
fn compute_metrics(inv: &InventoryFile, findings: &FindingsFile) -> Value {
    let mut counts_class: HashMap<String, i32> = HashMap::new();
    let mut counts_status: HashMap<String, i32> = HashMap::new();
    let mut counts_band: HashMap<String, i32> = HashMap::new();
    let mut open_p0 = 0i32;
    let mut open_p1 = 0i32;
    let terminal = HashSet::from(["closed", "verified"]);

    for f in &findings.findings {
        *counts_class.entry(f.classification.clone()).or_insert(0) += 1;
        *counts_status.entry(f.status.clone()).or_insert(0) += 1;
        *counts_band.entry(f.priority_band.clone()).or_insert(0) += 1;
        if !terminal.contains(f.status.as_str()) {
            if f.priority_band == "P0" {
                open_p0 += 1;
            }
            if f.priority_band == "P1" {
                open_p1 += 1;
            }
        }
    }

    let closed = findings
        .findings
        .iter()
        .filter(|f| terminal.contains(f.status.as_str()))
        .count();
    let open = findings.findings.len().saturating_sub(closed);

    serde_json::json!({
        "schema_version": 1,
        "inventory_claim_count": inv.claims.len(),
        "findings_total": findings.findings.len(),
        "findings_open": open,
        "findings_closed": closed,
        "counts_by_classification": counts_class,
        "counts_by_status": counts_status,
        "counts_by_priority_band": counts_band,
        "open_p0": open_p0,
        "open_p1": open_p1
    })
}
```

In `run_metrics`, delete the inline computation (from `let mut counts_class`
through the `let metrics = serde_json::json!({...});` literal) and replace with:

```rust
    let mut metrics = compute_metrics(&inv, &findings);
    let generated_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    metrics["generated_at"] = Value::String(generated_at);
```

`Value` is already imported (`:8`); `chrono` is already a dependency
(`Cargo.toml:17`); `HashMap`/`HashSet` are already imported (`:9`) and remain
used.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p vox-cli-ci docs_reality_audit::tests::compute_metrics_excludes`

Expected: PASS.

- [ ] **Step 5: Add the drift check to `run_verify`**

`metrics_val` is bound at `:245` and only borrowed by
`validate_json_against_schema`, so it is live. Immediately after the existing
`verify_findings_consistency(root, &inv, &findings)?;` line:

```rust
    // Metrics must match what the inputs imply. Without this, a stale
    // metrics.v1.json is a green build. `generated_at` is deliberately not
    // compared — compute_metrics never emits it.
    let expected_metrics = compute_metrics(&inv, &findings);
    let Value::Object(expected) = &expected_metrics else {
        unreachable!("compute_metrics always returns a JSON object")
    };
    for (key, want) in expected {
        let got = metrics_val.get(key);
        if got != Some(want) {
            anyhow::bail!(
                "metrics.v1.json is stale: field {:?} is {} but inputs imply {}. \
                 Run `vox ci docs-reality-audit metrics --write`.",
                key,
                got.map(|v| v.to_string())
                    .unwrap_or_else(|| "absent".to_string()),
                want
            );
        }
    }
    // The loop above only checks expected -> actual, so a field REMOVED from
    // compute_metrics but still present in the committed file passes
    // silently. additionalProperties:false in the schema catches this only
    // for keys in `required`; an optional stale key (there is none today,
    // but nothing prevents one tomorrow) would not be. Close it explicitly.
    if let Some(actual) = metrics_val.as_object() {
        let extra: Vec<&String> = actual
            .keys()
            .filter(|k| k.as_str() != "generated_at" && !expected.contains_key(*k))
            .collect();
        if !extra.is_empty() {
            anyhow::bail!(
                "metrics.v1.json has stale fields no longer emitted: {extra:?}. \
                 Run `vox ci docs-reality-audit metrics --write`."
            );
        }
    }
```

- [ ] **Step 6: Verify against the real contracts**

```bash
cargo run -q -p vox-cli -- ci docs-reality-audit metrics --write
cargo run -q -p vox-cli -- ci docs-reality-audit verify
```

Expected: `docs-reality-audit verify OK (10 claims, 0 findings)`, exit 0.

- [ ] **Step 7: Prove the guard catches drift**

No Python — AGENTS.md §VoxScript-First, and it may not be on the runner:

```bash
sed -i 's/"inventory_claim_count": 10/"inventory_claim_count": 999/' contracts/reports/docs-reality-audit/metrics.v1.json
cargo run -q -p vox-cli -- ci docs-reality-audit verify
```

Expected: **non-zero exit**, message naming `inventory_claim_count`. Restore
without churning `generated_at`:

```bash
git checkout -- contracts/reports/docs-reality-audit/metrics.v1.json
```

- [ ] **Step 8: Commit**

```bash
git add crates/vox-cli-ci/src/docs_reality_audit.rs contracts/reports/docs-reality-audit/metrics.v1.json
git commit -m "fix(ci): docs-reality-audit verify now catches stale metrics"
```

---

### Task 13: Short-circuit glob matching in claim verification

`glob_match_count` collects **every** match into a `Vec` and returns the length,
but the only caller compares it against zero. Measured: the inventory's
`crates/**` pattern expands to **6,012 entries**. This runs inside `ssot-drift`,
inside the 60-second fast pre-push tier, on every push.

**Two corrections from revision 1:** there is exactly **one** call site (inside a
single merged loop over both glob fields), not two — and the plan's replacement
snippet referenced a variable named `pattern` that does not exist; the binding is
`g`.

**Files:**
- Modify: `crates/vox-cli-ci/src/docs_reality_audit.rs` (`glob_match_count` at `:104`, call site at `:139`)
- Test: same file's `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `repo_root_for_tests` from Task 12.
- Produces: `fn glob_has_match(root: &Path, pattern: &str) -> Result<bool>` replacing `glob_match_count`. Private, one call site.

- [ ] **Step 1: Confirm the single call site**

```bash
grep -n 'glob_match_count' crates/vox-cli-ci/src/docs_reality_audit.rs
sed -n '135,152p' crates/vox-cli-ci/src/docs_reality_audit.rs
```

Expected: exactly two hits — the definition at `:104` and one call at `:139`,
inside `for globs in [&h.code_globs, &h.tests_globs].into_iter().flatten()` with
the pattern bound as `g`. There is no separate `tests_globs` loop and no
per-field error wording to preserve.

- [ ] **Step 2: Write the failing test**

```rust
    #[test]
    fn glob_has_match_detects_presence_and_absence() {
        let root = repo_root_for_tests();
        assert!(
            glob_has_match(&root, "crates/vox-cli-ci/src/*.rs").expect("glob ok"),
            "the crate's own source directory must match"
        );
        assert!(
            !glob_has_match(&root, "crates/vox-cli-ci/src/*.no-such-extension")
                .expect("glob ok"),
            "a pattern matching nothing must return false, not error"
        );
    }
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p vox-cli-ci docs_reality_audit::tests::glob_has_match -- --nocapture`

Expected: FAIL to compile — `cannot find function 'glob_has_match'`.

- [ ] **Step 4: Replace the function**

```rust
/// Whether a glob pattern matches at least one path.
///
/// Short-circuits at the first match. The previous implementation materialised
/// every match into a Vec purely to test the count against zero — with the
/// inventory's `crates/**` pattern that is ~6,000 entries, inside `ssot-drift`'s
/// 60-second fast pre-push budget, on every push.
///
/// Behaviour change: a `GlobError` encountered *after* the first match is no
/// longer surfaced. That is the correct trade for a "does anything match"
/// predicate, but it is a change, not purely a speedup.
fn glob_has_match(root: &Path, pattern: &str) -> Result<bool> {
    let full = root.join(pattern);
    let pat = full.to_string_lossy().to_string();
    let mut entries = glob(&pat).with_context(|| format!("invalid glob pattern {pat:?}"))?;
    match entries.next() {
        None => Ok(false),
        Some(Ok(_)) => Ok(true),
        Some(Err(e)) => Err(anyhow!("glob iteration failed for {pat:?}: {e}")),
    }
}
```

- [ ] **Step 5: Update the single call site**

Replace lines `:139-150` — preserving the existing `inventory claim` prefix and
`.with_context()` wrapper, which revision 1's snippet dropped:

```rust
            let matched = glob_has_match(root, g).with_context(|| {
                format!(
                    "inventory claim {}: glob expansion failed for {g:?}",
                    claim.id
                )
            })?;
            if !matched {
                anyhow::bail!(
                    "inventory claim {}: glob matched 0 paths (expected ≥1): {g}",
                    claim.id
                );
            }
```

- [ ] **Step 6: Run the tests and the real gate**

```bash
cargo test -p vox-cli-ci docs_reality_audit
cargo run -q -p vox-cli -- ci docs-reality-audit verify
```

Expected: all tests PASS; `verify` prints `docs-reality-audit verify OK (10 claims, 0 findings)`.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-cli-ci/src/docs_reality_audit.rs
git commit -m "perf(ci): short-circuit glob matching in docs-reality-audit verify"
```

---

### Task 14: Regenerate `doc-inventory`, run the full gate, push once

Revision 1 omitted `doc-inventory` entirely. It runs in `pre-push --complete`
and in CI, and `verify_fresh` regenerates the whole inventory and diffs it. The
committed file carries per-file line counts for `AGENTS.md`,
`documentation-governance.md`, `chat.rs`, `docs_reality_audit.rs`, all three
`037-*.md` paths, and 681 `/tests/` entries — so the new guard file adds a row
too. **Nearly every task above drifts it.**

**Files:** `docs/agents/doc-inventory.json` (generated — never hand-edited).

**Interfaces:** consumes every preceding task.

- [ ] **Step 1: Format**

Run: `vox run scripts/fmt.vox`

- [ ] **Step 2: Regenerate the doc inventory**

```bash
cargo run -q -p vox-cli -- ci doc-inventory generate --output docs/agents/doc-inventory.json
git add docs/agents/doc-inventory.json
```

Note: this file also contains a known-bad entry that regeneration will **not**
fix — `inventory_gen.rs:60` hardcodes `crates/vox-mcp/src/tools/mod.rs` into
`first_read_for_agents`, and that crate is `vox-orchestrator-mcp`. That is spec
item W7.3, not in this plan; do not paper over it by hand-editing the JSON.

- [ ] **Step 3: Run the touched crates' test suites**

```bash
cargo test -p vox-doc-pipeline
cargo test -p vox-cli-ci
cargo test -p vox-cli commands::llm
cargo test -p vox-language-surface
```

Expected: all PASS.

- [ ] **Step 4: Clippy on the touched crates**

```bash
cargo clippy -p vox-doc-pipeline --all-targets -- -D warnings
cargo clippy -p vox-cli-ci --all-targets -- -D warnings
cargo clippy -p vox-cli --all-targets -- -D warnings
cargo clippy -p vox-actor-runtime --all-targets -- -D warnings
```

Expected: clean.

- [ ] **Step 5: Run the docs and contract gates**

```bash
cargo run -p vox-doc-pipeline -- --lint-only
cargo run -q -p vox-cli -- ci check-links
cargo run -q -p vox-cli -- ci retired-symbol-check
cargo run -q -p vox-cli -- ci docs-reality-audit verify
cargo run -q -p vox-arch-check
cargo run -q -p vox-cli -- ci line-endings
```

Expected: all exit 0.

- [ ] **Step 6: Run the full pre-push tier**

Run: `vox ci pre-push --full`

**`--full`, not `--complete`.** `--complete` runs fmt, line-endings, ssot-drift,
doc lint + doctest, doc-inventory, workspace clippy, and scoped TOESTUB — but
**no tests**. Only `--full` adds `cargo nextest run --workspace`, which is the
only tier that executes the guards this plan adds.

- [ ] **Step 7: Push once**

```bash
git push -u origin HEAD
```

CodeRabbit reviews once on open. Re-request with a `@coderabbitai review`
comment; do not push incrementally.

---

## Self-Review

**1. Spec coverage.**

| Spec item | Task |
| --- | --- |
| W1.1 AGENTS.md decorator contradiction (+ lines 494/499/507, `@activity`) | 1 |
| W1.2 three missing retired crates | 3 |
| W1.3 governance vocabulary (+ ADR-002, `lint.rs:16`) | 4 |
| W1.4 astro comment | 7 |
| W1.5 `infer_with_retry` comment | 7 |
| W1.6 audit cadence claim | 7 |
| W1.7 ADR numbering (collision, index, prose, `NNN`) | 5 |
| W1.8 NUL byte | 6 |
| W3.2 root-cause detector fix (replaces retracted R5) | 2 |
| W3.5 check-links policy roots | 8 |
| W6.1 delete `rollout_milestone_pct` | 11 |
| W6.2 metrics recompute | 12 |
| W6.6 glob short-circuit | 13 |
| W7.1 `vox llm prompt` | 9 |
| W7.2 language-surface JSON | 10 |

Deliberately out of scope, with reasons in Global Constraints: W3.1 (needs the
severity valve first), W6.3/W6.4 (zero executions while the backlog is empty),
W7.3–W7.8, W2, W4, W5, W8.

**2. Placeholder scan.** No TBDs. Task 5 substitutes real numbers (044/045)
rather than revision 1's `NNN`/`MMM` placeholders. Task 2 and Task 10 each carry
one conditional fallback with an explicit trigger and instruction, not a
deferral.

**3. Type consistency.** `repo_root()` (Task 5) lives in
`policy_docs_guard.rs`, reused by Task 6. `repo_root_for_tests()` (Task 12) lives
in `docs_reality_audit.rs`'s test module, reused by Task 13 — a different file,
which is why both exist, and both use `.ancestors().nth(2)` matching the
workspace convention. `test_finding` (Task 12) sets all eight `FindingRow`
fields; Task 11 removes `rollout_milestone_pct` before Task 12 extracts
`compute_metrics`, so the extracted function omits the deleted fields. Task 13
fully replaces `glob_match_count` with `glob_has_match` and updates its single
call site in the same task.

**Ordering:** 1 → 2 → 3 (Task 2's acceptance check depends on Task 1; Task 3's
depends on Task 2). 11 → 12 → 13 strictly (each depends on the previous task's
edits to the same file). 4, 5 → 6, 7, 8, 9, 10 are mutually independent. 14 last.
