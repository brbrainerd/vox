---
title: "Codebase Cleanup & Signal Improvement Plan (2026-05-02)"
description: "Six-phase plan to retire stale code, fix broken references, and converge on single sources of truth."
category: "Architecture SSOTs"
status: roadmap
last_updated: "2026-05-08"
training_eligible: false
audience: contributors
---

# Codebase Cleanup & Signal Improvement Plan

> **Partially superseded 2026-05-08.** Phase 5 (`@island` cleanup) is now consolidated under [2026-05-08-llm-misleading-content-cleanup-plan](2026-05-08-llm-misleading-content-cleanup-plan.md). Phases 1–4 and 6 remain independently actionable as written.

> **For agentic workers:** Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Retire stale code, fix broken references, and converge on single sources of truth so future AI tool calls find higher-signal context.

**Architecture:** Five-phase sequence ordered by risk/payoff. Each phase produces independent, committable work. Phase 1 is mechanical (no behavioral risk). Phase 5 is parked behind upstream work (`@island` retirement, blocked on Phase 5 of the interop plan).

**Tech Stack:** Rust workspace + TypeScript packages + mdBook docs + `vox` CLI for codegen and ignore-file sync.

---

## Source Findings

This plan was synthesized from a five-agent codebase audit on 2026-05-02. Each finding below was verified against the working tree before being included.

**Verified facts referenced by tasks:**
- Files referenced from `AGENTS.md`, `CHANGELOG.md`, the PR template, and `infra/coolify/README.md` as living under `docs/src/architecture/` actually live under `docs/src/archive/research-2026-q1/`. They were archived; the back-references were not updated.
- `.cursorignore`, `.aiignore`, `.aiexclude` have a corrupted middle comment block (lines ~6–11 are stub `#` fragments). The source file `.voxignore` is intact. The generator stripped some lines and left fragments.
- `infra/containers/entrypoints/vox-entrypoint.sh` and `populi-entrypoint.sh` are dead — `.vox` equivalents exist and are wired into the Dockerfiles.
- 4 golden examples still contain `routes { ... }` blocks (`blog_fullstack.vox`, `dashboard_ui.vox`, `v0_shadcn_island.vox`, `web_routing_fullstack.vox`).
- `crates/vox-compiler/src/typeck/mod.rs:22` has an orphan one-line tombstone comment.
- As of 2026-05-03, `@island` was retired. The infrastructure cleanup is tracked in [2026-05-08-llm-misleading-content-cleanup-plan](2026-05-08-llm-misleading-content-cleanup-plan.md).

**Findings explicitly excluded from this plan:**
- The four overlapping orchestrator architecture docs — verified to have intentional non-overlapping scopes with cross-references. Not duplication.
- AGENTS.md / GEMINI.md / CLAUDE.md layering — verified to be properly deferred per `agent-instruction-architecture.md`.
- `BASELINE_VERSION` parity — already enforced by `vox ci check-codex-ssot`.

---

## Phase 1: Fix broken cross-references (mechanical, no behavior change)

These are pointers from authoritative surfaces (`AGENTS.md`, `CHANGELOG`, PR template, infra README) into `docs/src/architecture/` that should resolve to `docs/src/archive/research-2026-q1/`. Each is a one-line edit.

### Task 1.1: Update `AGENTS.md` archived-doc back-references

**Files:**
- Modify: `AGENTS.md` (lines 60, 64, 65, 66, 124 per audit; verify at edit time)

**Affected references (all redirect to `docs/src/archive/research-2026-q1/`):**

| Old path | New path |
|---|---|
| `docs/src/architecture/multi-repo-context-isolation-research-2026.md` | `docs/src/archive/research-2026-q1/multi-repo-context-isolation-research-2026.md` |
| `docs/src/architecture/telemetry-remote-sink-spec.md` | `docs/src/archive/research-2026-q1/telemetry-remote-sink-spec.md` |
| `docs/src/architecture/telemetry-unification-research-findings-2026.md` | `docs/src/archive/research-2026-q1/telemetry-unification-research-findings-2026.md` |
| `docs/src/architecture/telemetry-implementation-blueprint-2026.md` | `docs/src/archive/research-2026-q1/telemetry-implementation-blueprint-2026.md` |
| `docs/src/architecture/telemetry-implementation-backlog-2026.md` | `docs/src/archive/research-2026-q1/telemetry-implementation-backlog-2026.md` |
| `docs/src/architecture/vox-as-glue-research-2026.md` | `docs/src/archive/research-2026-q1/vox-as-glue-research-2026.md` |

