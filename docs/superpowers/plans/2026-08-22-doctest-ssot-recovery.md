# Doctest SSOT Recovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Vox compiler the automatic single source of truth for syntax in documentation, by fixing the doctest gate so fences can stop being skipped.

**Architecture:** The mechanism already exists — `vox-doc-pipeline` compiles ` ```vox ` fences, so a doc whose fence compiles cannot contain stale syntax. It is switched off for 210 of 276 live fences. Three defects in `doctest.rs` cause part of that and mask the rest: all fences in a file compile as one unit, `{{#include` silently counts as a skip token, and diagnostics report line 1 so failures are unreadable. Fix the gate first, convert the include-fences second, then drain the skip backlog as an ongoing per-PR obligation rather than a batch.

**Tech Stack:** Rust (`vox-doc-pipeline`), Markdown.

**Spec:** `docs/superpowers/specs/2026-08-22-docs-corpus-repair-design.md` (revision 3), workstream W8 — with W8's recovery estimate corrected downward by the measurement in this plan's Task 1.

## Global Constraints

- **This plan cannot be verified without the build lock.** Every task's acceptance check is `cargo run -p vox-doc-pipeline -- --lint-only`. At time of writing, other Claude sessions on this machine hold the cargo lock and every attempt stalls. **Do not land Task 2 or Task 3 without running that command and pasting its real output into the step.** The whole point of this plan is a gate that tells the truth; shipping it unverified would be self-defeating.
- **Expected recovery is ~12–17%, not "most fences".** Measured across all 210 skipped fences: 127 have a top-level declaration, but only ~31 would compile standalone, and only **5 files** show the true concat signature. An earlier draft of spec W8 implied the concat bug was the dominant cause of skipping. It is not. Do not size this work as if it were.
- **Do not batch-enable the skip-reason lint.** 165 of 189 markers are bare and no generator can write their justifications.
- **Line endings LF.** Verification tier `--full`, not `--complete`.
- **One agent per worktree.**

---

## File Structure

| File | Responsibility | Task |
| --- | --- | --- |
| `crates/vox-doc-pipeline/src/pipeline/doctest.rs` | Whole-file replacement: per-fence compilation, real line numbers, dead token removed, debug artifact deleted | 1 |
| `docs/src/explanation/expl-architecture.md`, `tutorials/tut-first-app.md`, `tutorials/tut-getting-started.md` | Inline the type declarations that per-fence compilation newly requires | 2 |
| 21 files carrying ` ```vox ` fences whose body is `{{#include …}}` | Convert to inline fences (AGENTS.md §Markdown Hygiene already requires this) | 3 |
| `crates/vox-doc-pipeline/src/pipeline/doctest.rs` | Skip-reason lint, scoped to changed paths only | 4 |

---

### Task 1: Fix the doctest gate

Three defects, one file, 66 lines. Confirmed by reading:

1. **`current_block` is declared once outside the loop and never cleared** (`:8`), and `check_file` is called once after the loop (`:45-50`). Every non-skipped `vox` fence in a file becomes one compile unit.
2. **`{{#include` and `Skip-Test` are silent skip tokens** (`:32-35`) alongside `vox:skip`. 21 live fences are uncompiled purely because they contain an include directive; `Skip-Test` has **zero** uses and is dead.
3. **A debug artifact writes `scratch_extract.vox` into the process CWD** whenever the path contains `expl-rosetta-inventory` (`:46-48`).

Plus: every `LintError` reports `line: 1`, and the per-diagnostic line is an offset into the concatenated buffer, so failures are unlocatable.

**Files:**
- Modify: `crates/vox-doc-pipeline/src/pipeline/doctest.rs` (whole-file replacement)

**Interfaces:**
- Consumes: `check_file` from `vox_compiler::pipeline`; `LintError`/`LintKind` from `crate::pipeline::types`. Unchanged.
- Produces: `check_doctests(path: &Path, content: &str, errors: &mut Vec<LintError>)` — same signature, so the caller at `pipeline/lint.rs:196-203` needs no edit. Behaviour changes: one `LintError` per failing fence instead of one per file, and `line` is the fence's opening line.

- [ ] **Step 1: Confirm the file still matches before replacing it**

```bash
wc -l crates/vox-doc-pipeline/src/pipeline/doctest.rs
grep -n 'current_block\|Skip-Test\|scratch_extract\|line: 1' crates/vox-doc-pipeline/src/pipeline/doctest.rs
```

Expected: ~66 lines; `current_block` declared once near line 8; the three-token skip condition around 32-35; `scratch_extract` at 46-48. If the shape differs, re-read the whole file and adapt the replacement rather than pasting it blind.

- [ ] **Step 2: Record the baseline, so you can prove the change did something**

```bash
cargo run -p vox-doc-pipeline -- --lint-only 2>&1 | tee /tmp/doctest-before.txt | tail -20
grep -c 'DocTest error' /tmp/doctest-before.txt
```

Paste the real count into your report. **If this command will not run because of build-lock contention, stop here** — the rest of this task cannot be verified, and an unverified doctest gate is worse than the current one.

- [ ] **Step 3: Replace the file**

```rust
use std::path::Path;

use vox_compiler::pipeline::check_file;

use crate::pipeline::types::{LintError, LintKind};

/// Compile each ```vox fence as its own unit.
///
/// Previously every fence in a file was concatenated into one compile unit,
/// which made a symbol declared in two fences a duplicate-symbol error and
/// pushed authors toward `// vox:skip` to escape collisions they had not
/// caused. Per-fence compilation is what lets the skip backlog drain.
pub fn check_doctests(path: &Path, content: &str, errors: &mut Vec<LintError>) {
    let path_str = path.to_string_lossy();
    let mut in_fence = false;
    let mut is_vox = false;
    let mut block = String::new();
    let mut fence_line = 0_usize;

    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            if in_fence {
                in_fence = false;
                if is_vox {
                    check_one(&block, &path_str, path, fence_line, errors);
                }
                block.clear();
            } else {
                in_fence = true;
                let count = trimmed.chars().take_while(|&c| c == '`').count();
                // Only ```vox is Vox source. Other fences (tsx, rust, ...) are
                // illustrative; parsing them as Vox produces noise.
                is_vox = trimmed[count..].trim() == "vox";
                fence_line = idx + 1; // 1-based line of the opening fence
                block.clear();
            }
        } else if in_fence && is_vox {
            block.push_str(line);
            block.push('\n');
        }
    }
}

/// A fence is skipped if it carries an explicit marker or an unresolved
/// include. `Skip-Test` was a third accepted token with zero uses in the
/// corpus and is deliberately not carried over.
fn is_skipped(block: &str) -> bool {
    block
        .lines()
        .any(|l| l.contains("vox:skip") || l.contains("{{#include"))
}

fn check_one(
    block: &str,
    path_str: &str,
    path: &Path,
    fence_line: usize,
    errors: &mut Vec<LintError>,
) {
    if block.trim().is_empty() || is_skipped(block) {
        return;
    }
    let diagnostics = check_file(block, path_str);
    if diagnostics.is_empty() {
        return;
    }
    let mut err_msg = format!("DocTest error in {}:{}\n", path.display(), fence_line);
    for diag in diagnostics {
        // Offset the in-block line back onto the markdown file.
        err_msg.push_str(&format!(
            "  - [{:?}] {} at line {}\n",
            diag.severity,
            diag.message,
            diag.span.start_line + fence_line
        ));
    }
    errors.push(LintError {
        file: path.to_owned(),
        line: fence_line,
        kind: LintKind::DocTestFailed { msg: err_msg },
    });
}
```

Note `Skip-Test` is dropped (zero uses) but `{{#include` is **retained** — Task 3 removes the need for it, and dropping both at once would fail 21 fences in this task instead of that one.

- [ ] **Step 4: Run the gate and diff against the baseline**

```bash
cargo run -p vox-doc-pipeline -- --lint-only 2>&1 | tee /tmp/doctest-after.txt | tail -40
grep -c 'DocTest error' /tmp/doctest-after.txt
diff <(grep -o 'DocTest error in [^:]*' /tmp/doctest-before.txt | sort -u) \
     <(grep -o 'DocTest error in [^:]*' /tmp/doctest-after.txt  | sort -u)
```

Paste the real diff. Newly-failing files are Task 2's work-list — the prediction is 3 files, but **the prediction came from a regex heuristic and the real list is whatever this command prints.** Trust the command.

- [ ] **Step 5: Confirm the debug artifact is gone**

```bash
git status --porcelain | grep scratch_extract || echo "no scratch_extract.vox produced"
```

- [ ] **Step 6: Commit — with Task 2's repairs, not before**

Task 1 and Task 2 land together. A commit where the gate is fixed but the newly-failing fences are not repaired leaves the tree red.

---

### Task 2: Repair the fences that per-fence compilation newly breaks

Under concatenation, a fence could rely on a type declared in an earlier fence of the same file. Per-fence compilation ends that. A static scan predicts 5 fences across 3 files:

| File | Fences | Missing symbol |
| --- | --- | --- |
| `docs/src/explanation/expl-architecture.md` | #2 @246, #3 @272, #4 @296 | `Task` |
| `docs/src/tutorials/tut-first-app.md` | #2 @40 | `Task` |
| `docs/src/tutorials/tut-getting-started.md` | #2 @67 | `Note` |

**Files:**
- Modify: whichever files Task 1 Step 4's diff actually names.

**Interfaces:** none.

- [ ] **Step 1: Use the real list, not the predicted one**

The table above is a regex heuristic over 15 multi-fence files. Task 1 Step 4 printed the truth. Work from that.

- [ ] **Step 2: Inline the declaration into each dependent fence**

For each newly-failing fence, add the type declaration it needs, inline. It is typically four lines:

```vox
table Task {
    title: str
    done: bool
}
```

**Inline the declaration; do not add `// vox:skip`.** Skipping is what this plan exists to reverse — repairing a fence by silencing it converts a recovery into a regression. If a fence genuinely cannot stand alone (it demonstrates a cross-file import, say), then a skip *with a written reason* is correct, and the reason must say which symbol is out of file.

- [ ] **Step 3: Re-run the gate to zero**

```bash
cargo run -p vox-doc-pipeline -- --lint-only 2>&1 | grep -c 'DocTest error'
```

Expected: the same count as the Task 1 Step 2 baseline, or lower. Higher means a repair is incomplete.

- [ ] **Step 4: Commit Tasks 1 and 2 together**

```bash
git add crates/vox-doc-pipeline/src/pipeline/doctest.rs docs/src
git commit -m "fix(docs): compile vox fences individually, with real line numbers"
```

---

### Task 3: Convert the 21 include-directive fences to inline

21 live ` ```vox ` fences contain `{{#include ../../../examples/golden/*.vox:anchor}}`. They are silently uncompiled, and they violate AGENTS.md §Markdown Hygiene, which already says: *"Always write inline ```vox``` blocks, do NOT use mdBook `{{#include}}` directives for new code."*

Converting them recovers **more compiled fences than Task 1 does** — the largest single win in this plan.

**Files:**
- Modify: the 21 files (enumerate them in Step 1).
- Modify: `crates/vox-doc-pipeline/src/pipeline/doctest.rs` (drop `{{#include` from `is_skipped`, once the count reaches zero).

**Interfaces:**
- Consumes: Task 1's per-fence gate.

- [ ] **Step 1: Enumerate them**

```bash
grep -rln '{{#include' docs/src --include='*.md' | grep -v '/archive/'
grep -rc '{{#include' docs/src --include='*.md' | grep -v '/archive/' | grep -v ':0'
```

- [ ] **Step 2: Convert one file first and prove the round-trip**

Pick the smallest. Replace the `{{#include path:anchor}}` line with the actual text from that anchor in the target `.vox` file, then run the gate on that path alone:

```bash
cargo run -p vox-doc-pipeline -- --lint-only --paths <that-file>.md
```

Expected: clean. If the inlined snippet does not compile, the *source* golden file's anchor is stale — that is a finding worth reporting, not something to paper over with a skip.

- [ ] **Step 3: Convert the rest, checking each**

Same pattern. The content is mechanical (copy the anchored region), but each conversion must be gate-checked, because inlining is exactly what exposes whether the snippet was ever valid.

- [ ] **Step 4: Drop the `{{#include` token from `is_skipped`**

Once `grep -rc '{{#include' docs/src` returns zero outside archive, remove that clause so the escape hatch cannot silently return.

- [ ] **Step 5: Full gate and commit**

```bash
cargo run -p vox-doc-pipeline -- --lint-only
git add docs/src crates/vox-doc-pipeline/src/pipeline/doctest.rs
git commit -m "fix(docs): inline 21 include-directive vox fences so the doctest gate sees them"
```

---

### Task 4: Enforce a reason on `// vox:skip` — scoped, not global

AGENTS.md §Markdown Hygiene and `pipeline/mod.rs:58` both require a reason after the marker. Measured: **165 of 189 markers are bare (87%).**

**Files:**
- Modify: `crates/vox-doc-pipeline/src/pipeline/doctest.rs`

**Interfaces:**
- Consumes: `check_one` from Task 1.

- [ ] **Step 1: Add the check inside `check_one`, before the skip early-return**

```rust
    if let Some((n, l)) = block.lines().enumerate().find(|(_, l)| l.contains("vox:skip")) {
        let reason = l.split("vox:skip").nth(1).unwrap_or("");
        if reason.trim_matches(|c: char| !c.is_alphanumeric()).len() < 8 {
            errors.push(LintError {
                file: path.to_owned(),
                line: fence_line + n + 1,
                kind: LintKind::DocTestFailed {
                    msg: format!(
                        "`// vox:skip` at {}:{} has no reason. Say which out-of-file \
                         symbol the snippet needs, or delete the marker and let it compile.",
                        path.display(),
                        fence_line + n + 1
                    ),
                },
            });
        }
    }
