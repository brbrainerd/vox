# Gate and Policy Honesty Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every policy file and every audit-machine number in this repository tell the truth, and add regression guards so the highest-value falsehoods cannot return.

**Architecture:** Two halves of one defect class. W1 fixes documents that state something false — most damagingly, `AGENTS.md` instructing every agent to emit syntax that is a hard parse error. W6 fixes the `docs-reality-audit` machinery whose numbers cannot distinguish "not started" from "full backlog" and whose verifier never notices its own metrics going stale. New guards are plain `#[test]` integration tests reading the repo tree — no new public API, no CLI wiring, no crate edges, no LoC-budget pressure.

**Tech Stack:** Rust (`vox-doc-pipeline`, `vox-cli-ci`), Markdown, JSON contracts.

**Spec:** `docs/superpowers/specs/2026-08-22-docs-corpus-repair-design.md`

## Global Constraints

- **This plan covers spec workstreams W1 and W6 only.** W2 (corpus repair), W3 (detector holes), W4 (visual), W5 (retirement) are separate plans. Do not start them here.
- **No new crate, no new GUI surface, no new findings schema, no new scheduler.**
- **No new crate edges.** Do not add entries to `contracts/ci/crate-edges.allow.v1.json`. Exception entries are user-authorized-only.
- **Test-first is binding.** Every new `pub fn` in `crates/*/src/**` needs a test in the same file before the implementation. The `tdd-guard` lefthook blocks commits that violate this. This plan avoids new `pub fn` almost entirely by using integration tests.
- **Never run `cargo fmt --all`** on this workspace — it overflows the Windows command-line limit (`os error 206`). Use `vox run scripts/fmt.vox`, or `cargo fmt -p <crate>` for one crate.
- **Line endings:** `md`, `rs`, `json`, `yaml` are all LF (`EXT_LF` in `crates/vox-cli-ci/src/line_endings.rs:10`). `.gitattributes:14` enforces `*.md text eol=lf`. If your editor writes CRLF, strip it before committing.
- **Verification tier:** `vox ci pre-push --complete`, not the default fast tier. Fast omits clippy and all tests.
- **`ssot-drift` is fail-fast** across ~30 sequential guards — one bad row fails the whole bundle with a single message.
- **PR discipline:** CodeRabbit reviews once on open (`auto_incremental_review: false`). Batch commits; push once when review-ready; re-request with a `@coderabbitai review` comment, never by re-pushing.
- **Do not modify `docs/src/archive/**`** beyond frontmatter CI requires (AGENTS.md §Archival Protocol).

---

## File Structure

| File | Responsibility | Task |
| --- | --- | --- |
| `crates/vox-doc-pipeline/tests/policy_docs_guard.rs` | **New.** Integration tests asserting repo policy documents are internally consistent. Four independent guards, one per defect. | 1–4 |
| `AGENTS.md` | Retired-surfaces table: fix the decorator replacement column; add three missing crate rows. | 1, 6 |
| `docs/src/contributors/documentation-governance.md` | Category vocabulary table: replace slugs with the enforced display labels. | 2 |
| `docs/src/adr/037-tauri-convergence.md` and siblings | Resolve the three-way ADR 037 number collision. | 3 |
| `docs/src/adr/index.md` | Add index rows for the renumbered ADRs. | 3 |
| `docs/src/architecture/data-storage-lint-and-ci-spec-2026.md` | Strip the embedded NUL byte that makes `grep` treat it as binary. | 4 |
| `docs-astro/astro.config.mjs:27` | Comment claims the sidebar comes from `SUMMARY.md`; it comes from frontmatter. | 5 |
| `crates/vox-actor-runtime/src/llm/chat.rs:359-361` | `infer_with_retry` doc comment describes error handling the body does not perform. | 5 |
| `docs/src/contributors/docs-reality-audit-program.md` | Declares a weekly/monthly cadence that has never run. | 5 |
| `crates/vox-cli-ci/src/docs_reality_audit.rs` | All six W6 correctness fixes plus their unit tests. | 7–12 |

Why `crates/vox-doc-pipeline/tests/` for the guards: `vox-doc-pipeline` is the docs-lint home and is **unbudgeted** (1,693 LoC, no `max_loc` in `layers.toml`), whereas `vox-cli-ci` is already 23,910 LoC against a 15,000 budget (159%). An integration test needs no `pub fn`, no CLI subcommand, and no registration, so it adds a gate without adding surface area.

---

### Task 1: Guard and fix the `AGENTS.md` decorator contradiction

This is the single highest-value fix in the plan. `AGENTS.md` is loaded into every agent session. Its §Retired Surfaces table gives the replacement for the removed `@endpoint(kind: ...)` decorator as the at-prefixed `@server fn` / `@query fn` / `@mutation fn` forms — while §Grammar Unification in the same file states those at-prefixed forms became **hard parse errors on 2026-06-30** (`cd7cc96874`). The policy file instructs agents to write code that cannot compile.

**Files:**
- Create: `crates/vox-doc-pipeline/tests/policy_docs_guard.rs`
- Modify: `AGENTS.md` (the §Retired Surfaces row whose left column begins `` `@endpoint(kind: server\|query\|mutation) fn` ``)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `fn repo_root() -> std::path::PathBuf` and `fn read_repo_file(rel: &str) -> String`, both private to this test file, reused by Tasks 2, 3, and 4.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-doc-pipeline/tests/policy_docs_guard.rs`:

```rust
//! Guards that repo policy documents do not contradict themselves.
//!
//! These are integration tests rather than lint rules on purpose: they assert
//! facts about specific policy files, not a property of every doc, and they
//! need no CLI wiring to run in CI's test tier.

use std::path::{Path, PathBuf};

/// Workspace root, two levels up from this crate's manifest.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("canonicalize repo root")
}

