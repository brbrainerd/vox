---
title: "Docs Corpus Repair and Visual Enablement"
description: "Evidence-backed repair of the documentation corpus: retired-surface drift, detector holes, retirement/archival, and mermaid rendering. No new subsystems."
category: "architecture"
status: "roadmap"
---

# Docs Corpus Repair and Visual Enablement — Design

**Date:** 2026-08-22
**Status:** approved for planning
**Scope decision:** content-first. No new subsystems, no new crates, no new schema, no new scheduler, no new GUI surface.

> **Why this file lives in `docs/superpowers/specs/`.** That path is outside
> `docs/src/`, so it is not walked by the doc-pipeline lint, not built into the
> Astro site, and — critically — it is exempt from `vox ci retired-symbol-check`
> via `is_historical_or_audit_doc()`. This document names retired symbols
> extensively by necessity; authoring it under `docs/src/` would trip the very
> detector it proposes to fix.

---

## 1. Origin and method

The request was to audit the codebase against all documentation and surface
dozens of high-value improvements, adding visuals, improving navigability, and
retiring stale documents.

An initial design proposed a nine-component auto-maintenance program. Eight
parallel critique tracks were run against the codebase to test it. **The design
did not survive.** This spec is what replaced it, and the reasoning is recorded
here because the negative results are more valuable than the original plan.

### 1.1 What the critique found

**The proposed machinery was ~90% redundant.** Eight of nine components already
exist in the tree:

| Proposed component | Already exists as |
| --- | --- |
| Proposal queue with accept/reject | `crates/vox-gui/src/commands/harness_issues.rs` — propose, `build_unified_diff`, `resolve_harness_fix_proposal`, staleness detection, path-traversal guard, verbatim-write invariant test |
| LLM drafts fixes from findings | `vox audit effort-route` (routes audit findings to verified proposals) |
| Wiki browse surface | `DocViewerDrawer` + `DocReader` + `omnibarFacets.ts` docs facet + `vox_docs_index` |
| Wiki index | Pagefind (`pagefind: true`) and `starlight-llms-txt` (`llmsFullTxt`) |
| Toasts on new work | `ui/Toasts.tsx` + `lib/toastQueue.ts` (group-key coalescing, overflow folding) |
| Human-review queue | `NeedsYou` surface + `useAttentionInbox` + `feedbackResolve` |
| Doc-vs-code drift backlog | `vox ci docs-reality-audit` + three schemas + taxonomy YAML |
| Retirement scanning | `vox ci retired-symbol-check`, `vox ci retirement-audit`, 7 `vox-code-audit` detectors |

Net new GUI surfaces required: **zero**.

**The backlog it would have fed is a documented graveyard.** 23 of 33
directories under `contracts/reports/` are stale by three weeks or more, 13 by
roughly three months, with a signature three-day burst on 2026-05-21/22/23
followed by silence across twelve of them. `findings.v1.json` has **one commit
in its entire history** (`3295a3bee`, 2026-05-12) and contains zero findings.
Meanwhile `docs` is the third-largest commit prefix across the last 400 commits.

The conclusion is not subtle: **this repository has a findings-consumption
deficit, not a findings-generation deficit.** Building more generation machinery
would produce a 34th stale directory.

### 1.2 What the critique also found

The opposite error was equally available. An early reading was that the corpus
needed only three to five line edits. That was wrong too, because the real
damage was hiding *behind* the existing detector's exemptions. A naive grep for
17 retired symbols produced 398 line-hits of which **384 were legitimate and 14
genuinely stale — 96.5% false positive**. The symbols that mattered were the
ones not being counted at all.

**This is the load-bearing insight of the whole audit:** the existing detector
has accreted so many hand-written carve-outs that it now suppresses real drift,
and the largest retirement classes in the corpus have no contract coverage at
all. The fix is not more machinery. It is repairing the corpus and closing the
holes that let it rot silently.

---

## 2. Goals and non-goals

### Goals

