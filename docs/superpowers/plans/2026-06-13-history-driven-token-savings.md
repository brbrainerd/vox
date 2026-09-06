# History-Driven Token-Savings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cut the recurring per-session and per-diff token cost revealed by 4 months / 2,520 commits of history — by collapsing generated-file diff noise, eliminating post-merge SSOT regen churn, auto-fixing format churn, and trimming the always-loaded memory index — without adding gates that duplicate what already exists.

**Architecture:** Deterministic git analytics over the full history (zero LLM tokens) located four measurable token-sinks. Each task closes one sink by extending an *existing* surface (`.gitattributes`, `lefthook.yml`, the merge-queue CI path, `AGENTS.md`, the auto-memory index) rather than inventing a new gate. Every code/config task is verified by running the exact command CI or the hook would run.

**Tech Stack:** Git, `.gitattributes` (linguist), lefthook, `vox ci *` subcommands (Rust CLI), GitHub Actions `merge_group`, the Claude auto-memory files under `~/.claude/projects/.../memory/`.

---

## Part 0 — Evolution Map (the "mapping" deliverable)

Built from deterministic analysis of all 2,520 commits (Feb 17 → Jun 13, 2026). No LLM extraction was run: the literal "graphify all diffs" path would have ingested **7,787,668 diff lines** — millions of tokens, defeating the goal. An optional navigable HTML graph of the 2,520 commit *messages* can be produced later in a non-sandbox session (corpus already staged at `graphify-corpus/commit-messages.md`; the semantic pass needs write-capable subagents, which this worktree sandbox denies — see `feedback_subagents_readonly_in_sandbox`).

**Phases of the codebase:**

| Month | Commits | Shape | Signal |
|---|---:|---|---|
| 2026-02 | 11 | Bootstrap (docs/config) | project init |
| 2026-03 | 58 | First features (32 feat / 9 fix) | compiler/parser stand-up |
| 2026-04 | 278 | Build-out begins (82 feat / 58 fix / 51 docs) | fix-rate climbing |
| 2026-05 | 1,419 | **Peak build-out** (562 feat / 270 fix / 212 docs / 78 refac) | the firehose; fix:feat ≈ 0.48 |
| 2026-06 | 754 | **Stabilization** (199 feat / 189 fix) | fix:feat ≈ **0.95** — rework now ~= new work |

**The inflection that matters:** the fix-to-feat ratio doubled from May (0.48) to June (0.95). The codebase has crossed from "build it" into "keep it green," which is exactly the regime where token-saving gates have the highest leverage — most tokens are now spent on rework, regeneration, and CI iteration rather than net-new features.

**Where the churn concentrates (commits touching a path):**
- Generated / lock artifacts dominate the top of the list: `Cargo.lock` (262), `docs/src/SUMMARY.md` (96), `docs/src/feed.xml` (94), `docs/src/architecture/where-things-live.md` (86), `layers.toml` (84), `architecture-index.md`, `research-index.md`.
- CI command plumbing thrashes: `ci/cmd_enums.rs`, `ci/run_body.rs`, `ci/mod.rs`, `commands/mod.rs` (all top-25).
- Bug hotspots (fix-commits by crate): `vox-compiler` (315), `vox-orchestrator` (282), `vox-cli` (281), `vox-integration-tests` (223), `vox-codegen` (221). Within codegen the repeat offenders are the Rust/TS emit files (`codegen_rust/emit/stmt_expr.rs`, `method_emit.rs`, `codegen_ts/hir_emit/mod.rs`).

**The token-cost ledger (the four sinks this plan targets):**

| # | Sink | Evidence | Why it costs tokens |
|---|---|---|---|
| A | Generated-file diff noise | `Cargo.lock` 262 touches; `feed.xml`/`SUMMARY.md`/indexes 60–96 each; none marked `linguist-generated`/`-diff` in `.gitattributes` | Every agent/reviewer that reads a diff containing these pays to read regenerated, zero-signal text |
| B | Post-merge SSOT regen churn | 64 commits mention "regen/regenerate"; 7 are explicit `fix(#N): regenerate … after merge`; lefthook regenerates on *commit* and pre-push checks drift, but nothing regenerates on *merge to main* | A whole agent round-trip (read, regen, commit, push) per merge that drifts a generated artifact |
| C | Format-only follow-up commits | 47 `style`/`rustfmt`/`cargo fmt` commits; lefthook runs fmt-**check** (blocks), not fmt-**fix** | Each is a wasted commit + the agent turn that produced it |
| D | Always-loaded memory bloat | `MEMORY.md` ≈ 60 entries, most are completed/superseded project records ("…MERGED", "…EXECUTED", "…DONE"); loaded into context **every session** | Fixed per-session tax paid on every conversation, forever |