fn read_repo_file(rel: &str) -> String {
    let path = repo_root().join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// The at-prefixed spellings of the data-layer keywords became hard parse
/// errors on 2026-06-30 (cd7cc96874). AGENTS.md must never prescribe them as a
/// replacement for anything, or every agent session emits uncompilable code.
#[test]
fn agents_md_does_not_prescribe_retired_decorator_forms() {
    let agents = read_repo_file("AGENTS.md");

    // Isolate the Retired Surfaces table: rows between its heading and the next heading.
    let start = agents
        .find("## Retired Surfaces")
        .expect("AGENTS.md must contain a '## Retired Surfaces' section");
    let rest = &agents[start..];
    let end = rest[3..].find("\n## ").map(|i| i + 3).unwrap_or(rest.len());
    let table = &rest[..end];

    // Only the replacement (right-hand) column matters. The left column names
    // the retired form on purpose.
    let mut offenders = Vec::new();
    for line in table.lines() {
        if !line.trim_start().starts_with('|') {
            continue;
        }
        let cols: Vec<&str> = line.split('|').collect();
        // "| left | right |" splits to ["", " left ", " right ", ""].
        if cols.len() < 4 {
            continue;
        }
        let replacement = cols[2];
        for retired in [
            "@server fn",
            "@query fn",
            "@mutation fn",
            "@table ",
            "@form ",
            "@resource ",
            "@index ",
        ] {
            if replacement.contains(retired) {
                offenders.push(format!("{retired:?} in replacement column: {}", line.trim()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "AGENTS.md Retired Surfaces prescribes retired at-prefixed decorator forms \
         that are hard parse errors per AGENTS.md's own Grammar Unification section. \
         Use the bare-keyword forms instead. Offending rows:\n{}",
        offenders.join("\n")
    );
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p vox-doc-pipeline --test policy_docs_guard agents_md_does_not_prescribe -- --nocapture`

Expected: FAIL. The panic message lists the row whose replacement column reads `` `@server fn` / `@query fn` / `@mutation fn` ``.

- [ ] **Step 3: Fix `AGENTS.md`**

In the §Retired Surfaces table, change the replacement cell of the `@endpoint` row from the at-prefixed forms to the bare-keyword forms, so the row reads:

```
| `@endpoint(kind: server\|query\|mutation) fn` (removed v0.6.0) | `server fn` / `query fn` / `mutation fn` (bare-keyword; the at-prefixed forms are hard parse errors as of 2026-06-30) |
```

Do not change the left column — naming the retired form is that column's job.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p vox-doc-pipeline --test policy_docs_guard agents_md_does_not_prescribe`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add AGENTS.md crates/vox-doc-pipeline/tests/policy_docs_guard.rs
git commit -m "fix(docs): AGENTS.md prescribed decorator forms that are parse errors"
```

---

### Task 2: Guard and fix the category vocabulary contradiction

`documentation-governance.md` is the SSOT that `AGENTS.md` points contributors at for frontmatter. Its §Category vocabulary table lists slugs (`architecture`, `how-to`, `reference`). The enforced list, `VALID_CATEGORIES` in `crates/vox-doc-pipeline/src/pipeline/lint.rs:18`, contains display labels (`"Architecture SSOTs"`, `"How-To Guides"`). Validation is an exact `VALID_CATEGORIES.contains(&value)` at `lint.rs:395` — there is no alias normalisation, and `suggest()` only produces a hint after failure. All 918 docs already use display labels; **zero** use the documented slugs. A contributor following the governance doc fails the lint.

**Files:**
- Modify: `crates/vox-doc-pipeline/tests/policy_docs_guard.rs` (add one test)
- Modify: `docs/src/contributors/documentation-governance.md:41-53`

**Interfaces:**
- Consumes: `read_repo_file` from Task 1.
- Produces: nothing consumed by later tasks.

- [ ] **Step 1: Write the failing test**

Append to `crates/vox-doc-pipeline/tests/policy_docs_guard.rs`:

```rust
/// Every `category` value the governance doc advertises must be one the lint
/// actually accepts. lint.rs:395 does an exact string match with no alias map,
/// so a slug in the governance table is an instruction to fail CI.
#[test]
fn governance_category_vocabulary_matches_enforced_list() {
    // Mirrors VALID_CATEGORIES in crates/vox-doc-pipeline/src/pipeline/lint.rs.
    // Kept as a literal so this test fails loudly if either side drifts.
    const ENFORCED: &[&str] = &[
        "Getting Started",
        "Tutorials",
        "How-To Guides",
        "Language Reference",
        "API Reference — Crates",
        "Examples",
        "Concepts",
        "Architecture Decisions (ADRs)",
        "Architecture SSOTs",
        "Contributors",
        "CI & Quality",
        "Operations",
        "archive",
    ];

    let doc = read_repo_file("docs/src/contributors/documentation-governance.md");
    let start = doc
        .find("### Category vocabulary")
        .expect("governance doc must have a '### Category vocabulary' section");
    let rest = &doc[start..];
    let end = rest[3..].find("\n### ").map(|i| i + 3).unwrap_or(rest.len());
    let table = &rest[..end];

    let mut advertised = Vec::new();
    for line in table.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("| `") {
            continue;
        }
        let cols: Vec<&str> = trimmed.split('|').collect();
        if cols.len() < 3 {
            continue;
        }
        let value = cols[1].trim().trim_matches('`');
        if value == "category" || value.is_empty() {
            continue;
        }
        advertised.push(value.to_string());
    }

    assert!(
        !advertised.is_empty(),
        "parsed zero category values from the governance table — the table shape changed, \
         so this guard is no longer checking anything"
    );

    let unknown: Vec<&String> = advertised
        .iter()
        .filter(|v| !ENFORCED.contains(&v.as_str()))
        .collect();

    assert!(
        unknown.is_empty(),
        "documentation-governance.md advertises category values the lint rejects \
         (lint.rs:395 is an exact match, there is no alias map): {unknown:?}"
    );
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p vox-doc-pipeline --test policy_docs_guard governance_category -- --nocapture`

Expected: FAIL, listing `["getting-started", "tutorial", "how-to", "explanation", "reference", "adr", "architecture", "ci", "contributor"]`.

- [ ] **Step 3: Fix the governance table**

Replace the table body at `docs/src/contributors/documentation-governance.md:41-52` with the enforced display labels:

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

Then replace line 53 with:

```markdown
These display labels are the enforced vocabulary — `VALID_CATEGORIES` in
`crates/vox-doc-pipeline/src/pipeline/lint.rs` matches them exactly, with no
alias normalisation. They must stay in sync with the `sections` array in
`contracts/documentation/docs-sidebar-section-order.v1.json`.
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p vox-doc-pipeline --test policy_docs_guard governance_category`

Expected: PASS.

- [ ] **Step 5: Verify the governance doc still lints**

Run: `cargo run -p vox-doc-pipeline -- --lint-only --paths contributors/documentation-governance.md`

Expected: no errors. (The doc's own frontmatter `category` is unchanged; this confirms the edit did not break the fence or frontmatter parsing.)

- [ ] **Step 6: Commit**

```bash
git add docs/src/contributors/documentation-governance.md crates/vox-doc-pipeline/tests/policy_docs_guard.rs
git commit -m "fix(docs): governance category vocabulary did not match the enforced list"
```

---

### Task 3: Guard and fix the ADR 037 number collision

Three files claim ADR number 037: `037-ai-fixture-subagent-decorator.md`, `037-tauri-convergence.md`, `037-tauri-gui-replaces-axum-dashboard.md`. Only one appears in `docs/src/adr/index.md` (line 51). An ADR number is a citation key; three documents sharing one makes every reference to "ADR-037" ambiguous.

**Files:**
- Modify: `crates/vox-doc-pipeline/tests/policy_docs_guard.rs` (add one test)
- Rename: two of the three `037-*.md` files in `docs/src/adr/`
- Modify: `docs/src/adr/index.md`

**Interfaces:**
- Consumes: `repo_root` from Task 1.
- Produces: nothing consumed by later tasks.

- [ ] **Step 1: Determine the next free ADR numbers**

Run: `ls docs/src/adr/ | grep -oE '^[0-9]{3}' | sort -n | tail -3`

Note the highest number in use. The two renamed ADRs take the next two free numbers above it. This plan calls them `NNN` and `MMM`; substitute the real values in every step below.

- [ ] **Step 2: Write the failing test**

Append to `crates/vox-doc-pipeline/tests/policy_docs_guard.rs`:

```rust
/// An ADR number is a citation key. Two documents sharing one makes every
/// "ADR-0NN" reference in the corpus ambiguous.
#[test]
fn adr_numbers_are_unique() {
    use std::collections::BTreeMap;

    let adr_dir = repo_root().join("docs").join("src").join("adr");
    let mut by_number: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for entry in std::fs::read_dir(&adr_dir).expect("read docs/src/adr") {
        let entry = entry.expect("read dir entry");
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.ends_with(".md") {
            continue;
        }
        let digits: String = name.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.len() != 3 {
            continue; // index.md, README.md, and named-not-numbered ADRs
        }
        by_number.entry(digits).or_default().push(name);
    }

    let collisions: Vec<_> = by_number
        .iter()
        .filter(|(_, files)| files.len() > 1)
        .collect();

    assert!(
        collisions.is_empty(),
        "ADR numbers must be unique; collisions: {collisions:?}"
    );
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p vox-doc-pipeline --test policy_docs_guard adr_numbers_are_unique -- --nocapture`

Expected: FAIL, reporting `"037"` mapped to three filenames.

- [ ] **Step 4: Renumber two of the three ADRs**

Keep `037-tauri-convergence.md` at 037 — it is the one already cited in `index.md:51`, so leaving it put avoids rewriting existing references.

```bash
git mv docs/src/adr/037-ai-fixture-subagent-decorator.md docs/src/adr/NNN-ai-fixture-subagent-decorator.md
git mv docs/src/adr/037-tauri-gui-replaces-axum-dashboard.md docs/src/adr/MMM-tauri-gui-replaces-axum-dashboard.md
```

Use `git mv`, not `mv` — history preservation matters because the doc pipeline derives `last_updated` from Git.

- [ ] **Step 5: Update the heading and any self-reference inside each renamed file**

Open each renamed file and change its H1 and any `ADR-037` self-reference to the new number. Check with:

```bash
grep -n '037' docs/src/adr/NNN-ai-fixture-subagent-decorator.md docs/src/adr/MMM-tauri-gui-replaces-axum-dashboard.md
```

- [ ] **Step 6: Add index rows for both renamed ADRs**

In `docs/src/adr/index.md`, add a row for each, following the existing format at line 51:

```markdown
| [NNN](NNN-ai-fixture-subagent-decorator.md) | **AI fixture subagent decorator** |
| [MMM](MMM-tauri-gui-replaces-axum-dashboard.md) | **Tauri GUI replaces the Axum dashboard** |
```

- [ ] **Step 7: Find and rewrite inbound links to the two renamed files**

```bash
grep -rn '037-ai-fixture-subagent-decorator\|037-tauri-gui-replaces-axum-dashboard' \
  docs AGENTS.md CLAUDE.md GEMINI.md crates contracts
```

Rewrite every hit to the new filename. Note that `vox ci check-links` does **not** scan `CLAUDE.md` or `GEMINI.md`, so those two must be checked by hand here — that gap is fixed in the W3 plan, not this one.

- [ ] **Step 8: Run the test and the link gate**

```bash
cargo test -p vox-doc-pipeline --test policy_docs_guard adr_numbers_are_unique
cargo run -q -p vox-cli -- ci check-links
```

Expected: test PASS, `check-links` exits 0.

- [ ] **Step 9: Commit**

```bash
git add docs/src/adr crates/vox-doc-pipeline/tests/policy_docs_guard.rs
git commit -m "fix(docs): resolve three-way ADR 037 number collision"
```

---

### Task 4: Guard and fix the NUL byte that hides a doc from grep

`docs/src/architecture/data-storage-lint-and-ci-spec-2026.md` contains exactly **one** embedded NUL byte in 37,977 bytes. That is enough for `grep` to classify the file as binary and skip it silently, so every grep-based audit — including the ones that produced this plan's spec — has a blind spot there. Rust detectors read it fine, so CI never noticed. It is the only live doc with this defect.

**Files:**
- Modify: `crates/vox-doc-pipeline/tests/policy_docs_guard.rs` (add one test)
- Modify: `docs/src/architecture/data-storage-lint-and-ci-spec-2026.md`

**Interfaces:**
- Consumes: `repo_root` from Task 1.
- Produces: nothing consumed by later tasks.

- [ ] **Step 1: Write the failing test**

Append to `crates/vox-doc-pipeline/tests/policy_docs_guard.rs`:

```rust
/// A single NUL byte makes grep treat a markdown file as binary and skip it
/// silently. Every grep-based audit then has an invisible blind spot.
#[test]
fn no_docs_contain_nul_bytes() {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
                out.push(path);
            }
        }
    }

    let docs_src = repo_root().join("docs").join("src");
    let mut files = Vec::new();
    walk(&docs_src, &mut files);

    assert!(
        !files.is_empty(),
        "walked zero markdown files under docs/src — the guard is not checking anything"
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

Run: `cargo test -p vox-doc-pipeline --test policy_docs_guard no_docs_contain_nul -- --nocapture`

Expected: FAIL, naming `data-storage-lint-and-ci-spec-2026.md (1 NUL bytes)`.

- [ ] **Step 3: Strip the NUL byte**

```bash
f=docs/src/architecture/data-storage-lint-and-ci-spec-2026.md
tr -d '\000' < "$f" > "$f.tmp" && mv "$f.tmp" "$f"
```

- [ ] **Step 4: Confirm the byte count dropped by exactly one and grep now sees text**

```bash
wc -c < docs/src/architecture/data-storage-lint-and-ci-spec-2026.md
grep -qI . docs/src/architecture/data-storage-lint-and-ci-spec-2026.md && echo text || echo binary
```

Expected: `37976`, and `text`.

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p vox-doc-pipeline --test policy_docs_guard no_docs_contain_nul`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add docs/src/architecture/data-storage-lint-and-ci-spec-2026.md crates/vox-doc-pipeline/tests/policy_docs_guard.rs
git commit -m "fix(docs): strip NUL byte hiding data-storage spec from grep"
```

---

### Task 5: Correct three comments that describe behavior the code does not have

No tests here — these are comments, and a test asserting a comment's prose would be worse than the defect. They are grouped into one commit because they share a cause: the code moved and the prose did not.

**Files:**
- Modify: `docs-astro/astro.config.mjs:27`
- Modify: `crates/vox-actor-runtime/src/llm/chat.rs:359-361`
- Modify: `docs/src/contributors/docs-reality-audit-program.md` (§Operating cadence)

**Interfaces:**
- Consumes: nothing.
- Produces: nothing.

- [ ] **Step 1: Fix the Astro sidebar comment**

`docs-astro/astro.config.mjs:27` reads:

```js
      // Sidebar is dynamically generated from SUMMARY.md to maintain SSOT
```

The sidebar comes from `getSidebar()` in `docs-astro/src/utils/sidebar.mjs`, which walks `docs/src`, parses frontmatter with `gray-matter`, and groups by `category` ordered by `sort_order` then `title`, with section order from `contracts/documentation/docs-sidebar-section-order.v1.json`. `SUMMARY.md` is gitignored and not read. Replace with:

```js
      // Sidebar is generated from each page's frontmatter (category / sort_order /
      // title) by src/utils/sidebar.mjs; section order comes from
      // contracts/documentation/docs-sidebar-section-order.v1.json.
```

- [ ] **Step 2: Fix the `infer_with_retry` doc comment**

`crates/vox-actor-runtime/src/llm/chat.rs:359-361` currently reads:

```rust
/// Exhaustive retry loop over multiple candidate LLM configurations.
/// Used for robust agent fallback routing. Iterates models sequentially until
/// one succeeds, skipping specific candidates on 401s or continuing on 429/timeout.
```

The body performs no error classification: every failure class — 401, 429, 5xx, timeout, DNS — takes the same branch, records `last_error`, and advances to the next candidate. There is no retry, no backoff, and no sleep. `EgressError::RateLimited { retry_after }` is captured at `crates/vox-llm-egress/src/wire.rs:179` and discarded. Replace with:

```rust
/// Sequential fallback over multiple candidate LLM configurations.
///
/// Tries each candidate exactly once, in order, and returns the first success.
/// There is deliberately no per-candidate retry and no error classification:
/// every failure class (401, 429, 5xx, timeout, transport) is treated
/// identically and simply advances to the next candidate. In particular a
/// 429's `retry_after` is not honoured — nothing sleeps.
///
/// Callers needing genuine provider fallback must pass a multi-candidate
/// vector; passing `vec![cfg]` yields exactly one attempt with no fallback.
```

Do not change the behavior in this task — that is a runtime change, out of scope for a docs plan, and is recorded in the spec §8 as deferred.

- [ ] **Step 3: Correct the audit program's cadence claim**

`docs/src/contributors/docs-reality-audit-program.md` §Operating cadence declares weekly, monthly, and release-gate cycles. Zero cycles have run: `findings.v1.json` has one commit in its history (`3295a3bee`, 2026-05-12) and contains zero findings. Replace the §Operating cadence body with:

```markdown
## Operating cadence

**Status: dormant.** The cadence below is the intended operating model, not a
description of current practice. As of 2026-08-22 the backlog holds zero
findings and `findings.v1.json` has one commit in its history (`3295a3bee`,
2026-05-12). Treat any metric derived from it as "not started" rather than
"healthy".

Intended cycles, when the program is resumed:

- **Weekly:** extend inventory / findings for files touched in the branch; re-run `verify`
- **Monthly:** full pass over `docs/src/` claims and hygiene of closed vs open findings
- **Release:** focus on `docs/src/reference/cli.md`, env vars, and operations catalog parity
```

- [ ] **Step 4: Verify the docs still lint and the JS still parses**

```bash
cargo run -p vox-doc-pipeline -- --lint-only --paths contributors/docs-reality-audit-program.md
node --check docs-astro/astro.config.mjs
cargo check -p vox-actor-runtime
```

Expected: all three succeed.

- [ ] **Step 5: Commit**

```bash
git add docs-astro/astro.config.mjs crates/vox-actor-runtime/src/llm/chat.rs docs/src/contributors/docs-reality-audit-program.md
git commit -m "docs: correct three comments describing behavior the code lacks"
```

---

### Task 6: Add three missing crates to the retired-surfaces table

`AGENTS.md` §Retired Surfaces is the table agents consult before naming a crate. Three renamed or deleted crates are absent from it, and the corpus consequently carries ~340 references to paths that do not exist: `crates/vox-dashboard` (299 references, named as the canonical implementation target in five ADRs and two reference docs), `vox-dei-shim` (26, renamed to `vox-research-shim`), and `crates/vox-oratio` (renamed to `crates/vox-speech`).

This task adds the table rows only. Fixing the ~340 references is W2, a separate plan — but the rows must exist first, or the fixes have no authority to cite.

**Files:**
- Modify: `AGENTS.md` (§Retired Surfaces table)
- Modify: `crates/vox-doc-pipeline/tests/policy_docs_guard.rs` (add one test)

**Interfaces:**
- Consumes: `read_repo_file` and `repo_root` from Task 1.
- Produces: nothing consumed by later tasks.

- [ ] **Step 1: Write the failing test**

Append to `crates/vox-doc-pipeline/tests/policy_docs_guard.rs`:

```rust
/// Every `crates/<name>` path AGENTS.md names in the *replacement* column of
/// the Retired Surfaces table must actually exist. A retirement table that
/// points at a nonexistent crate is worse than no table.
#[test]
fn agents_md_retired_table_replacements_exist() {
    let agents = read_repo_file("AGENTS.md");
    let start = agents
        .find("## Retired Surfaces")
        .expect("AGENTS.md must contain a '## Retired Surfaces' section");
    let rest = &agents[start..];
    let end = rest[3..].find("\n## ").map(|i| i + 3).unwrap_or(rest.len());
    let table = &rest[..end];

    let crates_dir = repo_root().join("crates");
    let mut missing = Vec::new();

    for line in table.lines() {
        if !line.trim_start().starts_with('|') {
            continue;
        }
        let cols: Vec<&str> = line.split('|').collect();
        if cols.len() < 4 {
            continue;
        }
        // Replacement column only; the retired column names dead crates on purpose.
        for token in cols[2].split('`') {
            let name = token.trim();
            if !name.starts_with("vox-") || name.contains(' ') || name.contains("::") {
                continue;
            }
            if !crates_dir.join(name).is_dir() {
                missing.push(format!("{name} (row: {})", line.trim()));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "AGENTS.md Retired Surfaces names replacement crates that do not exist \
         under crates/:\n{}",
        missing.join("\n")
    );
}
```

- [ ] **Step 2: Run the test to verify it passes today**

Run: `cargo test -p vox-doc-pipeline --test policy_docs_guard agents_md_retired_table_replacements_exist -- --nocapture`

Expected: **PASS**. This guard protects the rows you are about to add — it is written first so that a typo in Step 3 fails immediately rather than silently shipping a table pointing at a nonexistent crate. If it fails now, an existing row is already broken; fix that before continuing.

- [ ] **Step 3: Add the three rows to the `AGENTS.md` retired-surfaces table**

```markdown
| `crates/vox-dashboard` (never existed / Axum dashboard retired) | `crates/vox-gui` (Tauri 2) |
| `crates/vox-oratio` (crate renamed; the `vox oratio` CLI command is unaffected) | `crates/vox-speech` |
| `vox-dei-shim` | `vox-research-shim` |
```

Note the `vox-oratio` row's parenthetical: the **CLI command** `vox oratio` still exists in `crates/vox-cli/src/lib.rs`. Only the crate path is retired. Omitting that qualifier would send agents to delete a working command.

- [ ] **Step 4: Run both AGENTS.md guards**

Run: `cargo test -p vox-doc-pipeline --test policy_docs_guard agents_md`

Expected: both `agents_md_does_not_prescribe_retired_decorator_forms` and `agents_md_retired_table_replacements_exist` PASS.

- [ ] **Step 5: Commit**

```bash
git add AGENTS.md crates/vox-doc-pipeline/tests/policy_docs_guard.rs
git commit -m "docs(agents): add vox-dashboard, vox-oratio, vox-dei-shim to retired surfaces"
```

---

### Task 7: Make `rollout_milestone_pct` distinguish "not started" from "in progress"

`rollout_milestone_pct` returns 25 when the findings list is empty, and `25 + (closed/total) * 75` otherwise. Filing 80 open findings therefore produces **exactly 25** — identical to filing none. The program's single headline metric cannot tell an untouched backlog from a full one, which is precisely why `metrics.v1.json` reading `rollout_milestone_pct: 25` went unquestioned for three months.

**Files:**
- Modify: `crates/vox-cli-ci/src/docs_reality_audit.rs:264-279` (the `rollout_milestone_pct` function)
- Test: `crates/vox-cli-ci/src/docs_reality_audit.rs` (the existing `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `FindingScores`, `FindingRow` (already defined in this file).
- Produces: `rollout_milestone_pct(inv_claims: usize, findings: &[FindingRow]) -> u8` — signature unchanged; only the mapping changes.

- [ ] **Step 1: Write the failing test**

The existing test module already has a helper-free style, so build `FindingRow` values inline. Add to `#[cfg(test)] mod tests` in `crates/vox-cli-ci/src/docs_reality_audit.rs`:

```rust
    fn test_finding(id: &str, status: &str) -> FindingRow {
        FindingRow {
            id: id.to_string(),
            claim_ids: vec!["claim.test".to_string()],
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
    fn rollout_distinguishes_empty_backlog_from_all_open() {
        let empty = rollout_milestone_pct(10, &[]);
        let all_open = rollout_milestone_pct(
            10,
            &[test_finding("f.1", "new"), test_finding("f.2", "triaged")],
        );
        assert_eq!(empty, 25, "empty backlog stays at the documented 25");
        assert!(
            all_open > empty,
            "a backlog with open findings must not report the same milestone as an \
             empty one (got {all_open} vs {empty})"
        );
    }

    #[test]
    fn rollout_reaches_100_when_all_findings_closed() {
        let findings = [test_finding("f.1", "closed"), test_finding("f.2", "verified")];
        assert_eq!(rollout_milestone_pct(10, &findings), 100);
    }

    #[test]
    fn rollout_is_monotonic_in_closed_ratio() {
        let none_closed = rollout_milestone_pct(
            10,
            &[test_finding("f.1", "new"), test_finding("f.2", "new")],
        );
        let one_closed = rollout_milestone_pct(
            10,
            &[test_finding("f.1", "closed"), test_finding("f.2", "new")],
        );
        assert!(one_closed > none_closed, "{one_closed} must exceed {none_closed}");
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p vox-cli-ci docs_reality_audit::tests::rollout -- --nocapture`

Expected: `rollout_distinguishes_empty_backlog_from_all_open` FAILS with `25 > 25` being false. `rollout_reaches_100_when_all_findings_closed` passes already; keep it as a regression anchor.

- [ ] **Step 3: Fix the function**

Replace the body of `rollout_milestone_pct` at `crates/vox-cli-ci/src/docs_reality_audit.rs:264`:

```rust
/// Rollout milestone as a percentage.
///
/// Band meanings, chosen so the number is never ambiguous:
///   0        — no inventory; the program has not been set up
///   25       — inventory exists, backlog empty; nothing has been triaged yet
///   26..=100 — backlog non-empty, scaling with the closed ratio
///
/// The 26 floor matters: before it, a backlog of 80 open findings reported the
/// same 25 as an empty one, so the metric could not distinguish "not started"
/// from "nothing finished".
fn rollout_milestone_pct(inv_claims: usize, findings: &[FindingRow]) -> u8 {
    if inv_claims == 0 {
        return 0;
    }
    if findings.is_empty() {
        return 25;
    }
    let total = findings.len() as f64;
    let closed = findings
        .iter()
        .filter(|f| f.status == "closed" || f.status == "verified")
        .count() as f64;
    let pct = 26.0 + (closed / total) * 74.0;
    pct.round().clamp(0.0, 100.0) as u8
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p vox-cli-ci docs_reality_audit::tests::rollout`

Expected: all three PASS, and the pre-existing `rollout_milestone_empty_findings_is_25_when_inventory_nonempty` still PASSES.

- [ ] **Step 5: Regenerate metrics and confirm the committed file is consistent**

```bash
cargo run -q -p vox-cli -- ci docs-reality-audit metrics --write
git diff --stat contracts/reports/docs-reality-audit/metrics.v1.json
```

Expected: only `generated_at` changes, because the backlog is still empty and the empty case still returns 25.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-cli-ci/src/docs_reality_audit.rs contracts/reports/docs-reality-audit/metrics.v1.json
git commit -m "fix(ci): rollout_milestone_pct reported 25 for both empty and all-open backlogs"
```

---

### Task 8: Make `verify` recompute metrics instead of only schema-checking them

`run_verify` validates `metrics.v1.json` against its schema and stops. It never recomputes. Nothing in CI or `lefthook.yml` runs `metrics --write`, and `ssot-autoregen` does not touch this file. A 150-claim inventory sitting beside `inventory_claim_count: 10` is therefore a **green build** — which is exactly how the committed metrics reached 2026-05-12 and stayed there.

**Files:**
- Modify: `crates/vox-cli-ci/src/docs_reality_audit.rs` — extract the metrics computation from `run_metrics`, call it from `run_verify`
- Test: same file's `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `InventoryFile`, `FindingsFile`, `rollout_milestone_pct` (this file).
- Produces: `fn compute_metrics(inv: &InventoryFile, findings: &FindingsFile) -> Value` — private to this module; returns the metrics object **without** `generated_at`, which callers add. Consumed by `run_metrics` and `run_verify`.

- [ ] **Step 1: Write the failing test**

Add to `#[cfg(test)] mod tests`:

```rust
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

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p vox-cli-ci docs_reality_audit::tests::compute_metrics_excludes -- --nocapture`

Expected: FAIL to compile — `cannot find function 'compute_metrics' in this scope`.

- [ ] **Step 3: Extract `compute_metrics` and call it from both entry points**

Add the function just above `run_metrics` in `crates/vox-cli-ci/src/docs_reality_audit.rs`:

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
    let milestone = rollout_milestone_pct(inv.claims.len(), &findings.findings);

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
        "open_p1": open_p1,
        "rollout_milestone_pct": milestone,
        "rollout_notes": "Computed by `vox ci docs-reality-audit metrics`; see contracts/documentation/docs-reality-audit.program.v1.yaml."
    })
}
```

In `run_metrics`, delete the inline computation (the block from `let mut counts_class` through the `let metrics = serde_json::json!({...});` literal) and replace it with:

```rust
    let mut metrics = compute_metrics(&inv, &findings);
    let generated_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    metrics["generated_at"] = Value::String(generated_at);
```

Leave the rest of `run_metrics` (schema validation, the `write` branch) unchanged.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p vox-cli-ci docs_reality_audit::tests::compute_metrics_excludes`

Expected: PASS.

- [ ] **Step 5: Add the drift check to `run_verify`**

In `run_verify`, immediately after the existing `verify_findings_consistency(root, &inv, &findings)?;` line, insert:

```rust
    // Metrics must match what the inputs imply. Without this, a stale
    // metrics.v1.json is a green build — which is how the committed file sat at
    // 2026-05-12 for three months without anyone noticing.
    let expected_metrics = compute_metrics(&inv, &findings);
    if let Value::Object(expected) = &expected_metrics {
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
    }
```

`generated_at` is intentionally not compared — `compute_metrics` never emits it.

- [ ] **Step 6: Run verify against the real contracts**

```bash
cargo run -q -p vox-cli -- ci docs-reality-audit metrics --write
cargo run -q -p vox-cli -- ci docs-reality-audit verify
```

Expected: `verify` prints `docs-reality-audit verify OK (10 claims, 0 findings)` and exits 0.

- [ ] **Step 7: Prove the guard actually catches drift**

```bash
python -c "import json,pathlib; p=pathlib.Path('contracts/reports/docs-reality-audit/metrics.v1.json'); d=json.loads(p.read_text()); d['inventory_claim_count']=999; p.write_text(json.dumps(d,indent=2)+'\n')"
cargo run -q -p vox-cli -- ci docs-reality-audit verify
```

Expected: **non-zero exit**, message naming `inventory_claim_count`. Then restore:

```bash
cargo run -q -p vox-cli -- ci docs-reality-audit metrics --write
git diff --stat contracts/reports/docs-reality-audit/metrics.v1.json
```

- [ ] **Step 8: Commit**

```bash
git add crates/vox-cli-ci/src/docs_reality_audit.rs contracts/reports/docs-reality-audit/metrics.v1.json
git commit -m "fix(ci): docs-reality-audit verify now catches stale metrics"
```

---

### Task 9: Check that findings cite paths that exist

Inventory claims get hard path checks — `doc_path` must be a file, every contract must exist, every glob must match at least one path. Findings get none: `doc_paths`, `code_paths`, and `contract_paths` are not even fields on `FindingRow`, so serde discards them. A finding may cite files that do not exist and CI stays green. That is the exact drift class the program exists to catch, going undetected inside the program itself.

**Files:**
- Modify: `crates/vox-cli-ci/src/docs_reality_audit.rs` — add the three optional fields to `FindingRow`, check them in `verify_findings_consistency`
- Test: same file's `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `FindingRow` (extended here).
- Produces: `FindingRow` gains `doc_paths: Option<Vec<String>>`, `code_paths: Option<Vec<String>>`, `contract_paths: Option<Vec<String>>`. Task 7's `test_finding` helper must be updated to set all three to `None`.

- [ ] **Step 1: Extend `FindingRow` and update the test helper**

In `crates/vox-cli-ci/src/docs_reality_audit.rs`, add three fields to `FindingRow` (after `claim_ids`):

```rust
    doc_paths: Option<Vec<String>>,
    code_paths: Option<Vec<String>>,
    contract_paths: Option<Vec<String>>,
```

Then update the `test_finding` helper added in Task 7 to include them:

```rust
            doc_paths: None,
            code_paths: None,
            contract_paths: None,
```

- [ ] **Step 2: Write the failing test**

Add to `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn findings_citing_missing_paths_are_rejected() {
        let root = repo_root_for_tests();
        let inv = InventoryFile {
            schema_version: 1,
            claims: vec![],
        };
        let mut f = test_finding("f.ghost", "new");
        f.claim_ids = vec![];
        f.doc_paths = Some(vec!["docs/src/this-file-does-not-exist.md".to_string()]);
        let findings = FindingsFile {
            schema_version: 1,
            findings: vec![f],
        };

        let err = verify_findings_consistency(&root, &inv, &findings)
            .expect_err("a finding citing a nonexistent doc_path must fail verification");
        let msg = err.to_string();
        assert!(
            msg.contains("this-file-does-not-exist.md"),
            "error must name the missing path, got: {msg}"
        );
    }
```

Add this helper alongside it (the module has no repo-root helper yet):

```rust
    fn repo_root_for_tests() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .canonicalize()
            .expect("canonicalize repo root")
    }
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p vox-cli-ci docs_reality_audit::tests::findings_citing_missing_paths -- --nocapture`

Expected: FAIL — `verify_findings_consistency` returns `Ok`, so `expect_err` panics.

- [ ] **Step 4: Add the path checks**

In `verify_findings_consistency`, rename the unused `_root` parameter to `root` (update the call site in `run_verify` if it passes positionally — it already passes `root`). Then insert inside the `for f in &findings.findings` loop, after the `claim_ids` check:

```rust
        for (label, maybe_paths) in [
            ("doc_paths", &f.doc_paths),
            ("code_paths", &f.code_paths),
            ("contract_paths", &f.contract_paths),
        ] {
            let Some(paths) = maybe_paths else { continue };
            for p in paths {
                if !root.join(p).exists() {
                    anyhow::bail!(
                        "finding {}: {} entry {:?} does not exist on disk",
                        f.id,
                        label,
                        p
                    );
                }
            }
        }
```

`exists()` rather than `is_file()` because `code_paths` legitimately names directories.

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p vox-cli-ci docs_reality_audit::tests::findings_citing_missing_paths`

Expected: PASS.

- [ ] **Step 6: Confirm the real contracts still verify**

Run: `cargo run -q -p vox-cli -- ci docs-reality-audit verify`

Expected: exit 0. (The findings list is empty, so nothing is checked yet — but this confirms the signature change did not break the caller.)

- [ ] **Step 7: Commit**

```bash
git add crates/vox-cli-ci/src/docs_reality_audit.rs
git commit -m "fix(ci): findings citing nonexistent paths now fail verification"
```

---

### Task 10: Reject duplicate finding ids

Claim ids are deduplicated in `verify_inventory_paths`. Finding ids are not, and the schema has no `uniqueItems`. Any generator or hand-edit that emits a finding twice double-counts it into every `counts_by_*` bucket in the metrics.

**Files:**
- Modify: `crates/vox-cli-ci/src/docs_reality_audit.rs` (`verify_findings_consistency`)
- Test: same file's `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `FindingsFile`, `InventoryFile`, `test_finding` and `repo_root_for_tests` from Tasks 7 and 9.
- Produces: nothing new.

- [ ] **Step 1: Write the failing test**

Add to `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn duplicate_finding_ids_are_rejected() {
        let root = repo_root_for_tests();
        let inv = InventoryFile {
            schema_version: 1,
            claims: vec![],
        };
        let mut a = test_finding("f.dup", "new");
        let mut b = test_finding("f.dup", "triaged");
        a.claim_ids = vec![];
        b.claim_ids = vec![];
        let findings = FindingsFile {
            schema_version: 1,
            findings: vec![a, b],
        };

        let err = verify_findings_consistency(&root, &inv, &findings)
            .expect_err("duplicate finding ids must fail verification");
        assert!(
            err.to_string().contains("f.dup"),
            "error must name the duplicated id, got: {err}"
        );
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p vox-cli-ci docs_reality_audit::tests::duplicate_finding_ids -- --nocapture`

Expected: FAIL — verification returns `Ok`.

- [ ] **Step 3: Add the duplicate check**

At the top of `verify_findings_consistency`, before the `for f in &findings.findings` loop:

```rust
    let mut seen_ids: HashSet<&str> = HashSet::new();
    for f in &findings.findings {
        if !seen_ids.insert(f.id.as_str()) {
            anyhow::bail!(
                "duplicate finding id {:?} — ids must be unique or every counts_by_* \
                 bucket double-counts it",
                f.id
            );
        }
    }
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p vox-cli-ci docs_reality_audit::tests::duplicate_finding_ids`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli-ci/src/docs_reality_audit.rs
git commit -m "fix(ci): reject duplicate finding ids in docs-reality-audit"
```

---

### Task 11: Pin the band thresholds to the program YAML

`priority_band_from_score` hardcodes 22 and 14. `contracts/documentation/docs-reality-audit.program.v1.yaml` declares the same thresholds as `min_score` values. `verify` reads that YAML only to confirm it exists and is valid UTF-8 — the comment even says "YAML parse smoke", but no parse happens. Editing the YAML's bands silently desynchronises the contract from the enforcer.

The fix is a test, not a runtime parse: adding a YAML dependency to hardcode-check two integers would be a worse trade than a string assertion that fails loudly on drift.

**Files:**
- Modify: `crates/vox-cli-ci/src/docs_reality_audit.rs` (`#[cfg(test)] mod tests` only)

**Interfaces:**
- Consumes: `repo_root_for_tests` from Task 9, `priority_band_from_score` (this file).
- Produces: nothing.

- [ ] **Step 1: Confirm the YAML's current threshold spelling**

Run: `grep -n 'min_score' contracts/documentation/docs-reality-audit.program.v1.yaml`

Expected: two lines giving 22 and 14 (plus a 0 for P2). Note the exact indentation and spacing — the assertion below matches on substring, so it tolerates indentation but not a reformat to `min_score:22`.

- [ ] **Step 2: Write the failing test**

Add to `#[cfg(test)] mod tests`:

```rust
    /// The band thresholds live in two places: this module's
    /// `priority_band_from_score` and the program YAML's `min_score` fields.
    /// `run_verify` reads that YAML but never parses it, so nothing else keeps
    /// them in sync.
    #[test]
    fn band_thresholds_match_program_yaml() {
        let yaml = std::fs::read_to_string(
            repo_root_for_tests().join("contracts/documentation/docs-reality-audit.program.v1.yaml"),
        )
        .expect("read program YAML");

        assert!(
            yaml.contains("min_score: 22"),
            "program YAML must declare the P0 threshold as 22 to match \
             priority_band_from_score"
        );
        assert!(
            yaml.contains("min_score: 14"),
            "program YAML must declare the P1 threshold as 14 to match \
             priority_band_from_score"
        );

        // And the function must agree at the boundaries.
        assert_eq!(priority_band_from_score(22), "P0");
        assert_eq!(priority_band_from_score(21), "P1");
        assert_eq!(priority_band_from_score(14), "P1");
        assert_eq!(priority_band_from_score(13), "P2");
    }
```

- [ ] **Step 3: Run the test**

Run: `cargo test -p vox-cli-ci docs_reality_audit::tests::band_thresholds_match -- --nocapture`

Expected: **PASS** immediately — the two sides agree today. This test is a tripwire, not a bug fix; it fails the moment someone edits either side alone. If it fails now, the two have already drifted and that is a real finding to fix before continuing.

- [ ] **Step 4: Prove the tripwire works**

Temporarily change `22` to `23` in `priority_band_from_score`, re-run the test, and confirm it FAILS on the `priority_band_from_score(22)` assertion. Revert the change.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli-ci/src/docs_reality_audit.rs
git commit -m "test(ci): pin docs-reality-audit band thresholds to the program YAML"
```

---

### Task 12: Short-circuit glob matching in claim verification

`glob_match_count` collects **every** match into a `Vec` and returns the length, but the only caller asks whether the count is greater than zero. The seed inventory already contains a `crates/**` pattern, which walks 5,206 files. This runs inside `ssot-drift`, which runs in the **fast** pre-push tier with a 60-second budget, on every push. The waste scales linearly with claim count, so it gets worse exactly when the program starts being used.

**Files:**
- Modify: `crates/vox-cli-ci/src/docs_reality_audit.rs:104-112` (`glob_match_count`) and its call sites in `verify_paths_for_claim`
- Test: same file's `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `fn glob_has_match(root: &Path, pattern: &str) -> Result<bool>` replaces `glob_match_count`. No other module calls it (the function is private).

- [ ] **Step 1: Write the failing test**

Add to `#[cfg(test)] mod tests`:

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

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p vox-cli-ci docs_reality_audit::tests::glob_has_match -- --nocapture`

Expected: FAIL to compile — `cannot find function 'glob_has_match' in this scope`.

- [ ] **Step 3: Replace the function**

Replace `glob_match_count` at `crates/vox-cli-ci/src/docs_reality_audit.rs:104`:

```rust
/// Whether a glob pattern matches at least one path.
///
/// Short-circuits at the first match. The previous implementation materialised
/// every match into a Vec purely to test the count against zero — with a
/// `crates/**` pattern in the inventory that is ~5,200 stat calls per claim,
/// inside `ssot-drift`'s 60-second fast pre-push budget.
fn glob_has_match(root: &Path, pattern: &str) -> Result<bool> {
    let full = root.join(pattern);
    let pat = full.to_string_lossy().to_string();
    let mut entries =
        glob(&pat).with_context(|| format!("invalid glob pattern {pat:?}"))?;
    match entries.next() {
        None => Ok(false),
        Some(Ok(_)) => Ok(true),
        Some(Err(e)) => Err(anyhow!("glob iteration failed for {pat:?}: {e}")),
    }
}
```

- [ ] **Step 4: Update the call sites in `verify_paths_for_claim`**

Both glob checks (the `code_globs` loop and the `tests_globs` loop) currently compare a count against zero. Change each to use the boolean directly — for example:

```rust
            if !glob_has_match(root, pattern)? {
                anyhow::bail!(
                    "claim {}: code_globs pattern {:?} matched no paths",
                    claim.id,
                    pattern
                );
            }
```

Apply the same shape to the `tests_globs` loop, keeping that loop's existing error wording so the message stays accurate about which field failed.

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p vox-cli-ci docs_reality_audit::tests::glob_has_match`

Expected: PASS.

- [ ] **Step 6: Confirm verification still passes and got faster**

```bash
cargo run -q -p vox-cli -- ci docs-reality-audit verify
```

Expected: `docs-reality-audit verify OK (10 claims, 0 findings)`, exit 0.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-cli-ci/src/docs_reality_audit.rs
git commit -m "perf(ci): short-circuit glob matching in docs-reality-audit verify"
```

---

### Task 13: Full-gate verification and push

**Files:** none modified — this task only runs gates.

**Interfaces:**
- Consumes: every preceding task.
- Produces: a branch ready for one CodeRabbit review.

- [ ] **Step 1: Format**

Run: `vox run scripts/fmt.vox`

Never `cargo fmt --all` — it dies with `os error 206` on Windows.

- [ ] **Step 2: Run the two touched crates' full test suites**

```bash
cargo test -p vox-doc-pipeline
cargo test -p vox-cli-ci
```

Expected: all PASS, including the five new guards in `policy_docs_guard.rs` and the eight new unit tests in `docs_reality_audit.rs`.

- [ ] **Step 3: Clippy on the touched crates**

```bash
cargo clippy -p vox-doc-pipeline --all-targets -- -D warnings
cargo clippy -p vox-cli-ci --all-targets -- -D warnings
cargo clippy -p vox-actor-runtime --all-targets -- -D warnings
```

Expected: clean. The default fast pre-push tier does **not** run clippy, which is why this is explicit.

- [ ] **Step 4: Run the docs and contract gates**

```bash
cargo run -p vox-doc-pipeline -- --lint-only
cargo run -q -p vox-cli -- ci check-links
cargo run -q -p vox-cli -- ci docs-reality-audit verify
cargo run -q -p vox-cli -- ci retired-symbol-check
cargo run -q -p vox-arch-check
```

Expected: all exit 0. If `retired-symbol-check` now fails on `AGENTS.md`, that is a real interaction with Task 6 — the new rows name retired crates in the left column, which the detector's policy-root table-row skip should handle. If it does not, note the failure and hand it to the W3 plan rather than adding a carve-out here.

- [ ] **Step 5: Run the complete pre-push tier**

Run: `vox ci pre-push --complete`

Expected: green. Do not use the default fast tier — it omits clippy and all tests, and scopes the doc lint to changed paths only.

- [ ] **Step 6: Confirm no line-ending drift**

```bash
git diff --stat
cargo run -q -p vox-cli -- ci line-endings
```

Expected: exit 0. `md`, `rs`, and `json` are all LF.

- [ ] **Step 7: Push once, as a single review-ready branch**

```bash
git push -u origin HEAD
```

CodeRabbit reviews once on open. Do not push incrementally to trigger re-review; comment `@coderabbitai review` instead.

---

## Self-Review

**1. Spec coverage.** Every W1 and W6 item maps to a task:

| Spec item | Task |
| --- | --- |
| W1.1 AGENTS.md decorator contradiction | 1 |
| W1.2 three missing retired crates | 6 |
| W1.3 governance category vocabulary | 2 |
| W1.4 astro.config.mjs comment | 5 |
| W1.5 infer_with_retry doc comment | 5 |
| W1.6 audit-program cadence claim | 5 |
| W1.7 ADR 037 collision | 3 |
| W1.8 NUL byte | 4 |
| W6.1 rollout_milestone_pct | 7 |
| W6.2 metrics recompute | 8 |
| W6.3 findings path existence | 9 |
| W6.4 duplicate finding ids | 10 |
| W6.5 band threshold parity | 11 |
| W6.6 glob early exit | 12 |

No gaps. W2, W3, W4, W5 are out of scope by design and are named in Global Constraints.

**2. Placeholder scan.** `NNN` and `MMM` in Task 3 are the one intentional substitution, and Step 1 of that task tells the implementer exactly how to derive them. No TBDs, no "add appropriate error handling", no "similar to Task N" — Tasks 9, 10, and 11 each restate the helpers they need rather than cross-referencing.

**3. Type consistency.** `test_finding` is introduced in Task 7 and extended in Task 9 with the three new `Option<Vec<String>>` fields; Task 9 says so explicitly, and Tasks 10 and 11 consume the extended version. `repo_root_for_tests` is introduced in Task 9 and reused by Tasks 10, 11, and 12. `repo_root`/`read_repo_file` are introduced in Task 1 and reused by Tasks 2, 3, 4, and 6 — a different file (`policy_docs_guard.rs`) from `repo_root_for_tests` (`docs_reality_audit.rs`), which is why both exist. `glob_match_count` is fully replaced by `glob_has_match` in Task 12 with both call sites updated in the same task.

**Ordering note:** Tasks 7 → 9 → 10 must run in order (each extends the previous task's test helper). Tasks 1–6 are independent of each other and of 7–12, except that Task 6's guard reuses Task 1's helpers, so Task 1 precedes Task 6.