1. Repair documentation that actively misleads coding agents — highest value
   first, because every agent session pays the cost.
2. Close the detector holes that allowed the drift, so the repair does not
   need repeating.
3. Enable diagram rendering, which is broken today, without adding a
   build-time browser dependency or degrading agent-facing output.
4. Archive superseded documents with their inbound links rewritten and their
   training eligibility correctly set.
5. Fix the small correctness bugs found in the audit machinery itself, so its
   numbers stop lying.

### Non-goals

- No new crate, no new GUI surface, no new findings schema, no new scheduler.
- No LLM judge tier. (The `vox-cli-ci -> vox-actor-runtime` edge required for
  one has been authorized by the maintainer but is **deliberately not taken**,
  because this scope does not need it. It is banked for a follow-up.)
- No inventory expansion to ~150 claims. The existing 10 claims have produced
  zero findings in 102 days; scaling a zero-yield mechanism fifteen-fold yields
  zero fifteen times over.
- No modification of `docs/src/archive/**` content beyond frontmatter required
  by CI, per AGENTS.md §Archival Protocol.

---

## 3. Evidence base

Every claim below was verified against the tree at commit `693c012db`. Counts
that were corrected during the audit are shown with the correction, because the
first number was wrong in each case and the pattern matters.

| Claim | First estimate | Verified | Source of error |
| --- | --- | --- | --- |
| Docs with mermaid fences | 45 | **17 live** (28 archive, 2 outside content root) | Included tombstoned archive |
| Broken mermaid diagrams | unknown | **3 of 51** (1 live) | Parsed all 51 with real `mermaid@11` under jsdom |
| Retired-symbol references | 119 file-hits | **14 genuine** of 398 line-hits | 96.5% false positive; double-counted files across symbols |
| Refs to nonexistent crates | not counted | **~340** (`vox-dashboard` 299, `vox-dei-shim` 26, `vox-oratio` ~15) | Not searched for |
| Ungated retired decorators | not counted | **~500** | No contract coverage |
| Retirement candidates | ~150 by shape | **54 with evidence** | Shape-based rule had load-bearing false positives |
| Inbound links to rewrite | unknown | **83 edges** for 150 files, ~40 for 54 | — |

### 3.1 Diagram validity (all 51 fences parsed, not sampled)

48 valid, 3 broken:

| File | Error | Cause |
| --- | --- | --- |
| `docs/src/how-to/how-to-rust-crate-imports.md` **(live)** | Lexical error line 2 | Backticks *inside* a quoted label; mermaid requires them wrapping the whole label. All 6 nodes affected. |
| `docs/src/archive/.../web-architecture-analysis-2026.md` | Expecting 'SQE', got 'STR' | Unbalanced quote: opens with an unquoted bracket, closes with a quoted one |
| `docs/src/archive/.../toestub-self-healing-architecture-2026.md` | Expecting ..., got 'GRAPH' | Node id `graph` is a reserved flowchart keyword |

---

## 4. Workstreams

Five workstreams, ordered by value per unit of effort. Each is independently
shippable; W1 is worth doing even if nothing else lands.

### W1 — Policy and reference correctness (highest value)

These mislead every agent in every session. They are one-line fixes.

**W1.1 — `AGENTS.md` contradicts itself, and the wrong half is instruction.**
§Retired Surfaces gives the replacement for the removed `@endpoint(kind: ...)`
decorator as the at-prefixed `server` / `query` / `mutation` function forms.
§Grammar Unification states that the at-prefixed forms of
`table`/`query`/`mutation`/`server`/`tool`/`resource`/`form`/`index` became
**hard parse errors on 2026-06-30** (`cd7cc96874`), replaced by bare-keyword
forms. The always-loaded policy file therefore instructs every agent to emit
code that cannot parse. Fix: the replacement column must give the bare-keyword
forms, consistent with §Grammar Unification.

