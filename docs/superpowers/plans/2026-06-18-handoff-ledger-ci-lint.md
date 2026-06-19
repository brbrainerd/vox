---
title: "Handoff-Ledger CI Lint — `vox ci handoff-ledger`"
description: "Antigravity/Gemini-3.5-Flash-executable TDD plan for a dependency-free `vox ci handoff-ledger` gate that validates every entry in docs/superpowers/antigravity-handoff-ledger.md against the fixed schema (required keys, id format, outcome/verdict/category vocab) and computes the §A loop metrics. Makes the prompt-engineering CI/CD ledger machine-checked and mineable. Mirrors the existing commit-lint gate."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
---

# Handoff-Ledger CI Lint Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans. Steps use `- [ ]`.

**Goal:** A `vox ci handoff-ledger` gate that validates the append-only Antigravity handoff ledger against its documented schema, so loop entries stay structured and mineable.

**Architecture:** A dependency-free line-based validator in `vox-cli-ci` (no YAML lib — the ledger blocks are semi-structured; we check key presence + enum values by line), wired into the `vox ci` subcommand surface exactly like the existing `commit-lint` gate. Validates `docs/superpowers/antigravity-handoff-ledger.md`.

**Tech Stack:** Rust; `anyhow`; std only (no serde_yaml). Mirrors `crates/vox-cli-ci/src/commit_lint.rs`.

**Execution target:** Gemini 3.5 Flash in Antigravity.

## Operating rules (every task)
1. Atomic + green + committed. 2. Verify-before-use. 3. Self-contained. 4. Two-strike circuit breaker. 5. Tag `[PARALLEL-SAFE]`/`[SEQUENTIAL]`. 6. **No unplanned shared-config edits** (AGH-0001 §B-2). 7. **Branch isolation + delivery manifest** (AGH-0001 §B-3/4) — work on a fresh branch off `origin/main`. 8. House rules: no `cargo fmt --all`; no-stub via `rg -n 'todo!|unimplemented!|TODO|FIXME' <changed>`; no `vox stub-check`.

Per-task ritual: `cargo test -p vox-cli-ci` → `cargo clippy -p vox-cli-ci -- -D warnings` → stub `rg` → `cargo fmt -p vox-cli-ci`.

## Pre-flight
- [ ] `git switch -c claude/handoff-ledger-lint origin/main` (after `git fetch origin main`).
- [ ] `rg -n 'pub fn run' crates/vox-cli-ci/src/commit_lint.rs` — confirm the gate signature `run(workspace_root: &Path, ...) -> anyhow::Result<Vec<_>>` to mirror.
- [ ] `rg -n 'CommitLint|commit-lint' crates/vox-cli/src/commands/ci/cmd_enums.rs crates/vox-cli/src/commands/ci/run_body.rs` — confirm where to add the new subcommand variant + dispatch arm.
- [ ] `rg -n 'pub mod commit_lint' crates/vox-cli-ci/src/lib.rs` — confirm module-decl location.
- [ ] `test -f docs/superpowers/antigravity-handoff-ledger.md && echo present` — confirm the ledger exists.

---

## Task 1: Block extraction + violation type [SEQUENTIAL]

**Files:**
- Create: `crates/vox-cli-ci/src/handoff_ledger.rs`
- Modify: `crates/vox-cli-ci/src/lib.rs` (add `pub mod handoff_ledger;`)

- [ ] **Step 1: Write the module with block extraction + a failing test.**

