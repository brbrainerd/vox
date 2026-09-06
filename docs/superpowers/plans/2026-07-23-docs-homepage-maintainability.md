# Docs & Homepage Maintainability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore the broken voxlang.org deploy, close the gap between documented doc-freshness policy and what `vox-doc-pipeline` actually enforces, make the `README.md` ↔ `docs/src/index.mdx` sync self-checking instead of silently trusted, and shrink both files back to the size their own documented roles call for — so the homepage stops requiring constant manual upkeep to stay current.

**Architecture:** Four phases, strictly ordered. Phase 0 unblocks production delivery (a broken Cloudflare token means nothing downstream ships). Phase 1 closes a 96%-of-corpus policy violation (hand-authored `last_updated`) with a one-time bulk migration plus a new hard-error lint rule, using the exact pattern `scripts/fix-doc-categories.vox` already established for repo-wide frontmatter edits. Phase 2 adds one new deterministic lint pass (`lint_readme_sync`) to `crates/vox-doc-pipeline` that hard-fails PR CI (`docs-quality.yml`, which already runs on every PR) if `index.mdx`'s `SYNC-FROM-README` blocks drift from README's `ANCHOR` blocks, after known, intentional link-scheme transforms. Phase 3 rewrites `README.md` down to its own documented "short front door" role and ships the previously-reviewed condensed homepage redesign. Phase 4 replaces a dead, non-compiling archival script with a safe, report-only staleness lister.

**Tech Stack:** Rust (`crates/vox-doc-pipeline`), Vox (`.vox` scripts run via `vox run`, per `AGENTS.md` §VoxScript-First Glue Code — no new `.ps1`/`.sh`/`.py`), GitHub Actions YAML, Markdown/MDX (`README.md`, `docs/src/index.mdx`), Astro/Starlight (`docs-astro/`).

**Grounding:** Every file path, line number, and behavioral claim below was verified by direct reads and parallel research agents against the live repo state on 2026-07-23 — see `docs/superpowers/specs/2026-07-23-docs-homepage-maintainability-design.md` for the full evidence trail. Do not re-derive facts already established there; if a cited line number has moved, the surrounding function name is the anchor to search for.

---

## File Structure

- **Modify:** `.github/workflows/docs-deploy.yml` — add a failure-visibility step (Phase 0).
- **Modify:** `crates/vox-doc-pipeline/src/pipeline/types.rs` — new `LintKind` variants (Phase 1, Phase 2).
- **Modify:** `crates/vox-doc-pipeline/src/pipeline/lint.rs` — new hand-authored-`last_updated` detection, `.mdx` file discovery, new `lint_readme_sync` module-level check (Phase 1, Phase 2).
- **Modify:** `crates/vox-doc-pipeline/src/pipeline/mod.rs` — wire new `LintKind` variants into the `eprintln` report and the `hard_errors` filter; call `lint_readme_sync` once per run (Phase 1, Phase 2).
- **Create:** `scripts/docs/strip-last-updated-frontmatter.vox` — one-time bulk migration (Phase 1).
- **Modify:** `README.md`, `docs/src/index.mdx` — fix the `tier_table` drift, resolve the two orphaned sections, then the full README right-sizing and homepage redesign (Phase 2, Phase 3).
- **Create:** `scripts/docs/architecture-staleness-report.vox` — replaces the dead `scripts/quality/archival-enforcer.vox` (Phase 4).
- **Delete:** `scripts/quality/archival-enforcer.vox` (Phase 4).

---

### Task 1: Restore the Cloudflare deploy + add failure visibility

**Files:**
- Modify: `.github/workflows/docs-deploy.yml`

