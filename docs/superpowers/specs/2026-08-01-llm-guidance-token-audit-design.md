---
title: LLM guidance file token audit and consolidation
description: "Audit of AGENTS.md, CLAUDE.md, GEMINI.md, copilot-instructions.md, nested AGENTS.md overlays, and .cursor/rules/*.mdc for token bloat, redundancy, and staleness, plus a consolidation plan to cut standing token cost and prevent recurring mistakes."
category: "Architecture SSOTs"
status: "approved"
training_eligible: false
---

# LLM guidance file token audit and consolidation — design

## Context

Vox's coding-agent guidance is spread across `AGENTS.md` (root, cross-tool SSOT), `CLAUDE.md`, `GEMINI.md`, `.github/copilot-instructions.md`, three nested `AGENTS.md` overlays (`docs/agents/`, `docs/src/`, `apps/vox-mental-tracker/`), the meta-doc `docs/src/contributors/agent-instruction-architecture.md`, and nine `.cursor/rules/*.mdc` files. This is already a deliberately layered design (base policy → tool overlay → continuation prompt → CI enforcement, per `agent-instruction-architecture.md`), not an accidental sprawl — but it has grown to 556 lines / ~5950 words in `AGENTS.md` alone, and two research passes (one auditing the repo directly, one surveying current best practices) turned up concrete, fixable waste and drift on top of that base cost.

**Local audit findings** (full detail in the session; summarized here):
- **4 stale references**: `crates/vox-secrets/src/spec.rs` (now a `spec/` module, cited in ~12 files), the orchestrator `TOOL_REGISTRY` path (moved to `vox-orchestrator-mcp/src/registry.rs`), `scripts/quality/toestub_scoped.sh` (retired in favor of `vox ci toestub-scoped`, but still the instruction agents are given), and `AGENTS.md`'s own claim of "four `.mdc` rule files" (there are nine, seven always-on).
- **4 duplicated tables/rules** with no drift yet but no single source of truth: the secrets rule (6 files), the VoxScript-first execution-tier table (4 files), the retired-crate table (3 files), and the 500-line god-object threshold (7 files).
- **A recurring mistake class not yet in AGENTS.md's "Perennial Bug Patterns"**: fmt drift left behind by parallel/concurrent agent sessions (8 occurrences across history).
- **7 always-on `.cursor/rules/*.mdc` files** (~1350 words) impose a standing per-session tax on Cursor, much of it re-deriving tables that already live in `AGENTS.md`.

**Best-practice research findings** (Anthropic docs, the AGENTS.md open standard, Chroma's context-rot study, HumanLayer):
- Anthropic's own guidance targets under ~200 lines for a memory file; shorter files measurably improve instruction adherence independent of the model's context window size, and Claude Code now supports a native `@AGENTS.md` import so `CLAUDE.md` can defer to it instead of restating "read AGENTS.md first" as unenforced prose.
- HumanLayer's litmus test: *if a violation would fail CI, the rule belongs in CI, not prose.* Several of this repo's duplicated tables are exactly this shape.
- The delete-and-test heuristic (remove a rule, see if agent behavior changes; if not, it was pure token cost) is the recommended way to find prose that no longer earns its place.
- Context-rot research shows degradation is non-uniform and worsened by "distractor" content — topically related but non-load-bearing text — which argues for trimming rationale/history paragraphs down to the minimum that still changes behavior, and keeping the highest-priority rules near the top or bottom of the file rather than buried in the middle.
- Prompt caching rewards stable, front-loaded static content; volatile info should stay out of the always-loaded block.

## Goals

1. Fix every confirmed stale reference so agents stop being told to use paths/commands/counts that no longer exist.
2. Eliminate standing duplication: each rule has exactly one owning file; every other mention is a one-line pointer.
3. Shrink `AGENTS.md`'s non-load-bearing prose (rationale, history, reference tables agents rarely act on directly) without touching the sections that prevent invalid Vox code or hallucinated APIs (`§Vox Language Enforcement Rules`, `§Retired Surfaces`, `§Grammar Unification`'s decorator rules) — those stay as-is per the research finding that dense-but-load-bearing content is exactly what should NOT be cut.
4. Reduce the always-on `.cursor/rules/*.mdc` token tax by converting the duplicative always-on files to cross-references.
5. Add one missing entry to `AGENTS.md`'s Perennial Bug Patterns (parallel-agent fmt drift).
6. Make `CLAUDE.md` use Claude Code's native `@AGENTS.md` import instead of unenforced prose.
7. Add a small CI drift-guard so the "AGENTS.md cites a stale file count/path" class of mistake can't silently recur, closing the loop instead of producing another one-time fix.