702 of 2,520 commit subjects (28%) mention `ci|clippy|fmt|gate|ssot|drift|regen` — a quarter of all history is process/regeneration churn, not product. Sinks A–D are the addressable slice.

**What already exists (do NOT duplicate):** lefthook pre-commit already runs `fmt-check`, `sync-ignore-files`, `command-sync`, `gui-version-sync`, `plugin-catalog-docs`, and a TOESTUB `tdd-guard` (skeleton, enforce-strict). Pre-push has 7 tiers (`vox ci pre-push [--complete|--full|…]`) covering fmt, line-endings, ssot-drift, scoped doc lint/doctest, clippy, scoped TOESTUB, nextest, budgets. Merge-queue is enabled (ruleset 17640801). `.gitattributes` has thorough EOL/binary rules. The gaps below are what those surfaces do **not** yet cover.

---

## Part 1 — File Structure

| File | Responsibility | Task |
|---|---|---|
| `.gitattributes` (modify) | Mark truly-generated artifacts `linguist-generated` (+ `-diff` for the pure-generated ones) so diffs/reviews collapse them | Task 1 |
| `lefthook.yml` (modify) | Flip `fmt-check` → auto-fix-and-restage so format never produces a follow-up commit | Task 2 |
| `.github/workflows/ci.yml` (modify) | Add an SSOT-regen verify step on the `merge_group` event so post-merge drift is caught in the queue, not hand-fixed on main | Task 3 |
| `crates/vox-cli/src/commands/ci/` + a new `vox ci workflow-lint` subcommand (create) | Static lint for the recurring workflow-invocation mistakes (`vox … -- ci`, stale action majors) | Task 4 (optional / lowest ROI) |
| `AGENTS.md` (modify) | Two short policy notes: generated-file diff discipline; "don't hand-regenerate after merge — the queue does it" | Task 5 |
| Auto-memory: `MEMORY.md` + `memory/*.md` (modify) | Archive completed/superseded project entries; keep active facts + durable feedback | Task 6 |

Tasks are independent and can land in any order / separate PRs. Recommended sequence by ROI: **6 → 1 → 3 → 5 → 2 → 4**. Task 6 is the single biggest token win (per-session, compounding) and touches no repo code.

---

## Task 1: Collapse generated-file diff noise in `.gitattributes`

> **(Historical, 2026-06: superseded 2026-09 — research-index retired.)** **IMPLEMENTATION OUTCOME (this task is a plan; Step 4 is a verify-then-prune step — read it before the Step 2 block below).** Verification (Step 4) found that `SUMMARY.md`, `feed.xml`, and `architecture-index.md` are **`.gitignore`d** (already out of diffs — marking them is a no-op, consistent with Part 0) and that `research-index.md` is **hand-curated** (not generated). So the **shipped** `.gitattributes` block does NOT include those four; the proposed block in Step 2 lists them only so Step 4's verification has something to prune. The `**/gui-surface-*.md` placeholder in Step 2 resolved to the actual generated artifacts **`contracts/reports/gui-surface-{coverage,registry}.v1.json`** (the `.md` of that name is a hand-authored arch doc and is excluded).

**Files:**
- Modify: `.gitattributes` (append a "Generated artifacts" block)

Marking a path `linguist-generated=true` makes GitHub collapse it in PR diffs by default; adding `-diff` makes local `git diff` and `git show` render it as "Binary files differ" (no line-by-line text), so agents reading a diff don't pay for regenerated content. Apply `-diff` only to artifacts no human ever needs to read in a diff; keep `Cargo.lock` diffable (security-relevant) but `linguist-generated` so GitHub collapses it.

- [ ] **Step 1: Capture the current diff-token cost of a representative generated file (baseline)**

