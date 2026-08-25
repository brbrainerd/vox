---
title: "Retired-Syntax Prose Lint Investigation"
description: "Measurement log for the attempt to add at-prefixed data-layer patterns to the retired-symbols lint, archived after two shipped-then-reverted attempts."
category: "Architecture SSOTs"
status: "research"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-08-23
---

# Retired-Syntax Prose Lint Investigation

`contracts/documentation/retired-symbols.v1.yaml` has no pattern for any at-prefixed
data-layer form — no `@table`, `@query`, `@mutation`, `@server`, `@tool`, `@resource`,
`@form`, `@index`, or `@endpoint`. It carries the retired `@component`-plus-`fn` form but
not the eight forms that became hard parse errors on 2026-06-30 (`cd7cc96874`). So
`vox ci retired-symbol-check` could not fire on the defect it exists to prevent, and
`ref-decorators.md` and `migration-0.5-to-0.6.md` told readers to migrate *onto* dead
syntax for months with every gate green.

**Adding the nine patterns is not the fix — measured 2026-08-23.** They work (verified
against an injected probe) but produce **195 violations on a clean tree**: 65 `@endpoint`,
26 `@table`, 22 `@query`, 20 each `@tool`/`@mutation`, 19 `@form`, 16 `@server`, 3 each
`@resource`/`@index`. Concentrated in `boilerplate-reduction-gap-analysis-2026.md` (27),
`vox-gui-native-roadmap-2026.md` (21), `migration-0.5-to-0.6.md` (14), `ref-decorators.md`
(10). The last two are *correct* — the retired forms live in their "Retired" columns and
migration tables, which is what those pages are for.

A line-level migration-cue allowlist (`retired|removed|deprecated|instead|→|no longer|
superseded|…`) was measured against all 194 decorator hits: it clears **62**, leaving
**132**. Insufficient — the remainder are legitimate mentions inside historical and design
docs whose individual lines carry no cue.

**Block-level context was implemented and measured — still not enough (2026-08-23).**
`scan_source_lines` in `retired_symbol_check.rs` now tracks markdown heading depth and
skips a section opened by a "## Retired"/"### Historical"/"#### Superseded" heading until
the next heading at the same or shallower level (bounded, unlike the whole-file
`is_historical_or_audit_doc` carve-out). It is a strict widening with zero regression
against the shipped 14 patterns — `retired-symbol-check OK` before and after — so it is
kept regardless of the nine at-prefixed patterns' status.

Re-measured with the nine patterns re-added: **195 → 180 violations.** Two new obstacles,
beyond what section-scoping can reach:

- Single-sentence mentions with no enclosing "Retired" section — e.g. `AGENTS.md:244` and
  `:510`, plain prose ("Removed in v0.6.0: `@endpoint`") inside otherwise-current sections
  that aren't themselves headed "Retired". The cue-word filter (62/194) was built for
  exactly this case and remains the more promising direction than heading-scoping; the two
  should probably compose.
- `docs/agents/vox-language-surface.v1.json` is a machine-generated **data file**, not
  prose — it deliberately documents the retired `@`-forms as part of the language-surface
  SSOT. `cfg.is_md` gates all of the frontmatter/fence/heading logic, so a `.json` file gets
  none of it; JSON needs its own carve-out (e.g. skip a `"note"` field whose text contains a
  migration cue) or exclusion from the scan entirely, since the check's doc comment already
  says Rust sources are "intentionally out of scope" for a symmetric reason.

Still not shipping the nine patterns. A gate that fails on legitimate SSOT data and
legitimate single-sentence retirement notices is not ready.

Related measurement: of 629 stale-syntax occurrences repo-wide, only **6.2%** sit inside
` ```vox ` fences (all correctly `vox:skip`-marked as of 2026-08-23), **60.9%** are prose
and **25.1%** table cells. The doctest gate can never reach the latter two.