## Non-goals

- Not rewriting the layering architecture itself (`agent-instruction-architecture.md`'s model is sound and stays).
- Not touching `docs/src/contributors/continuation-prompt-engineering.md` — already reviewed and confirmed well-scoped (it explicitly documents what NOT to duplicate).
- Not building a general doc-linter; the CI drift-guard is scoped to the specific stale-count/path classes found in this audit, not a speculative framework.
- Not changing enforcement behavior (CI gates, hooks, detectors) beyond the one new drift-guard — this is a docs-content pass.

## Design

### 1. Stale-reference fixes

Grep-and-fix pass across all files citing the four stale targets. Concretely:
- `crates/vox-secrets/src/spec.rs` → `crates/vox-secrets/src/spec/` (or the specific submodule if a file cites a symbol, e.g. `spec/ids.rs`, `spec/types.rs`, `spec/registry/*.rs`) — `AGENTS.md`, `.cursor/rules/secrets-policy.mdc`, and the ~10 other docs the audit agent found.
- Orchestrator `TOOL_REGISTRY` path → `crates/vox-orchestrator-mcp/src/registry.rs` — `docs/agents/orchestrator.md`, `docs/agents/governance.md`, `agent-time-awareness.md` (or wherever that reference actually lives — verify path at implementation time).
- `scripts/quality/toestub_scoped.sh` → `vox ci toestub-scoped [ROOT]` — `docs/agents/governance.md`, `docs/agents/orchestrator.md`, `docs/agents/cli-toolchain.md`, `docs/src/reference/cli.md`.
- `AGENTS.md`:276 "four `.mdc` rule files" → "nine `.mdc` rule files" (or drop the count entirely and just say "several `.mdc` rule files" to avoid re-staling on the next file added — prefer this phrasing since the count is exactly what drifted).

Each fix is verified by grep for the old string returning zero hits post-edit (excluding this spec doc and archive/changelog references, which are historical and correctly left alone).

### 2. Redundancy consolidation

For each of the four duplicated rule sets, keep the fullest/most authoritative copy and reduce every other occurrence to a one-line cross-reference:

| Rule | Keep full copy in | Reduce to pointer in |
|---|---|---|
| Secrets (`vox_secrets::resolve_secret`, never raw env reads) | `AGENTS.md` §Secret Management | `.cursor/rules/secrets-policy.mdc`, `.cursor/rules/data-storage-policy.mdc`, `.cursor/rules/voxscript-first-automation.mdc`, `toestub-contributor-guide.md` |
| VoxScript-first execution tiers | `AGENTS.md` §VoxScript-First Glue Code | `GEMINI.md` §VoxScript-First Glue, `.cursor/rules/voxscript-first-automation.mdc` |
| Retired-crate table | `AGENTS.md` §Retired Surfaces | `.cursor/rules/retired-surfaces.mdc`, `coding-agents.md` |
| God-object 500-line threshold | `docs/agents/governance.md` (the multi-tier 300/400/500 SSOT) | `coding-agents.md`, `contributor-hub.md`, `toestub-contributor-guide.md`, `docs/agents/AGENTS.md`, `cli-toolchain.md` |

A "pointer" is a single sentence, e.g. `> Secrets: see AGENTS.md §Secret Management — never read secrets from env vars directly; use \`vox_secrets::resolve_secret(...)\`.` — short enough to still carry the rule inline (so a tool that only loads the overlay isn't blind to it) while eliminating the maintenance burden of a second full copy.

### 3. Perennial Bug Patterns addition

Add one bullet to `AGENTS.md` §Perennial Bug Patterns, matching the existing style:

> **Parallel-agent fmt drift.** When multiple agents/worktrees touch overlapping crates concurrently, `rustfmt` drift from one session's edits routinely lands unformatted in another's commit. Before merging work from parallel sessions, run `vox run scripts/fmt.vox` (or `VOX_FMT_CHECK=1 vox run scripts/fmt.vox` to check only).

### 4. CLAUDE.md import syntax

Replace the current "This project uses `AGENTS.md` ... (required reading first)" prose with Claude Code's native `@AGENTS.md` import directive at the top of the file, keeping the existing Claude-specific additions below it unchanged. Verify via Claude Code's memory docs that the import syntax is exactly `@AGENTS.md` (relative to the file it's imported from) before landing — confirm current syntax at implementation time rather than trusting this spec's paraphrase.

### 5. AGENTS.md trim

Apply the delete-and-test / push-rationale-out heuristic to four identified sections. Each cut moves detail to an already-linked doc rather than deleting information outright:

- **§Versioning Policy** (~25 lines) → collapse to 2-3 lines: version lives in `Cargo.toml [workspace.package]`, don't hand-bump PATCH or maintain a separate doc version, full scheme detail moves to (or is confirmed already present in) a linked reference doc.
- **§Local CI Gate Tiers table** (7 rows) → keep only `fast` (default/hook) and `full` (the two tiers agents actually invoke day-to-day) inline; the other 5 rows move to the already-linked `docs/src/contributors/local-ci-pre-push.md` (confirm they're already there; add if missing rather than deleting the only copy).
- **§PR & Review Discipline** (~25 lines) → keep the 4 actionable rules (batch commits, push once when ready, `@coderabbitai review` for re-review, keep early pushes as Draft); cut the rate-limit-number rationale paragraph to one clause.
- **§Grammar Unification** implementation-status paragraph → keep the current rule (decorators vs. bare keywords), cut the ADR-028-superseded-by-ADR-041 historical narrative to a single link.

Sections explicitly OUT of scope for trimming (confirmed load-bearing, dense-but-necessary per the research's own "don't cut this kind of content" finding): §Vox Language Enforcement Rules, §Retired Surfaces, §Grammar Unification's decorator/keyword table itself, §Model-Agnostic LLM Boundary, §Test-First Policy.

Target: a meaningfully shorter file (rough goal: 15-20% word reduction from trimming alone, before counting the redundancy-consolidation savings in §2) — not a hard line-count target, since some sections are correctly dense.

### 6. `.cursor/rules/*.mdc` restructure

Of the 7 always-on files, shrink `secrets-policy.mdc`, `retired-surfaces.mdc`, `voxscript-first-automation.mdc`, and `data-storage-policy.mdc` to short cross-references into `AGENTS.md` (same pointer pattern as §2), rather than restating full tables. `build-environment.mdc`, `ci-runner-convention.mdc`, and `cross-platform-source-hygiene.mdc` are left as-is (not found to duplicate AGENTS.md content in the audit — verify at implementation time; only shrink files confirmed duplicative). Glob-scoped files (`cli-command-registry.mdc`, `documentation-policy.mdc`) are untouched — they already only load when relevant.

### 7. CI drift-guard

Add a small, scoped check (not a general doc-linter) that catches the specific class of drift found in §1's "four vs. nine `.mdc` files" — e.g. a step in the existing docs-reality-audit tooling (or a new small check alongside it) that counts `.cursor/rules/*.mdc` and fails if a hardcoded count elsewhere in the docs tree doesn't match. Implementation approach (which existing gate to extend vs. a new standalone check) is decided during planning — this spec commits to the check existing and being scoped narrowly, not to a specific implementation mechanism.

## Verification

- `grep` for each of the 4 stale strings (§1) across the repo returns zero hits post-fix (excluding intentionally historical references, e.g. changelogs).
- Word/line count of `AGENTS.md` before vs. after, reported in the plan's final summary.
- Every cross-reference pointer added in §2/§6 resolves to a section that still exists *and actually contains the rule/distinction being pointed at* (no dangling `see AGENTS.md §X` after a trim renames or removes `§X`, and no pointer to a section that exists but never states the specific claim the pointer promises — e.g. a "see §Retired Surfaces for the current distinction" pointer requires that the distinction actually be written there, not just that the section header exist).
- `vox ci pre-push --complete` (or the relevant doc-lint scope) passes — frontmatter, doc-lint, and any new CI drift-guard all green.
- Manual read-through of the trimmed `AGENTS.md` confirms no rule was silently dropped (each trim in §5 is a *move*, not a *deletion*, of the detailed content) and no contradiction was introduced between the shortened prose and the linked detail doc.

## Risks

- **Over-trimming**: a rule that looks like restatable rationale might actually be load-bearing for a tool that doesn't follow links well (e.g. a lightweight overlay-only agent that never opens the linked doc). Mitigated by keeping the *rule* inline as a one-line pointer everywhere it's referenced (§2), only cutting the *rationale/history*, never the actionable instruction itself.
- **Cross-tool drift during the edit**: touching 15+ files in one pass risks leaving one inconsistent. Mitigated by doing the grep-verify step in Verification for every stale reference and every new pointer.
- **`@AGENTS.md` import syntax assumption**: this spec's §4 describes the mechanism based on research; the plan must confirm the exact current syntax against Claude Code's docs before landing, not trust this paraphrase.