Run:
```bash
git show HEAD --stat -- docs/src/feed.xml docs/src/SUMMARY.md docs/src/architecture/architecture-index.md | tail -5
git log --oneline -1 -- docs/src/feed.xml
```
Expected: a recent commit touches one of these; note that `git show <that-commit> -- docs/src/feed.xml` currently prints the full-text diff (the cost we are removing).

- [ ] **Step 2: Append the generated-artifacts block to `.gitattributes`**

Add at the end of `.gitattributes`:
```gitattributes
# --- Generated artifacts (collapse in diffs to save review + agent tokens) ---
# `linguist-generated` collapses on GitHub; `-diff` also collapses local `git diff`/`git show`.
# Regenerated by tools (never hand-edited per AGENTS.md §Auto-generated documentation files):
docs/src/SUMMARY.md                         linguist-generated=true -diff
docs/src/feed.xml                           linguist-generated=true -diff
docs/src/architecture/architecture-index.md linguist-generated=true -diff
docs/src/architecture/research-index.md     linguist-generated=true -diff
docs/src/**/*.generated.md                  linguist-generated=true -diff
**/gui-surface-*.md                         linguist-generated=true -diff
crates/vox-cli/tests/fixtures/command_catalog_paths_baseline.txt linguist-generated=true -diff
# Lockfile: collapse on GitHub but keep diffable locally (supply-chain review).
Cargo.lock                                  linguist-generated=true
```

> NOTE for the implementer: do **not** add `where-things-live.md` or `layers.toml` here — those are hand-authored architecture SSOT (CLAUDE.md instructs adding rows to them by hand), not generated. Only list files confirmed generated by AGENTS.md §Auto-generated documentation files / a `vox ci` generator.

- [ ] **Step 3: Verify the collapse takes effect**

Run:
```bash
git check-attr -a docs/src/feed.xml
git -c core.attributesFile=.gitattributes show HEAD -- docs/src/feed.xml | head -5
```
Expected: `check-attr` reports `diff: unset` and `linguist-generated: true` for `feed.xml`; the `show` renders the change as collapsed/binary rather than full text (for a commit that actually touched it). For `Cargo.lock`, `git check-attr -a Cargo.lock` shows `linguist-generated: true` and no `diff: unset`.

- [ ] **Step 4: Confirm AGENTS.md generated-file list and this block agree**

Run:
```bash
sed -n '41,60p' AGENTS.md
```
Expected: every path you marked `-diff` appears in (or is covered by) AGENTS.md §"Auto-generated documentation files". If any marked file is NOT listed there, remove it from the block (it may be hand-edited) — do not guess.

- [ ] **Step 5: Commit**

```bash
git add .gitattributes
git commit -m "chore(repo): collapse generated artifacts in diffs (linguist-generated + -diff) to cut review/agent token cost"
```

---

## Task 2: Auto-fix formatting in lefthook so format never produces a follow-up commit

**Files:**
- Modify: `lefthook.yml:` (the `fmt-check` command under `pre-commit`)

Today `fmt-check` *blocks* the commit and the developer/agent must run `cargo fmt`, re-stage, and commit again — that is the source of the 47 format-only commits. Flip it to format-and-restage using the existing Windows-safe writer (`scripts/fmt.vox`, which chunks per-crate to dodge the os-error-206 arg-limit; never `cargo fmt --all`). Verify the writer exposes a write mode before wiring it.

- [ ] **Step 1: Confirm the Windows-safe formatter has a write path**

Run:
```bash
sed -n '1,40p' scripts/fmt.vox
cargo run -p vox-cli --quiet -- ci fmt-fix --help 2>&1 | head -20 || echo "no fmt-fix subcommand"
```
Expected: identify the write entrypoint. Two acceptable options — (a) `vox run scripts/fmt.vox` (writes by default; `VOX_FMT_CHECK=1` is the check mode), or (b) a `vox ci fmt-fix --write` subcommand. Use whichever actually exists. If only `scripts/fmt.vox` exists, use it.

- [ ] **Step 2: Replace the `fmt-check` command with a format-and-restage command**