**W1.2 — Three renamed or deleted crates are absent from the retired table.**
Add rows for `crates/vox-dashboard` (does not exist), `crates/vox-oratio` ->
`crates/vox-speech` (note: the `vox oratio` **CLI command** still exists, so
only the crate path is stale), and `vox-dei-shim` -> `vox-research-shim`.

**W1.3 — The governance `category` vocabulary does not overlap the enforced
one.** `documentation-governance.md` §Category vocabulary lists slugs
(`architecture`, `how-to`, `reference`); `VALID_CATEGORIES` in
`crates/vox-doc-pipeline/src/pipeline/lint.rs:18` requires display labels
("Architecture SSOTs", "Language Reference"). The documented vocabulary is
simply wrong. Fix the doc to match the lint, which is the SSOT AGENTS.md points
at.

**W1.4 — `astro.config.mjs` comment is false.** It says the sidebar is
generated from `SUMMARY.md`; `sidebar.mjs` derives it from frontmatter
(`category`/`sort_order`/`title`) with section order from
`contracts/documentation/docs-sidebar-section-order.v1.json`.

**W1.5 — `infer_with_retry`'s doc comment is false.** It claims the function
"skips specific candidates on 401s or continues on 429/timeout". The body does
no error classification whatsoever: every failure class takes an identical
branch and advances to the next candidate, with no retry and no backoff.
`retry_after` is captured at `crates/vox-llm-egress/src/wire.rs:179` and
discarded. Fix the comment to describe actual behavior. (Fixing the *behavior*
is out of scope but should be filed.)

**W1.6 — `docs-reality-audit-program.md` documents a cadence that has never
run.** It declares weekly/monthly/release-gate cycles; zero of roughly 14
weekly cycles occurred. Either mark the program dormant or state the real
cadence. Documentation that describes an unperformed ritual as current practice
is the exact class this audit exists to catch.

**W1.7 — Three ADRs share number 037.** `037-ai-fixture-subagent-decorator.md`,
`037-tauri-convergence.md`, `037-tauri-gui-replaces-axum-dashboard.md`.
Renumber two; update `docs/src/adr/index.md`.

**W1.8 — `data-storage-lint-and-ci-spec-2026.md` contains 516 embedded NUL
bytes.** `grep` classifies the file as binary and silently skips it, so every
grep-based audit — including the ones that produced this spec's first draft —
has a blind spot there. The Rust detectors read it fine, so this is a hazard for
humans and agents, not for CI. Strip the NULs.

### W2 — Retired-surface corpus repair

**W2.1 — The 14 verified stale references.** Listed with file:line in the
implementation plan. The worst is `docs/src/architecture/wire-format-v1-ssot.md`
— a **live SSOT** specifying the v1 wire format entirely in the removed
`@endpoint(kind: ...)` spelling. Two of its six occurrences sit inside `vox`
fences marked `// vox:skip`, so the doctest gate never sees them either. It
passed both gates by construction.