**Decision required before editing:** AGENTS.md is supposed to point at *currently authoritative* docs. If a referenced doc has been archived, the right move may be (a) update the link to the archive path with an "(archived)" annotation, or (b) replace it with the current canonical doc, or (c) drop the reference entirely. Default to (a) for "research findings" docs (the research is frozen but useful as evidence) and (b) for telemetry-implementation-* (point at `adr/023-optional-telemetry-remote-upload.md` and `architecture/telemetry-trust-ssot.md` instead).

- [ ] **Step 1: Confirm the canonical-doc decision per row above with the maintainer.** This is a content judgment, not a mechanical edit.
- [ ] **Step 2: Re-grep to lock current line numbers.** Run: `grep -n "telemetry-implementation\|telemetry-unification\|telemetry-remote-sink\|multi-repo-context-isolation\|vox-as-glue-research" AGENTS.md`. Use returned line numbers, not the audit's line numbers (file may have shifted).
- [ ] **Step 3: Apply edits.** Use `Edit` tool with sufficient context to disambiguate.
- [ ] **Step 4: Verify all updated paths resolve.** Run: `for p in <new paths>; do test -f "$p" && echo OK $p || echo MISSING $p; done`.
- [ ] **Step 5: Commit.**

```bash
git add AGENTS.md
git commit -m "docs(agents): repair archived-doc back-references"
```

### Task 1.2: Update `CHANGELOG.md` archived-doc reference

**Files:** Modify: `CHANGELOG.md:52` (verify line number).

- [ ] **Step 1:** `grep -n "terminal-exec-policy-research-findings-2026" CHANGELOG.md`
- [ ] **Step 2:** Replace with `docs/src/archive/research-2026-q1/terminal-exec-policy-research-findings-2026.md` (or with `docs/src/architecture/terminal-exec-policy-ssot.md` if pointing at the active SSOT is preferred for a release-notes context — confirm with maintainer).
- [ ] **Step 3:** Verify path resolves.
- [ ] **Step 4: Commit.** `git commit -m "docs(changelog): repair archived-doc reference in v0.5.0 notes"`

### Task 1.3: Update PR template reference

**Files:** Modify: `.github/PULL_REQUEST_TEMPLATE.md:7`.

- [ ] **Step 1:** `grep -n "agent-event-kind-ludus-matrix" .github/PULL_REQUEST_TEMPLATE.md`
- [ ] **Step 2:** The PR-template purpose is "remind contributors to update X when Y" — if X has been archived (frozen), the reminder is no longer actionable. Either (a) point to the archive path (and keep reminding), or (b) drop the line if that surface is now governed by a different artifact. Confirm with maintainer.
- [ ] **Step 3:** Apply edit.
- [ ] **Step 4: Commit.** `git commit -m "ci(pr-template): repair archived-doc reference"`

### Task 1.4: Update `infra/coolify/README.md` references

**Files:** Modify: `infra/coolify/README.md:12, 25, 33`.

**Affected references:**
- `../../docs/src/architecture/deployment-compose-ssot.md` → `../../docs/src/archive/research-2026-q1/deployment-compose-ssot.md`
- `../../docs/src/architecture/codex-baas.md` → `../../docs/src/archive/research-2026-q1/codex-baas.md`

- [ ] **Step 1:** `grep -n "deployment-compose-ssot\|codex-baas" infra/coolify/README.md`
- [ ] **Step 2:** **Sanity-check first:** if the Coolify deploy guide depends on guidance that is now *archived*, that may indicate the deploy guide itself is stale, not just the link. Skim the content of both archived docs and confirm with maintainer whether `infra/coolify/README.md` should be (a) updated to archive paths, (b) rewritten against current architecture, or (c) marked stale. Default action: (a) plus a note at the top of `infra/coolify/README.md` saying "Architecture references in this doc point to archived research; verify against current state before deploying."
- [ ] **Step 3:** Apply edits.
- [ ] **Step 4: Commit.** `git commit -m "docs(infra/coolify): repair archived-doc references"`

### Task 1.5: Resolve `contracts/README.md` script references

