# Research-Index Retirement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `docs/src/architecture/research-index.md` obsolete: archive the hand-curated file, stop every live policy/test/CI surface from treating it as a completeness gate, retarget every live link, and land the cleanup on `main`.

**Architecture:** Discoverability already comes from page frontmatter (`title`, `description`, `category`, `sort_order`, `status`) plus `docs-astro/src/utils/sidebar.mjs` `collectPages()`. That is the same pattern that retired `architecture-index.md` (gitignored; Starlight sidebar is the browse surface). This plan does **not** add a generated replacement catalog and does **not** CI-fail on “forgot a bullet.” Completeness is: valid frontmatter + `vox-doc-pipeline --lint-only` + the sidebar walk. The retired file is moved under `docs/src/archive/` (tombstoned; agents must not ingest it) and old URLs become static redirect stubs.

**Tech Stack:** Markdown + YAML frontmatter, Starlight/`sidebar.mjs`, `vox-doc-pipeline`, `vox ci check-links`, `vox ci doc-inventory`, GitHub Pages HTML stubs + `docs-astro/public/_redirects`, Rust integration tests in `vox-integration-tests`.

## Global Constraints

- Do **not** recreate `docs/src/architecture/research-index.md` or `docs/src/architecture/architecture-index.md` as a committed completeness index.
- Do **not** add a `vox ci research-index-check` (or any “every current page must appear in file X” gate). The dormant `architecture-index.md` completeness loop in `crates/vox-cli/src/commands/ci/run_body_helpers/docs.rs` stays dormant (it only fires if that file exists); do not copy it for research-index.
- Do **not** generate a committed catalog. `SUMMARY.md` / `feed.xml` stay gitignored Astro outputs. No new `research-index.generated.md`.
- Do **not** ingest or “refresh” files under `docs/src/archive/` for new planning (root `AGENTS.md` §Archival Protocol).
- Do **not** touch `mesh-phase3-*` worktrees or `vox-mens-hub`.
- Shared worktree `/Users/brbrainerd/dev/vox` is multi-agent. Implement from an isolated clone (`git clone /Users/brbrainerd/dev/vox /private/tmp/vox-research-index && git remote set-url origin https://github.com/vox-foundation/vox.git`) branched from `origin/main`.
- Line endings LF. No new `.sh`/`.ps1`/`.py` glue. Secrets via `vox_secrets`. No `TURSO_*`.
- `exceptions` ledger entries and crate-edge baseline enlargements are USER-AUTHORIZED-ONLY.
- `#539` (`docs/true-workflow-durability`) currently adds a bullet to `research-index.md`. If `#539` merges first, this plan archives the file including that bullet. If this plan merges first, rebase `#539` and **drop** the `research-index.md` hunk (frontmatter on the new design doc is enough).
- Dependabot majors (`#535` vitest 3→5, `#536` plugin-react 5→6, `#538` setup-node 6→7) are out of scope; do not merge them from this plan.
- Continue merging unrelated PRs onto `main` only when the fail bucket is empty **and** a high code review is clean (fix findings before merge).

---

## File map

| Path | Responsibility after this plan |
| --- | --- |
| `docs/src/architecture/research-index.md` | **Deleted** from the live tree. |
| `docs/src/archive/research-index-hand-curated-retired-2026-09.md` | Frozen snapshot of the last hand-curated index + tombstone. Distinct from the already-archived `docs/src/archive/research-2026-q1/research-index.md`. |
| `docs-astro/public/architecture/research-index.html` | Client redirect stub (GitHub Pages). |
| `docs-astro/public/architecture/research-index/index.html` | Trailing-slash stub so `/architecture/research-index/` does not 404 after Starlight stops emitting the page. |
| `docs-astro/public/architecture/architecture-index.html` | Create (doc claims it exists; it does not). Point at contributor-hub, **not** research-index. |
| `docs-astro/public/_redirects` | Retarget both index HTML routes to contributor-hub. |
| `AGENTS.md`, `docs/src/AGENTS.md` | Stop instructing agents to edit the index. Fix the stale `architecture-index.md` pointer. |
| `docs/src/.well-known/llms.txt`, `llms-full.txt` | Drop Research Index / Architecture Index rows; keep where-things-live + contributor-hub. |
| `crates/vox-skills/skills/superpowers/deep-research.skill.md` | Frontmatter-only close-out; no index edit. |
| `crates/vox-integration-tests/tests/speech_audit_contract_test.rs` | Discoverability = frontmatter, not an index filename grep. |
| `crates/vox-integration-tests/tests/research_index_retirement_test.rs` | **New.** Ratchet: live path absent; live policy files must not instruct updating it. |
| `crates/vox-cli/src/commands/ci/run_body_helpers/docs.rs` | Fail if the live research-index path is recreated. |
| `.github/workflows/docs-quality.yml` | Stop special-casing `research-index.md` in markdownlint. |
| `scripts/quality/doc-policy-lint.vox` | Drop the `research-index.md` auto-generated exemption. |
| `contracts/documentation/link-allowlist.v1.yaml` | Delete rows whose `source` is `docs/src/architecture/research-index.md` only. |
| `docs/src/contributors/contributor-hub.md`, `docs/src/index.mdx` | Navigation → sidebar + where-things-live + contributor-hub. |
| `docs/src/architecture/github-pages-redirects.md` | Document the new stub targets; stop claiming a missing `architecture-index.html` that pointed at research-index. |
| `docs/agents/doc-inventory.json` | Regenerated via `vox ci doc-inventory generate` after the move. |
| Live architecture / reference / skill docs that link the index | Retarget (Task 6 table). |
| Historical superpowers plans | Mechanical instruction rewrite (Task 7 table). Do not “execute” those old plans. |

