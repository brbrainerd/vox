---
title: "Docs Corpus Repair and Visual Enablement"
description: "Evidence-backed repair of the documentation corpus: agent-facing artifact drift, retired-surface references, detector holes, retirement/archival, and mermaid rendering. No new subsystems."
category: "architecture"
status: "roadmap"
---

# Docs Corpus Repair and Visual Enablement — Design

**Date:** 2026-08-22 (revision 3)
**Status:** approved for planning
**Scope decision:** content-first. No new subsystems, no new crates, no new schema, no new scheduler, no new GUI surface.

> **Why this file lives in `docs/superpowers/specs/`.** That path is outside
> `docs/src/`, so it is not walked by the doc-pipeline lint, not built into the
> Astro site, and — critically — it is exempt from `vox ci retired-symbol-check`
> via `is_historical_or_audit_doc()`. This document names retired symbols
> extensively by necessity; authoring it under `docs/src/` would trip the very
> detector it proposes to fix.

---

## 0. Revision note — read this first

Three revisions, audited by twenty-four parallel critique tracks against the
codebase.

- **Revision 1** — 7 numeric claims wrong, one sequencing rule inverted, one
  detector diagnosis backwards.
- **Revision 2** — corrected those, then introduced **14 new errors**: one that
  changed an already-correct figure to a wrong one, three that silently
  re-included `docs/src/archive/`, and four internal contradictions.
- **Revision 3** — stops authoring numbers. See §3.

The correction *pattern* is the most useful finding in the document, and it
recurred in the very section written to announce it had been eliminated:

**Every count that included `docs/src/archive/` was inflated, and every count
independently re-verified came back smaller.**

The same pattern appeared in the plans: five separate guards were written to
catch a defect, and **five could not fire** — two read the wrong markdown
column, one skipped the rows it was written to protect, one was permanently red,
one tested a hardcoded string against a file it never read. All five were
reasoned about rather than executed.

**Two rules now bind this program:**

1. **No number is authored.** Counts come from `scripts/docs-corpus-census.vox`
   or they are not stated (§3).
2. **No checker enters a plan until it has been run against the real tree and
   its actual output pasted into the step.** Expected-failure text is not a
   prediction; it is a transcript.

---

## 1. Origin and method

The request was to audit the codebase against all documentation and surface
dozens of high-value improvements, adding visuals, improving navigability, and
retiring stale documents.

An initial design proposed a nine-component auto-maintenance program. It did not
survive review. This spec is what replaced it.

### 1.1 The proposed machinery was ~90% redundant

Eight of nine components already exist:

| Proposed component | Already exists as |
| --- | --- |
| Proposal queue with accept/reject | `crates/vox-gui/src/commands/harness_issues.rs` — propose, `build_unified_diff`, `resolve_harness_fix_proposal`, staleness detection, path-traversal guard |
| LLM drafts fixes from findings | `vox audit effort-route` |
| Wiki browse surface | `DocViewerDrawer` + `DocReader` + `omnibarFacets.ts` docs facet + `vox_docs_index` |
| Wiki index | Pagefind (`pagefind: true`) and `starlight-llms-txt` (`llmsFullTxt`) |
| Toasts on new work | `ui/Toasts.tsx` + `lib/toastQueue.ts` |
| Human-review queue | `NeedsYou` surface + `useAttentionInbox` + `feedbackResolve` |
| Doc-vs-code drift backlog | `vox ci docs-reality-audit` + three schemas + taxonomy YAML |
| Retirement scanning | `vox ci retired-symbol-check`, `vox ci retirement-audit`, 7 `vox-code-audit` detectors |

Net new GUI surfaces required: **zero**.

### 1.2 The backlog it would have fed is a graveyard

Every directory under `contracts/reports/` is stale by three weeks or more, and
most were last touched in May 2026. Reproduce with:

```bash
for d in contracts/reports/*/; do
  echo "$(git log -1 --format=%cs -- "$d")  $d"
done | sort
```

`findings.v1.json` has **exactly one commit in its history** — `3295a3bee`,
2026-05-12 — and contains zero findings (`git log --oneline -- contracts/reports/docs-reality-audit/findings.v1.json`).
Meanwhile `docs` is among the largest commit prefixes in recent history
(`git log --oneline -400 | cut -d'(' -f1 | sort | uniq -c | sort -rn`).

*(Revision 1 said "23 of 33 stale". The denominator was fabricated; every
directory is stale, so the conclusion is stronger than revision 1 stated.)*

**This repository has a findings-consumption deficit, not a findings-generation
deficit.** More generation machinery produces a 23rd stale directory.

### 1.3 But the corpus damage is real — it was just somewhere else