`crates/vox-cli-ci/src/handoff_ledger.rs`:
```rust
//! `vox ci handoff-ledger` — validates the append-only Antigravity handoff ledger
//! (`docs/superpowers/antigravity-handoff-ledger.md`) against its documented schema.
//! Dependency-free: the ledger entries are semi-structured YAML-ish blocks; we
//! validate key presence and enum values line-by-line (no YAML dependency).

use std::path::Path;

/// A schema violation in a ledger entry.
#[derive(Debug, PartialEq)]
pub struct LedgerViolation {
    pub entry: String, // AGH-NNNN or "(unidentified block)"
    pub reason: String,
}

/// Default ledger path relative to the workspace root.
pub const LEDGER_PATH: &str = "docs/superpowers/antigravity-handoff-ledger.md";

/// Extract the text of each ```yaml fenced block that contains an `id: AGH-` line.
/// Returns each block's inner text (without the ``` fences).
pub(crate) fn extract_entry_blocks(markdown: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut in_fence = false;
    let mut cur = String::new();
    for line in markdown.lines() {
        let trimmed = line.trim_start();
        if !in_fence && (trimmed == "```yaml" || trimmed == "```yml") {
            in_fence = true;
            cur.clear();
            continue;
        }
        if in_fence && trimmed == "```" {
            in_fence = false;
            if cur.lines().any(|l| l.trim_start().starts_with("id: AGH-")) {
                blocks.push(std::mem::take(&mut cur));
            }
            cur.clear();
            continue;
        }
        if in_fence {
            cur.push_str(line);
            cur.push('\n');
        }
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_only_id_bearing_yaml_blocks() {
        let md = "intro\n```yaml\nid: AGH-0001\noutcome: green\n```\nprose\n```yaml\nkey: not-an-entry\n```\n";
        let blocks = extract_entry_blocks(md);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].contains("AGH-0001"));
    }
}
```

- [ ] **Step 2: Declare the module.** Add `pub mod handoff_ledger;` to `crates/vox-cli-ci/src/lib.rs` (alphabetical position, near `pub mod frozen_crates;`).

- [ ] **Step 3: Run → PASS.** `cargo test -p vox-cli-ci extracts_only_id_bearing`

- [ ] **Step 4: Clippy, fmt, commit.**

```bash
cargo clippy -p vox-cli-ci -- -D warnings
cargo fmt -p vox-cli-ci
git add crates/vox-cli-ci/src/handoff_ledger.rs crates/vox-cli-ci/src/lib.rs
git commit -m "feat(ci): handoff-ledger block extraction"
```

---

## Task 2: Required-keys + id-format validation [SEQUENTIAL]

**Files:**
- Modify: `crates/vox-cli-ci/src/handoff_ledger.rs`

- [ ] **Step 1: Add validation + tests.** Add to `handoff_ledger.rs` (above the test module):

```rust
/// Top-level keys every entry must declare.
const REQUIRED_KEYS: &[&str] = &[
    "id", "date", "plan", "prompt_version", "subsystem", "target", "outcome",
];

/// Return the value of a `key:` line in a block, if present.
fn field<'a>(block: &'a str, key: &str) -> Option<&'a str> {
    block.lines().find_map(|l| {
        let l = l.trim_start();
        l.strip_prefix(key)
            .and_then(|rest| rest.strip_prefix(':'))
            .map(|v| v.trim())
    })
}

/// Validate one entry block; push any violations.
pub(crate) fn validate_block(block: &str, out: &mut Vec<LedgerViolation>) {
    let id = field(block, "id").unwrap_or("(unidentified block)").to_string();

    for key in REQUIRED_KEYS {
        if field(block, key).is_none() {
            out.push(LedgerViolation {
                entry: id.clone(),
                reason: format!("missing required key `{key}`"),
            });
        }
    }

    if let Some(id_val) = field(block, "id") {
        // AGH-NNNN where NNNN is 4 digits
        let ok = id_val.strip_prefix("AGH-").map(|n| n.len() == 4 && n.chars().all(|c| c.is_ascii_digit())).unwrap_or(false);
        if !ok {
            out.push(LedgerViolation {
                entry: id.clone(),
                reason: format!("id `{id_val}` must match AGH-NNNN (4 digits)"),
            });
        }
    }
}

