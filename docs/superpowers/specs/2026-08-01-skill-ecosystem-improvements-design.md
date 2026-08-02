---
title: "Skill Ecosystem Improvements — Design Spec (6-Phase Roadmap)"
description: "Design for consolidating and hardening the skill ecosystem across both harnesses (Claude Code / assets/skills and Vox Axis-MENS / crates/vox-skills+vox-plugin-skill-*): frontmatter consolidation, graphify skill parity, a plan/spec review gate, relevance-ranked skill selection routed through vox-search, a GUI admin surface, and provenance/reliability hardening."
category: "architecture"
status: "current"
training_eligible: true
---

# Skill Ecosystem Improvements — Design Spec (6-Phase Roadmap)

**Status:** Approved design (brainstorming output).
**Research basis:** [`../../src/architecture/skill-ecosystem-audit-2026-08-01.md`](../../src/architecture/skill-ecosystem-audit-2026-08-01.md) (this session), plus [`skill-discovery-and-induction-research-2026-07-30.md`](../../src/architecture/skill-discovery-and-induction-research-2026-07-30.md), [`skill-marketplace-security-and-provenance-research-2026-07-30.md`](../../src/architecture/skill-marketplace-security-and-provenance-research-2026-07-30.md), [`skill-registry-trust-and-curation-research-2026-07-30.md`](../../src/architecture/skill-registry-trust-and-curation-research-2026-07-30.md).
**Normative constraints this design must satisfy:** AGENTS.md §"Agent Skills (Required, SSOT)" — one canonical YAML+`metadata.vox-*` frontmatter format (TOML is legacy, not co-equal); skill retrieval MUST route through `vox-search`, never a bespoke index; discovery is root-based and automatic, never hand-registered; all new glue automation is `.vox`/`vox run`, never `.sh`/`.ps1`/`.py`.

## 1. Scope

Six phases, each independently shippable and independently reviewable. This spec covers the
**architecture and sequencing of all six**; only Phase 1 gets a detailed implementation plan in
this cycle (per your instruction — plan to go with this spec). Phases 2–6 get their own
brainstorm→plan cycle later, each seeded from this doc so nothing here has to be re-derived.

Ordering locked in brainstorming: **fix + consolidate first, then build new capability.**

```
Phase 1: Content audit & consolidation      (foundation — everything else builds on a clean, canonical skill set)
Phase 2: Graphify skill parity              (your named priority)
Phase 3: Review-gate integration            (your named priority)
Phase 4: Smart selection/loading upgrade    (your named priority)
Phase 5: GUI admin/audit surface            (your "might consider")
Phase 6: Security/provenance hardening      (this session's own research finding)
```

Phase 1 blocks 2–6 in one concrete way: Phase 4 retires the bespoke BM25 index in favor of
`vox-search`, and `vox-search` indexes skill *manifests* — migrating those manifests onto one
canonical frontmatter dialect first (Phase 1) means Phase 4 writes one indexer, not three.

## 2. Phase 1 — Content audit & consolidation

**Goal:** every skill file in the repo is canonical-format, has no dead cross-references, has
no duplicate SSOT, and the superpowers skill set is portable to Vox Axis/MENS. No new
capability — this phase only removes debt the audit found.

**Scope, itemized (feeds the Phase 1 plan directly):**