A naive grep for 17 retired symbols produced 398 line-hits of which **384 were
legitimate and 14 genuinely stale — 96.5% false positive**. The symbols that
mattered were not being counted at all, and the worst damage is not in `docs/`
but in the **agent-facing artifacts and the shipped CLI** (§W7).

**Load-bearing insight:** the existing detector has accreted so many hand-written
carve-outs that it now suppresses real drift, and the largest retirement classes
have no contract coverage at all.

---

## 2. Goals and non-goals

### Goals

1. Repair artifacts that actively mislead coding agents — highest value first,
   because every agent session pays the cost.
2. Close the detector holes that allowed the drift.
3. Enable diagram rendering without a build-time browser dependency or a
   regression in agent-facing output.
4. Archive superseded documents with inbound links rewritten and training
   eligibility correctly set.
5. Fix correctness bugs in the audit machinery so its numbers stop lying.

### Non-goals

- No new crate, GUI surface, findings schema, or scheduler.
- No LLM judge tier. The `vox-cli-ci -> vox-actor-runtime` edge required for one
  has been authorized by the maintainer but is **deliberately not taken**.
- No inventory expansion to ~150 claims.
- No modification of `docs/src/archive/**` beyond frontmatter CI requires.

---

## 3. Evidence base — generated, not authored

**This section deliberately contains no numbers.**