In `lefthook.yml`, under `pre-commit: commands:`, change the `fmt-check` block to:
```yaml
    # Auto-format and re-stage so a format slip never becomes a separate `style:` commit.
    # Windows-safe: scripts/fmt.vox chunks per-crate (cargo fmt --all overflows os error 206).
    fmt-fix:
      run: vox run scripts/fmt.vox
      stage_fixed: true
      glob: "**/*.rs"
      fail_text: "rustfmt could not format (syntax error?). Fix the parse error, then re-commit."
```
(If Step 1 found a `vox ci fmt-fix --write` subcommand, use `run: cargo run -p vox-cli --quiet -- ci fmt-fix --write` instead, keeping `stage_fixed: true`.)

- [ ] **Step 3: Verify it formats and restages instead of blocking**

Run (introduce a deliberate format error in a scratch file, stage, and run the hook):
```bash
printf 'pub fn f( )->i32{1}\n' >> crates/vox-cli/src/lib.rs
git add crates/vox-cli/src/lib.rs
lefthook run pre-commit
git diff --cached crates/vox-cli/src/lib.rs | grep -E 'fn f' 
```
Expected: the hook **passes** (exit 0), the staged content is now rustfmt-clean (`fn f() -> i32 { 1 }`), with no separate manual fmt step. Then revert the scratch edit:
```bash
git restore --staged crates/vox-cli/src/lib.rs && git checkout -- crates/vox-cli/src/lib.rs
```

- [ ] **Step 4: Confirm CI still has an independent fmt *check***