**Files:** Modify: `contracts/README.md:5, 19`.

**Issue:** `contracts/README.md` references `scripts/check_codex_ssot.sh` and `scripts/check_codex_ssot.ps1`, neither of which exists in the working tree.

- [ ] **Step 1: Find the actual guard.** Run: `grep -rn "check.codex.ssot\|check_codex_ssot" --include='*.vox' --include='*.rs' --include='*.toml' --include='*.yaml' --include='*.md'`. The likely current implementation is a `vox ci ...` subcommand (per audit, the project uses `vox ci command-sync`, `vox ci check-codex-ssot`).
- [ ] **Step 2:** If the guard is a `vox ci` subcommand, replace both shell-script references in `contracts/README.md` with `vox ci check-codex-ssot` (or actual command). This also satisfies the VoxScript-First policy.
- [ ] **Step 3:** If no guard exists, surface this — the README claims a guard, the guard does not exist, and CI may have silently lost coverage. Flag for maintainer; do not invent a replacement.
- [ ] **Step 4: Commit.** `git commit -m "docs(contracts): point at vox ci guard instead of missing shell scripts"`

### Task 1.6: Add a CI guard against future archived-link rot

**Files:** Add or modify a CI step (likely under `.github/workflows/` or a `vox ci` subcommand).

- [ ] **Step 1: Investigate what link-checking already exists.** Run: `grep -rn "link.*check\|check.*link\|markdown.*link" .github/ scripts/ crates/vox-cli/ 2>/dev/null`. If there's already a doc link checker, ensure it exits non-zero on broken refs and runs on `AGENTS.md`, `CHANGELOG.md`, `*.md` at root, and `infra/**/*.md`. If none exists, the lift to add one is significant; flag as a separate task and do not implement here.
- [ ] **Step 2: Document outcome in this plan as a follow-up if a new tool is needed; otherwise wire up coverage of the four files Phase 1 just fixed.**
- [ ] **Step 3: Commit if changes were made.** `git commit -m "ci: extend doc link guard to AGENTS.md and root-level docs"`

---

## Phase 2: Regenerate corrupted derived ignore files

The middle comment block of `.cursorignore`, `.aiignore`, and `.aiexclude` has been mangled — fragments like `# .aiexclude directly. Edit this file, then run:` and stranded `#` lines. The source `.voxignore` is intact.

### Task 2.1: Run the regenerator and verify output

**Files:**
- Read-only source: `.voxignore`
- Regenerated: `.cursorignore`, `.aiignore`, `.aiexclude`

- [ ] **Step 1: Stash any local changes to derived files** so the regen output isn't conflated with prior state.
  ```bash
  git stash push -- .cursorignore .aiignore .aiexclude
  ```
- [ ] **Step 2: Run the regenerator.**
  ```bash
  vox ci sync-ignore-files
  ```
  Expected: command exits 0; the three files are rewritten.
- [ ] **Step 3: Diff to confirm corruption is repaired.**
  ```bash
  git diff -- .cursorignore .aiignore .aiexclude | head -80
  ```
  Expected: the `# .aiexclude directly. Edit this file, then run:` fragment is gone, replaced by the full instruction block from `.voxignore`. Stranded `#` lines are gone.
- [ ] **Step 4: If the diff shows the corruption is *still present after regen*, the bug is in the generator, not in stale output.** In that case, locate the generator (likely `crates/vox-cli/src/commands/ci/sync_ignore_files.rs` or similar — `grep -rn "sync.ignore.files\|sync_ignore_files" crates/`), patch the comment-handling logic, add a regression test, and rerun. Treat this as a sub-task; do not commit a no-op regen.
- [ ] **Step 5:** Drop the stash if it was empty, otherwise inspect what was stashed. `git stash drop` only after confirming.
- [ ] **Step 6: Commit.**
  ```bash
  git add .cursorignore .aiignore .aiexclude
  git commit -m "chore: regenerate ignore files (repair mangled headers)"
  ```

### Task 2.2: Document the three undocumented generators

**Files referenced by audit (no documented source/generator):**
- `contracts/capability/model-manifest.generated.json`
- `contracts/scientia/social-execution-board.generated.yaml`
- `apps/editor/vox-vscode/src/core/mcpToolRegistry.generated.ts`