```

Reusing `DocTestFailed` avoids touching `types.rs` and `mod.rs`. Add a `SkipWithoutReason` variant only if the summary needs them grouped separately.

- [ ] **Step 2: Confirm it fires on the real corpus, and how loudly**

```bash
cargo run -p vox-doc-pipeline -- --lint-only 2>&1 | grep -c 'has no reason'
```

Expected: ~165. **This is why it must not be global.**

- [ ] **Step 3: Scope it to changed paths**

The pre-push doc lint already runs `--paths <changed>`. Confirm the scoped invocation in `crates/vox-cli/src/commands/ci/pre_push.rs` and ensure this check only fires there, not in the full-corpus run — otherwise CI is red on 165 pre-existing markers and the gate gets disabled within a day.

Read the real invocation before deciding the mechanism; if the pipeline has no per-check scoping today, the honest options are (a) gate the check behind a flag the scoped run passes, or (b) defer this task until the backlog is drained. Pick one and record which.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-doc-pipeline/src/pipeline/doctest.rs crates/vox-cli/src/commands/ci/pre_push.rs
git commit -m "feat(docs): require a reason on vox:skip, scoped to changed paths"
```

---

## Self-Review

**1. Spec coverage.** W8.1 (fences uncompiled) → Tasks 1–3. W8.2 (concat bug) → Task 1, with the recovery estimate corrected. W8.3 (skip reason unenforced) → Task 4. W8.4 (undocumented escape hatches) → Task 1 (`Skip-Test` removed) and Task 3 (`{{#include` removed once unused).

**2. Placeholder scan.** No TBDs. Task 2's file table is explicitly labelled a prediction to be replaced by Task 1 Step 4's real output — that is an instruction, not a deferral. Task 4 Step 3 presents two named options with a requirement to record the choice, because the right one depends on a scoping mechanism that must be read first.

**3. Type consistency.** `check_doctests` keeps its exact signature so `lint.rs:196-203` is untouched. `check_one` and `is_skipped` are introduced in Task 1 and extended in Tasks 3 and 4. `LintKind::DocTestFailed` is reused throughout; no new variant unless Task 4 Step 1's note is taken.

**Ordering:** 1 → 2 in the same commit (the gate and its fallout). 3 after (needs the per-fence gate to validate conversions). 4 last (its count is only meaningful once 3 has removed the include-fences).