**W2.2 — `crates/vox-dashboard`: 299 references to a crate that does not
exist.** Sixteen non-exempt live files, including five ADRs and two reference
docs that name it as the canonical implementation target
(`reference/frontend-surface-ownership.md:18` — "implement under
`crates/vox-dashboard` first"; `adr/024-dashboard-axum-spa.md:25` — "the
canonical home for the orchestration UI"). Per the retired-surfaces table,
Axum-based dashboards were superseded by the Tauri 2 GUI. ADRs get a
supersession note in place (see W5.3); reference docs get corrected.

**W2.3 — `vox-dei-shim`: 26 references, and the detector actively hides them.**
`retired_symbol_check.rs` contains a carve-out suppressing any `vox-dei` line
containing `vox-dei-shim`, written to protect a crate that was subsequently
renamed to `vox-research-shim`. The exemption now conceals the stale references
it was meant to permit. Remove the carve-out and fix the references.

**W2.4 — `crates/vox-oratio` -> `crates/vox-speech`.** Notably
`docs/src/ci/workspace-root-manifest.md:22` uses it as *the worked example* of
adding a workspace dependency.

**W2.5 — ~500 retired decorator spellings, zero contract coverage.** Counts by
spelling: `query` 126, `table` 91, `mutation` 87, `server` 74, `form` 28,
`resource` 8, `index` 7, `tool` 3 (all in their retired at-prefixed form), plus
`@mcp.tool` 40 (soft-deprecated, warning only) and `@v0` 39 (parses but lowers
to a no-op, slated for retirement). This is a retirement class larger than the
entire 17-symbol list that started the audit. W1.1 is a hard prerequisite:
fixing the corpus while the policy file still prescribes the broken form
guarantees reintroduction.

**W2.6 — `VoxPlayground.astro` ships retired syntax on the landing page.** Its
static demo uses the retired at-prefixed `table` and `endpoint(kind: query)`
forms. This is user-facing on `/` and is the first Vox code a visitor reads.

### W3 — Close the detector holes

Repair without this is repair that must be repeated.

**W3.1 — Contract coverage.** Eight of the 17 audited symbols are absent from
`contracts/documentation/retired-symbols.v1.yaml` entirely: `vox-ludus`,
`@endpoint`, `@py.import`, `@native`, `@capacitor`, `axum::serve`,
`rust-embed`, `vox-sherpa-transcribe`. Add these plus the three crates from
W1.2 and the decorator class from W2.5.

**W3.2 — Precision rules.** Naive matching yields ~90 findings at ~16%
precision, which is why the detector accreted carve-outs. Four rules bring it
to ~14 findings at ~86%:

- **R1 — same-line replacement suppression.** Suppress when the line also
  contains the canonical replacement token. Already implemented for
  `sync-recall-api`; generalize to all entries. A migration table row *must*
  name both sides. Precision ~100%, recall cost ~0.
- **R2 — retirement-vocabulary proximity.** Extend the existing literal
  `DEPRECATED|Historical note|ARCHIVED` test to a case-insensitive match on
  retired / deprecated / removed / formerly / renamed / no longer / replaced by
  / legacy / superseded, plus one line of leading context for headings.
- **R3 — doc-class gate replaces the filename-suffix gate.** The 13-entry
  `HISTORY_SUFFIXES` list is unmaintainable and already leaks. Use the
  frontmatter that is already mandatory.
- **R5 — `vox:skip` must not exempt retired decorators.** A `vox` fence marked
  `// vox:skip` escapes the doctest gate; it must not also escape the retirement
  gate. This is exactly how `wire-format-v1-ssot.md` survived both.

**R6** (dropping the `skip_md_table_rows` asymmetry between policy roots and
`docs/`) becomes safe once R1 lands, and removes the need for the bespoke
`lookup_fact_by_key` carve-out.

**W3.3 — R4: path-existence detection is a better detector than symbol
matching.** Any documentation reference naming a `crates/<name>` or
`apps/<path>` directory, or a `-p <crate>` argument, where the path does not
exist on disk. No allowlist to maintain, no vocabulary heuristics, and it caught
4 of the 14 true positives plus all ~340 nonexistent-crate references. Expected
precision ~85%; the failure mode is design docs proposing not-yet-existing
crates, which R3 handles.

**W3.4 — `cli-command-surface.generated.md` is not a usable oracle.** It lists
285 operations but omits real top-level commands that exist in
`crates/vox-cli/src/lib.rs` — `vox plugin`, `vox new`, `vox workflow`,
`vox repair`, `vox gui`, `vox chat`. A generated SSOT with holes is worse than
none, because downstream checks trust it. File as a generator bug.

**W3.5 — `vox ci check-links` does not scan `CLAUDE.md` or `GEMINI.md`.** Both
are policy roots that link into `docs/`. A stale link there passes the merge
gate and only surfaces in the nightly lychee run. Add them to the source set.

### W4 — Visual enablement

**W4.1 — Approach: client-side `astro-mermaid@2.1.0`, placed before
`starlight()` in `integrations`.** Rejected alternatives, with reasons:

- `rehype-mermaid@3` and `@beoe/rehype-mermaid` declare a `playwright` peer
  dependency via `mermaid-isomorphic`. That puts a **Chromium download on the
  docs build path**, including the self-hosted Linux fleet, which has no browser
  provisioning today.
- `starlight-client-mermaid` **does not exist**. The nearest package is
  `@pasqal-io/starlight-client-mermaid@0.1.0`, single release, last published
  2025-02-27. Abandonware.
- **The decisive argument is agent-facing output.** `starlight-llms-txt@0.8.1`
  renders each entry to HTML and converts back via
  `rehypeParse -> rehypeRemark -> remarkStringify`; `rawContent` is not set in
  this config. Client-side rendering emits a `pre.mermaid` element containing
  the diagram source, which round-trips back into a fenced code block, so
  `llms-full.txt` keeps the source. Build-time SVG **deletes the diagram content
  from agent-facing output** — a direct regression against a stated project
  constraint.

Secondary advantages: fails soft (a parse error renders an error box rather
than failing `pnpm build` and both deploy jobs); dark mode is free because its
client script keys on Starlight's own `data-theme` attribute; and it
lazy-imports mermaid only when a `pre.mermaid` element is present.

**W4.2 — Ordering is not negotiable.** Fix the one live broken diagram
(W4.4) *before* enabling the renderer. `astro-expressive-code` registers on
`markdown.rehypePlugins`, so a remark-stage transform necessarily wins — this
is proven by `remark-vox-include`, which already mutates code nodes before
expressive-code highlights them. But if ordering ever resolves the other way,
the symptom is a **silent no-op** (a normal-looking code block), so verify
visually after wiring rather than assuming.

**W4.3 — The parse-check gate must land before the ASCII conversion, not
after.** This is the single highest-value item in the visual workstream. The
existing 51 fences are 94% valid; 54 hand-authored ASCII-to-mermaid conversions
will not be, and client-side rendering **fails silently in CI** — nothing
catches a broken diagram until a human looks at the page. The gate is a
`mermaid.parse()` harness run over every fence. Note for whoever builds it:
jsdom is required, or DOMPurify fails to initialise and every block spuriously
fails with a `DOMPurify.sanitize is not a function` error.

**W4.4 — Fix the live broken diagram** in `how-to-rust-crate-imports.md`. The
two archive diagrams are out of scope per §Archival Protocol; they render
noindexed pages.

**W4.5 — `--frozen-lockfile` is used in all four CI install steps.** Adding
`astro-mermaid` without committing a regenerated `docs-astro/pnpm-lock.yaml` in
the **same commit** fails CI immediately. `docs-astro/node_modules` does not
exist locally, so a real `pnpm install` is required.

**W4.6 — `docs/design-system/` is 0 of 8 specs implemented.** Claimed: a React
landing page, a `/concepts/` route, a `/showcase/` route, a shadcn HSL token
map, generated imagery, five components. Shipped: `VoxPlayground.astro` alone,
hand-written as a vanilla custom element. `docs-astro/package.json` contains no
`react`, no `tailwindcss`, no `shadcn` — the kit's entire target runtime is
absent. The README points at an `integration-notes.md` that does not exist.
These files carry no frontmatter (they sit outside `docs/src/`). Either mark
them `status: roadmap` or archive them; do not let "author new diagrams" quietly
become a ninth unbuilt spec.

**W4.7 — Accessibility.** Rendering replaces readable preformatted text with an
SVG. Authored diagrams should carry `accTitle:` and `accDescr:` directives.

### W5 — Retirement and archival

**W5.1 — 54 evidenced candidates**, tiered: three already carrying
`status: "deprecated"` with zero inbound links; twelve point-in-time snapshots
with zero inbound, several of which say so in their own body text ("Superseded
by §14", "superseded by vox-capability-registry", "Shipped"); the remainder
findings/handoff/plan documents with zero or one inbound link.

**W5.2 — Load-bearing documents that a shape-based rule would destroy.**
`vox-as-llm-target-audit-and-plan-2026.md` matches the audit-and-plan filename
shape but is **opened by path** at `crates/vox-arch-check/src/main.rs:825` for
the evidence ledger; archiving it breaks `arch-check`. Likewise
`v1-release-criteria.md` (`main.rs:280,292`) and `where-things-live.md`
(`cache.rs:26,76,81`). These require an explicit blocklist, not a heuristic.

**W5.3 — All 46 ADRs are excluded.** An ADR is an append-only decision record;
a superseded one gets `status: "deprecated"` plus a supersession note **in
place**. `adr/012-internal-web-ir-strategy.md` has 17 inbound links — archiving
it would be actively wrong.

**W5.4 — Archiving does not remove a document from the MENS training corpus.**
Corpus mode emits `docs/src/corpus.jsonl` from an unfiltered `docs/src` walk. If
retirement is meant to stop feeding superseded designs to the model, the file
move accomplishes nothing on its own — `training_eligible: false` is the lever,
and CI already requires it (plus `archived_date:`) on every archived file, via
`check_archival_pipeline`, which runs inside `ssot-drift` on the fast pre-push
tier. Omitting either key is a hard failure.

**W5.5 — Two of five exclusion surfaces do not exclude archive.** The Astro
sidebar, `llms.txt` (via the docs loader's `archive/**` exclusion), and
`.voxignore` all exclude it correctly. **`vox-doc-inventory` does not** —
`SKIP_DIR_NAMES` omits `archive`, and `docs/agents/doc-inventory.json` already
contains 303 archive references. **The doc-pipeline lint does not either** —
archive files are still fully linted, with only two exemptions.

**W5.6 — `superseded_by` will be silently invisible unless added in two
places.** The doc-pipeline lint has no unknown-key rejection, so it passes. But
`docs-astro/src/content.config.ts` extends the Starlight schema with an explicit
Zod key list, and Zod **strips** unknown keys — the field does nothing on the
site until it is declared there as an optional string. Add it unenforced first;
add a `LintKind` variant only if drift appears.

**W5.7 — `docs/superpowers/` needs a different rule.** 284 plans and 147 specs
sit outside `docs/src/`, so they are unlinted, unbuilt, and absent from the
corpus — but **not** in `.voxignore`, so agents ingest all 431. Only 3 of 284
plans are marked completed, and the `status` vocabulary there is uncontrolled
free text ("green-after-remediation", "two-strike-stop"). Forcing them through
the `docs/src/` frontmatter regime is roughly 400 files of busywork for zero
enforcement gain. The correct fix is a `docs/superpowers/plans/archive/` bucket
plus one `.voxignore` line.

**W5.8 — Move procedure.** Batches of at most 25 files into
`docs/src/archive/architecture-2026-q3/`, matching the existing
`research-2026-q1/` precedent. `git mv` (preserves history, which the pipeline
reads for `last_updated`); set `status: "deprecated"`, `archived_date:`,
`training_eligible: false`, and `superseded_by:`; leave `category` unchanged,
matching the 243 existing archive files. Rewrite the ~40 inbound edges — and
**grep `CLAUDE.md` and `GEMINI.md` by hand**, since W3.5 is not yet fixed when
this runs. Hand-edit `research-index.md` (hand-curated despite the name); do
**not** touch `architecture-index.md` or `SUMMARY.md`, both gitignored and
regenerated. Move `architecture/planning-meta/` as a whole directory or not at
all — it is a coherent self-referential methodology set with its own index.

**Links *from* moved files are already safe:** `check_links.rs:318` skips any
path with an `archive` component as a *source*, so relative-depth changes inside
moved files are never checked.

### W6 — Audit-machine correctness

Small fixes so the existing program's numbers stop lying. No new capability.

**W6.1 — `rollout_milestone_pct` is a constant.** The body returns 25 when
findings are empty, else 25 plus a closed-ratio share of the remaining 75. The
current value of 25 means "inventory exists, no findings". Filing 80 open
findings leaves it at **exactly 25**. The program's one headline number cannot
distinguish "not started" from "full backlog".

**W6.2 — Metrics have no regenerator and no drift gate.** `run_verify`
schema-checks `metrics.v1.json` but never recomputes it, and nothing in CI or
`lefthook.yml` runs `metrics --write`. A 150-claim inventory alongside
`inventory_claim_count: 10` is a **green build**. This is precisely how the file
reached 2026-05-12 unnoticed. Fix: have `run_verify` recompute and diff, reusing
`run_metrics`' body.

**W6.3 — Findings paths are never existence-checked.** Inventory `doc_path`,
contracts, and globs are hard-checked, but findings' `doc_paths`, `code_paths`
and `contract_paths` are not even deserialized into `FindingRow`. A finding may
cite files that do not exist and CI stays green — the exact drift class the
program exists to catch.

**W6.4 — Duplicate finding ids are not rejected.** Claim ids are deduped;
findings are not, and the schema lacks `uniqueItems`.

**W6.5 — Band thresholds are duplicated** between the program YAML (22/14/0)
and `priority_band_from_score`. They agree today; nothing enforces that they
continue to.

**W6.6 — `glob_match_count` has no early exit.** It materializes every match
into a vector, uncached, once per claim. The seed inventory already contains a
`crates/**` pattern (5,206 files). This runs inside `ssot-drift` on the fast
pre-push tier's 60-second budget. Short-circuit at the first match.

---

## 4a. Decisions taken

| Decision | Choice | Consequence |
| --- | --- | --- |
| Backlog home | Fill the existing `docs-reality-audit` contracts | No new store; `verify` already gates it |
| Provenance | Human-grade findings only; **no schema change** | Every filed finding is evidence-backed and hand-verified. A future generated tier files to a separate artifact rather than polluting a trusted backlog. |
| Retirement | Tombstone in place, never delete | `archive/` is already excluded from sidebar and llms.txt; W5.4/W5.5 close the remaining gaps |
| Visual | Renderer plus authored diagrams | Client-side only, gated by W4.3 |
| LLM judge tier | **Deferred.** Edge authorized but not taken | No `vox-cli-ci -> vox-actor-runtime` edge is added by this work |

### Why the LLM tier is deferred rather than rejected

The original framing asked for auto-updating docs via openrouter/local/fallback
models with "no single point of failure". The audit established that this cannot
honestly be claimed of the current stack:

- `chat_once` has zero retry; `infer_with_retry` does not retry, it iterates
  once per candidate with no error classification and no backoff.
- The activity retry loop is dead code for LLM errors, because `llm_chat`
  returns a success-wrapped error on provider failure, so `execute_activity`
  sees success. `ActivityOptions::default()` sets zero retries regardless.
- `FallbackCondition::ProviderUnavailable` is read from the environment
  variable `VOX_PROVIDER_UNAVAILABLE` before dispatch. Selection runs once,
  before any HTTP call; a live 503 never feeds back. There is no circuit
  breaker.
- "Local is always available" is a hardcoded true in
  `models/key_guard.rs:58-62` with no liveness probe, deliberately asserted by
  a test. The selector will choose a local model on a machine where Ollama was
  never installed.
- MENS contributes zero models — `MensCatalog::refresh` enumerates a
  `mens/runs/` tree, which does not exist in this repository.
- **`durable_scheduler` has no runner.** No `impl DurableJobStore` exists
  anywhere in the workspace; its own module doc defers implementations to "a
  follow-up PR". It is a library of pure scheduling arithmetic, not
  infrastructure. The real out-of-process template is
  `.github/workflows/harness-eval-nightly.yml`.

A resilient version is achievable, but only with the deterministic tier as the
**primary** path and the LLM tier as strictly optional enrichment — the
inventory's `evidence_hints.code_globs` can be checked for existence and
staleness with no model at all. Then being offline degrades output *quality*,
not availability. That inversion is a design in its own right and is not part of
this scope.

---

## 5. Sequencing

Ordering constraints are real, not stylistic:

1. **W1.1 before W2.5.** Fixing ~500 decorator references while the policy file
   still prescribes the broken form guarantees reintroduction.
2. **W3 before W2 completes.** Landing the precision rules and contract
   entries first means the corpus repair is verified by a gate rather than by
   inspection.
3. **W4.4 before W4.1.** Fix the live broken diagram before enabling the
   renderer.
4. **W4.3 before W4's conversion work.** The parse gate must exist before 54
   hand-authored diagrams are written against a renderer that fails silently.
5. **W5.5/W5.6 before W5.8.** Close the inventory/lint exclusion gaps and add
   the Zod key before moving files, or the moves are invisible where it counts.
6. **W6 is independent** and may land at any point.

---

## 6. Verification

Every workstream must be green on:

```text
vox ci pre-push --complete
cargo run -q -p vox-cli -- ci retired-symbol-check
cargo run -q -p vox-cli -- ci check-links
cargo run -p vox-doc-pipeline -- --lint-only
cargo run -q -p vox-arch-check
cargo run -q -p vox-cli -- ci docs-reality-audit verify
vox run scripts/fmt.vox
```

`--complete` rather than the default fast tier: fast omits clippy and all
tests, and its doc lint is scoped to changed paths only, while this work touches
the whole `docs/src/` tree.

W4 additionally requires `pnpm install` in `docs-astro/` with the regenerated
lockfile committed in the same commit, and a visual check of one diagram page in
both themes.

Per AGENTS.md, all glue scripts introduced by this work are `.vox` run via
`vox run` — no `.ps1`, `.sh`, or `.py`. Any new public function gets its failing
test first. `cargo fmt --all` is never used on this workspace.

---

## 7. Risks

| Risk | Mitigation |
| --- | --- |
| ASCII-to-mermaid conversion produces broken diagrams that fail silently | W4.3 parse gate lands first; this is the highest-value single item |
| Archive move breaks inbound links | ~40 edges enumerated in the plan; `check-links` is the gate; `CLAUDE.md`/`GEMINI.md` grepped by hand until W3.5 |
| A shape-based retirement rule archives a load-bearing doc | Explicit blocklist (W5.2); `arch-check` catches it |
| Precision rules over-suppress and hide real drift | R1/R2/R3/R5 measured at ~86% precision on this corpus; R4 is allowlist-free and independent |
| `--frozen-lockfile` CI failure | Lockfile committed in the same commit as the dependency |
| Repair regresses because the policy file still prescribes retired forms | W1.1 sequenced first |
| This spec becomes the 34th stale report directory | It ships no recurring artifact. Every workstream is a finite repair with a CI gate that prevents recurrence. |

---

## 8. Deferred, with reasons

- **LLM judge tier** — see §4a. Edge authorized, not taken.
- **Inventory expansion to ~150 claims** — the existing 10 produced zero
  findings in 102 days. Revisit only if a manual run of the existing 10 yields
  a finding a human actually merges.
- **GUI wiki surface** — zero new surfaces needed; `DocViewerDrawer` +
  `DocReader` + the omnibar docs facet already cover browsing. Note that
  `docs/src` is **not shipped** with the GUI (`tauri.conf.json` has no
  `bundle.resources` key), so today's doc surface is a developer-in-repo feature
  that fails silently in an installed app. That is a prerequisite decision for
  any future wiki work, not a task here.
- **`infer_with_retry` behavioral fix** (as opposed to the doc fix in W1.5) —
  real, but it is an LLM-runtime change, not a docs change.
- **`cli-command-surface.generated.md` generator repair** (W3.4) — filed, but
  the generator fix belongs with its owner.