**Non-files (already sufficient, do not invent replacements):**

- `docs-astro/src/utils/sidebar.mjs` `collectPages()` — live browse surface.
- `vox-doc-pipeline --lint-only` — frontmatter completeness.
- `docs/src/architecture/where-things-live.md` — concept → crate map (keep; not an index of research essays).

---

## Review findings this plan fixes

High-severity (behavior / agent-instruction bugs), found while reviewing the live index machinery:

1. **Completeness gate in tests.** `speech_audit_docs_are_published_and_indexed` requires `research-index.md` to contain four filenames. A new speech (or any) doc that forgets a bullet fails CI even when frontmatter is valid and the sidebar lists it.
2. **Agents are still told to hand-edit a 323-line file.** Root `AGENTS.md` line 38 and `docs/src/AGENTS.md` line 24 plus `deep-research.skill.md` line 40. That is the reason the index rots and why `#539` had to touch it.
3. **Conflicting SSOT claims.** `AGENTS.md` says the file is hand-curated and safe to edit. `mesh-phase4-dashboard-control-plan-2026.md` / `mesh-phase6-grand-network-plan-2026.md` say it is tool-regenerated and must not be edited. There is no generator.
4. **`architecture-index.md` pointer in `AGENTS.md` is already dead** (file is `.gitignore`d). `llms-full.txt` still advertises `/architecture/architecture-index/`. Same class of bug; fix in this PR.
5. **Redirect chain / missing stub.** `github-pages-redirects.md` lists `architecture/architecture-index.html`; the file is not on disk. The live `research-index.html` stub refreshes to `/architecture/research-index/` (the Starlight page). After deletion that URL 404s unless a trailing-slash stub is added.
6. **Allowlist debt exists only because the index uses bare `crates/` paths.** Five `source: docs/src/architecture/research-index.md` rows in `link-allowlist.v1.yaml`. Deleting the live file removes the need for those rows. Do **not** delete allowlist rows whose `source` is some other file even if the `reason` text mentions research-index (those are copy-paste reasons on `vox-as-llm-target-audit-and-plan-2026.md`).

---

### Task 1: Failing retirement tests

**Files:**
- Create: `crates/vox-integration-tests/tests/research_index_retirement_test.rs`
- Modify: `crates/vox-integration-tests/tests/speech_audit_contract_test.rs:90-120`
- Test: the two files above

**Interfaces:**
- Consumes: `workspace_root()` pattern already in `speech_audit_contract_test.rs` (copy, do not extract a crate edge).
- Produces: `research_index_live_path_is_absent`, `live_policy_does_not_instruct_research_index_updates`, rewritten `speech_audit_docs_are_published_and_indexed`.

- [ ] **Step 1: Write the failing retirement test**

```rust
#![allow(missing_docs)]

//! Research-index is retired. Discoverability is frontmatter + Starlight sidebar.

use std::fs;
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates/")
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn research_index_live_path_is_absent() {
    let live = workspace_root().join("docs/src/architecture/research-index.md");
    assert!(
        !live.exists(),
        "docs/src/architecture/research-index.md must not exist; \
         Starlight sidebar.mjs collectPages() is the browse surface. \
         If you need the last hand-curated snapshot, see \
         docs/src/archive/research-index-hand-curated-retired-2026-09.md \
         (do not ingest archive/ for new work)."
    );
}

#[test]
fn live_policy_does_not_instruct_research_index_updates() {
    let root = workspace_root();
    let policy_files = [
        "AGENTS.md",
        "docs/src/AGENTS.md",
        "CLAUDE.md",
        "GEMINI.md",
        "crates/vox-skills/skills/superpowers/deep-research.skill.md",
        "docs/src/.well-known/llms.txt",
        "docs/src/.well-known/llms-full.txt",
        "docs/src/contributors/contributor-hub.md",
        "docs/src/contributors/documentation-governance.md",
    ];
    let banned = [
        "update `docs/src/architecture/research-index.md`",
        "update docs/src/architecture/research-index.md",
        "After writing to `docs/`, update [`docs/src/architecture/research-index.md`]",
        "After creating a new research page, update `docs/src/architecture/research-index.md`",
        "After writing, update `docs/src/architecture/research-index.md`",
        "hand-curated SSOT index",
    ];
    for rel in policy_files {
        let path = root.join(rel);
        if !path.exists() {
            continue;
        }
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        for needle in banned {
            assert!(
                !text.contains(needle),
                "{rel} still instructs research-index maintenance: {needle:?}"
            );
        }
    }
}
```