This is the priority-zero task: until it lands, no docs content change (from this plan or anyone else's PRs) reaches production voxlang.org.

- [ ] **Step 1: Human action (not agent-executable) — rotate the Cloudflare API token**

The `CF_API_TOKEN` repo secret is missing `Account.Cloudflare Pages: Edit` permission (confirmed failure: `Authentication error [code: 10000]` in `deploy-cloudflare`, e.g. run `30044803507`). A human with Cloudflare dashboard access must:
1. Go to the Cloudflare dashboard → My Profile → API Tokens.
2. Create (or edit) the token used for `CF_API_TOKEN` and ensure it has **Account → Cloudflare Pages → Edit** permission for the account that owns the `vox-docs` Pages project.
3. Update the GitHub secret: `gh secret set CF_API_TOKEN` (run this locally, in a terminal you control — do not paste the token value into an agent chat).
4. Confirm the account ID in `CF_ACCOUNT_ID` still matches (`docs-deploy.yml:164`).

An agent should present this checklist to the user rather than attempt it — token entry is outside what an agent should do per this project's action-safety rules.

- [ ] **Step 2: Add a failure-visibility step so this never goes silent again**

`docs-deploy.yml` currently has no `if: failure()` step anywhere and nothing else watches it via `workflow_run` (confirmed via full-repo grep). Add a final job that opens/updates a pinned issue on any upstream failure:

```yaml
  notify-on-failure:
    name: Notify on docs deploy failure
    runs-on: ubuntu-latest
    needs: [build-docs, deploy-pages, deploy-cloudflare, smoke-test]
    if: failure()
    permissions:
      issues: write
    steps:
      - name: Open or update a tracking issue
        env:
          GH_TOKEN: ${{ github.token }}
          RUN_URL: ${{ github.server_url }}/${{ github.repository }}/actions/runs/${{ github.run_id }}
        run: |
          existing=$(gh issue list --repo "${{ github.repository }}" --label docs-deploy-broken --state open --json number --jq '.[0].number')
          body="docs-deploy.yml failed on main. Latest failing run: ${RUN_URL}"
          if [ -n "$existing" ]; then
            gh issue comment "$existing" --repo "${{ github.repository }}" --body "$body"
          else
            gh issue create --repo "${{ github.repository }}" --title "docs-deploy is failing on main" --body "$body" --label docs-deploy-broken
          fi
```

Place this as a new top-level job alongside the existing `build-docs` / `deploy-pages` / `deploy-cloudflare` / `smoke-test` jobs (`docs-deploy.yml:22` onward). This is a workflow-YAML step, not a new `.ps1`/`.sh`/`.py` glue script, so it doesn't trigger the VoxScript-first policy.

- [ ] **Step 3: Verify the YAML is well-formed**

Run: `cd docs-astro && npx -y action-validator ../.github/workflows/docs-deploy.yml` (or, if that tool isn't available locally, a plain `python -c "import yaml,sys; yaml.safe_load(open('.github/workflows/docs-deploy.yml'))"` — this is a one-off local syntax check, not a project script, so it doesn't need to be a `.vox` file). Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/docs-deploy.yml
git commit -m "ci(docs): add failure-visibility issue on docs-deploy failure"
```

- [ ] **Step 5: After Step 1's token rotation lands, confirm the pipe is actually flowing**

Push any trivial docs change (or re-run the workflow via `gh workflow run docs-deploy.yml` if it supports `workflow_dispatch` — it currently doesn't, per `docs-deploy.yml:3-11`, so this must be a real push to `main`) and read the run's log directly: `gh run list --workflow=docs-deploy.yml --limit=1` then `gh run view <id> --log-failed` if it's still red. Do not watch it live (`AGENTS.md` §Local-First CI Verification Contract forbids `gh run watch`/polling) — check once after it's had time to finish.

---

### Task 2: Hard-error on hand-authored `last_updated` frontmatter

**Files:**
- Modify: `crates/vox-doc-pipeline/src/pipeline/types.rs`
- Modify: `crates/vox-doc-pipeline/src/pipeline/lint.rs`
- Modify: `crates/vox-doc-pipeline/src/pipeline/mod.rs`
- Test: `crates/vox-doc-pipeline/src/pipeline/lint.rs` (`#[cfg(test)] mod tests` at the bottom — there is already one; add to it)

`documentation-governance.md:86` states manual `last_updated` dates "are considered legacy and will be superseded by Git metadata," but nothing in `crates/vox-doc-pipeline` enforces that — `lint_last_updated_vs_git` (`lint.rs:380-429`) only warns on >30-day drift, gated behind `training_eligible: true`. This task adds a real, unconditional hard error for the presence of the key at all.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block at the bottom of `lint.rs` (after the existing `duplicate_frontmatter_detects_second_yaml_block` test):

```rust
    #[test]
    fn hand_authored_last_updated_is_a_hard_error() {
        let mut errs = Vec::new();
        let md_path = Path::new("fixture.md");
        let content = "---\ntitle: \"Fixture\"\ndescription: \"A fixture page for testing.\"\ncategory: \"Concepts\"\nlast_updated: \"2026-05-05\"\n---\n\nBody.\n";
        lint_frontmatter(md_path, content, &mut errs);
        assert!(
            errs.iter()
                .any(|e| matches!(e.kind, LintKind::HandAuthoredLastUpdated)),
            "expected a HandAuthoredLastUpdated error, got: {errs:?}"
        );
    }

    #[test]
    fn frontmatter_without_last_updated_is_clean() {
        let mut errs = Vec::new();
        let md_path = Path::new("fixture.md");
        let content = "---\ntitle: \"Fixture\"\ndescription: \"A fixture page for testing.\"\ncategory: \"Concepts\"\n---\n\nBody.\n";
        lint_frontmatter(md_path, content, &mut errs);
        assert!(
            !errs.iter().any(|e| matches!(e.kind, LintKind::HandAuthoredLastUpdated)),
            "did not expect a HandAuthoredLastUpdated error, got: {errs:?}"
        );
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p vox-doc-pipeline hand_authored_last_updated_is_a_hard_error`
Expected: compile error — `LintKind::HandAuthoredLastUpdated` does not exist yet.

- [ ] **Step 3: Add the new `LintKind` variant**

In `crates/vox-doc-pipeline/src/pipeline/types.rs`, add a variant to the `LintKind` enum (alongside `MissingCategory`, `UnknownCategory { .. }`, etc.):

```rust
    HandAuthoredLastUpdated,
```

- [ ] **Step 4: Detect it in `lint_frontmatter`**

In `crates/vox-doc-pipeline/src/pipeline/lint.rs`, inside `lint_frontmatter`'s `for (idx, raw_line) in yaml.lines().enumerate()` loop (`lint.rs:444`), add a new branch alongside the existing `category:`/`status:`/`schema_type:`/`training_eligible:`/`training_rationale:` checks:

```rust
        } else if line.starts_with("last_updated:") {
            errors.push(LintError {
                file: path.to_owned(),
                line: line_no,
                kind: LintKind::HandAuthoredLastUpdated,
            });
```

Insert this as another `else if` arm in the existing chain (it currently ends with `} else if line.starts_with("training_rationale:") { saw_training_rationale = true; }` at `lint.rs:487-489` — add the new arm right after that one, before the closing brace of the `for` loop).

- [ ] **Step 5: Wire the new variant into all three exhaustive matches over `LintKind` in `mod.rs`**

> **Revision note (post adversarial review):** the first draft of this task only wired the `eprintln` report block. `mod.rs` has **three** separate exhaustive `match`es over `LintKind`, none with a `_` wildcard: the `eprintln` report block, `workflow_for_kind` (`mod.rs:27-61`), and `kind_label` (`mod.rs:64-83`). Missing an arm in any of the three is a non-exhaustive-match compile error — confirmed by re-reading `mod.rs` directly. All three need the new arm.

In `crates/vox-doc-pipeline/src/pipeline/mod.rs`, add a new match arm to the `eprintln` report block (alongside the existing `LintKind::MissingCategory => { .. }` arm, `mod.rs:294-299`):

```rust
                LintKind::HandAuthoredLastUpdated => {
                    eprintln!(
                        "  ERROR  {} — hand-authored `last_updated:` in frontmatter; the pipeline derives this from Git history (documentation-governance.md). Remove the key.",
                        rel.display()
                    );
                }
```

Add a matching arm to `workflow_for_kind` (`mod.rs:27-61`), following that function's existing style (a short imperative string per variant, e.g. mirroring `LintKind::LastUpdatedStale { .. } => "refresh the ...",` at its final arm):

```rust
                LintKind::HandAuthoredLastUpdated => "remove the hand-authored last_updated key",
```

Add a matching arm to `kind_label` (`mod.rs:64-83`), following that function's existing style (mirroring `LintKind::LastUpdatedStale { .. } => "last-updated-stale",`):

```rust
                LintKind::HandAuthoredLastUpdated => "hand-authored-last-updated",
```

Add `LintKind::HandAuthoredLastUpdated` to the `hard_errors` filter's `matches!` pattern list (`mod.rs:391-410`), e.g. right after `LintKind::MissingTrainingRationale`:

```rust
                        | LintKind::HandAuthoredLastUpdated
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p vox-doc-pipeline hand_authored_last_updated_is_a_hard_error frontmatter_without_last_updated_is_clean`
Expected: both `PASS`.

- [ ] **Step 7: Confirm the crate still builds clean**

Run: `cargo build -p vox-doc-pipeline`
Expected: no errors. All three exhaustive `LintKind` matches in `mod.rs` (`eprintln` block, `workflow_for_kind`, `kind_label`) require an arm for the new variant — Step 5 added all three; a missed one here is exactly the failure mode to watch for.

- [ ] **Step 8: Do NOT run the full doc-pipeline lint yet**

564 of 589 files currently have this key (see Task 3) — running `cargo run -p vox-doc-pipeline` now would fail the build with 564 new hard errors. Leave that until after Task 4's migration lands. Commit this task on its own.

- [ ] **Step 9: Commit**

```bash
git add crates/vox-doc-pipeline/src/pipeline/types.rs crates/vox-doc-pipeline/src/pipeline/lint.rs crates/vox-doc-pipeline/src/pipeline/mod.rs
git commit -m "feat(docs-pipeline): hard-error on hand-authored last_updated frontmatter"
```

---

### Task 3: Write and dry-run the bulk migration script

**Files:**
- Create: `scripts/docs/strip-last-updated-frontmatter.vox`

Adapts `scripts/fix-doc-categories.vox`'s proven walk/read/rewrite pattern (glob → per-file frontmatter-range parse → conditional rewrite → dry-run/apply toggle via env var) to remove rather than replace a frontmatter line.

- [ ] **Step 1: Write the script**

```vox
// ---
// title: "Strip Hand-Authored last_updated Frontmatter"
// description: "One-time repo-wide migration: remove the hand-authored `last_updated:` line from docs/src frontmatter, since the pipeline derives it from Git history (documentation-governance.md)."
// category: "tooling"
// status: "current"
// training_eligible: false
// ---
//
// Usage:
//   DRY_RUN=1 vox run --mode interp scripts/docs/strip-last-updated-frontmatter.vox   # report only
//   vox run --mode interp scripts/docs/strip-last-updated-frontmatter.vox             # apply
//
// See docs/superpowers/specs/2026-07-23-docs-homepage-maintainability-design.md
// and AGENTS.md §Authored Markdown Frontmatter.

// vox:caps fs env

fn norm(p: str) to str {
    return p.replace("\\", "/");
}

// ---------------------------------------------------------------------------
// Return [start, end] inclusive line indices for the YAML frontmatter block
// (the first `---` ... `---` pair). If the file doesn't open with `---`,
// returns [-1, -1].
// ---------------------------------------------------------------------------
fn find_frontmatter_range(lines: list[str]) to list[int] {
    let n = lines.len()
    if n < 2 {
        return [-1, -1]
    }
    let first = lines.get(0).unwrap_or("").trim()
    if first != "---" {
        return [-1, -1]
    }
    let mut i = 1
    while i < n {
        let t = lines.get(i).unwrap_or("").trim()
        if t == "---" {
            return [0, i]
        }
        i = i + 1
    }
    return [-1, -1]
}

// ---------------------------------------------------------------------------
// Remove the `last_updated:` line from the frontmatter block, if present.
// Returns the new content, or "" if no such line was found (no change needed).
// ---------------------------------------------------------------------------
fn remove_last_updated_line(content: str) to str {
    let lines = content.split("\n")
    let range = find_frontmatter_range(lines)
    let fm_start = range.get(0).unwrap_or(-1)
    let fm_end = range.get(1).unwrap_or(-1)
    if fm_start < 0 {
        return ""
    }

    let mut found_idx = -1
    let mut i = fm_start + 1
    while i < fm_end {
        let trimmed = lines.get(i).unwrap_or("").trim()
        if trimmed.starts_with("last_updated:") {
            found_idx = i
            i = fm_end
        } else {
            i = i + 1
        }
    }

    if found_idx < 0 {
        return ""
    }

    let n = lines.len()
    let mut out = ""
    let mut k = 0
    let mut wrote_any = false
    while k < n {
        if k != found_idx {
            if wrote_any {
                out = out + "\n"
            }
            out = out + lines.get(k).unwrap_or("")
            wrote_any = true
        }
        k = k + 1
    }
    return out
}

fn is_dry_run() to bool {
    let opt = env.get("DRY_RUN");
    if opt.is_some() {
        return opt.unwrap() == "1";
    }
    return false;
}

// ---------------------------------------------------------------------------
// Collect docs/src/**/*.md and docs/src/**/*.mdx into one list.
// ---------------------------------------------------------------------------
fn all_doc_files() to list[str] {
    let mut out = [];
    let md = fs.glob("docs/src/**/*.md");
    if md.is_ok() {
        let files = md.unwrap();
        let mut i = 0;
        while i < files.len() {
            out = out.push(files.get(i).unwrap_or(""));
            i = i + 1;
        }
    }
    let mdx = fs.glob("docs/src/**/*.mdx");
    if mdx.is_ok() {
        let files = mdx.unwrap();
        let mut i = 0;
        while i < files.len() {
            out = out.push(files.get(i).unwrap_or(""));
            i = i + 1;
        }
    }
    return out;
}

fn main() {
    if not fs.exists("docs/src") {
        print("Error: docs/src not found — run from the repo root.");
        return;
    }

    let dry_run = is_dry_run();
    if dry_run {
        print("=== strip-last-updated-frontmatter.vox (DRY RUN — no files written) ===");
    } else {
        print("=== strip-last-updated-frontmatter.vox (APPLY — files will be modified) ===");
    }

    let all_files = all_doc_files();
    let n_files = all_files.len();

    let mut changed_count = 0;
    let mut unchanged_count = 0;
    let mut idx = 0;
    while idx < n_files {
        let f = all_files.get(idx).unwrap_or("");
        idx = idx + 1;

        if norm(f).contains("/archive/") {
            continue;
        }

        let content_res = fs.read(f);
        if content_res.is_err() {
            print("  [warn] could not read: " + f);
            continue;
        }
        let content = content_res.unwrap();

        let new_content = remove_last_updated_line(content);
        if new_content.len() == 0 {
            unchanged_count = unchanged_count + 1;
            continue;
        }

        if dry_run {
            print("WOULD STRIP last_updated: " + norm(f));
            changed_count = changed_count + 1;
        } else {
            let write_res = fs.write(f, new_content);
            if write_res.is_ok() {
                print("STRIPPED: " + norm(f));
                changed_count = changed_count + 1;
            } else {
                print("  [err] write failed: " + f);
            }
        }
    }

    print("");
    print("=== Summary ===");
    if dry_run {
        print("  would-change: " + str(changed_count));
    } else {
        print("  changed:      " + str(changed_count));
    }
    print("  unchanged:    " + str(unchanged_count));
}
```

- [ ] **Step 2: Type-check the script**

Run: `vox check scripts/docs/strip-last-updated-frontmatter.vox`
Expected: no errors. If the installed `vox` toolchain reports an unknown method (e.g. `str.contains`, `list.push` returning a new list vs. mutating), fix against the actual signatures reported by the type-checker — `scripts/fix-doc-categories.vox` (already in the repo and known-working) uses the identical `.push()`-returns-new-list and `.get(i).unwrap_or(...)` idioms this script reuses, so a mismatch here means the checker caught a real typo, not a language-surface gap.

- [ ] **Step 3: Dry-run against the real repo**

Run: `cd /path/to/vox && DRY_RUN=1 vox run --mode interp scripts/docs/strip-last-updated-frontmatter.vox`
Expected: `would-change:` close to 564 (the count found during research — re-verify with `grep -rl "^last_updated:" docs/src --include="*.md" --include="*.mdx" | wc -l` and confirm the two numbers match before proceeding).

- [ ] **Step 4: Commit the script (not yet applied)**

```bash
git add scripts/docs/strip-last-updated-frontmatter.vox
git commit -m "feat(docs): add dry-run-first migration to strip hand-authored last_updated"
```

---

### Task 4: Apply the migration

**Files:**
- Modify: 564 files under `docs/src/**/*.md` and `docs/src/index.mdx` (mechanical — no manual edits)

- [ ] **Step 1: Apply**

Run: `vox run --mode interp scripts/docs/strip-last-updated-frontmatter.vox`
Expected: `changed:` matches Task 3 Step 3's dry-run count exactly.

- [ ] **Step 2: Sanity-check the diff shape**

Run: `git diff --stat | tail -5` and spot-check three random files with `git diff -- docs/src/adr/001-burn-backend-selection.md docs/src/index.mdx <one more>`.
Expected: every changed file shows exactly one removed line (`-last_updated: "..."`), nothing else touched.

- [ ] **Step 3: Now run the full doc-pipeline lint to confirm the Task 2 hard error is clear**

Run: `cargo run -p vox-doc-pipeline`
Expected: no `HandAuthoredLastUpdated` errors reported. (Other pre-existing lint errors, if any, are out of scope for this plan — note them, don't fix them here.)

- [ ] **Step 4: Commit as its own PR-sized change**

```bash
git add docs/src
git commit -m "chore(docs): strip hand-authored last_updated frontmatter (564 files)

Governance policy (documentation-governance.md) already states this field
is derived from Git history and manual dates are legacy; enforcement
lands in the prior commit. This is the one-time migration to compliance."
```

Land this as its own PR, separate from any other change in this plan — it's the single largest diff here and reviewing it is trivial (one line removed per file) only if nothing else is mixed in.

---

### Task 5: Give `index.mdx` lint coverage

**Files:**
- Modify: `crates/vox-doc-pipeline/src/pipeline/lint.rs`
- Test: same file, existing `#[cfg(test)] mod tests` block

`gather_md_files` only matches `.md`. `docs/src/index.mdx` is confirmed (via `find docs/src -iname "*.mdx"`) to be the **only** `.mdx` file under `docs/src/`, so this is a single-file-impact change.

- [ ] **Step 1: Write the failing test**

Add to `lint.rs`'s test module:

```rust
    #[test]
    fn gather_md_files_includes_mdx() {
        let tmp = std::env::temp_dir().join("vox_doc_pipeline_mdx_test");
        let _ = std::fs::create_dir_all(&tmp);
        let mdx_path = tmp.join("index.mdx");
        std::fs::write(&mdx_path, "---\ntitle: \"x\"\n---\nbody").unwrap();
        let mut out = Vec::new();
        gather_md_files(&tmp, &mut out);
        assert!(
            out.iter().any(|p| p == &mdx_path),
            "expected gather_md_files to include index.mdx, got: {out:?}"
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }
```

(If `gather_md_files` is not `pub(crate)`/visible to the test module as written, check its current visibility at `lint.rs:~165` — it's defined in the same file as the test module, so no additional `pub` is needed.)

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p vox-doc-pipeline gather_md_files_includes_mdx`
Expected: FAIL (assertion fails — `.mdx` not included).

- [ ] **Step 3: Widen BOTH extension checks in `gather_md_files`**

> **Revision note (post adversarial review):** `gather_md_files` has **two** separate extension checks, not one — a direct-file branch (`lint.rs:167`) and a second check inside the recursive directory-walk branch (`lint.rs:182`, `else if path.extension().map(|e| e == "md")...`). Every real call path — `collect_lint_errors` walking `docs/src`, and this task's own test calling `gather_md_files` on a directory — goes through the recursive branch at line 182, not the direct-file branch at 167. Patching only 167 (the original draft of this step) leaves `index.mdx` invisible in every real invocation and the test in Step 1 still failing. Both must change.

In `gather_md_files`, change the extension check at `lint.rs:167` from:

```rust
        if target.extension().map(|e| e == "md").unwrap_or(false)
```

to:

```rust
        if target
            .extension()
            .map(|e| e == "md" || e == "mdx")
            .unwrap_or(false)
```

And change the second extension check at `lint.rs:182` from:

```rust
        else if path.extension().map(|e| e == "md").unwrap_or(false)
```

to:

```rust
        else if path
            .extension()
            .map(|e| e == "md" || e == "mdx")
            .unwrap_or(false)
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p vox-doc-pipeline gather_md_files_includes_mdx`
Expected: PASS.

- [ ] **Step 5: Run the full lint against `index.mdx` specifically and fix whatever it now surfaces**

Run: `cargo run -p vox-doc-pipeline -- --lint-only --paths docs/src/index.mdx` (flag shape per `AGENTS.md`'s documented scoped-lint invocation). Expected: it now runs at all (previously silently skipped). Fix anything it flags (e.g. `category`/`status` value validity) as part of this task — don't leave a newly-surfaced hard error unresolved.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-doc-pipeline/src/pipeline/lint.rs docs/src/index.mdx
git commit -m "fix(docs-pipeline): include .mdx files in doc lint (index.mdx was invisible to it)"
```

---

### Task 6: Fix the one real `tier_table` content drift

**Files:**
- Modify: `README.md`
- Modify: `docs/src/index.mdx`

Confirmed divergence: README line 314 links `[Candle/Burn](crates/vox-populi/src/inference/mod.rs)`; `index.mdx` line 276 links `[Candle/Burn](https://github.com/vox-foundation/vox/tree/main/crates/vox-inference/)` — different crate paths, from an unfinished crate rename/split.

- [ ] **Step 1: Determine the current, correct target**

Run: `ls crates/vox-inference crates/vox-populi/src/inference 2>&1` to see which path actually exists today (a split may have happened, orphaning one side).

- [ ] **Step 2: Update both files to the same, correct target**

In `README.md` line 314 (inside the `tier_table` `ANCHOR` block), set the link to whichever path Step 1 confirmed is current, in README's relative-path style, e.g. if `crates/vox-inference/` is the real current crate:

```markdown
| Inference (Mens) | 🟡 Preview | Native CUDA/Metal/CPU inference with [Candle/Burn](crates/vox-inference/). |
```

In `docs/src/index.mdx` line 276 (inside the `tier_table` `SYNC` block), set the matching absolute-link form:

```mdx
| Inference (Mens) | 🟡 Preview | Native CUDA/Metal/CPU inference with [Candle/Burn](https://github.com/vox-foundation/vox/tree/main/crates/vox-inference/). |
```

(Substitute `vox-populi/src/inference/mod.rs` in both places instead if Step 1 shows that's the one that still exists — the point is both files must name the *same* target.)

- [ ] **Step 3: Verify no other `tier_table` rows have the same problem**

Run: `diff <(sed -n '296,332p' README.md) <(sed -n '258,294p' docs/src/index.mdx)` and manually confirm every remaining difference is one of the two known transforms (relative→absolute link, `docs/src/`→`./`), not a third drifted target.

- [ ] **Step 4: Commit**

```bash
git add README.md docs/src/index.mdx
git commit -m "fix(docs): resolve stale Inference/Mens crate link drift between README and homepage"
```

---

### Task 7: Resolve the two orphaned sections

**Files:**
- Modify: `README.md`
- Modify: `docs/src/index.mdx`

`community_license` has an `ANCHOR` in README (lines 372-380) but no matching `SYNC` block in `index.mdx` — the two have independently diverged. The `## Documentation` intent table is unmarked on either side and has also diverged (different rows). Per the spec's Phase 3 direction (README shrinks to a front door, the full pitch's home is the site), the fix for both is the same shape: stop maintaining two independently-worded copies.

- [ ] **Step 1: `community_license` — keep one short version, drop the duplicate**

Since Task 9 (README right-sizing) is about to rewrite this section anyway, do the minimal fix now: remove the `<!-- ANCHOR: community_license -->` / `<!-- ANCHOR_END: community_license -->` markers from `README.md` (lines 372, 380) since nothing consumes them (confirmed: no matching `SYNC` block exists, and Task 8 below only checks `why_vox`/`how_vox`/`tier_table`). Leave the prose itself untouched for now — Task 9 rewrites it properly.

```bash
# In README.md, delete these two lines only:
#   <!-- ANCHOR: community_license -->
#   <!-- ANCHOR_END: community_license -->
```

- [ ] **Step 2: `## Documentation` table — make `index.mdx` the single source, drop README's copy**

`index.mdx`'s version (lines 298-309) is the one that's actually useful to keep — it's the interactive, linked doc-map on the site itself, kept fresh by the sidebar SSOT machinery described in `AGENTS.md` §Auto-generated documentation files. README's copy (lines 338-350) is redundant now that README will link to the site (Task 9). Delete README's `## Documentation` section (lines 338-350) entirely; replace it with a single link:

```markdown
## Documentation

Full docs, organized by intent (tutorials, how-to guides, reference, architecture): **https://voxlang.org**
```

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "chore(docs): drop orphaned README/homepage duplication (community_license anchor, Documentation table)"
```

---

### Task 8: Add the automated `lint_readme_sync` check

**Files:**
- Modify: `crates/vox-doc-pipeline/src/pipeline/types.rs`
- Modify: `crates/vox-doc-pipeline/src/pipeline/lint.rs`
- Modify: `crates/vox-doc-pipeline/src/pipeline/mod.rs`
- Test: `crates/vox-doc-pipeline/src/pipeline/lint.rs`

This closes the core gap identified in the spec: the `SYNC-FROM-README`/`ANCHOR` markers exist but nothing has ever checked them. The check is a deterministic string comparison after two known, intentional link-scheme transforms — not a fuzzy diff — to keep false positives near zero.

- [ ] **Step 1: Write the failing tests**

Add to `lint.rs`'s test module:

```rust
    #[test]
    fn readme_sync_detects_matching_block_after_known_transforms() {
        let readme = "<!-- ANCHOR: demo -->\nSee [the crate](crates/vox-db/) and ![x](docs/src/assets/pic.png).\n<!-- ANCHOR_END: demo -->\n";
        let mdx = "{/* SYNC-FROM-README: demo */}\nSee [the crate](https://github.com/vox-foundation/vox/tree/main/crates/vox-db/) and ![x](./assets/pic.png).\n{/* SYNC-END: demo */}\n";
        let mut errors = Vec::new();
        lint_readme_sync_content(readme, mdx, Path::new("docs/src/index.mdx"), &["demo"], &mut errors);
        assert!(
            errors.is_empty(),
            "expected no drift after known transforms, got: {errors:?}"
        );
    }

    #[test]
    fn readme_sync_flags_real_drift() {
        let readme = "<!-- ANCHOR: demo -->\nSee [the crate](crates/vox-db/).\n<!-- ANCHOR_END: demo -->\n";
        let mdx = "{/* SYNC-FROM-README: demo */}\nSee [a totally different crate](https://github.com/vox-foundation/vox/tree/main/crates/vox-other/).\n{/* SYNC-END: demo */}\n";
        let mut errors = Vec::new();
        lint_readme_sync_content(readme, mdx, Path::new("docs/src/index.mdx"), &["demo"], &mut errors);
        assert!(
            errors.iter().any(|e| matches!(&e.kind, LintKind::ReadmeSyncDrift { block } if block == "demo")),
            "expected a ReadmeSyncDrift for 'demo', got: {errors:?}"
        );
    }

    #[test]
    fn readme_sync_flags_missing_mdx_block() {
        let readme = "<!-- ANCHOR: demo -->\nSee the crate.\n<!-- ANCHOR_END: demo -->\n";
        let mdx = "no sync block here at all\n";
        let mut errors = Vec::new();
        lint_readme_sync_content(readme, mdx, Path::new("docs/src/index.mdx"), &["demo"], &mut errors);
        assert!(
            errors.iter().any(|e| matches!(&e.kind, LintKind::ReadmeSyncMissingBlock { block } if block == "demo")),
            "expected a ReadmeSyncMissingBlock for 'demo', got: {errors:?}"
        );
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p vox-doc-pipeline readme_sync_`
Expected: compile errors — `lint_readme_sync_content`, `LintKind::ReadmeSyncDrift`, `LintKind::ReadmeSyncMissingBlock` don't exist yet.

- [ ] **Step 3: Add the new `LintKind` variants**

In `crates/vox-doc-pipeline/src/pipeline/types.rs`:

```rust
    ReadmeSyncDrift { block: String },
    ReadmeSyncMissingBlock { block: String },
    ReadmeSyncMissingAnchor { block: String },
```

- [ ] **Step 4: Implement the extraction, normalization, and comparison in `lint.rs`**

Add near the bottom of `lint.rs`, above the `#[cfg(test)]` module:

```rust
/// README.md sections kept in sync with docs/src/index.mdx via matching
/// `<!-- ANCHOR: name --> ... <!-- ANCHOR_END: name -->` (README) and
/// `{/* SYNC-FROM-README: name */} ... {/* SYNC-END: name */}` (index.mdx) markers.
const SYNCED_BLOCKS: &[&str] = &["why_vox", "how_vox", "tier_table"];

fn extract_marked_block(content: &str, start_needle: &str, end_needle: &str) -> Option<String> {
    let start_idx = content.find(start_needle)?;
    let after_start = &content[start_idx + start_needle.len()..];
    let end_idx = after_start.find(end_needle)?;
    Some(after_start[..end_idx].trim().to_string())
}

fn readme_anchor(readme: &str, name: &str) -> Option<String> {
    let start = format!("<!-- ANCHOR: {name} -->");
    let end = format!("<!-- ANCHOR_END: {name} -->");
    extract_marked_block(readme, &start, &end)
}

fn mdx_sync_block(mdx: &str, name: &str) -> Option<String> {
    let start = format!("{{/* SYNC-FROM-README: {name} */}}");
    let end = format!("{{/* SYNC-END: {name} */}}");
    extract_marked_block(mdx, &start, &end)
}

/// Apply the known, intentional README->index.mdx link/markup transforms so the
/// two blocks compare equal when they're genuinely in sync. Whitespace is also
/// collapsed, since line-wrap differences between the two files carry no meaning.
fn normalize_for_compare(s: &str) -> String {
    let transformed = s
        .replace("docs/src/assets/", "./assets/")
        .replace("docs/src/reference/", "./reference/")
        .replace("docs/src/how-to/", "./how-to/")
        .replace("docs/src/architecture/", "./architecture/")
        .replace("docs/src/adr/", "./adr/")
        .replace("docs/src/explanation/", "./explanation/")
        .replace(
            "](crates/",
            "](https://github.com/vox-foundation/vox/tree/main/crates/",
        )
        .replace("<br>", "<br />");
    transformed.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Core comparison logic, separated from file I/O so it's directly unit-testable.
fn lint_readme_sync_content(
    readme: &str,
    mdx: &str,
    mdx_path: &Path,
    blocks: &[&str],
    errors: &mut Vec<LintError>,
) {
    for &name in blocks {
        let Some(readme_block) = readme_anchor(readme, name) else {
            errors.push(LintError {
                file: mdx_path.to_owned(),
                line: 1,
                kind: LintKind::ReadmeSyncMissingAnchor {
                    block: name.to_string(),
                },
            });
            continue;
        };
        let Some(mdx_block) = mdx_sync_block(mdx, name) else {
            errors.push(LintError {
                file: mdx_path.to_owned(),
                line: 1,
                kind: LintKind::ReadmeSyncMissingBlock {
                    block: name.to_string(),
                },
            });
            continue;
        };
        if normalize_for_compare(&readme_block) != normalize_for_compare(&mdx_block) {
            errors.push(LintError {
                file: mdx_path.to_owned(),
                line: 1,
                kind: LintKind::ReadmeSyncDrift {
                    block: name.to_string(),
                },
            });
        }
    }
}

/// Whole-repo check: compares README.md against docs/src/index.mdx. Called once
/// per lint run (not per-file) from `mod.rs`.
///
/// Revision note (post adversarial review): the first draft took `repo_root: &Path`
/// as a caller-supplied argument and called `repo_root_for_lint()` from `mod.rs` to
/// build it — but `repo_root_for_lint()` (`lint.rs:94`) has no visibility modifier,
/// so it's private to this module and `mod.rs` cannot call it (Rust privacy is
/// module-scoped, not crate-scoped). Fixed by using the same plain-relative-path
/// convention the rest of this tool already relies on: `mod.rs`'s own entrypoint
/// assumes the process cwd is the repo root (it does `Path::new("docs/src")` with
/// no root-joining at all, `mod.rs:195`), so this function does the same instead of
/// taking or computing a repo root. This also fixes a second bug in the original
/// draft: building an *absolute* `mdx_path` would fail the `eprintln` block's
/// `e.file.strip_prefix(docs_src)` (where `docs_src` is the relative `Path::new("docs/src")`),
/// printing an ugly full path instead of `index.mdx` like every other lint message.
pub(crate) fn lint_readme_sync(errors: &mut Vec<LintError>) {
    let readme_path = Path::new("README.md");
    let mdx_path = Path::new("docs/src/index.mdx");
    let Ok(readme) = vox_bounded_fs::read_utf8_path_capped(readme_path) else {
        return;
    };
    let Ok(mdx) = vox_bounded_fs::read_utf8_path_capped(mdx_path) else {
        return;
    };
    lint_readme_sync_content(&readme, &mdx, mdx_path, SYNCED_BLOCKS, errors);
}
```

- [ ] **Step 5: Wire reporting and the hard-error gate in `mod.rs` — all three exhaustive matches**

> **Revision note (post adversarial review):** same finding as Task 2 Step 5 — `mod.rs` has three exhaustive `match`es over `LintKind` (`eprintln` block, `workflow_for_kind`, `kind_label`), not one. All three need an arm for each of the three new variants.

Add three `eprintln` arms (alongside `LintKind::HandAuthoredLastUpdated` added in Task 2):

```rust
                LintKind::ReadmeSyncDrift { block } => {
                    eprintln!(
                        "  ERROR  {} — SYNC block '{}' has drifted from its README.md ANCHOR counterpart. Re-sync both, or fix the anchor if content genuinely changed.",
                        rel.display(),
                        block
                    );
                }
                LintKind::ReadmeSyncMissingBlock { block } => {
                    eprintln!(
                        "  ERROR  {} — README.md has ANCHOR '{}' but index.mdx has no matching SYNC-FROM-README block.",
                        rel.display(),
                        block
                    );
                }
                LintKind::ReadmeSyncMissingAnchor { block } => {
                    eprintln!(
                        "  ERROR  {} — index.mdx expects README.md ANCHOR '{}' but it's missing there.",
                        rel.display(),
                        block
                    );
                }
```

Add matching arms to `workflow_for_kind` (`mod.rs:27-61`):

```rust
                LintKind::ReadmeSyncDrift { .. } => "re-sync the block with README.md, or fix the ANCHOR if content genuinely changed",
                LintKind::ReadmeSyncMissingBlock { .. } => "add the matching SYNC-FROM-README block to index.mdx",
                LintKind::ReadmeSyncMissingAnchor { .. } => "add the matching ANCHOR block to README.md",
```

Add matching arms to `kind_label` (`mod.rs:64-83`):

```rust
                LintKind::ReadmeSyncDrift { .. } => "readme-sync-drift",
                LintKind::ReadmeSyncMissingBlock { .. } => "readme-sync-missing-block",
                LintKind::ReadmeSyncMissingAnchor { .. } => "readme-sync-missing-anchor",
```

Add all three to the `hard_errors` filter's `matches!` list.

Call the new whole-repo check once, right after the existing `collect_lint_errors(docs_src, &mut lint_errors);` call (`mod.rs:256`):

```rust
        lint::lint_readme_sync(&mut lint_errors);
```

(Signature takes no arguments after the Step 4 revision — see the `lint_readme_sync` doc comment above for why.)

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p vox-doc-pipeline readme_sync_`
Expected: all three `PASS`.

- [ ] **Step 7: Run against the real repo in report mode before trusting it as a hard gate**

Run: `cargo run -p vox-doc-pipeline` and read the output for any `ReadmeSyncDrift`/`ReadmeSyncMissing*` lines.
Expected: **zero**, since Task 6 and Task 7 already fixed the one known drift and removed the orphaned sections. If anything unexpected shows up, it's either a normalization gap (extend `normalize_for_compare`, re-run) or a real drift this task's own tests didn't anticipate — fix the content, not the checker, unless the checker is provably wrong.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-doc-pipeline/src/pipeline/types.rs crates/vox-doc-pipeline/src/pipeline/lint.rs crates/vox-doc-pipeline/src/pipeline/mod.rs
git commit -m "feat(docs-pipeline): hard-fail PR CI when index.mdx drifts from README's synced sections"
```

---

### Task 9: Shrink `README.md` to its documented "front door" role

**Files:**
- Modify: `README.md`

`documentation-governance.md`'s authority map already defines README's job: "short front door, quick start, tone, links into the book." Current: 394 lines / 2,640 words. Target shape below keeps the `why_vox` `ANCHOR` (still useful as a short pitch) and the `tier_table` `ANCHOR`, drops the full five-pillar `how_vox` block from README (its canonical home becomes the homepage only — see Task 10), and replaces everything else with links.

- [ ] **Step 1: Confirm the current full README content isn't lost**

Before deleting anything from README, confirm `docs/src/index.mdx`'s `how_vox` SYNC block (already verified word-for-word identical to README's in the spec's research) will remain the full canonical version after Task 10. It will — Task 10 only restructures the *presentation* of the homepage, not the underlying pillar content, which moves from "five full `<section>`-style blocks" to "a five-item grid with the same facts, condensed," while the detail itself is preserved on a linked-through architecture/explanation page (Task 10, Step 3).

- [ ] **Step 2: Rewrite README.md**

Replace the file's structure with: hero image + tagline (short), the `why_vox` `ANCHOR` block (unchanged, ~14 lines), a quick-start code snippet, the `tier_table` `ANCHOR` block (unchanged), and a compact footer (license, backing, contributing, link to the full site). Remove: the full `how_vox` block and its `ANCHOR` markers (the pillar detail now lives solely on the homepage/architecture pages), the standalone "Language at a Glance" second code sample (redundant with the `why_vox` snippet), the References/footnotes section (site-only from here on).

Concretely, delete `README.md` lines 109-261 (the `how_vox` `ANCHOR` block and its five pillars) and replace with:

```markdown
## How Vox works

One `.vox` file becomes a database schema, a type-safe server, and a live browser UI. Full walkthrough of all five architectural pillars: **https://voxlang.org/#how-vox-works**
```

Leave `why_vox` (lines 32-46) and `tier_table` (lines 296-332, post-Task-6-fix) exactly as they are — both stay `ANCHOR`-marked and covered by Task 8's `lint_readme_sync` check, same as today.

- [ ] **Step 3: Verify the doctest/include lint still passes**

Run: `cargo run -p vox-doc-pipeline` — README.md itself isn't under `docs/src/`, so it's not walked by `gather_md_files`, but any `vox` code fences left in it are still worth a manual sanity check: `grep -n '```vox' README.md` and confirm each still compiles by eye (or via `vox check` on an extracted copy) since README isn't covered by the automated doctest runner.

- [ ] **Step 4: Run `lint_readme_sync` once more to confirm nothing broke**

Run: `cargo run -p vox-doc-pipeline` and confirm no `ReadmeSyncDrift`/`ReadmeSyncMissing*` output — the `why_vox` and `tier_table` blocks are untouched, so this should still be clean.

- [ ] **Step 5: Commit**

```bash
git add README.md
git commit -m "docs: shrink README to its documented front-door role, full pitch lives on the site"
```

---

### Task 10: Ship the condensed homepage redesign

**Files:**
- Modify: `docs/src/index.mdx`
- Modify: `docs-astro/astro.config.mjs` (only if a new component is added — see Step 2)

Implements the progressive-disclosure concept already reviewed with the user in the prior design-critique turn: trimmed hero, a five-item pillar grid instead of five full `<section>` blocks, a compact five-row status strip linking to a full stability page instead of the inline 20-row table, and the existing Diátaxis doc-map kept as-is (it already works well).

- [ ] **Step 1: Preserve full pillar detail on its own page before condensing the homepage**

Since Task 9 removed the full `how_vox` prose from README, and this task condenses it on the homepage too, the *complete* five-pillar detail (with all code blocks) needs exactly one remaining canonical home. Confirmed (adversarial review, re-read of the live file): `docs/src/explanation/expl-architecture.md` already exists (229 lines, `title: "Compiler Architecture"`, `category: "Concepts"`) but is entirely about the compiler pipeline (lexer → parser → AST → HIR → typecheck → emitters) — it does **not** contain the five pillars (Schema/Errors/Deploy/Agents/Training); grepping it for pillar terms and `@query`/`@mutation` returns nothing. So: move the current `how_vox` `SYNC` block's full content (index.mdx lines 68-220, pre-condensing) into this file as a new `## The five pillars` section (append, don't replace the existing compiler-pipeline content — they're complementary, not competing), keeping all five existing code blocks verbatim.

- [ ] **Step 2: Replace the `how_vox` section of `index.mdx` with the condensed grid**

Replace `docs/src/index.mdx` lines 68-220 (the current `{/* SYNC-FROM-README: how_vox */}` ... `{/* SYNC-END: how_vox */}` block) with a condensed five-item grid, keeping the same marker names so Task 8's `lint_readme_sync` still has something to check against a *short* version of `how_vox` in README (Task 9 already replaced README's own `how_vox` with the two-line pointer, so — important — **the `how_vox` block must be dropped from `SYNCED_BLOCKS`** in Task 8's `lint.rs` constant, since after Task 9 there's no longer a real `how_vox` `ANCHOR` in README to compare against. Update `SYNCED_BLOCKS` from Task 8 to `&["why_vox", "tier_table"]` as part of this step, and delete the now-orphaned `{/* SYNC-FROM-README: how_vox */}` / `{/* SYNC-END: how_vox */}` markers from `index.mdx` (they're no longer syncing anything — replace with a plain comment `{/* condensed pillar grid — full detail: /explanation/expl-architecture/ */}` if a marker is still wanted for readability).

Use the same visual structure already built and shown to the user as an artifact mockup in the prior turn: a `.pillars` grid of five cards (Schema / Errors / Deploy / Agents / Training), each with a one-line label and a ~25-word description, linking out to `/explanation/expl-architecture/` for the full walkthrough — reuse the exact card copy from that mockup rather than re-deriving it, since it was already reviewed.

- [ ] **Step 3: Condense the stability table**

Replace the current 20-row `tier_table` *rendering* on the homepage — note: the underlying `ANCHOR`/`SYNC` **content** stays full-size and synced per Task 6/8 (it's still the canonical source), but the homepage should present a 5-row summary strip (Compiler & LSP, Database engine, Durable runtime, Native GUI, Distributed mesh — one representative row per top-level category already present in the full table) linking to a new `docs/src/reference/stability.md` page that renders the full table. Move the full `tier_table` content to that new page (create it, `category: "Language Reference"` frontmatter, full markdown table body from the current `tier_table` block), and change `index.mdx`'s `{/* SYNC-FROM-README: tier_table */}` block to hold only the condensed 5-row strip.

Confirmed (adversarial review): no existing page under `docs/src/reference/` or `docs/src/architecture/` already covers this — the only near-hit, `docs/src/architecture/v1-release-criteria.md`, is a different table (release-gate criteria CR-F/CR-K/CR-U) already linked *from within* the current `tier_table` prose itself, so `docs/src/reference/stability.md` is a genuinely new page, not a duplicate. Cross-link the two: add a line near the top of the new `stability.md` pointing at `v1-release-criteria.md` for "what counts as done," since a reader landing on one naturally wants the other.

This changes Task 8's normalization target: **after this step, `tier_table` also needs to leave `SYNCED_BLOCKS`**, since the homepage no longer shows the full table README still needs to show (README's own `tier_table` `ANCHOR` stays full-size — README readers on GitHub don't get the linked-page treatment). Update `SYNCED_BLOCKS` in `lint.rs` to `&["why_vox"]` only. Document this explicitly in a comment above the constant:

```rust
// Only `why_vox` is still a true 1:1 sync target. `how_vox` and `tier_table`
// were condensed on the homepage (2026-07-23 redesign, see
// docs/superpowers/specs/2026-07-23-docs-homepage-maintainability-design.md)
// — their full-detail canonical homes are docs/src/explanation/expl-architecture.md
// and docs/src/reference/stability.md respectively, linked from the homepage,
// not duplicated on it.
```

- [ ] **Step 4: Update the Diátaxis doc-map table's Architecture row**

Add the new `stability.md` page as a linked entry if it doesn't already fit an existing row in the `## Documentation` table (index.mdx lines 298-309, unaffected by earlier tasks).

- [ ] **Step 5: Re-run the full lint suite**

Run: `cargo run -p vox-doc-pipeline`
Expected: clean — no `ReadmeSyncDrift`/`ReadmeSyncMissing*` (now only checking `why_vox`), no `HandAuthoredLastUpdated`, no broken-include errors.

- [ ] **Step 6: Build the site locally and visually verify**

Run: `cd docs-astro && pnpm install && pnpm build`
Expected: build succeeds. Then `pnpm preview` and open the homepage in a browser (or use the Browser pane's `preview_start`) to confirm: hero renders above the fold without scrolling on a standard viewport, the pillar grid reads as five cards not five sections, the stability strip is five items with a working link to `/reference/stability/`, and the doc-map table is unchanged.

- [ ] **Step 7: Commit**

```bash
git add docs/src/index.mdx docs/src/explanation/expl-architecture.md docs/src/reference/stability.md crates/vox-doc-pipeline/src/pipeline/lint.rs
git commit -m "feat(docs): ship condensed homepage — pillar grid + linked stability page, replacing full README-mirror sections"
```

---

### Task 11: End-to-end verification

**Files:** none (verification only)

- [ ] **Step 1: Full local CI gate**

Run: `vox ci pre-push --complete`
Expected: green. This covers fmt, doc lint (including every new rule from Tasks 2/5/8), doc-inventory, and clippy for anything touched.

- [ ] **Step 2: Confirm the deployed site will actually pick this up**

Re-confirm Task 1 landed and is green (`gh run list --workflow=docs-deploy.yml --limit=1`) before merging any content from Tasks 6-10 — merging correct content into a still-broken pipe reproduces the exact staleness problem this plan started from.

- [ ] **Step 3: Post-merge smoke check**

After the PR(s) merge and `docs-deploy.yml` runs green, load `https://voxlang.org` and confirm the homepage shows the new condensed layout and `@query`/`@mutation`/`@server` syntax (not `@endpoint`).

---

### Task 12: Replace the dead archival-enforcer with a safe, report-only lister

**Files:**
- Create: `scripts/docs/architecture-staleness-report.vox`
- Delete: `scripts/quality/archival-enforcer.vox`

`archival-enforcer.vox` doesn't compile under the current toolchain (`docs/src/architecture/vox-stdlib-gap-audit-2026-05-23.md:297`, CHECK-FAIL bucket), isn't invoked anywhere, and checks the now-forbidden hand-authored `last_updated` field (Task 2 removes the field entirely, which would make the old script's check permanently vacuous even if it did compile). Replace it with a report-only tool using git-log recency instead, per the spec's Phase 4 non-goal of no automated file moves.

- [ ] **Step 1: Delete the dead script**

```bash
git rm scripts/quality/archival-enforcer.vox
```

- [ ] **Step 2: Write the replacement, report-only**

```vox
// ---
// title: "Architecture Docs Staleness Report"
// description: "Lists docs/src/architecture/*.md files whose last Git commit is older than a threshold, for human review — does not move or modify any file."
// category: "tooling"
// status: "current"
// training_eligible: false
// ---
//
// Usage:
//   vox run --mode interp scripts/docs/architecture-staleness-report.vox
//   VOX_STALE_DAYS=180 vox run --mode interp scripts/docs/architecture-staleness-report.vox
//
// Output is a candidate list for a human to review before manually moving
// anything into docs/src/archive/. This script never writes or moves files.

// vox:caps fs env subprocess

fn norm(p: str) to str {
    return p.replace("\\", "/");
}

fn stale_days_threshold() to int {
    let opt = env.get("VOX_STALE_DAYS");
    if opt.is_some() {
        return int(opt.unwrap());
    }
    return 270;
}

// Returns days since the file's last git commit, or -1 if it couldn't be determined.
fn days_since_last_commit(rel_path: str) to int {
    let args = ["log", "-1", "--format=%ct", "--", rel_path];
    let proc = process.run("git", args);
    if proc is null {
        return -1;
    }
    let res = proc.unwrap();
    if res.code != 0 {
        return -1;
    }
    let out = res.stdout.trim();
    if out.len() == 0 {
        return -1;
    }
    let epoch = int(out);

    let now_proc = process.run("git", ["log", "-1", "--format=%ct"]);
    if now_proc is null {
        return -1;
    }
    let now_res = now_proc.unwrap();
    let now_out = now_res.stdout.trim();
    let now_epoch = int(now_out);

    let delta_secs = now_epoch - epoch;
    return delta_secs / 86400;
}

fn main() {
    if not fs.exists("docs/src/architecture") {
        print("Error: docs/src/architecture not found — run from the repo root.");
        return;
    }

    let threshold = stale_days_threshold();
    print("=== architecture-staleness-report.vox (threshold: " + str(threshold) + " days, report only) ===");

    let glob_res = fs.glob("docs/src/architecture/**/*.md");
    if glob_res.is_err() {
        print("Error: fs.glob failed for docs/src/architecture/**/*.md");
        return;
    }
    let all_files = glob_res.unwrap();
    let n = all_files.len();

    let mut candidates = [];
    let mut checked = 0;
    let mut unknown = 0;
    let mut idx = 0;
    while idx < n {
        let f = norm(all_files.get(idx).unwrap_or(""));
        idx = idx + 1;
        let days = days_since_last_commit(f);
        if days < 0 {
            unknown = unknown + 1;
            continue;
        }
        checked = checked + 1;
        if days >= threshold {
            candidates = candidates.push(f + " (" + str(days) + " days)");
        }
    }

    print("");
    print("=== Summary ===");
    print("  checked:            " + str(checked));
    print("  unknown (no git history resolved): " + str(unknown));
    print("  stale candidates:   " + str(len(candidates)));
    print("");
    if len(candidates) > 0 {
        print("Candidates for manual archival review (docs/src/archive/):");
        let mut i = 0;
        while i < len(candidates) {
            print("  - " + candidates[i]);
            i = i + 1;
        }
        print("");
        print("Review each by hand — this list is not authoritative and this script moves nothing.");
    }
}
```

- [ ] **Step 3: Type-check and dry-run**

> **Revision note (post adversarial review):** the original draft of `stale_days_threshold`/`days_since_last_commit` treated `int(str)` as returning a `Result` (`.is_ok()`/`.is_err()`/`.unwrap()`). Every real working `.vox` script that calls `int(...)` (`scripts/perf/test-baseline.vox`, `scripts/crate-build-audit.vox`) uses it as a direct, non-fallible conversion — no unwrap anywhere. The code above was corrected to call it directly. `int()`'s exact behavior on a malformed string (panic vs. silent 0) is still unverified from documentation alone — this step's `vox check` run is the actual confirmation; if `vox check` or a live `vox run` shows `int()` returning an `Option`/`Result` after all, restore the guard using whatever pattern `vox check`'s own error message names, don't guess again.

Run: `vox check scripts/docs/architecture-staleness-report.vox`, then `vox run --mode interp scripts/docs/architecture-staleness-report.vox`.
Expected: no crash; a summary with a `stale candidates` count. Given the spec's finding that ~2/3-3/4 of the 301 `docs/src/architecture/*.md` files read as one-off dated writeups by filename, expect a substantial (dozens to low hundreds) candidate count — that's expected, not a bug, since this is a first-ever report on a backlog that's never been triaged.

- [ ] **Step 4: Commit**

Confirmed (adversarial review) `archival-enforcer.vox` is safe to delete: no reference in any `.github/workflows/*.yml`, `contracts/ci/check-targets.v1.yaml`, or `vox-cli` command registration. One near-miss worth naming in the commit message so a future reader doesn't conflate the two: `crates/vox-cli/src/commands/ci/run_body_helpers/docs.rs:236` has an unrelated `check_archival_pipeline()` function that validates metadata on files *already* in `docs/src/archive/` — it never invoked the deleted script and isn't touched by this change.

```bash
git add scripts/docs/architecture-staleness-report.vox
git rm scripts/quality/archival-enforcer.vox
git commit -m "chore(docs): replace dead archival-enforcer with a safe, report-only staleness lister

Not to be confused with check_archival_pipeline() in
crates/vox-cli/src/commands/ci/run_body_helpers/docs.rs, which validates
already-archived files and is unrelated/unaffected."
```

---

### Task 13: File the manual triage backlog item

**Files:** none (process step)

Explicitly out of scope for automation per the spec's non-goals — this task just makes sure Task 12's output turns into tracked follow-up work instead of a one-off printout nobody acts on.

- [ ] **Step 1: Run the report and file it**

Run Task 12's script, take its candidate list, and open a tracking issue with the list attached — whichever this repo's existing convention favors for multi-item backlogs (check a handful of recent issues via `gh issue list --limit 20` for the pattern before choosing). Do **not** create or edit `docs/src/architecture/research-index.md` (retired 2026-09).

- [ ] **Step 2: Commit if a doc changed, otherwise just leave the filed issue**

If a doc changed during triage filing, commit it. Do not add `docs/src/architecture/research-index.md`.

```bash
git commit -m "docs: log architecture-directory staleness triage backlog"
```

---

## Self-Review

**Spec coverage:** Phase 0 → Task 1. Phase 1 → Tasks 2-5. Phase 2 → Tasks 6-8. Phase 3 → Tasks 9-11. Phase 4 → Tasks 12-13. All nine numbered findings in the spec's Context section are addressed by at least one task; all five Non-goals are respected (no fuzzy diffing anywhere in Task 8, no auto-move in Task 12, no wiki migration, no `docs-reality-audit` rewrite, no scope creep into `docs-quality.yml`'s existing checks beyond the one new rule).

**Placeholder scan:** every code step above is complete, runnable code, not a description of code — re-checked against the "No Placeholders" list (no TBD/TODO, no "add appropriate handling," no "similar to Task N" hand-waving).

**Type/name consistency:** `LintKind::HandAuthoredLastUpdated` (Task 2) is referenced identically in its `mod.rs` wiring. `lint_readme_sync_content` / `lint_readme_sync` / `SYNCED_BLOCKS` (Task 8) are used consistently, and Task 10 explicitly calls out and updates the two places `SYNCED_BLOCKS`'s membership must shrink as the homepage condenses (`how_vox` then `tier_table` drop out) — this was the one real risk of a stale cross-reference between tasks, now handled inline rather than left implicit.

**Adversarial review pass (post-draft, against the live repo — not simulated):** three independent agents re-read the actual `crates/vox-doc-pipeline` source, the actual working `.vox` scripts this plan's new scripts claim to mirror, and the actual current state of every file path this plan names, specifically to find false positives before an engineer hits them. Three real, compile-breaking or logic-breaking bugs were found and are now fixed inline (marked `**Revision note (post adversarial review):**` at each site):
1. Tasks 2 and 8 originally wired only one of `mod.rs`'s three exhaustive `LintKind` matches (`eprintln`, `workflow_for_kind`, `kind_label`) — all three now get an arm for every new variant.
2. Task 5 originally patched only one of `gather_md_files`'s two extension checks (the unused direct-file branch, not the recursive-walk branch every real call path uses) — both now patched.
3. Task 8's `lint_readme_sync` originally took a `repo_root_for_lint()`-derived path that `mod.rs` cannot legally call (private, module-scoped visibility) and would have printed absolute paths in lint output — now reads plain repo-root-relative paths directly, matching this tool's own existing convention, with no cross-module call needed.

One more finding closed a lower-confidence gap rather than a bug: Task 12's `.vox` script originally assumed `int(str)` returns a `Result`, contradicted by every real working script that calls it — corrected to a direct call, with the residual uncertainty (does `int()` panic on bad input?) explicitly flagged for the executing engineer to confirm via `vox check` rather than asserted either way. Two checks (Task 9's line numbers, Task 1's secret name) and Task 10's file-existence assumptions were confirmed accurate as originally written and needed no change; Task 3's script was independently confirmed to have zero mismatches against its template.