/// Validate the whole ledger file at `workspace_root/LEDGER_PATH`.
pub fn run(workspace_root: &Path) -> anyhow::Result<Vec<LedgerViolation>> {
    let path = workspace_root.join(LEDGER_PATH);
    let md = std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", path.display()))?;
    let mut out = Vec::new();
    let blocks = extract_entry_blocks(&md);
    // ids must be unique
    let mut seen = std::collections::HashSet::new();
    for block in &blocks {
        validate_block(block, &mut out);
        if let Some(id) = field(block, "id") {
            if !seen.insert(id.to_string()) {
                out.push(LedgerViolation { entry: id.to_string(), reason: "duplicate id".into() });
            }
        }
    }
    Ok(out)
}
```

Add to the `tests` module:
```rust
    #[test]
    fn flags_missing_required_key() {
        let mut v = Vec::new();
        validate_block("id: AGH-0001\noutcome: green\n", &mut v);
        assert!(v.iter().any(|x| x.reason.contains("missing required key `plan`")));
    }

    #[test]
    fn flags_bad_id_format() {
        let mut v = Vec::new();
        validate_block("id: AGH-1\noutcome: green\n", &mut v);
        assert!(v.iter().any(|x| x.reason.contains("AGH-NNNN")));
    }
```

- [ ] **Step 2: Run → PASS.** `cargo test -p vox-cli-ci handoff_ledger`
- [ ] **Step 3: Clippy, fmt, commit.**

```bash
cargo clippy -p vox-cli-ci -- -D warnings
cargo fmt -p vox-cli-ci
git add crates/vox-cli-ci/src/handoff_ledger.rs
git commit -m "feat(ci): handoff-ledger required-keys + id-format validation"
```

---

## Task 3: Enum-value validation (outcome / verdict / category) [SEQUENTIAL]

**Files:**
- Modify: `crates/vox-cli-ci/src/handoff_ledger.rs`

- [ ] **Step 1: Add enum checks + tests.** Add to `handoff_ledger.rs` (above the test module):

```rust
const VALID_OUTCOME: &[&str] = &["green", "partial", "failed"];
const VALID_VERDICT: &[&str] = &["approve", "approve-with-followups", "request-changes"];
/// Fixed failure-category vocabulary (must match the ledger header §C `category` vocab).
const VALID_CATEGORY: &[&str] = &[
    "hallucinated-api", "wrong-path", "wrong-crate", "arch-check-gate", "fmt-gate",
    "build-gate", "branch-hygiene", "scope-creep", "already-done", "perf", "robustness",
    "test-hygiene", "unplanned-shared-change", "ssot-fork", "unit-mismatch",
];

/// Validate enum-valued fields in a block.
pub(crate) fn validate_enums(block: &str, out: &mut Vec<LedgerViolation>) {
    let id = field(block, "id").unwrap_or("(unidentified block)").to_string();

    if let Some(o) = field(block, "outcome") {
        if !VALID_OUTCOME.contains(&o) {
            out.push(LedgerViolation { entry: id.clone(), reason: format!("outcome `{o}` not in {VALID_OUTCOME:?}") });
        }
    }
    if let Some(v) = field(block, "verdict") {
        if !VALID_VERDICT.contains(&v) {
            out.push(LedgerViolation { entry: id.clone(), reason: format!("verdict `{v}` not in {VALID_VERDICT:?}") });
        }
    }
    // every `category: X` line (inside errors_encountered list items) must be known
    for line in block.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("category:") {
            let cat = rest.trim().trim_matches('"');
            if !VALID_CATEGORY.contains(&cat) {
                out.push(LedgerViolation { entry: id.clone(), reason: format!("category `{cat}` not in the fixed vocab") });
            }
        }
    }
}
```

Wire `validate_enums` into `run` by calling it alongside `validate_block` in the loop:
```rust
        validate_block(block, &mut out);
        validate_enums(block, &mut out);
```

Add tests:
```rust
    #[test]
    fn flags_bad_outcome() {
        let mut v = Vec::new();
        validate_enums("id: AGH-0001\noutcome: maybe\n", &mut v);
        assert!(v.iter().any(|x| x.reason.contains("outcome `maybe`")));
    }

    #[test]
    fn flags_unknown_category() {
        let mut v = Vec::new();
        validate_enums("id: AGH-0001\n  - { what: x, category: \"made-up-cat\" }\n", &mut v);
        assert!(v.iter().any(|x| x.reason.contains("made-up-cat")));
    }

    #[test]
    fn accepts_known_category_and_outcome() {
        let mut v = Vec::new();
        validate_enums("id: AGH-0001\noutcome: green\n  - { what: x, category: \"perf\" }\n", &mut v);
        assert!(v.is_empty());
    }