- [ ] **Step 2: Run the new tests and confirm they fail**

Run: `cargo test -p vox-integration-tests --test research_index_retirement_test -- --nocapture`

Expected: FAIL — `research_index_live_path_is_absent` because the live file still exists; `live_policy_does_not_instruct_research_index_updates` because `AGENTS.md` still has the update instruction.

- [ ] **Step 3: Rewrite the speech discoverability assertion (still passing after rewrite, before archive)**

Replace the index grep in `speech_audit_docs_are_published_and_indexed` with frontmatter checks. Keep the four required docs.

```rust
#[test]
fn speech_audit_docs_are_published_and_indexed() {
    let root = workspace_root();
    let required_docs = [
        "docs/src/architecture/vox-speech-surface-inventory-2026.md",
        "docs/src/architecture/vox-speech-audit-findings-2026.md",
        "docs/src/architecture/vox-speech-improvement-backlog-2026.md",
        "docs/src/architecture/vox-speech-ci-gates-proposal-2026.md",
    ];
    for rel in required_docs {
        let abs = root.join(rel);
        assert!(abs.exists(), "missing speech audit doc: {}", abs.display());
        let raw =
            fs::read_to_string(&abs).unwrap_or_else(|e| panic!("read {}: {e}", abs.display()));
        assert!(raw.contains("title:"), "{rel} must have frontmatter title");
        assert!(
            raw.contains("category:"),
            "{rel} must have frontmatter category so Starlight sidebar.mjs collectPages() lists it"
        );
        assert!(
            !rel.contains("/archive/"),
            "{rel} must stay in the live docs tree, not docs/src/archive/"
        );
    }
}
```

- [ ] **Step 4: Run the speech test and confirm it still passes**

Run: `cargo test -p vox-integration-tests --test speech_audit_contract_test speech_audit_docs_are_published_and_indexed -- --nocapture`

Expected: PASS (docs still have frontmatter; we have not archived anything yet).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-integration-tests/tests/research_index_retirement_test.rs \
        crates/vox-integration-tests/tests/speech_audit_contract_test.rs
git commit -m "$(cat <<'EOF'
test: ratchet research-index retirement and drop index filename grep

EOF
)"
```

---

### Task 2: Stop live policy from requiring the index

**Files:**
- Modify: `AGENTS.md:26`, `AGENTS.md:38`, `AGENTS.md:56-58`
- Modify: `docs/src/AGENTS.md:21-24`
- Modify: `crates/vox-skills/skills/superpowers/deep-research.skill.md:37-40`
- Modify: `docs/src/.well-known/llms.txt:17-20`
- Modify: `docs/src/.well-known/llms-full.txt:22-25`
- Modify: `docs/src/contributors/documentation-governance.md:51`, `docs/src/contributors/documentation-governance.md:117-122`
- Test: `cargo test -p vox-integration-tests --test research_index_retirement_test live_policy_does_not_instruct_research_index_updates`

**Interfaces:**
- Consumes: Task 1 banned-string list (keep that test in sync if you reword).
- Produces: agents write frontmatter only; sidebar picks the page up.

- [ ] **Step 1: Replace the research-storage bullets in root `AGENTS.md`**

Primary navigation (around line 26) — remove the architecture-index pointer (file is gitignored). Keep architecture-index.md mentioned only as retired/gitignored in the auto-generated section.

```md
- Architecture map: Starlight sidebar (frontmatter `category` / `sort_order`) — do not revive `docs/src/architecture/architecture-index.md`
```

Research storage (around lines 36-38):

```md
- Research docs follow the naming pattern: `*-research-2026.md`, `*-findings-2026.md`
- Architecture SSoT docs: `*-ssot.md` or descriptive names in `docs/src/architecture/`
- After writing a page, set valid YAML frontmatter (`title`, `description`, `category`, `status`). Starlight `docs-astro/src/utils/sidebar.mjs` lists it automatically. Do **not** create or edit `docs/src/architecture/research-index.md` (retired; snapshot under `docs/src/archive/`).
- Do not store Vox-specific research in IDE knowledge bases that are only accessible to one tool
```

Manually-maintained files (around lines 56-58) — **delete** the research-index bullet. Keep ADR index + individual docs.

- [ ] **Step 2: Replace `docs/src/AGENTS.md` research storage**

```md
## Research storage
- Research findings → `docs/src/architecture/`
- Naming: `*-research-2026.md` or `*-findings-2026.md`
- After writing, set frontmatter (`title`, `description`, `category`, `status`). Do not edit `research-index.md` (retired).
```

- [ ] **Step 3: Replace the skill close-out**

In `deep-research.skill.md`, replace the last bullet:

```md
  - After creating a new research page, set YAML frontmatter (`title`, `description`, `category`, `status`, `training_eligible`). Do not edit `docs/src/architecture/research-index.md` (retired).