- [ ] **Step 1: For each, find the generator.** Run: `grep -rn "model-manifest.generated\|social-execution-board.generated\|mcpToolRegistry.generated" --include='*.rs' --include='*.vox' --include='*.mjs' --include='*.ts' --include='*.toml' --include='*.json'`. The vscode one is almost certainly `apps/editor/vox-vscode/scripts/generate-mcp-tool-registry.mjs` per Phase 5 audit. The contracts ones are unknown.
- [ ] **Step 2: For each generator found, ensure the *generated* file has a header comment block stating: source file(s), generator command, and a `DO NOT EDIT` notice.** If the file already has this, skip. If not, the fix is in the *generator*, not the output (so the next regen re-emits it correctly).
- [ ] **Step 3: For any file with no findable generator,** flag as a separate investigation task. Do not delete; the file may be checked-in golden output of a one-shot script no longer in the tree, or be a stranded artifact.
- [ ] **Step 4: Commit.** `git commit -m "chore: document provenance for *.generated.* files"`

---

## Phase 3: Delete shadowed glue scripts

`infra/containers/entrypoints/vox-entrypoint.sh` and `populi-entrypoint.sh` are confirmed dead — the corresponding `.vox` files exist and are referenced from the Dockerfiles (`Dockerfile:33,44` and `Dockerfile.populi:65,78` per audit). Verify before deleting.

### Task 3.1: Verify shadowing and delete

**Files:**
- Delete: `infra/containers/entrypoints/vox-entrypoint.sh`
- Delete: `infra/containers/entrypoints/populi-entrypoint.sh`

- [ ] **Step 1: Confirm the Dockerfiles reference the `.vox` versions, not the `.sh`.**
  ```bash
  grep -n "entrypoint" infra/containers/Dockerfile infra/containers/Dockerfile.populi
  ```
  Expected: results reference `vox-entrypoint.vox` and `populi-entrypoint.vox` only.
- [ ] **Step 2: Confirm nothing else references the `.sh` files.**
  ```bash
  grep -rn "vox-entrypoint.sh\|populi-entrypoint.sh" --exclude-dir=node_modules --exclude-dir=target
  ```
  If anything outside the `.sh` files themselves references them, stop and surface to maintainer.
- [ ] **Step 3: Delete.**
  ```bash
  git rm infra/containers/entrypoints/vox-entrypoint.sh infra/containers/entrypoints/populi-entrypoint.sh
  ```
- [ ] **Step 4: Re-run Step 2 to confirm no dangling references.**
- [ ] **Step 5: Commit.**
  ```bash
  git commit -m "chore(infra): drop shadowed shell entrypoints (.vox versions live)"
  ```

### Task 3.2: Update populi reference doc

**Files:** Modify: `docs/src/reference/populi.md:172`.

- [ ] **Step 1:** `grep -n "vox-entrypoint" docs/src/reference/populi.md`
- [ ] **Step 2:** If the reference points at the now-deleted `.sh` file, update it to point at `vox-entrypoint.vox`. If the reference is illustrative pseudocode, leave it but update the filename.
- [ ] **Step 3: Commit.** `git commit -m "docs(populi): update entrypoint reference to .vox"`

---

## Phase 4: Mechanical retired-flag cleanup

These are tiny edits with limited blast radius. Each can ship independently.

### Task 4.1: Remove the orphan tombstone comment in typeck

**Files:** Modify: `crates/vox-compiler/src/typeck/mod.rs:22` (verify).

The line `// DEPRECATED typecheck_module (AST path) removed in Wave 1.` is a tombstone for code that's already gone. It adds noise without telling future readers anything actionable.

- [ ] **Step 1:** `grep -n "DEPRECATED typecheck_module" crates/vox-compiler/src/typeck/mod.rs`
- [ ] **Step 2:** Confirm there is no surrounding code that depends on this comment for context (it should be stranded).
- [ ] **Step 3: Delete the line.**
- [ ] **Step 4: Build to confirm nothing broke.** `cargo build -p vox-compiler`
- [ ] **Step 5: Commit.** `git commit -m "chore(typeck): drop tombstone comment for removed AST path"`

### Task 4.2: Audit `legacy_direct` flag usage

**Files:**
- Read: `crates/vox-cli/src/cli_args.rs:442-444`
- Read: `crates/vox-cli/src/commands/generate.rs` (audit cited this file)