```

- [ ] **Step 2: Run → PASS.** `cargo test -p vox-cli-ci handoff_ledger`
- [ ] **Step 3: Clippy, fmt, commit.**

```bash
cargo clippy -p vox-cli-ci -- -D warnings
cargo fmt -p vox-cli-ci
git add crates/vox-cli-ci/src/handoff_ledger.rs
git commit -m "feat(ci): handoff-ledger enum validation (outcome/verdict/category)"
```

---

## Task 4: Wire `vox ci handoff-ledger` subcommand [SEQUENTIAL]

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/cmd_enums.rs`
- Modify: `crates/vox-cli/src/commands/ci/run_body.rs`

- [ ] **Step 1 (verify-before-use):** `rg -n 'CommitLint' crates/vox-cli/src/commands/ci/cmd_enums.rs` and `rg -n 'CiCmd::CommitLint' crates/vox-cli/src/commands/ci/run_body.rs` — read both to mirror the exact enum-variant + match-arm shape.

- [ ] **Step 2: Add the enum variant.** In `cmd_enums.rs`, next to the `CommitLint` variant, add:
```rust
    /// Validate the Antigravity handoff ledger against its schema.
    #[command(name = "handoff-ledger")]
    HandoffLedger,
```

- [ ] **Step 3: Add the dispatch arm.** In `run_body.rs`, next to the `CiCmd::CommitLint { .. } =>` arm, add:
```rust
        CiCmd::HandoffLedger => {
            let violations = vox_cli_ci::handoff_ledger::run(&root)?;
            if !violations.is_empty() {
                for v in &violations {
                    eprintln!("handoff-ledger: [{}] {}", v.entry, v.reason);
                }
                anyhow::bail!("handoff-ledger failed with {} violation(s)", violations.len());
            }
            println!("handoff-ledger passed.");
        }
```
(Confirm `root` is the workspace-root `PathBuf` in scope, as it is for `CommitLint`.)

- [ ] **Step 4: Build.** `cargo build -p vox-cli`
- [ ] **Step 5: Run against the real ledger.** `cargo run -p vox-cli -- ci handoff-ledger`
Expected: `handoff-ledger passed.` (AGH-0001 is schema-valid). If it reports violations, the ledger entry — not the validator — is wrong; fix whichever is actually at fault.

- [ ] **Step 6: Clippy, fmt, commit.**

```bash
cargo clippy -p vox-cli -- -D warnings
cargo fmt -p vox-cli
git add crates/vox-cli/src/commands/ci/cmd_enums.rs crates/vox-cli/src/commands/ci/run_body.rs
git commit -m "feat(ci): wire vox ci handoff-ledger gate"
```

---

## Task 5: Final verification [SEQUENTIAL]

- [ ] **Step 1:** `cargo test -p vox-cli-ci handoff_ledger` — paste counts (≥ 7 tests).
- [ ] **Step 2:** `cargo run -p vox-cli -- ci handoff-ledger` — `handoff-ledger passed.`
- [ ] **Step 3:** `cargo clippy -p vox-cli-ci -p vox-cli -- -D warnings` — clean.
- [ ] **Step 4: Negative check.** Temporarily append a `\n```yaml\nid: AGH-9999\noutcome: bogus\n``` ` block to the ledger, run the gate, confirm it FAILS with the outcome violation, then revert the temporary edit. (Do not commit the temporary edit.)
- [ ] **Step 5: Delivery manifest** (AGH-0001 §B-4): list files changed (handoff_ledger.rs, lib.rs, cmd_enums.rs, run_body.rs).

## Self-Review (author)
- **Coverage:** block extraction (T1), required keys + id format + uniqueness (T2), enum vocab outcome/verdict/category (T3), `vox ci` wiring (T4), end-to-end + negative (T5).
- **Type consistency:** `run(&Path) -> anyhow::Result<Vec<LedgerViolation>>` mirrors `commit_lint::run`; `field`/`validate_block`/`validate_enums`/`extract_entry_blocks` consistent.
- **Dependency-free:** no serde_yaml (research flagged it unmaintained); line-based parse only.
</content>