```

- [ ] **Step 4: Replace llms.txt / llms-full.txt architecture authority rows**

`llms.txt` Architecture Authority:

```md
## Architecture Authority
- [Where things live](https://voxlang.org/architecture/where-things-live/): Concept → crate lookup table
- [Classification SSOT](https://voxlang.org/architecture/classification-ssot-2026/): Canonical surface classifications
- [Contributor Hub](https://voxlang.org/contributors/contributor-hub/): Start here; Architecture SSOTs are listed in the Starlight sidebar
```

`llms-full.txt` — delete both of these lines:

```md
- [Architecture Index](https://voxlang.org/architecture/architecture-index/)
- [Research Index](https://voxlang.org/architecture/research-index/)
```

Keep where-things-live and the SSOTs that follow.

- [ ] **Step 5: Tighten documentation-governance**

Category table row `Architecture SSOTs` — change “research indexes” to “research pages (sidebar-listed via frontmatter)”.

`D-index` row — add: “Do not maintain a committed research-index.md. The Starlight sidebar is the D-index for architecture/research pages.”

- [ ] **Step 6: Re-run the policy test**

Run: `cargo test -p vox-integration-tests --test research_index_retirement_test live_policy_does_not_instruct_research_index_updates -- --nocapture`

Expected: PASS. `research_index_live_path_is_absent` still FAIL.

- [ ] **Step 7: Commit**

```bash
git add AGENTS.md docs/src/AGENTS.md \
        crates/vox-skills/skills/superpowers/deep-research.skill.md \
        docs/src/.well-known/llms.txt docs/src/.well-known/llms-full.txt \
        docs/src/contributors/documentation-governance.md
git commit -m "$(cat <<'EOF'
docs: stop instructing agents to maintain research-index.md

EOF
)"
```

---

### Task 3: Archive the file and install redirects

**Files:**
- Create: `docs/src/archive/research-index-hand-curated-retired-2026-09.md`
- Delete: `docs/src/architecture/research-index.md`
- Modify: `docs-astro/public/architecture/research-index.html`
- Create: `docs-astro/public/architecture/research-index/index.html`
- Create: `docs-astro/public/architecture/architecture-index.html`
- Modify: `docs-astro/public/_redirects:24`
- Modify: `docs/src/architecture/github-pages-redirects.md:23-41`

**Interfaces:**
- Consumes: last committed contents of `research-index.md` (copy, then delete).
- Produces: live path absent; old URLs land on contributor-hub.

- [ ] **Step 1: Confirm the live file still exists and copy it**

```bash
test -f docs/src/architecture/research-index.md
git log -1 --format='%H %s' -- docs/src/architecture/research-index.md
```

Expected: file exists; print the last commit that touched it (needed for the tombstone).

- [ ] **Step 2: Write the archive tombstone + snapshot**

Create `docs/src/archive/research-index-hand-curated-retired-2026-09.md` with this frontmatter **then** the full previous body under a `## Frozen snapshot` heading. Do not try to keep the snapshot “current.”

```md
---
title: "research-index (retired 2026-09)"
description: "Frozen hand-curated architecture/research index. Not a completeness gate. Do not ingest for new work."
category: "archive"
status: "archived"
training_eligible: false
---

# research-index — retired 2026-09

**Do not update this file.** It is a snapshot of `docs/src/architecture/research-index.md` as of the retirement PR.

Replacement discoverability:

- Starlight sidebar via `docs-astro/src/utils/sidebar.mjs` `collectPages()` (frontmatter `category` / `sort_order` / `status`)
- Concept → crate: [`docs/src/architecture/where-things-live.md`](../architecture/where-things-live.md)
- Contributor start: [`docs/src/contributors/contributor-hub.md`](../contributors/contributor-hub.md)

Old URLs `/architecture/research-index/` and `/architecture/research-index.html` redirect to `/contributors/contributor-hub/`.

A still-older copy already lives at `docs/src/archive/research-2026-q1/research-index.md`. Leave that Q1 file untouched.

## Frozen snapshot

```

Immediately after that heading, paste the **body** of the old file (everything after its frontmatter). Do not paste the old frontmatter.

- [ ] **Step 3: Delete the live file**

```bash
git rm docs/src/architecture/research-index.md
```

- [ ] **Step 4: Replace HTML stubs**

`docs-astro/public/architecture/research-index.html` and `docs-astro/public/architecture/research-index/index.html` and `docs-astro/public/architecture/architecture-index.html` — identical payload except the comment:

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta http-equiv="refresh" content="0; url=/contributors/contributor-hub/">
  <link rel="canonical" href="https://voxlang.org/contributors/contributor-hub/">
  <title>Redirecting…</title>
</head>
<body>
  <p><a href="/contributors/contributor-hub/">Click here if not redirected.</a></p>
</body>
</html>
```

- [ ] **Step 5: Retarget `_redirects`**

Replace:

```text
/architecture/research-index.html     /architecture/research-index/     301
```

with:

```text
/architecture/research-index.html     /contributors/contributor-hub/    301
/architecture/research-index          /contributors/contributor-hub/    301
/architecture/research-index/         /contributors/contributor-hub/    301
/architecture/architecture-index.html /contributors/contributor-hub/    301
/architecture/architecture-index      /contributors/contributor-hub/    301
/architecture/architecture-index/     /contributors/contributor-hub/    301
```

GitHub Pages ignores `_redirects`; the HTML stubs are the real Pages behavior. Keep `_redirects` honest for any future Cloudflare/Netlify move.

- [ ] **Step 6: Update `github-pages-redirects.md` file list**

In the `docs-astro/public/` list, keep `architecture/research-index.html` and add `architecture/research-index/index.html` plus `architecture/architecture-index.html`. State that both indexes redirect to `/contributors/contributor-hub/`, not to each other.

- [ ] **Step 7: Run the retirement tests**

Run: `cargo test -p vox-integration-tests --test research_index_retirement_test -- --nocapture`

Expected: PASS (live path gone; policy already cleaned in Task 2).

- [ ] **Step 8: Commit**

```bash
git add docs/src/archive/research-index-hand-curated-retired-2026-09.md \
        docs-astro/public/architecture/research-index.html \
        docs-astro/public/architecture/research-index/index.html \
        docs-astro/public/architecture/architecture-index.html \
        docs-astro/public/_redirects \
        docs/src/architecture/github-pages-redirects.md
git add -u docs/src/architecture/research-index.md
git commit -m "$(cat <<'EOF'
docs: archive research-index.md and redirect old URLs to contributor-hub

EOF
)"
```

---

### Task 4: CI ratchet so the live file cannot return

**Files:**
- Modify: `crates/vox-cli/src/commands/ci/run_body_helpers/docs.rs:154-179` (add after the architecture-index block)
- Modify: `.github/workflows/docs-quality.yml:88`
- Modify: `scripts/quality/doc-policy-lint.vox:28-30`
- Modify: `contracts/documentation/link-allowlist.v1.yaml:131-160`
- Test: existing `vox-cli` docs helper tests if any; plus a new unit test in the same file’s `#[cfg(test)]` if that module already tests path checks. If `docs.rs` has no test module, put the behavioral test only in `research_index_retirement_test.rs` (already covers absence) and keep the CLI check as defense-in-depth.

**Interfaces:**
- Consumes: `root: &Path` already threaded through the docs helper.
- Produces: `vox ci` / docs-quality path that used to lint the index now ignores it; recreating the live file fails CI.

- [ ] **Step 1: Write the failing CLI assertion (add this function next to the architecture-index block)**

```rust
    let retired_research_index = root.join("docs/src/architecture/research-index.md");
    if retired_research_index.is_file() {
        return Err(anyhow!(
            "retired completeness index recreated: {} — Starlight sidebar.mjs collectPages() \
             is the browse surface. Snapshot: docs/src/archive/research-index-hand-curated-retired-2026-09.md",
            retired_research_index.display()
        ));
    }
```

Do **not** add a loop that requires every `status: current` page to be mentioned in any index.

- [ ] **Step 2: Drop markdownlint’s special-case path**

`.github/workflows/docs-quality.yml` Markdown lint step — change to contributors only (research-index is gone):

```yaml
      - name: Markdown lint
        run: pnpm dlx markdownlint-cli2 "docs/src/contributors/**/*.md"
```

Do not add a replacement architecture glob in this PR; that is a lint-scope expansion and will fail on pre-existing architecture prose.

- [ ] **Step 3: Drop the auto-generated exemption**

`scripts/quality/doc-policy-lint.vox`:

```vox
fn is_auto_generated(file_path: str) to bool {
    let normalized = file_path.replace("\\", "/");
    return normalized.ends_with("/SUMMARY.md") or normalized.ends_with("/architecture-index.md") or normalized.ends_with("feed.xml");
}
```

- [ ] **Step 4: Delete allowlist rows whose source is the live research-index**

Remove the block from the `# ── model-routing placeholder text` research-index row through the last `source: "docs/src/architecture/research-index.md"` row (currently five rows, targets `model-routing.v1.yaml-not-direct-link`, `crates/vox-compiler/src/typeck/effect_check.rs`, `crates/vox-code-audit/src/detectors/`, `crates/vox-cli/src/commands/repair.rs`, `crates/vox-compiler/src/typeck/diagnostics.rs:96`, `crates/vox-compiler/src/pipeline.rs:21`).

Leave every `vox-as-llm-target-audit-and-plan-2026.md` row even when `reason` mentions research-index.

- [ ] **Step 5: Verify check-links still compiles the allowlist**

Run: `cargo run -q -p vox-cli -- ci check-links`

Expected: FAIL on remaining live markdown that still links `research-index.md` (Task 6). That failure is the punch list. If it unexpectedly PASSES, `rg -n 'research-index' --glob '!docs/src/archive/**' --glob '!docs/superpowers/**'` and treat any live hit as a Task 6 miss.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-cli/src/commands/ci/run_body_helpers/docs.rs \
        .github/workflows/docs-quality.yml \
        scripts/quality/doc-policy-lint.vox \
        contracts/documentation/link-allowlist.v1.yaml
git commit -m "$(cat <<'EOF'
ci: fail if research-index.md is recreated; drop index-only allowlist rows

EOF
)"
```

---

### Task 5: Navigation surfaces

**Files:**
- Modify: `docs/src/contributors/contributor-hub.md:46`, `docs/src/contributors/contributor-hub.md:62`
- Modify: `docs/src/index.mdx:131`
- Modify: `docs/src/architecture/search-retrieval-ssot-2026.md:115`
- Modify: `docs/src/contributors/dependency-policy.md:49`
- Modify: `docs/src/reference/ref-stdlib-index.md:17`
- Modify: `docs/src/architecture/legacy-tombstone-remediation-ledger-2026.md:68`
- Test: `cargo run -q -p vox-cli -- ci check-links` (still expected to fail on remaining architecture companions until Task 6)

**Interfaces:**
- Consumes: contributor-hub as the human/agent start page; where-things-live as the crate map.
- Produces: no live nav table points at `/architecture/research-index/`.

- [ ] **Step 1: Rewrite contributor-hub rows**

Need “Read architecture or research context”:

```md
| Read architecture or research context | [Where things live](../architecture/where-things-live.md) · Starlight sidebar section **Architecture SSOTs** |
```

Need “Architecture or roadmap context”:

```md
| Architecture or roadmap context | [Where things live](../architecture/where-things-live.md) · Starlight sidebar **Architecture SSOTs** |
```

- [ ] **Step 2: Rewrite homepage architecture cell**

`docs/src/index.mdx` line 131:

```md
| **Architecture** | [Where things live](/architecture/where-things-live/) · [Contributor hub](/contributors/contributor-hub/) |
```

- [ ] **Step 3: Rewrite the three remaining “start here” pointers**

`search-retrieval-ssot-2026.md` change checklist:

```md
- After new architecture pages: set frontmatter so the Starlight sidebar lists them; add a row to [`where-things-live.md`](where-things-live.md) if the page introduces a new concept→crate mapping. Link from root [`AGENTS.md`](../../../AGENTS.md) only when the page is a new always-loaded policy pointer.
```

`dependency-policy.md`:

```md
See also: [Workspace dependency audit findings](../architecture/workspace-dependency-audit-2026.md).
```

`ref-stdlib-index.md` (drop the research-index parenthetical; keep the phase-spec meaning):

```md
| **C — HTTP / net (keywords)** | `query`/`mutation`/`server`, HTTP client ergonomics | Phase HTTP specs under `docs/src/architecture/` (sidebar: Architecture SSOTs) |
```

`legacy-tombstone-remediation-ledger-2026.md` — replace the Research index link with contributor-hub + where-things-live.

- [ ] **Step 4: Commit**

```bash
git add docs/src/contributors/contributor-hub.md docs/src/index.mdx \
        docs/src/architecture/search-retrieval-ssot-2026.md \
        docs/src/contributors/dependency-policy.md \
        docs/src/reference/ref-stdlib-index.md \
        docs/src/architecture/legacy-tombstone-remediation-ledger-2026.md
git commit -m "$(cat <<'EOF'
docs: retarget live navigation off research-index.md

EOF
)"
```

---

### Task 6: Remaining live architecture / news / skill references

**Files:** every non-archive, non-superpowers-plan file that still mentions `research-index` after Tasks 2–5. Inventory at plan-write time (57 files total; this task is the live remainder):

- `docs/src/architecture/deep-research-competitive-landscape-2026-08-01.md`
- `docs/src/architecture/deep-research-verification-2026-08-01.md`
- `docs/src/architecture/deep-research-fundamentals-2026-08-01.md`
- `docs/src/architecture/deep-research-cross-domain-methods-survey-2026-08-01.md`
- `docs/src/architecture/deep-research-domain-agnosticism-audit-2026-08-01.md`
- `docs/src/architecture/deep-research-implementation-divergence-audit-2026-08-01.md`
- `docs/src/architecture/deep-research-gui-representation-design-2026-08-01.md`
- `docs/src/architecture/deep-research-prior-art-and-vox-roadmap-2026.md`
- `docs/src/architecture/free-tier-model-selection-and-onboarding-research-2026-08-01.md`
- `docs/src/architecture/ai-first-fixtures-research-2026.md`
- `docs/src/architecture/shiki-mdbook-doc-platform-research-2026.md`
- `docs/src/architecture/vox-language-rules-and-enforcement-plan-2026.md`
- `docs/src/architecture/vox-language-rules-phase1-ssot-collapse-2026.md`
- `docs/src/architecture/vox-gui-native-roadmap-2026.md`
- `docs/src/architecture/plugin-system-redesign-2026.md`
- `docs/src/architecture/plugin-system-redesign-sp1-plan-2026.md`
- `docs/src/architecture/populi-mesh-north-star-2026.md`
- `docs/src/architecture/mesh-phase0-foundations-plan-2026.md`
- `docs/src/architecture/mesh-phase4-dashboard-control-plan-2026.md`
- `docs/src/architecture/mesh-phase6-grand-network-plan-2026.md`
- `docs/src/architecture/model-orchestration-ssot-audit-2026.md`
- `docs/src/architecture/data-storage-migration-backlog-2026.md`
- `docs/src/architecture/ludus-adjudication-implementation-plan-2026.md`
- `docs/src/architecture/tauri-convergence-migration-plan-2026.md`
- `docs/src/architecture/unified-task-hopper-research-2026.md`
- `docs/src/architecture/warp-research-findings-2026.md`
- `docs/src/architecture/multi-agent-vcs-replication-impl-plan-phase1-2026.md`
- `docs/news/2026-05-11-ai-fixtures-runtime-wired.md`

**Interfaces:**
- Consumes: replacement sentences below (use the matching class; do not invent a third index).
- Produces: `rg 'research-index' docs/src docs/news AGENTS.md crates --glob '!docs/src/archive/**'` returns only the retirement/tombstone mentions allowed in Task 8.

Replacement classes (copy verbatim):

**Class A — companion “see research-index.md” in a Scope/See-also line.** Replace `[research-index.md](research-index.md)` with nothing extra if sibling links already exist; otherwise:

```md
Browse **Architecture SSOTs** in the Starlight sidebar, or start at [contributor-hub](../contributors/contributor-hub.md).
```

**Class B — “update / add a row to research-index.md” in a still-current SSOT checklist.** Replace with:

```md
Set valid frontmatter (`title`, `description`, `category`, `status`). Do not recreate `research-index.md`.
```

**Class C — “research-index is hand-edited / regenerated / do not edit.”** Replace with:

```md
`research-index.md` is retired (archived 2026-09). The Starlight sidebar is the browse surface. `architecture-index.md` stays gitignored.
```

**Class D — news / historical “see research-index for SSOT links.”** Replace with a concrete remaining link (the article’s own contract/SSOT), e.g. for the AI-fixtures news item: `contracts/agentos/ai-first-fixtures.v1.yaml`.

- [ ] **Step 1: Classify each file**

```bash
rg -n 'research-index' \
  docs/src/architecture docs/src/reference docs/src/contributors docs/news \
  --glob '!docs/src/archive/**'
```

Expected: each hit maps to A, B, C, or D. If a hit is a filename-only mention inside a table of “files we will edit,” convert it to Class B/C.

- [ ] **Step 2: Apply the class replacement in each file**

No placeholders. If a sentence only exists to point at the index, delete the sentence.

Mesh plans that say “do not edit research-index because it is generated” are **wrong today**; use Class C so a future agent does not revive a generator.

- [ ] **Step 3: Confirm live-tree grep is clean except github-pages-redirects and the CLI ratchet string**

```bash
rg -n 'research-index' \
  docs/src AGENTS.md CLAUDE.md GEMINI.md crates docs/news docs/agents \
  --glob '!docs/src/archive/**' --glob '!docs/agents/doc-inventory.json'
```

Allowed remaining hits:

- `docs/src/architecture/github-pages-redirects.md` (documents the stub filenames)
- `crates/vox-cli/src/commands/ci/run_body_helpers/docs.rs` (the recreation error)
- `crates/vox-integration-tests/tests/research_index_retirement_test.rs`
- `crates/vox-skills/skills/superpowers/deep-research.skill.md` only if it says “retired”
- This plan file, once it lives under `docs/superpowers/plans/`

- [ ] **Step 4: Commit**

```bash
git add docs/src/architecture docs/src/reference docs/src/contributors docs/news
git commit -m "$(cat <<'EOF'
docs: retarget remaining live research-index companions

EOF
)"
```

---

### Task 7: Historical plans and specs (do not execute them)

**Files (instruction rewrite only):**

- `docs/superpowers/plans/2026-09-05-true-workflow-durability.md`
- `docs/superpowers/plans/2026-07-23-docs-homepage-maintainability.md`
- `docs/superpowers/plans/2026-08-09-vox-syntax-optimization-program.md`
- `docs/superpowers/plans/2026-07-08-axis-workbench-tabs.md`
- `docs/superpowers/plans/2026-06-18-graphify-run-lifecycle.md`
- `docs/superpowers/plans/2026-06-13-history-driven-token-savings.md`
- `docs/superpowers/plans/2026-06-29-main-green-pass-completion.md`
- `docs/superpowers/plans/2026-06-02-vox-golden-corpus-and-compiler-reality.md`
- `docs/superpowers/plans/2026-09-01-vox-efficacy-benchmark-and-leaderboard.md`
- `docs/superpowers/plans/handoff/2026-05-12-claude-dashboard-ingestion-plan.md`
- `docs/superpowers/plans/tooling/2026-05-09-astro-migration-and-doc-cleanup.md`
- `docs/superpowers/specs/2026-08-22-docs-corpus-repair-design.md`

**Interfaces:**
- Consumes: Class B replacement from Task 6.
- Produces: an agent executing an old plan will not recreate the index.

- [ ] **Step 1: Rewrite each “add a row / update research-index.md” step**

Use this block in place of the old git-add-index steps:

```md
Set valid frontmatter on the new page (`title`, `description`, `category`, `status`).
Starlight lists it. Do **not** create or edit `docs/src/architecture/research-index.md` (retired 2026-09).
```

If the old step also `git add docs/src/architecture/research-index.md`, delete that path from the `git add` list.

- [ ] **Step 2: Leave historical narrative that describes 2026-05 behavior**

In `2026-06-13-history-driven-token-savings.md`, the paragraph that records “verification found research-index is hand-curated” is historical evidence. Prefix that paragraph with `**(Historical, 2026-06: superseded 2026-09 — research-index retired.)**` and do not delete the measurement.

- [ ] **Step 3: Confirm no plan still `git add`s the live index**

```bash
rg -n 'architecture/research-index.md' docs/superpowers
```

Allowed: this retirement plan; historical notes marked superseded; the durability plan after its hunk is rewritten.

- [ ] **Step 4: Commit**

```bash
git add docs/superpowers
git commit -m "$(cat <<'EOF'
docs: stop historical plans from recreating research-index.md