- [ ] **Step 1: Map all uses of the flag.**
  ```bash
  grep -rn "legacy_direct\|legacy-direct" crates/ docs/
  ```
- [ ] **Step 2: Determine status.** Recent commit `994fa8e7` added a bail when `--server-url` is used without `--legacy-direct`. That suggests the flag is in *deprecation* state, not yet retired. Confirm with maintainer:
  - Is `--legacy-direct` a planned-removal flag? If yes, what's the timeline?
  - Is the dashboard/generate path that requires it scheduled for removal, or staying as an optional fallback indefinitely?
- [ ] **Step 3:** If timeline is "soon," add a `#[deprecated]` annotation and a deprecation note in `docs/src/reference/cli.md`. If indefinite, document it as supported and remove the deprecation framing.
- [ ] **Step 4: Commit.** `git commit -m "chore(cli): clarify legacy_direct deprecation status"`

### Task 4.3: Audit stale `routes { ... }` in golden examples

**Files:**
- `examples/golden/blog_fullstack.vox:37`
- `examples/golden/dashboard_ui.vox` (header comment)
- `examples/golden/v0_shadcn_island.vox:22` (also flagged for Phase 5)
- `examples/golden/web_routing_fullstack.vox:42`

The `path-b-decommission-2026.md` (cited by audit) claims these were scrubbed; they were not. Determine whether `routes { ... }` syntax is (a) actively supported, (b) deprecated but accepted, or (c) compiler-rejected.