Run:
```bash
grep -rE "fmt-check|fmt_check|fmt-fix" .github/workflows/*.yml
```
Expected: CI retains a `fmt-check` (verification) step — the local hook auto-fixes, but CI must still *verify* (so a contributor without the hook can't merge unformatted code). If CI's fmt step is gone, this task must also re-add a `vox ci fmt-check` gate to the relevant workflow. Do not remove the verification side.

- [ ] **Step 5: Commit**

```bash
git add lefthook.yml
git commit -m "chore(hooks): auto-format and re-stage on commit (was fmt-check/block) to kill format-only follow-up commits"
```

---

## Task 3: Regenerate SSOT artifacts in the merge queue, not by hand after merge

> **IMPLEMENTATION NOTE — this task was PIVOTED during execution.** The design below (a verify-only job named **`ssot-merge-regen`** triggered on **`merge_group`**) was found **redundant**: `vox ci ssot-drift` already runs as a hard gate inside the existing `guards-fast` job on every event *including* `merge_group`, so adding a merge-queue verify job would duplicate it. The historical churn came from queue *bypass* (admin-merges), now rare since the merge queue is enabled. What actually shipped is a **`ssot-autoregen`** job triggered on **`pull_request`** (same-repo guard: `github.event.pull_request.head.repo.full_name == github.repository`) that **auto-regenerates and commits** the drift back to the PR branch (removing the human round-trip) rather than merely failing. References to `ssot-merge-regen` / `merge_group` below (and in Task 5's deferred note) describe the original plan, not the shipped job — read them as `ssot-autoregen` / `pull_request`.

**Files:**
- Modify: `.github/workflows/ci.yml` (add a `merge_group`-triggered job, or a step in the existing merge_group path, that regenerates SSOT and fails if it drifts)

64 "regen" commits — and the explicit `fix(#N): regenerate … after merge` pattern — exist because two PRs that are each individually green produce a drifted generated artifact once both land on `main`. The lefthook (commit-time) and pre-push (push-time) gates cannot see a *cross-PR* merge. The merge queue can: it builds the prospective merged state. Regenerating + verifying there catches the drift before it lands, eliminating the manual post-merge commit.

- [ ] **Step 1: Find the regenerate-everything entrypoint and the merge_group trigger**

Run:
```bash
cargo run -p vox-cli --quiet -- ci --help 2>&1 | grep -iE "regen|sync|ssot|drift|generate" 
grep -nE "merge_group|on:|ssot-drift|regenerat" .github/workflows/ci.yml | head -30
```
Expected: identify (a) the single command (or short list) that regenerates all SSOT artifacts — likely `vox ci ssot-drift` (check mode) plus the generators it covers — and (b) whether `ci.yml` already lists `merge_group:` under `on:`. The existing fast pre-push tier runs `ssot-drift`; reuse that exact command in `--write`/regenerate mode here.

- [ ] **Step 2: Add the merge-queue regen-verify job to `ci.yml`**

Under `on:`, ensure `merge_group:` is present. Add a job (adjust the regen command to what Step 1 found):
```yaml
  ssot-merge-regen:
    # Runs only in the merge queue, on the prospective merged commit — catches
    # cross-PR generated-file drift that per-PR gates structurally cannot see.
    if: github.event_name == 'merge_group'
    runs-on: [self-hosted, linux]   # match repo's local-first runner policy
    steps:
      - uses: actions/checkout@v4
      - name: Regenerate all SSOT artifacts
        run: cargo run -p vox-cli --quiet -- ci ssot-drift --write
      - name: Fail if regeneration changed anything
        run: |
          if ! git diff --quiet; then
            echo "::error::SSOT artifacts drift when these PRs merge together. Regenerate locally and update the PR:"
            git diff --name-only
            exit 1
          fi
```

> If `ssot-drift` has no `--write`, call the concrete generators Step 1 listed (e.g. `command-sync`, `generate-plugin-catalog-docs`, the docs index generator), then the same `git diff --quiet` guard. The guard is what makes drift fail the queue.

- [ ] **Step 3: Validate the workflow parses**

Run:
```bash
cargo run -p vox-cli --quiet -- ci workflow-lint 2>/dev/null || \
  python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/ci.yml')); print('ci.yml parses')"
```
Expected: `ci.yml parses` (or the repo's own workflow validator passes). Optionally dry-run in Docker: `vox ci pre-push --act` to exercise the merge_group path locally.

- [ ] **Step 4: Confirm no duplicate regen already runs in the queue**

Run:
```bash
grep -nE "ssot|regen|merge_group" .github/workflows/*.yml
```
Expected: the new job is the only `merge_group` SSOT-regen step (Part 0 found `ci.yml` and `docs-quality.yml` reference regen — confirm `docs-quality.yml` runs on `pull_request`, not `merge_group`, so there's no double-run). If `docs-quality.yml` already regenerates docs in the queue, fold the gui-surface/command-sync regen into that job instead of adding a new one.

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci(merge-queue): regenerate+verify SSOT on merge_group to kill 'regenerate after merge' churn (refs sink B)"
```

---

## Task 4: (Optional, lowest ROI) `vox ci workflow-lint` for recurring invocation mistakes

**Files:**
- Create: `crates/vox-cli/src/commands/ci/workflow_lint.rs`
- Modify: `crates/vox-cli/src/commands/ci/cmd_enums.rs` (register the subcommand), `crates/vox-cli/src/commands/ci/run_body.rs` (dispatch)
- Test: `crates/vox-cli/tests/workflow_lint_test.rs`

Only ~2 historical instances (`#277`/`#278` `vox … -- ci`, `#287` stale action major), so this is a guard against *recurrence*, not a hot fix. Build only if Tasks 1–3+5 land and there's appetite. TDD.

- [ ] **Step 1: Write the failing test**

`crates/vox-cli/tests/workflow_lint_test.rs`:
```rust
#[test]
fn flags_double_dash_before_ci_subcommand() {
    let bad = "run: vox --quiet -- ci check-links\n";
    let findings = vox_cli::ci::workflow_lint::lint_str(bad, ".github/workflows/x.yml");
    assert_eq!(findings.len(), 1, "should flag the spurious `-- ci`");
    assert!(findings[0].message.contains("-- ci"));
}

#[test]
fn clean_invocation_has_no_findings() {
    let ok = "run: vox ci check-links\n";
    assert!(vox_cli::ci::workflow_lint::lint_str(ok, ".github/workflows/x.yml").is_empty());
}
```

- [ ] **Step 2: Run it to confirm it fails**

Run: `cargo test -p vox-cli --test workflow_lint_test`
Expected: FAIL — `vox_cli::ci::workflow_lint` does not exist.

- [ ] **Step 3: Implement the minimal linter**

`crates/vox-cli/src/commands/ci/workflow_lint.rs`:
```rust
pub struct Finding { pub message: String, pub file: String, pub line: usize }

/// Lint a single workflow file's contents for known-bad vox/CI invocations.
pub fn lint_str(contents: &str, file: &str) -> Vec<Finding> {
    let mut out = Vec::new();
    for (i, line) in contents.lines().enumerate() {
        // Recurrence of #277/#278: `vox [flags] -- ci <sub>` makes clap exit 2.
        if let Some(pos) = line.find("vox") {
            let rest = &line[pos..];
            if rest.contains(" -- ci ") || rest.contains(" -- ci\n") {
                out.push(Finding {
                    message: "spurious `-- ci`: drop the `--` (clap treats `ci` as a positional, exits 2)".into(),
                    file: file.to_string(), line: i + 1,
                });
            }
        }
    }
    out
}
```
Wire `pub mod workflow_lint;` into `crates/vox-cli/src/commands/ci/mod.rs`, add a `WorkflowLint` arm to `cmd_enums.rs`, and dispatch in `run_body.rs` to glob `.github/workflows/*.yml`, call `lint_str`, print findings, and exit non-zero if any. Follow the existing pattern of a sibling `ci` subcommand (e.g. `runner-policy-check`) for the CLI plumbing.

- [ ] **Step 4: Run tests to confirm pass**

Run: `cargo test -p vox-cli --test workflow_lint_test`
Expected: PASS (2 tests).

- [ ] **Step 5: Add it to the complete pre-push tier (advisory)**

Wire `workflow-lint` into the `complete` tier's command list (alongside the existing scoped TOESTUB) so it runs only when `.github/workflows/**` is touched. Verify: `cargo run -p vox-cli --quiet -- ci workflow-lint` returns 0 on the current tree.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-cli/src/commands/ci/ crates/vox-cli/tests/workflow_lint_test.rs
git commit -m "feat(ci): vox ci workflow-lint — guard against recurring `-- ci` invocation bug (#277/#278)"
```

---

## Task 5: Two policy notes in `AGENTS.md`

**Files:**
- Modify: `AGENTS.md` (extend §"Auto-generated documentation files" and §"Local CI Gate Tiers")

The behaviors that produced sinks A and B are not written down where agents read policy. Two short additions make the savings durable across every tool/agent.

- [ ] **Step 1: Add a generated-file diff-reading note**

In `AGENTS.md` §"Auto-generated documentation files (do not edit manually)", append:
```markdown
**Reading diffs cheaply (Required for agents):** generated artifacts are marked
`linguist-generated`/`-diff` in `.gitattributes`, so `git show`/`git diff` already
collapse them. When you must diff a range that regenerates them, exclude them
explicitly to save tokens, e.g.
`git diff <base>..<head> -- . ':(exclude)docs/src/SUMMARY.md' ':(exclude)docs/src/feed.xml' ':(exclude)Cargo.lock'`.
Never read a regenerated index/feed line-by-line to "understand a change" — read the
generator's input instead.
```

- [ ] **Step 2: Add a "don't hand-regenerate after merge" note**

In `AGENTS.md` §"Local CI Gate Tiers (SSOT)" (or §"PR & Review Discipline"), append:
```markdown
**Do not hand-regenerate SSOT after a merge.** Cross-PR generated-file drift is
regenerated and verified automatically in the **merge queue** (`ci.yml`
`ssot-merge-regen`, runs on `merge_group`). If you see drift on `main`, it is a
queue/generator bug to fix at the source — do **not** open a `fix(#N): regenerate …
after merge` commit (that pattern cost 60+ commits historically).
```

- [ ] **Step 3: Verify the frontmatter/doc-pipeline still accepts the file**

Run:
```bash
cargo run -p vox-cli --quiet -- ci doc-lint --since HEAD 2>/dev/null || echo "run repo doc lint manually"
git diff --stat AGENTS.md
```
Expected: AGENTS.md (repo root, not under `docs/src/`, so no frontmatter requirement) passes whatever doc lint applies; the diff shows only the two appended blocks.

- [ ] **Step 4: Commit**

```bash
git add AGENTS.md
git commit -m "docs(agents): generated-file diff discipline + no-hand-regen-after-merge policy (refs sinks A/B)"
```

---

## Task 6: Trim the always-loaded auto-memory index (biggest per-session win)

**Files:**
- Modify: `C:\Users\Owner\.claude\projects\C--Users-Owner-vox\memory\MEMORY.md` (the index loaded every session)
- Modify/Move: completed-project entries under `…\memory\*.md` → an archive the index does not load

`MEMORY.md` carries ~60 one-line pointers, most to **completed/superseded** project records (entries whose text says MERGED / EXECUTED / DONE / superseded). Every session pays to load all of them. The active, still-useful set is: durable `feedback_*` rules + genuinely in-flight `project_*` work. Archiving the rest cuts the fixed per-session token tax roughly in half, compounding across every future conversation. This task touches no repo code — it edits the Claude auto-memory directory.

- [ ] **Step 1: Invoke the consolidation skill (do not hand-roll)**

This is exactly what `anthropic-skills:consolidate-memory` is for. Invoke it; it does a reflective pass — merging duplicates, fixing stale facts, pruning the index. Let it propose the archive set rather than deleting by hand.

- [ ] **Step 2: Classify each `MEMORY.md` entry as KEEP / ARCHIVE**

Heuristic for the skill (or manual pass):
- **KEEP**: every `feedback_*` entry (durable working rules — e.g. `feedback_windows_cargo_fmt`, `feedback_no_stubs`, `feedback_auto_generated_docs`, `feedback_subagents_readonly_in_sandbox`), and any `project_*` whose status is plan-only/in-flight/awaiting-go.
- **ARCHIVE**: `project_*` entries whose hook contains MERGED / EXECUTED / DONE / "merged to main" / "superseded" and whose work is fully landed (e.g. the PR-merge-train entries, `project_scientia_micropub_ssot_design` ✅ DONE, `project_pr273/274` reconciled+merged, `project_vox_mental_tracker_modernization` PR #281, etc.). Keep the file on disk under an archive subdir; remove its line from `MEMORY.md`.

- [ ] **Step 3: Move archived files out of the loaded set**

For each ARCHIVE entry, move the file into `…\memory\archive\` (the session loader reads `MEMORY.md` + recalls individual `memory\*.md`; an `archive\` subdir keeps the fact retrievable without it sitting in the always-loaded index). Delete only the `MEMORY.md` pointer line, not the file.

- [ ] **Step 4: Verify the index shrank and still links validly**

Run:
```bash
wc -l "C:\Users\Owner\.claude\projects\C--Users-Owner-vox\memory\MEMORY.md"
```
Expected: substantially fewer lines than the starting ~60. Spot-check that no KEEP entry's `[[link]]` now points into `archive\` without a corresponding pointer (broken-link audit). Confirm all `feedback_*` entries remain in the index.

- [ ] **Step 5: No git commit**

The memory directory is outside the repo (`~/.claude/...`) and is not version-controlled here. Nothing to commit — the change takes effect on the next session load. Report the before/after line count to the user.

---

## Self-Review

**1. Spec coverage (against the original ask):**
- "map our entire commit message history… and all diffs… understand how the codebase has evolved" → Part 0 evolution map + token-cost ledger (deterministic, full-history). ✔
- "choose how to update agents.md memories" → Task 5 (AGENTS.md) + Task 6 (auto-memory index). ✔
- "suggest rules for new CI/CD, TOESTUB gates, or pre-push that save tokens" → Tasks 1–4; explicitly noted that the TOESTUB gate (lefthook `tdd-guard`) and pre-push tiers **already exist**, so the recommendation is to *extend*, not duplicate. ✔
- "or in just what we already have if it is costing too much" → Part 0 "What already exists" inventory + Task 2 (existing fmt-check is the cost) + Task 3 (existing per-PR drift gate can't see merges). ✔

**2. Placeholder scan:** No "TBD/add error handling/similar to Task N". Every code/config step shows the actual content. The two genuinely conditional spots (Task 1 `where-things-live`/`layers.toml` exclusion; Task 3 `--write` vs concrete generators) are written as explicit verify-then-branch steps, not deferrals.

**3. Type/name consistency:** `lint_str(contents, file) -> Vec<Finding>` and `Finding { message, file, line }` are used identically in Task 4 Steps 1 and 3. The `ssot-merge-regen` job name in Task 3 matches the AGENTS.md reference in Task 5 Step 2. The `vox run scripts/fmt.vox` writer in Task 2 matches the CLAUDE.md/AGENTS.md Windows-safe formatting rule.

**Open verification the implementer must resolve before asserting done** (each is a Step in-task, not a gap): the exact regenerate-all command for Task 3 Step 1; whether a `fmt-fix --write` subcommand exists vs `scripts/fmt.vox` for Task 2 Step 1; and the confirmed generated-vs-authored status of each path in Task 1 Step 4.