EOF
)"
```

---

### Task 8: Inventory, lint, and merge to main

**Files:**
- Modify: `docs/agents/doc-inventory.json` (regenerated)
- Test: commands below

**Interfaces:**
- Consumes: Tasks 1–7 tree.
- Produces: green local docs gates; PR onto `main`.

- [ ] **Step 1: Regenerate doc-inventory**

```bash
cargo run -q -p vox-cli -- ci doc-inventory generate
cargo run -q -p vox-cli -- ci doc-inventory verify
```

Expected: `docs/src/architecture/research-index.md` row gone; archive snapshot present. Commit the JSON if verify reports drift.

- [ ] **Step 2: Frontmatter + links + scoped lint**

```bash
cargo run -p vox-doc-pipeline -- --lint-only --paths docs/src/archive/research-index-hand-curated-retired-2026-09.md
cargo run -q -p vox-cli -- ci check-links
cargo test -p vox-integration-tests --test research_index_retirement_test --test speech_audit_contract_test
```

Expected: all PASS. If check-links fails, the leftover path is a Task 6 miss — fix it in this task, do not weaken the allowlist.

- [ ] **Step 3: Final live-tree grep**

```bash
rg -n 'research-index' \
  --glob '!docs/src/archive/**' \
  --glob '!docs/superpowers/plans/2026-09-06-research-index-obsolete.md' \
  --glob '!docs/agents/doc-inventory.json' \
  --glob '!target/**'