- [ ] **Step 1: Test compile each golden.**
  ```bash
  for f in blog_fullstack dashboard_ui web_routing_fullstack; do
    cargo run -p vox-cli -- check examples/golden/$f.vox || echo FAIL $f
  done
  ```
  (Skip `v0_shadcn_island.vox` — it's part of Phase 5 retirement.)
- [ ] **Step 2: If they compile, the syntax is alive — leave the goldens. If they fail with a "removed syntax" error, the goldens are testing dead behavior and need to be updated to current routing syntax.**
- [ ] **Step 3:** If goldens need updates, the rewrite is a content task per file (not mechanical) — surface to maintainer rather than guessing at intended replacement.
- [ ] **Step 4: Commit only if goldens were updated.** `git commit -m "chore(examples): update goldens to current routing syntax"`

---

## Phase 5: Document `@island` retirement work (DO NOT DELETE YET)

Per `docs/src/architecture/external-frontend-interop-plan-2026.md`, the `@island` directive is retired *in plan* but the implementation phase (Phase 5 of the interop plan) has not landed. The audit identified the following surfaces tied to `@island`:

- `crates/vox-cli/src/commands/island/` (entire directory: `generate`, `upgrade`, `list`, `cache`, `build`)
- `crates/vox-codegen-ts/src/island_emit.rs`
- `crates/vox-cli/src/templates/islands.rs`
- `examples/golden/v0_shadcn_island.vox`
- `docs/src/how-to/how-to-islands-and-pages.md`
- `islands/` Vite app at repo root (referenced by `templates/islands.rs`)

**Do not delete in this plan.** This is active code on a known decommission timeline. Premature deletion will block in-progress work and confuse anyone reading interop history.

### Task 5.1: Add a tracking checklist to the Phase 5 spec

**Files:**
- Modify: `docs/src/architecture/external-frontend-interop-plan-2026.md` (or its Phase-5-specific companion `phase5-react-interop-spec-2026.md` if that's where decommission tracking lives — verify).

- [ ] **Step 1: Find the Phase 5 doc.** `ls docs/src/architecture/ | grep -i "phase5\|phase-5\|external-frontend-interop"`
- [ ] **Step 2: Locate the existing "Retire @island" section.** If it has a file-by-file checklist, no edit needed — verify the six surfaces above are listed. If not, add a "Retirement checklist" subsection enumerating exactly these six paths plus any others uncovered.
- [ ] **Step 3:** Cross-reference: ensure `docs/src/how-to/how-to-islands-and-pages.md` carries a top-of-file banner stating "This documents a feature that will be retired in Phase 5; see <link>" — if not, add it.
- [ ] **Step 4: Commit.** `git commit -m "docs(interop): add @island retirement checklist for Phase 5"`

### Task 5.2: Fix internally-contradictory `@island` how-to

**Files:** Modify: `docs/src/how-to/how-to-islands-and-pages.md`.

The audit found this file says `@island` was "removed in v0.3" but then proceeds to document its use. The contradiction is high-noise for AI tools.

- [ ] **Step 1: Read the full file.** Determine whether the contradictory line is a stale paragraph, a typo (e.g. should reference a different decorator), or an authoring artifact.
- [ ] **Step 2: Either remove the false claim, fix the typo, or rewrite the doc as a "deprecated as of Phase 5; here's the migration target" guide.** Choose with maintainer input.
- [ ] **Step 3: Commit.** `git commit -m "docs(how-to): resolve contradiction in @island guide"`

### Task 5.3: Confirm archived react-interop research is properly fenced

The audit identified ~9 archived react-interop docs under `docs/src/archive/research-2026-q1/`. They are correctly archived but `.voxignore`/`.aiignore` should be excluding the archive directory from AI ingestion (per `AGENTS.md §Archival Protocol`).

- [ ] **Step 1: Confirm exclusion is in place.**
  ```bash
  grep -n "archive" .voxignore
  ```
  Expected: a rule excluding `docs/src/archive/**` or similar.
- [ ] **Step 2: If not excluded, add the rule to `.voxignore` (the SSOT) and re-run `vox ci sync-ignore-files`.** Do not edit derived files directly (per project memory rule).
- [ ] **Step 3: Commit.** `git commit -m "chore: ensure archived research is fenced from AI context"` — only if a change was needed.

---

## Phase 6 (deferred): Items requiring deeper investigation

These came up in the audit but need maintainer judgment or larger scope before committing to action. Listed for traceability; **not** action items in this plan.

1. **`crates/vox-orchestrator/src/dei_shim/`** — DEPRECATED-marked shim. Audit could not determine if callers still exist. Owner: orchestrator track.
2. **`VOX_MENS_EXPERIMENTAL_OPTIMIZER`** — experimental feature flag in `vox-men`. Decide: productionize, gate more strictly, or remove. Owner: mens track.
3. **`HISTORICAL_ALLOWLIST` for retired orchestrator / ARS codenames** — grandfathered labels being migrated to `vox-orchestrator` / OpenClaw runtime paths. Migration in progress; remove allowlist entries as renames complete. Owner: rename-tracking task.
4. **293 files in `docs/src/archive/research-2026-q1/`** — fenced from AI context (Phase 5.3 above) but still indexed by full-text search and visible to humans browsing. Question for maintainer: move deeper (e.g. to `.git/archive/` orphan branch) or accept the noise floor.
5. **Naming-overlap of `telemetry-trust-ssot.md`** (active stub, 21 lines) vs `archive/.../telemetry-trust-ssot.md` (frozen 81-line research). Same filename, different scope. Renaming the archive file would break archive integrity (frozen-by-convention); renaming the active file weakens the SSOT signal. Probably leave as-is, but flag.

---

## Self-review

**Spec coverage:** All five audit slices have at least one phase. Retired code → Phase 5 + Phase 4. Duplicated SoT → mostly excluded (project layering is sound) but Phase 1 covers the broken back-references that originated from archive moves. Broken refs → Phase 1. Auto-gen drift → Phase 2. Non-Vox glue → Phase 3.

**Placeholder scan:** No "TODO/TBD/implement later" steps. Several tasks contain explicit "confirm with maintainer" gates — these are intentional decision points, not placeholders, since the audit cannot decide content questions (e.g. whether an archived-doc back-reference should redirect to the archive or get replaced by a current canonical).

**Type/path consistency:** All file paths in tasks were verified against the working tree at audit time (2026-05-02). Re-grep at task start in case of drift.

**Risk ranking:** Phase 1 (low risk, high signal payoff) → Phase 2 (mechanical, but Task 2.1 has a fork if the bug is in the generator) → Phase 3 (file deletes, irreversible — verify shadowing twice) → Phase 4 (small targeted edits) → Phase 5 (documentation only; no code deletion until Phase 5 of interop plan ships).

**Generator hand-off:** Adding this plan file to `docs/src/architecture/` will make `SUMMARY.md` and `architecture-index.md` stale on next build. Run `cargo run -p vox-doc-pipeline` (or the project's equivalent) before merging to refresh both. **Do not hand-edit those files** — they are auto-generated.