1. **Dedupe `populi.skill.md`.** Keep `crates/vox-plugin-populi-mesh/populi.skill.md` — it's
   the load-bearing copy, referenced by `Plugin.toml`'s `[plugin.payload.skill] skill-md =
   "populi.skill.md"` and shipped via `catalog.toml`'s `populi-mesh` plugin entry. Delete
   `crates/vox-skills/skills/populi.skill.md` — confirmed (grep) unreferenced by any sync
   script, test, or catalog entry; it's a stray duplicate, not a second distribution path.
2. **Migrate frontmatter dialects to canonical.** Per AGENTS.md, canonical = YAML frontmatter
   (`name` matching directory name conceptually, `description` 1–1024 chars) with Vox
   extensions under `metadata` as `vox-*` keys (`vox-id`, `vox-category`, `vox-tools`,
   `vox-permissions`). Two migrations:
   - Flat-TOML `populi.skill.md` (post-dedupe, the surviving copy) → canonical YAML.
   - The 9 `[metadata]`-nested-TOML plugin skills (`crates/vox-plugin-skill-*/*.skill.md` +
     the surviving populi copy if it lands there instead) → canonical YAML, preserving every
     existing `tools`/`permissions` value losslessly.
   - The 13 `superpowers/*.skill.md` files are already plain-YAML `name`+`description` — verify
     each against the parser SSOT (`vox-plugin-host::skill_parser`) and add empty/absent
     `metadata` blocks only where a Vox extension field is actually needed, not speculatively.
   - Run `vox ci agentskills-compliance` after migration as the acceptance check — it already
     gates this format.
3. **Delete or relocate orphaned files**, each independently:
   - `assets/skills/brainstorming/spec-document-reviewer-prompt.md` and
     `assets/skills/writing-plans/plan-document-reviewer-prompt.md` — confirmed unreferenced by
     any SKILL.md body. Delete, unless Phase 3's review-gate skill turns out to want to revive
     one as its dispatch template — check Phase 3 design first (this task can wait until Phase 3
     is scoped, or proceed now and let Phase 3 re-add a fresh template if needed; either is
     fine, this is not a hard dependency).
   - `assets/skills/systematic-debugging/CREATION-LOG.md` — an authoring log, not a runtime
     reference. Move out of the shipped skill directory (e.g. into a `docs/superpowers/` archive
     or delete if the history is already in git log).
4. **Fix `claude-api` staleness.** Refresh the pricing/model table (cached 2026-06-04) to
   current model IDs/pricing, or — cheaper and more durable — replace the hardcoded table with
   a pointer to the live `claude-api` skill maintained upstream in `anthropic-skills`, if the
   vendoring model supports partial-file overrides. Default to the cheaper fix unless it breaks
   vendoring parity.
5. **Add `assets/skills/using-superpowers/references/vox-axis-tools.md`**, following the exact
   pattern of the existing `codex-tools.md`/`copilot-tools.md`/`gemini-tools.md`: map each
   Claude-Code-specific tool name (`Skill`, `TodoWrite`, `Task`) the superpowers skill set
   references to its Vox Axis/MENS MCP-tool equivalent (`vox_skill_use`, task-tracking via
   whatever `vox-plugin-skill-orchestrator` exposes, subagent dispatch via
   `vox-plugin-skill-orchestrator`'s multi-agent submission). This is the concrete unblock for
   "superpowers skills usable by MENS," not a rewrite of the skills themselves.
6. **Fix the confirmed graphify tool-name mismatch.** `vox-graph/SKILL.md` documents tools and
   CLI verbs that don't exist (`vox_search_query`, `vox search rebuild`, etc.) — confirmed by
   reading `graph_tools.rs` and `vox-cli/src/commands/graphify/mod.rs` directly. Rewrite the
   skill's "Key MCP tools" and "Graph verbs (CLI)" sections to name the tools/verbs that are
   actually registered today (`vox_graphify_status/search/query/path/compare/callers/callees/
   rebuild`; CLI `vox graphify status|query|rebuild|coverage|index|refresh|crate-map`).
   Completing the `vox_search_*` rename in `graph_tools.rs` itself is a separate, larger,
   code-touching change — out of scope for a docs-consolidation phase; note it as a fast-follow
   in the skill's own text if the rename is still wanted, but don't block this phase on it.

**Out of scope for Phase 1:** `canvas-design`/`algorithmic-art` redundancy (both are
intentionally-different-media templates per the vendored source, not a Vox-authored redundancy
— leave as-is, vendoring shouldn't diverge from upstream without cause) and
`brand-guidelines`/`slack-gif-creator`/`internal-comms` low-relevance flags (removing vendored,
license-tracked upstream skills is a policy call, not a cleanup task — raise separately if
wanted, don't fold into this phase).

**Testing/acceptance:** `vox ci agentskills-compliance` passes for every migrated file;
`vox ci plugin-skill-parity` passes; `vox skill discover` finds no orphans; a fresh MENS session
under Vox Axis can successfully invoke at least one superpowers skill end-to-end using the new
`vox-axis-tools.md` mapping (manual smoke test, since MENS harness automation isn't in scope
here).

## 3. Phase 2 — Graphify skill parity (roadmap-level)

Author `crates/vox-skills/skills/superpowers/graphify.skill.md` (canonical YAML format per
Phase 1), mirroring `assets/skills/vox-graph/SKILL.md`'s "graph-first before grep" instruction
but pointing at whichever tool names Phase 1's verification step confirmed are real. Register it
so both `vox_skill_list` and the GUI Marketplace/Installed tabs surface it identically to the
Claude-Code version — one skill, two harnesses, per AGENTS.md's "one skill set serves every
tool" rule (this is a correction of an existing violation, not new policy). Depends on Phase 1
task 6 being resolved first (can't document tool names that might be about to change).

## 4. Phase 3 — Review-gate integration (roadmap-level)

Two independent sub-deliverables, both blocked only by Phase 1's frontmatter migration (so the
new skill ships canonical from day one):

- **New "review a plan or spec" skill**, one per harness (Claude-format +
  `crates/vox-skills/skills/superpowers/`), inserted in the trigger chain between
  `writing-plans` and `executing-plans`: reads a spec/plan doc, checks it against the same
  placeholder/contradiction/scope/ambiguity criteria brainstorming's own self-review already
  uses (don't invent new criteria — extract and formalize the existing ones), and produces a
  pass/revise verdict. This directly answers "following continuation skill or review of plans
  and specs" from the original ask.
- **Rebuild `vox-skill-review`** (the orphaned pre-publish gate from
  `docs/superpowers/plans/2026-06-18-skill-review-gate.md`), using the dangling commit history
  (e.g. `6e2a055621`) as reference material, not a rebase target — recover the design intent,
  re-implement clean on current `main`. Scope: frontmatter completeness, stub detection,
  MCP-SSOT drift, dedup against installed skills, `Pass`/`NeedsHuman` verdict, exactly as
  originally specced.

## 5. Phase 4 — Smart selection/loading upgrade (roadmap-level)

Retire `skill_search_index.rs`'s bespoke BM25 `SkillSearchIndex` — it's a direct AGENTS.md
violation. Route both `vox_skill_search` and a new relevance-ranked tier-1 catalog selector
through the existing `vox-search` hybrid stack (tantivy lexical + semantic indexer + RRF
fusion), indexing skill manifests (name/description/tags — the same fields
`SkillSearchIndex` used) as a `vox-search` document type. `render_skill_catalog`'s cap-64
truncation changes from alphabetical/reliability-sort-then-truncate to: rank all installed
skills against the current conversation/task context via `vox-search`, take top 64. Preserves
the existing tier-2 (`vox_skill_use`) and pinned-full-body mechanisms unchanged — this phase
only fixes *which* skills make it into the tier-1 catalog, not the tiering architecture itself.

## 6. Phase 5 — GUI admin/audit surface (roadmap-level)

Extend `SkillsPluginsView.tsx` (or add a sibling settings surface) with per-skill: permission
list (from the manifest's `permissions`/`vox-permissions`), trust level, and
`skill_reliability` telemetry (win/fail counts, last-used, promotion history if promoted via
`skill_promotion.rs`'s 8-gate pipeline). Read-only in v1 — no new mutation surface — since the
underlying trust-level/reliability write paths are Phase 6's concern, not this phase's.

## 7. Phase 6 — Security/provenance hardening (roadmap-level)

Turn the two 2026-07-30 research docs' recommendations into code: Sigstore-style provenance
binding at promotion time (extends `skill_promotion.rs`'s existing gate 8, "provenance," which
the audit notes is currently a documented partial-fidelity gap, not a stub to build from
scratch), mandatory re-verification triggered on any post-promotion body hash change, and
`skill_reliability`-driven automatic transition to `deprecated` status (stricter than the MCP
Registry's advisory-only posture, per the registry-trust research's explicit recommendation that
Vox can afford to be stricter since it controls the whole stack).

## 8. Error handling / rollback

Every phase operates on files/tables that are already git-tracked or already have migration
tooling (`vox ci agentskills-compliance`, `vox skill discover`) — no phase introduces
irreversible state. Phase 1's frontmatter migration is the highest-touch change; do it file-by-file,
running the full `vox ci agentskills-compliance` scan (it has no per-path scoping — it checks
every skill file in one pass) and committing after each file, not as one bulk rewrite, so a bad
migration is caught and revertable per-file rather than discovered after all 9 files are
touched.

## 9. Testing

Phase 1: `vox ci agentskills-compliance`, `vox ci plugin-skill-parity`, `vox skill discover`
(zero orphans), manual MENS smoke test via the new `vox-axis-tools.md` mapping. Phases 2–6: each
phase's own plan defines its acceptance criteria when brainstormed; not specified here to avoid
designing implementation details this session wasn't asked to go that deep on.