```

Expected: only the allowed Task 6 remainder (redirects doc, CLI ratchet, tests, “retired” skill line).

- [ ] **Step 4: Fast pre-push on the isolated clone**

```bash
vox ci pre-push --complete --since origin/main
```

Expected: green for the touched crates/docs. If clippy fires on `docs.rs`, fix it here (`unused_mut` / rustfmt via `cargo fmt -p vox-cli`).

- [ ] **Step 5: Open the PR from the isolated clone**

```bash
git push -u origin HEAD
gh pr create --title "docs: retire research-index.md; sidebar is the browse surface" --body "$(cat <<'EOF'
## Summary
- Archive the hand-curated `docs/src/architecture/research-index.md` and stop treating it as a completeness gate.
- Discoverability is frontmatter + Starlight `sidebar.mjs`. Old URLs redirect to contributor-hub.
- Tests and `vox ci` fail if the live path is recreated. Live policy/skills no longer instruct an index edit.

## Test plan
- [ ] `cargo test -p vox-integration-tests --test research_index_retirement_test --test speech_audit_contract_test`
- [ ] `cargo run -q -p vox-cli -- ci check-links`
- [ ] `cargo run -q -p vox-cli -- ci doc-inventory verify`
- [ ] Confirm `/architecture/research-index/` stub redirects (local `pnpm --dir docs-astro build` + open the stub HTML)
- [ ] `rg research-index docs/src AGENTS.md crates --glob '!docs/src/archive/**'` shows only retirement/redirect hits

EOF
)"
```

- [ ] **Step 6: High review, then merge to main when fail is empty**

Review the full branch range (`git diff origin/main...HEAD`). Fix Critical/Important findings in a new commit (do not `--amend` after push). Merge only when `gh pr checks` fail bucket is empty. Admin-merge is allowed under the standing empty-fail policy; prefer a normal merge if the required hosted checks have completed.

If `#539` is still open, rebase it and delete any `research-index.md` hunk.

---

## Self-review

**Spec coverage**

- Obsolete live file → Task 3 delete + Task 1/4 ratchets.
- Fix all live references → Tasks 2, 5, 6.
- Fully archive → Task 3 tombstone; Q1 archive left untouched.
- Full cleanup / no replacement index → Global Constraints + no generator.
- Land on main → Task 8.
- Keep merging other passing PRs → Global Constraints (out of this file’s task checkboxes; do it in parallel, do not block Task 1 on `#539`).

**Placeholder scan:** none. Every step has the replacement text or the exact command.

**Type consistency:** `workspace_root()` copied (not shared). Banned strings in Task 1 match the Task 2 deletions. Redirect target is `/contributors/contributor-hub/` everywhere (stubs, `_redirects`, tombstone).