Revision 1 carried 7 wrong counts. Revision 2 corrected them and introduced
**14 more** — including one that changed an already-correct figure to a wrong
one, three that silently re-included `docs/src/archive/`, and four internal
contradictions (one section's total was arithmetically impossible against
another's). Three rounds of careful review did not converge.

The method was the problem, not the effort. Roughly forty hand-maintained
counts about a corpus that changes daily **is the drift class this spec
exists to eliminate.** The spec had become an instance of its own subject.

So every count now comes from one re-runnable command:

```bash
vox run scripts/docs-corpus-census.vox
```

**Do not quote its output here.** Cite the command. If a number matters to a
decision, run the census; if the census cannot produce it, the claim does not
belong in this spec.

### 3.1 Definitions the census applies uniformly

Ambiguity in these definitions caused most of the wrong numbers, so they are
stated once and enforced in code rather than in prose:

| Term | Definition |
| --- | --- |
| **LIVE** | `docs/src/**/*.md` excluding any path containing `/archive/` |
| **ARCHIVE** | the excluded remainder — tombstoned per AGENTS.md §Archival Protocol |
| occurrence vs file | every count is an *occurrence* count unless the label says "files" |
| **fence carrying a skip** | a `// vox:skip` marker appearing *between* a ```` ```vox ```` opener and its closer. Markers in prose are counted separately — conflating the two produced revision 2's impossible "260 markers / 257 + 77 = 334" arithmetic |
| **decorator STRICT** | `@X` immediately followed by `fn ` or `type ` — an actual declaration |
| **decorator LOOSE** | `@X` followed by any letter — includes prose such as "@form rigor" |

The two decorator definitions are **reported separately and never summed.**
Revision 2's headline figure blended them across five rows, which is why no
single regex reproduced it.

### 3.2 What the census established

Findings, stated without quoting values:

- The retired-decorator surface **shrinks at every increase in measurement
  rigour** — revision 1's estimate, revision 2's correction, and the census
  differ by more than an order of magnitude. W2.5 is correspondingly demoted;
  see §W2.5.
- Revision 2 **understated** the live `vox` fence corpus and **overstated** the
  share carrying a skip marker, because bare-skip counting was archive-inclusive.
  The real skip share is materially lower than "almost everything", which
  weakens W8's framing (see §W8).
- The `vox-dashboard`, `vox-oratio`, and `vox-dei-shim` reference counts are the
  only symbol figures that survived all three rounds unchanged.

### 3.3 A platform limit found while building the census

`vox run --interp` cannot run a corpus-wide script. `Interpreter::new(10_000_000)`
is hardcoded at `crates/vox-cli/src/commands/run.rs:63` with **no flag and no
environment override**, and one pass over the live corpus exhausts it partway
through. The census therefore requires the native tier.

This is a standing tension with AGENTS.md §VoxScript-First, which mandates that
*all* project automation be `.vox` run via `vox run`: corpus-scale automation
does not fit the interpreter tier, and the native tier's first-run compile is
slow enough to discourage the pattern. Filed here because the census is the
first script in this program to hit it.

### 3.4 Diagram validity

All live and archive mermaid fences were parsed with the real `mermaid@11`
parser under jsdom. Exactly **one live** diagram fails —
`docs/src/how-to/how-to-rust-crate-imports.md`, where backticks sit inside
quoted node labels (5 of its 7 nodes). Two archive diagrams also fail and are
out of scope. Plan 2 (`2026-08-22-mermaid-rendering-and-parse-gate.md`) turns
this check into a build-time gate; run it via that plan's harness rather than
re-deriving counts here.

## 4. Workstreams

### W1 — Policy and reference correctness

**W1.1 — `AGENTS.md` contradicts itself, and the wrong half is instruction.**
§Retired Surfaces (`AGENTS.md:454`) gives the replacement for `@endpoint` as the
at-prefixed `server`/`query`/`mutation` function forms. §Grammar Unification
(`:245-248`) states those became **hard parse errors on 2026-06-30**
(`cd7cc96874`).

**Verified from the parser, not the doc:** `descent/mod.rs:822` calls
`reject_retired_decorator(...)`, which pushes `ParseSeverity::Error`;
`parse_module` returns `Err` on any error-severity entry. Same at lines 810, 814,
828, 833, 1024, 1028, 1076. Commit `cd7cc96874` exists, dated Tue Jun 30 2026,
titled "Hard-error flip". All 79 `examples/golden/**/*.vox` use bare forms
exclusively.

The row was **correct when written** (`5ba104fde`, 2026-05-26) and has been false
for ~7 weeks. §Grammar Unification is accurate line-by-line against the parser,
including that `@mcp.tool` warns rather than errors, `@mcp.resource` is fully
valid, and `@v0` still parses.

**Three further lines prescribe the same parse-error syntax outside the table:**
`AGENTS.md:494`, `:499`, `:507`. And **`:499` names `@activity`, a decorator
that has never existed** — no lexer token, no parser arm. Any fix scoped to the
table alone leaves the most-read prescriptive text wrong.

**W1.2 — Three renamed or deleted crates absent from the retired table.**
`crates/vox-dashboard` (273 live refs), `crates/vox-oratio` → `crates/vox-speech`,
`vox-dei-shim` → `vox-research-shim`. All three replacements are **documented,
not inferred**: ADR-037 decommissions vox-dashboard in favour of `vox-gui`;
`81681e81b` and `5463bc16c` are the rename commits.

Two precision notes: `vox-dashboard` **did exist** (created ~2026-05-10, deleted
2026-05-12 in `af5f26278`) — do not write "never existed". And the canonical CLI
command is now **`vox speech`**, with `oratio` retained as a `visible_alias`.

**W1.3 — The governance `category` vocabulary does not overlap the enforced one.**
`documentation-governance.md:41-52` lists 9 slugs; `VALID_CATEGORIES`
(`lint.rs:18`) requires 13 display labels, matched exactly at `lint.rs:395` with
no alias map. **All 918 docs use display labels; zero use slugs** — nobody has
ever followed the governance doc. It is also *incomplete*, omitting `Examples`,
`Concepts`, `Operations`, and `archive`.

Two same-class defects nearby: `docs/src/adr/002-diataxis-doc-architecture.md:58`
ships the dead slug vocabulary in a yaml fence, and **`lint.rs:16-17` claims
"slug aliases are kept for grep-safety" when the array contains none.**

**W1.4 — `astro.config.mjs:27` comment is false.** The sidebar derives from
frontmatter via `sidebar.mjs`; `SUMMARY.md` is only ever *excluded*.

**W1.5 — `infer_with_retry`'s doc comment is false — and revision 1's proposed
replacement was also false.** The existing comment claims 401-skip and
429-continue; the body does no error classification at all. **But revision 1's
"a 429's `retry_after` is not honoured — nothing sleeps" is wrong at the system
level:** `wire.rs:178-182` passes it to `throttle::on_rate_limited`, which sets
`cooldown_until = now + retry_after`, and `throttle.rs:57-66` sleeps in
`acquire`. The accurate statement is that backoff is not *this function's* job.
`ActivityResult::Cancelled` also returns immediately rather than advancing.

**W1.6 — the audit program documents a cadence that has never run.** Zero of ~14
weekly cycles occurred.

**W1.7 — ADR numbering is broken in three ways.** Three files share 037. **ADR
numbers 042 and 043 are already taken** by `docs/src/architecture/adr-042-*.md`
and `adr-043-*.md` — outside `docs/src/adr/`, and ADR-042 is cited from
`layers.toml:158` and two Rust source files. So renumber targets are **044/045**,
and ADRs live in **two directories**. Third: `docs/src/adr/index.md` is missing
rows for **five** ADRs (both 037 duplicates plus 038, 039, 040). Fourth:
`docs/src/architecture/adr-NNN-scope-tauri-desktop-only.md` has a literal `NNN`
placeholder filename and falsely claims ADR-037 is "not yet filed as a doc".

Renumbering must also sweep **bare-prose citations** (`ADR-037`, `ADR 037`) in
`no_tauri_in_core.rs:1,28`, `tauri_convergence_snapshots.rs:1`,
`tauri_endpoint_client_parity_test.rs:4`, and several architecture docs.
Filename-only rewriting leaves prose citations silently pointing at the wrong
decision — worse than an obviously-ambiguous collision.

**W1.8 — one NUL byte hides a live spec from grep.**
`data-storage-lint-and-ci-spec-2026.md` contains **exactly 1** NUL, at offset
37976 — the final byte, after the last newline. Enough for `grep` to classify the
file binary and skip it silently. Only such file in the repo (1,906 markdown
files scanned).

### W2 — Retired-surface corpus repair

**W2.1 — the 14 verified stale references.** Worst is
`docs/src/architecture/wire-format-v1-ssot.md` — a live SSOT specifying the v1
wire format entirely in the removed `@endpoint(kind:)` spelling.

**W2.2 — `crates/vox-dashboard`: 273 live references.** Five ADRs and two
reference docs name it as the canonical implementation target
(`frontend-surface-ownership.md:18`, `adr/024:25`). ADRs get supersession notes
in place (W5.3); reference docs get corrected.

**W2.3 — `vox-dei-shim`: the detector actively hides it.**
`retired_symbol_check.rs:219-221` suppresses any `vox-dei` line containing
`vox-dei-shim` — a carve-out written to protect a crate that was subsequently
renamed. It now conceals the stale references it was meant to permit.

**W2.4 — `crates/vox-oratio` → `crates/vox-speech`**, notably
`docs/src/ci/workspace-root-manifest.md:22`, which uses it as *the worked
example* of adding a workspace dependency.

**W2.5 — retired decorator spellings: a handful of files.** Run the census
(§3) for the current STRICT count; it has fallen by more than an order of
magnitude at each successive increase in measurement rigour, and the LOOSE
figure is prose, not code. This is an edit set, not a workstream. Chiefly `migration-0.5-to-0.6.md:28-30`. **`CHANGELOG.md:28`
belongs here too** — it states the at-prefixed forms are "the **only** endpoint
declaration surface", and both it and the migration guide call them
"**bare-form**" while AGENTS.md uses "**bare-keyword**" to mean the opposite. An
agent reading both gets contradictory definitions of "bare".

**W2.6 — `VoxPlayground.astro` ships retired syntax on the landing page**
(`@table type Task`, `@endpoint(kind: query)`) — the first Vox code a visitor
reads.

### W3 — Close the detector holes

**W3.1 — Contract coverage.** Eight audited symbols are absent from
`retired-symbols.v1.yaml`: `vox-ludus`, `@endpoint`, `@py.import`, `@native`,
`@capacitor`, `axum::serve`, `rust-embed`, `vox-sherpa-transcribe`, plus the
three crates from W1.2.

**W3.2 — Precision rules.** Naive matching yields ~90 findings at ~16% precision.
R1 (same-line replacement suppression, already implemented for
`sync-recall-api` — generalize it), R2 (retirement-vocabulary proximity), and R3
(doc-class gate via frontmatter, replacing the leaky 13-entry `HISTORY_SUFFIXES`)
bring it to ~14 findings at ~86%. R6 (dropping the `skip_md_table_rows`
asymmetry) becomes safe once R1 lands.

**R5 is retracted.** Revision 1 claimed `vox:skip` exempts fences from the
retirement gate. It does not — `scan_source_lines` skips **all** fenced blocks
unconditionally (`retired_symbol_check.rs:161-174`). `wire-format-v1-ssot.md`
survived because `@endpoint` has no contract entry at all. Implementing R5 as
written would be a no-op.

**Instead — the root-cause fix that replaces a bespoke guard:** narrow
`ScanCfg::skip_md_table_rows` to skip only the **first cell** rather than the
whole row. That covers the replacement column of `AGENTS.md`, `CLAUDE.md`,
`GEMINI.md`, and all nine `.cursor/rules/*.mdc`, for **every** contract entry
present and future — one predicate change instead of a new hand-maintained list.

**W3.3 — R4: path-existence detection beats symbol matching.** Any doc reference
to a `crates/<name>` or `apps/<path>` that does not exist. No allowlist, no
vocabulary heuristics. **Scope is larger than revision 1 assumed: 89 distinct
nonexistent `crates/vox-*` names** are referenced in live docs, not 3 —
including `vox-stdlib`, `vox-mcp`, `vox-schema`, `vox-protocol`.

**W3.4 — `cli-command-surface.generated.md` is not a usable oracle**, and the
bug is **not** in the generator: `contracts/operations/catalog.v1.yaml` has zero
entries for `vox gui`, `chat`, `plugin`, `new`, `repair`, `workflow`. The
generator faithfully renders an incomplete input. Separately,
`docs/src/reference/cli.md` omits ~30 real top-level commands.

**W3.5 — `check-links` misses two policy roots.** `check_links.rs:337` reads
`for rel in ["README.md", "AGENTS.md", "CONTRIBUTING.md"]`. Adding `CLAUDE.md`
and `GEMINI.md` is a **one-line fix** and removes the "grep by hand" caveat from
W5.8. Promoted into Plan 1.

**W3.6 — the sequencing rule from revision 1 is INVERTED.** It said "W3 before
W2 completes". `retired_symbol_check.rs` has **no severity tier** — `run()`
accumulates failures and bails. Adding contract entries while the references
still exist produces **~680 hard CI failures** on the first run:
`vox-dashboard` 195, `@endpoint` 73, `vox-oratio` 44, decorator class ~276, and
so on. That is an unmergeable tree.

**Correct order:** add a per-symbol `severity: warn` (or the `path_allowlist:`
already proposed in `docs/superpowers/specs/2026-05-24-pr92-handoff.md` §5.3)
**first**, land W3.1 entries as warnings, complete W2, then flip to error.

### W4 — Visual enablement

**W4.1 — client-side `astro-mermaid@2.1.0`, placed before `starlight()`.**
Rejected: `rehype-mermaid@3` and `@beoe/rehype-mermaid` declare a `playwright`
peer dependency via `mermaid-isomorphic`, putting a Chromium download on the docs
build path including the self-hosted Linux fleet. `starlight-client-mermaid`
**does not exist** (nearest is an abandoned `@pasqal-io` 0.1.0 from 2025-02).

**The decisive argument is agent-facing output.** `starlight-llms-txt@0.8.1`
renders to HTML and converts back via `rehypeParse → rehypeRemark →
remarkStringify`; `rawContent` is not set. Client-side emits a `pre.mermaid`
carrying the source, which round-trips into a fenced code block, so
`llms-full.txt` keeps the diagram. **Build-time SVG deletes diagram content from
agent-facing output.**

Secondary: fails soft; dark mode free via Starlight's own `data-theme`; lazy
import only when a diagram is present.

**W4.2 — ordering.** Fix the live broken diagram before enabling the renderer.
Expressive-code registers on `rehypePlugins`, so a remark-stage transform wins —
proven by `remark-vox-include`. If ordering ever resolves the other way the
symptom is a **silent no-op**, so verify visually.

**W4.3 — the parse gate lands before the conversion.** Existing fences are 94%
valid; 54 hand-authored conversions will not be, and client-side rendering
**fails silently in CI**. Note for the implementer: jsdom is required or
DOMPurify fails to initialise and every block spuriously fails.

**W4.4 — fix the one live broken diagram.** Archive diagrams are out of scope.

**W4.5 — `--frozen-lockfile` in all four CI install steps.** Commit the
regenerated `pnpm-lock.yaml` in the same commit.

**W4.6 — `docs/design-system/` is 0 of 8 specs implemented.** No `react`, no
`tailwindcss`, no `shadcn` in `package.json`; the README points at an
`integration-notes.md` that does not exist. Mark `status: roadmap` or archive.

**W4.7 — accessibility.** Authored diagrams carry `accTitle:`/`accDescr:`.

### W5 — Retirement and archival

**W5.1 — 54 evidenced candidates**, tiered; several say "Superseded by …" in
their own body text. *(The claimed 83/40 inbound-edge counts are UNVERIFIED — the
candidate list was never written down. Plan 5 must enumerate it before quoting
an edge count.)*

**W5.2 — the load-bearing blocklist must be derived mechanically, not
hand-listed.** Revision 1 named 3 files. Grepping `"docs/src/` literals in
`crates/*/src` finds many more: `vox-cli-ci/src/constants.rs:8-28` (~14 entries),
`runner_policy_check.rs:10`, `workflow_concurrency_guard.rs:15`,
`capability_snapshot.rs:20`, `vox-audit/.../stdlib_coverage.rs:21`. Derive the
blocklist from that grep in the same PR.

**W5.3 — all 46 ADRs excluded.** `adr/012` has 17–22 inbound links; archiving it
would be actively wrong. Superseded ADRs get `status: deprecated` in place.

**W5.4 — archiving does not remove a doc from the MENS training corpus.**
`--mode corpus` walks `docs/src` unfiltered. `training_eligible: false` is the
lever, and CI already requires it plus `archived_date:` on every archived file
via `check_archival_pipeline` in the fast pre-push tier.

**Related, and not limited to archive:** `lint.rs:457-461` requires
`training_rationale` only for `research`/`roadmap`. **82 docs are
research/roadmap + trainable with unvalidated free-text rationale, and 2 are
explicitly `deprecated` + trainable with no gate at all.**

**W5.5 — two of five exclusion surfaces do not exclude archive.** Sidebar,
llms.txt, and `.voxignore` do. **`vox-doc-inventory` does not** (`SKIP_DIR_NAMES`
omits `archive`; `doc-inventory.json` already carries 296 archive paths).
**The doc-pipeline lint does not** either.

**W5.6 — `superseded_by` is silently stripped** unless declared in
`docs-astro/src/content.config.ts`'s Zod key list. Add unenforced first.

**W5.7 — `docs/superpowers/` needs its own rule.** Several hundred plans
(across 10 subdirectories) plus their specs — run the census (§3) for current
counts; an earlier revision hardcoded them here and they drifted within this
same work program, which wrote new plan files. Unlinted and unbuilt but
**not** in `.voxignore`, so agents ingest all of them. Only 3–4 of 275 are marked
completed; the `status` vocabulary is uncontrolled free text. Fix: a
`plans/archive/` bucket plus one `.voxignore` line.

**W5.8 — move procedure.** Batches ≤25 into
`docs/src/archive/architecture-2026-q3/`, matching the existing
`research-2026-q1/` precedent — run the census (§3) for the current archive
file count; revision 1's 243 was wrong.
`git mv`; set `status: "deprecated"`, `archived_date:`,
`training_eligible: false`, `superseded_by:`; leave `category` unchanged. Rewrite
inbound edges; set valid frontmatter on moved pages (`title`, `description`, `category`, `status`) — Starlight lists them; do **not** create or edit `docs/src/architecture/research-index.md` (retired 2026-09); never touch `architecture-index.md`
or `SUMMARY.md` (gitignored, regenerated). Move `planning-meta/` whole or not at
all. **Links *from* moved files are already safe** — `check_links.rs:318` skips
archive as a source.

### W6 — Audit-machine correctness

**W6.1 — delete `rollout_milestone_pct`, do not fix it.** It returns 25 for an
empty backlog and 25 for 80 open findings. **Nothing consumes it** — every
reference is its own definition, output JSON, schema, and tests. The other four
fields already permit any ratio a consumer wants. Deleting is a smaller diff than
a floor constant plus three tests, and "a number nobody read went unquestioned
for three months" argues against its existence.

**W6.2 — `verify` must recompute metrics.** Nothing in CI or `lefthook.yml` runs
`metrics --write`. *(Correction: this would **not** have caught the 2026-05-12
staleness, because `generated_at` is excluded from comparison by design. The
committed file is numerically correct today. This guards a real future hazard,
not the cited incident.)* `run_verify` is wired into `ssot-drift`, so this is the
one W6 change that becomes genuinely enforced.

**W6.3 — findings paths are never existence-checked.** The schema declares
`doc_paths`/`code_paths`/`contract_paths`; `FindingRow` does not deserialize
them, so serde silently drops schema-declared fields. Real, but **zero
executions** while the backlog is empty — defer to the first real finding.

**W6.4 — duplicate finding ids are not rejected.** Same deferral.

**W6.5 — band thresholds are duplicated** between the program YAML (22/14) and
`priority_band_from_score`. *(Correction: revision 1 justified a string
assertion by claiming a YAML dependency would have to be added. **`serde_yaml` is
already a dependency of `vox-cli-ci`**, used in 10+ files. Keep the four boundary
unit assertions; drop the substring tripwire, which false-fails on
`min_score:22` or a flow-style rewrite.)*

**W6.6 — `glob_match_count` has no early exit.** Measured: the inventory's
`crates/**` expands to **6,012 entries**, on one claim, inside `ssot-drift`,
inside the 60-second fast pre-push tier, on every push. Worth fixing today.
*(Correction: it is one glob on one claim — not "5,206 files per claim" and not
a linear-in-claim-count argument.)* Note the behaviour change: short-circuiting
stops reporting `GlobError`s encountered after the first match.

### W7 — Agent-facing artifact repair (NEW — highest value in the program)

Revision 1 missed this entirely. These are what agents consume
*programmatically*, and they are worse than the prose defects in W1.

**W7.1 — `vox llm prompt` prints hard-parse-error syntax as a "Golden Example".**
`crates/vox-cli/src/commands/llm.rs:24-35` prints the **correct** bare form
(`query get_user(id: u64) -> User`), then four lines later prints
`@query\npub fn get_profile()` labelled "Golden Example", then an "MCP Schema
Excerpt" declaring `"decorator": "@query"`. One invocation, contradictory syntax,
plus `pub fn` which is not Vox. Same for `mutation` at `:41-52`. This is the
subcommand whose entire purpose is telling an LLM how to write Vox.

**W7.2 — `docs/agents/vox-language-surface.v1.json` teaches six decorators;
five are hard parse errors.** `@server`, `@table`, `@query`, `@mutation`,
`@tool` — plus **`@island`, which has never existed in the compiler**. The
`@table` example reads `@table struct User` and `struct` is not a Vox keyword.
Stamped `updated_at: 2026-04-19`, two months before the retirement.
`llm.rs:57` directs users to it by name.

**Fix: generate it.** `crates/vox-language-surface/src/lib.rs:336-348` already
holds `LEXER_DEPRECATED_DECORATORS` as the code SSOT. The JSON should be derived
from it rather than hand-maintained.

**W7.3 — `doc-inventory.json`'s `first_read_for_agents` names a nonexistent
path, and regeneration cannot fix it.**
`crates/vox-doc-inventory/src/inventory_gen.rs:60` hardcodes
`"crates/vox-mcp/src/tools/mod.rs"`. The crate is `vox-orchestrator-mcp`. One of
five files agents are told to read first is unreadable.

**W7.4 — `ai-ide-feature-matrix-2026.json`: 14 of 31 cited paths (45%) do not
exist**, including 2 of the 3 entries in the "what Vox already has" list.

**W7.5 — `llms.txt:18` points agents at the tombstoned archive.** It advertises
`architecture-index` as the master architecture map; that file exists **only**
under `docs/src/archive/`, which the LLM guard forbids ingesting — and the URL
404s. `docs-astro/public/_redirects:21` 301s into that 404. Lines 7-8 also
advertise `voxlang.org/GEMINI.md` and `/CLAUDE.md`, which have no publishable
source.

**W7.6 — `README.md` sends readers to two nonexistent things.** `:53` links
`crates/vox-protocol` (consolidated into `vox-foundation`; 404s on GitHub); `:90`
cites `scripts/clean-cache.vox`, which does not exist — the README's single
concrete VoxScript example. `:86` claims "20+ first-party agent skills" against a
catalog of 18.

**W7.7 — `CONTRIBUTING.md:44` instructs contributors to run the command
`AGENTS.md:213` forbids** (`cargo fmt`, which at a virtual-workspace root is the
all-members invocation that dies with `os error 206`). It never mentions
`scripts/fmt.vox`, `vox ci pre-push`, or the Test-First Policy whose `tdd-guard`
hook will block their commits.

**W7.8 — `script-registry.json`: all 15 `"status": "wrapper"` rows point at
deleted files**, and 20 of 34 rows reference nonexistent paths. The field
consumers are told to trust is the stale one.

**Two systemic root causes** account for most of W7:
1. **The `vox-mcp` → `vox-orchestrator-mcp` split never propagated to agent
   metadata** — `doc-inventory.json`, its generator, 10 paths in the feature
   matrix, `query-all-allowlist.txt:15`, `orchestrator.md:58`,
   `contracts/README.md:9`.
2. **The 2026-06-30 decorator retirement landed in the compiler and AGENTS.md
   and nowhere else** — the language-surface JSON, `vox llm prompt`,
   `CHANGELOG.md`, the migration guide, and the v0.5 case study all still teach
   it.

### W8 — Doctest reality (NEW)

**W8.1 — much of the documented Vox is never compiled.**
`docs/src/reference/ref-syntax.md` — the syntax reference — carries 18 `vox`
fences of which 17 are skipped; that one file was hand-verified. For the
corpus-wide split between fences that carry a skip and fences that actually
compile, run the census (§3): it reports them separately, which revision 2 did
not, and the true skip share is materially lower than revision 2 implied.
These fences are also the MENS positive training corpus.

**W8.2 — the root cause is a tooling bug, not author laziness.**
`doctest.rs` **never resets `current_block` between fences**, so every `vox`
fence in a file concatenates into one compile unit. Authors reach for `vox:skip`
to escape cross-fence collisions. Until this is fixed, "delete the skip and make
it compile" is impossible for any multi-example file.

**W8.3 — the required reason is unenforced.** AGENTS.md §Markdown Hygiene and
`pipeline/mod.rs:58` both require a reason after `// vox:skip`; nothing checks
it, and most markers are bare. Run the census (§3) for the bare-versus-reasoned
split — revision 2's figure here was archive-inclusive. A one-line lint converts
every silent bypass into a justification-or-fix.

**W8.4 — two undocumented escape hatches.** `doctest.rs:32-35` also accepts
`Skip-Test` (documented nowhere) and `{{#include`, which exempts all 39
remaining include directives.

---

## 5. Decisions taken

| Decision | Choice |
| --- | --- |
| Backlog home | Existing `docs-reality-audit` contracts |
| Provenance | Human-grade findings only; no schema change |
| Retirement | Tombstone in place, never delete |
| Visual | Client-side renderer, gated by W4.3 |
| LLM judge tier | Deferred; edge authorized but not taken |
| `rollout_milestone_pct` | **Deleted, not fixed** (zero consumers) |

### Why the LLM tier is deferred

"No single point of failure" cannot honestly be claimed of the current stack:
`chat_once` has zero retry; `infer_with_retry` iterates once per candidate with
no error classification; the activity retry loop is dead code for LLM errors;
`FallbackCondition::ProviderUnavailable` reads an env var before dispatch, so a
live 503 never feeds back; "local is always available" is a hardcoded `true` with
no liveness probe; MENS contributes zero models; and **`durable_scheduler` has no
runner** — no `impl DurableJobStore` exists anywhere.

A resilient version requires the deterministic tier as **primary** and the LLM
tier as optional enrichment, so that offline degrades output *quality*, not
availability. That inversion is a design in its own right.

---

## 6. Sequencing

1. **W1.1 + W7.1 + W7.2 before W2.5 and any decorator repair.** Fixing references
   while the policy file, the CLI, and the machine-readable surface still
   prescribe the broken form guarantees reintroduction.
2. **W3.6 severity valve before W3.1 contract entries** — otherwise ~680 hard CI
   failures. *(This reverses revision 1's rule.)*
3. **W8.2 before W8.1** — the concatenation bug must be fixed before any fence
   can lose its skip.
4. **W4.4 before W4.1**; **W4.3 before the conversion.**
5. **W5.5 / W5.6 before W5.8.**
6. **W6 is independent.**

---

## 7. Verification

```text
vox ci pre-push --full        # NOT --complete: complete runs no tests
cargo run -q -p vox-cli -- ci retired-symbol-check
cargo run -q -p vox-cli -- ci check-links
cargo run -q -p vox-cli -- ci doc-inventory generate --output docs/agents/doc-inventory.json
cargo run -p vox-doc-pipeline -- --lint-only
cargo run -q -p vox-arch-check
cargo run -q -p vox-cli -- ci docs-reality-audit verify
vox run scripts/fmt.vox
```

**`--complete` runs no tests.** It is fmt, line-endings, ssot-drift, doc lint +
doctest, doc-inventory, workspace clippy, and scoped TOESTUB. Only `--full` adds
`cargo nextest run --workspace`. Revision 1 prescribed `--complete` for work
whose entire point was new tests.

**`doc-inventory` drifts on nearly every task in this program** and is verified
in `--complete` and CI. Regenerate and commit it.

All glue scripts are `.vox` via `vox run`. Any new `pub fn` gets its failing test
first. `cargo fmt --all` is never used.

---

## 8. Risks

| Risk | Mitigation |
| --- | --- |
| Adding contract entries before repair | W3.6 severity valve first; ~680 failures otherwise |
| ASCII→mermaid conversion fails silently | W4.3 parse gate lands first |
| Archive move breaks inbound links | Derived blocklist (W5.2); `check-links` gate; W3.5 closes the CLAUDE/GEMINI hole |
| Precision rules over-suppress | R1/R2/R3 measured ~86%; R4 is allowlist-free |
| Repair regresses | W1.1 + W7.1 + W7.2 sequenced first |
| **This becomes the sixth abandoned docs plan** | See below — the dominant risk |

**The dominant risk, stated plainly.** Five prior docs plans exist in
`docs/superpowers/plans/` with **328 open checkboxes and zero ticked between
them**. Two ledgers (`legacy-tombstone-remediation-ledger-2026.md`,
`repo-cleanup-ledger-2026.md`) are open, `status: current`, and unenforced.
`docs/superpowers/specs/2026-05-24-pr92-handoff.md` §5.3 **already documents W3
and already proposes the severity valve**.

The repository's failure mode is not overlapping plans — it is ledgers that
declare intent, never close, and get rewritten six weeks later. **W2, W3, and W5
should update those existing ledgers rather than spawn three more plan
documents.** Only W1, W6, W7, and W4 are genuinely net-new.

---

## 9. Deferred

- **LLM judge tier** (§5).
- **Inventory expansion to ~150 claims** — the existing 10 produced zero findings
  in 102 days.
- **GUI wiki surface** — zero new surfaces needed. Note `docs/src` is **not
  shipped** with the GUI (`tauri.conf.json` has no `bundle.resources`), so
  today's doc surface is a developer-in-repo feature that fails silently in an
  installed app.
- **`infer_with_retry` behavioral fix** (as opposed to W1.5's doc fix).
- **`contracts/operations/catalog.v1.yaml` completion** (W3.4) — different owner.
- **W6.3 / W6.4** — real but zero-execution until the backlog is non-empty.
- **`contracts/index.yaml` coverage** — 188 of 412 contract files indexed, and
  `contracts_index.rs:80-89` validates index→disk only, so the gap can only grow.
  At least 7 unindexed files are read literally by Rust source.
