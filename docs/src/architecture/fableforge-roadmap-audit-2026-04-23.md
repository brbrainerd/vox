---
title: "FableForge Roadmap Audit — 2026-04-23"
description: "Document-level audit of the FableForge End-to-End Roadmap (280 tasks / 14 phases). Covers internal consistency, redundancies, mis-prioritizations, prunable items, missing-context flags, and a re-ranked top-30 execution list."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
training_rationale: "Project planning artifact for a separate codebase (FableForge, TypeScript/Convex)."
last_updated: "2026-04-23"
---

# FableForge Roadmap Audit — 2026-04-23

**Scope:** Document-level analysis of the FableForge End-to-End Roadmap (280 tasks, 14 phases).
The FableForge codebase (TypeScript / Next.js / Convex) is a **separate project** and was not
directly accessible during this audit. All findings are therefore grounded in the roadmap document
itself — verified for internal consistency, logical dependency correctness, and prioritization
quality. Items flagged under "Missing Context" require real-code verification before implementation.

---

## 1. Critical Internal Consistency Errors

These must be resolved before any team member picks up the affected tasks; they will cause confusion
or implementation conflicts if left open.

### 1.1 Schema version collision (T-021 / T-022)

T-021 body text bumps from `"0.1.0"` → `"0.2.0"` (and names the target "FFScript v0.2"). However,
T-021's own acceptance criteria reads:

> "existing `FFScriptV1` validates as `1.0`; new format validates as `1.1`"

This mixes two incompatible version strings (`0.2.0` in the task body; `1.0` / `1.1` in the
acceptance criteria). T-022 reinforces `0.1.0 → 0.2.0`. Whichever version naming scheme the team
chooses must be consistent across both tasks and the migration runner.

**Resolution:** pick one scheme before starting T-021. Recommended: stay on semver 0.x until the
Panel contract is battle-tested, then promote with a separate "v1.0 promotion" task. Delete the
`1.0`/`1.1` references from T-021's acceptance criteria.

### 1.2 Non-existent task reference T-268b (T-021)

T-021 notes: "A later task (T-268b) will promote to `1.0.0`." T-268b does not exist anywhere in
the roadmap. T-268 is the FFScript fuzzer. Either create the task, or remove the reference.

### 1.3 T-002 conflicts with T-101/T-102

- T-002 renames `StudioWizard.tsx` → `WizardCreatePage.tsx`.
- T-102 says "Wizard is a 'narrow' storyboard — one implementation, two presets."

If T-102 is implemented, the wizard is no longer a standalone component and the rename in T-002
becomes meaningless (or actively misleading). These two tasks need to be reconciled: either T-002
is a transitional rename that T-102 will later absorb, or T-002 should be skipped in favor of doing
T-102 directly. **Mark T-002 blocked by the T-101/T-102 design decision.**

### 1.4 T-029 phase assignment (Phase 1, P1/L)

T-029 deprecates `gameDrafts.beats` in favor of panels and migrates all legacy drafts on open.
This is a destructive migration of the primary authoring data model. Placing it in Phase 1 — before
the storyboard GUI (Phase 3) and wizard integration (Phase 4) exist — means the migration runs
against a UI that hasn't been updated to consume panels yet. **Move T-029 to Phase 4 or later.**

### 1.5 T-040 — docs written before the feature exists (Phase 1, P1/S)

T-040 writes `docs/FFSCRIPT_PANELS.md` in Phase 1, before panels are implemented in the compiler
(T-023, Phase 1), validated (T-043, Phase 2), or used by any GUI (Phase 3+). Document the schema
after it is stable. **Move T-040 to Phase 3; lower to P2.**

### 1.6 T-085 "Presence-aware cursor hook" in Phase 3 (P2/S)

The note says it "ships the cursor broadcast hook now, activated later." Shipping dead infrastructure
ahead of its Phase 12 activation creates a maintenance burden with no near-term payoff. **Move T-085
to Phase 12 where the collaboration work actually happens.**

---

## 2. Redundant / Overlapping Task Pairs

Each pair below should be explicitly linked in the tracker or merged into a single task.

| Pair | Issue |
|------|-------|
| T-003 + T-017 | Both address per-beat regeneration: T-003 is the UI, T-017 is the backend contract. They must land together in one PR or the UI calls a non-existent action. Mark T-003 `blockedBy: T-017`. |
| T-005 + T-006 | T-005 (structured error surfaces in wizard) and T-006 (remove silent `catch` blocks) are the same effort: one changes what errors look like, the other ensures errors are actually thrown. Merge into a single task. |
| T-013 + T-129 | T-013 routes the outline LLM call through `selectModelForTask`. T-129 builds the full cross-provider orchestrator. T-013 should be noted as "a temporary fix pending T-129; revisit when T-129 lands." |
| T-106 + T-065 + T-179 | Character sheet (Phase 4), foreground layer editor (Phase 3), and character library panel (Phase 6) all build character UI. Three-phase character UI buildup risks inconsistent component structure. Consolidate design upfront even if implementation spans phases. |
| T-193 + T-205 | Auth mutation audit and siloing test suite are the same work. T-193 is the manual grep; T-205 is the automated coverage. They belong in one sprint. |

---

## 3. Mis-prioritized Tasks

### 3.1 P0 tasks buried in late phases (should move to Phase 0 or Phase 1)

| Task | Current phase | Issue |
|------|---------------|-------|
| T-191 — cascade delete | Phase 8 | Data safety / orphan risk exists from day one. Every game created before this lands is a potential orphan liability. Move to Phase 0 alongside T-001. |
| T-193 — auth mutation audit | Phase 8 | Security invariant. Should be enforced before any public-facing mutations are added. Move to Phase 0. |
| T-205 — siloing test suite | Phase 8 | Same as T-193. |
| T-226 — server-side age gate | Phase 10 | Tier-2 content is generated starting Phase 5. A client-only gate is bypassable from the moment tier-2 generation is wired. Move to Phase 5 as a precondition for T-149 (SFW/NSFW routing). |

### 3.2 Tasks labeled P0 that are not functional blockers

| Task | Label | Recommendation |
|------|-------|----------------|
| T-007 — unify provider/tier stamps | P0/S | Data hygiene, not a blocker for any journey step. Lower to **P1**. |
| T-010 — audit log of wizard transitions | P0/S | Observability, not functional. Lower to **P2**. |
| T-018 — structured generation progress feed | P0/M | Useful but the wizard ships working feedback today (however rough). Lower to **P1**. |
| T-030 — panel-level seed locking | P1/S | Style consistency feature. Lower to **P2**. |

### 3.3 Tasks labeled P1/P2 that should be P0

| Task | Label | Reason to elevate |
|------|-------|-------------------|
| T-191 / T-193 / T-205 | P0 label but Phase 8 | Already noted above — data safety. |
| T-031 — aspect ratio drives workflow dims | P0 noted, but buried in Phase 1 | Hardcoded dims break resolution switching for every user immediately. Confirmed P0, should be executed alongside T-021. |
| T-128 — resolution picker UI | P0/M Phase 5 | This is the user-facing side of T-031 and should land at the same time. |

---

## 4. Prunable Tasks (safe to defer past MVP)

These items have effort costs that outweigh their pre-launch value. None should block the critical
path to a usable MVP.

| Task | Recommendation |
|------|----------------|
| T-014 — migrate abandoned drafts | Scheduled job, not needed until user base exists. Defer to post-launch. |
| T-040 — FFScript Panel docs (Phase 1) | Already noted; defer to Phase 3. |
| T-075 — zoom/pan state per panel | P1/S in Phase 3; nice-to-have, not blocking. Defer to Phase 5. |
| T-083 — Panel templates library | P2/M; only valuable after core editing is solid. Defer to Phase 5. |
| T-085 — presence cursor hook | Already noted; defer to Phase 12. |
| T-089 — rule-of-thirds guide | P2/S; visual polish. Defer to Phase 14. |
| T-090 — safe-area overlay | P2/S; only needed for CBZ export (Phase 11). Move there. |
| T-098 — saveable view state | P2/S; UX polish, not blocking authoring. Defer to Phase 14. |
| T-100 — command palette mutation logs | P2/S; debugging aid, not user-facing. Defer to Phase 13. |
| T-134 — PuLID for Flux | P2/M; third identity-preservation option. Defer until IP-Adapter FaceID Plus v2 is stable. |
| T-141 — SUPIR upscaler | P2/L; luxury restoration pass. Defer until export pipeline exists. |
| T-148 — relight/re-tone pass | P2/M; advanced compositing. Defer to Phase 12+. |
| T-155 — lip-sync | P2/L; cinematic mode only. Defer to Phase 11+. |
| T-239 — Electron desktop export | P3/XL; enormous effort, unclear demand. Defer indefinitely. |
| T-243 — share links with tracking | P2/S; analytics feature. Defer to Phase 13. |
| T-244 — embed snippet | P2/S; developer feature, low initial demand. Defer to Phase 11. |
| T-256 — CRDT fork/merge | P3/XL; overkill before multi-user editing (Phase 12) is even proven. Defer indefinitely. |

---

## 5. Dependency Graph Corrections

The §10 graph omits several real dependencies that will cause integration pain if missed.

| Dependency | Missing from graph |
|------------|--------------------|
| T-226 (age gate) must land before T-149 (SFW/NSFW routing) | Not shown |
| T-031 (aspect ratio in schema) must land before T-128 (resolution picker UI) | Not shown |
| T-124 (SAM 2 auto-mask) must land before T-122 and T-123 (bg/fg inpaint) | Listed in prose but not in the dependency graph |
| T-041 (mutation API) must land before T-061, T-062, T-063, T-065, T-067, T-069, T-071 (all storyboard GUI panels) | Critical; not shown |
| T-055 (runtime part 1) must land before T-115 (pre-publish playtest) | Not shown |
| T-199 (game versions) must land before T-204 (saves scoped by version) | Implied but not explicit |
| T-191 (cascade delete) must land before T-192 (soft-delete), T-196 (R2 orphan sweep) | Ordering implied but not stated |

---

## 6. Missing Context — Requires Real-Code Verification

The following roadmap claims cite specific files and line numbers. None can be confirmed without the
FableForge codebase. **Block execution of dependent tasks until each is verified.**

| Claim | Tasks affected |
|-------|----------------|
| `CreateWizardPage.tsx` exists as a dead mock, linked from routes | T-001 |
| `StudioWizard.tsx` is wired to Convex `gameDrafts` | T-002, T-003 |
| `catch (e) { /* ignore */ }` at `generation.ts:166`, `faceExtraction.ts:74`, `router.ts:54` | T-006 |
| `vnGeneratePanels.ts:31` hardcodes `"fal"` | T-009 |
| `games/studio/actions.ts:101` pins `LLM_MODELS.gpt4o` directly | T-013 |
| FFScript schema version regex is `/^0\.\d+\.\d+$/` and current version is `"0.1.0"` | T-021, T-022 |
| `gameDrafts.beats` is the current SSOT for beat data (not `panels`) | T-029 |
| `processBeatJob` routes to FAL hardcoded regardless of `selectedProvider` | T-004 |
| `timeline-save.ts` uses localStorage only (no Convex writes) | T-206, T-207 |
| `BrowseClientPage.tsx` is a stub (~6 KB placeholder, no real filtering) | T-231 |
| `deleteGame` at `convex/games/mutations.ts:212–299` skips `saves`, `characterAnchors`, etc. | T-191 |
| `importJobs` table exists but no mutation is wired | T-197 |
| `VACE` is video-only; `Lustify` is full-image, not regional inpaint | T-121, T-122, T-123 |
| `poseReferenceImage` param exists in `generation.ts:87` but is silently ignored | T-135 |
| `activeLoras` accepted but not loaded in workflows | T-131 |

---

## 7. Effort Estimate Concerns

These tasks appear under-estimated relative to their described scope.

| Task | Stated effort | Concern |
|------|---------------|---------|
| T-041 — FFScript mutation API | M (½–2 days) | Twelve public methods + Immer internals + post-mutation revalidation + type exports. More realistically L (3–7 days). |
| T-046 — Linter engine | M | A pluggable rule engine with 10+ default rules is realistically L. |
| T-087 — Bidirectional GUI ↔ Code sync | L (3–7 days) | Real-time JSON patch application into Monaco while Monaco is live-edited is notoriously hard to get right without race conditions. Closer to XL with proper conflict handling. |
| T-129 — Cross-provider fallback router | L | Three providers, per-attempt analytics, circuit breaker state — this is likely XL when combined with T-130. |
| T-151 — Music generation runtime | L | Suno V5 + Stable Audio 2.0 + MusicGen fallback chain, per-scene cues, baking — XL given the audio pipeline complexity. |
| T-197 — Import-from-ZIP pipeline | L | ZIP extract → Ren'Py parse → FFScript transpile → linter pass → wizard integration. Historically transpiler work is XL. |
| T-237 — Ren'Py export action | L | Round-tripping a complex game format through Ren'Py's Python DSL reliably is XL. |

---

## 8. Open Questions from §12 — Recommended Decisions

The roadmap lists six open questions. Recommended answers to unblock implementation:

1. **Panel-per-beat or beat-per-panel?**
   Start 1:1 at wizard time; allow many panels per beat only when the storyboard editor is in place
   (Phase 3). Hard-coding 1:1 in the compiler (T-023) simplifies Phase 1 substantially.

2. **Branching in comics?**
   Permit it in the schema (T-021) but do not render it in the Panel Grid until Phase 6 (T-072
   branch visualization). Comics with branches become "split pages" visually; VNs get a choice node.

3. **Which LLMs for which stages?**
   Set defaults now to unblock T-013 and stop the policy violation:
   - Story outline: Claude 3.7 Sonnet (best long-context coherence)
   - Character voices / dialogue: Gemini 2.5 Flash (fast, cheap, vision-capable)
   - Cheap refinement / batch: DeepSeek V3
   Hard-code these in `selectModelForTask` until cost telemetry (T-259) can drive dynamic selection.

4. **Creator compute pricing?**
   Minimum viable: credits per model call (already implied by T-016 refund logic). Decide on credit
   price points before Phase 10 publish goes live.

5. **Minimum viable moderation before public publish?**
   Auto-classification (T-224) + trusted-creator allowlist is the right call. Build the queue (T-223)
   first; open allowlist to beta testers only at first publish.

6. **Fork policy?**
   Default: not forkable. Owner opts in per game. Implement at Phase 11 (T-200) with explicit
   `forkable: boolean` field on games table.

---

## 9. Re-Ranked Top-30 Execution List

This replaces §11 of the roadmap. Tasks are ranked by: (unblocks most downstream) × (P0 safety
risk) ÷ (effort). Tasks that overlap are grouped into delivery units.

**Delivery Unit 1 — Foundation (Phases 0–1, ~2 weeks)**

| Rank | Task(s) | Why first |
|------|---------|-----------|
| 1 | T-001 — Kill `CreateWizardPage.tsx` | Eliminates a live user trap; zero-risk change |
| 2 | T-191 + T-193 + T-205 — Cascade delete, auth audit, siloing tests | Data safety before anything else; parallelizable with the rest of this unit |
| 3 | T-005 + T-006 — Structured errors + remove silent catches | Can't debug any later work without this |
| 4 | T-004 — Wire `selectedProvider` end-to-end | Every generation in every later phase uses this |
| 5 | T-013 — Route outline through `selectModelForTask` | AGENTS.md compliance; 1-line fix once code is open |
| 6 | T-017 then T-003 — Idempotent `processBeatJob` then per-beat regen UI | Critical UX: users can't recover from partial failures without this |

**Delivery Unit 2 — Panel type + compiler (Phase 1, ~1.5 weeks)**

| Rank | Task(s) | Why next |
|------|---------|----------|
| 7 | T-021 + T-031 — Panel schema + aspect ratio field | Schema foundation; must be stable before anything else |
| 8 | T-022 — Migration to v0.2 | Ships with T-021 |
| 9 | T-023 — Compiler emits panels | Draft pipeline starts producing panels |
| 10 | T-025 + T-026 + T-027 — Panel TS types, CRUD mutations, Convex table | GUI can now read/write panels |

**Delivery Unit 3 — Mutation API + Linter (Phase 2, ~1.5 weeks)**

| Rank | Task(s) | Why next |
|------|---------|----------|
| 11 | T-041 + T-042 — Mutation API package + Immer internals | All GUI edits in Phase 3 depend on this |
| 12 | T-043 + T-051 — Post-mutation revalidation + forgiving validator | Correctness guarantee before GUI uses mutations |
| 13 | T-044 — JSON Patch undo/redo | Mandatory for any serious editor |
| 14 | T-046 + T-047 + T-048 — Linter engine + wizard integration + lint-on-publish | Quality gate; linter must exist before publish opens |

**Delivery Unit 4 — Storyboard core (Phase 3, ~3 weeks)**

| Rank | Task(s) | Why next |
|------|---------|----------|
| 15 | T-061 + T-062 — Panel Grid + insert-between | Primary authoring surface |
| 16 | T-063 + T-074 — Panel Detail canvas + toolbar | Per-panel editing unlocked |
| 17 | T-064 + T-065 — Background and foreground layer editors | Core composition |
| 18 | T-067 + T-069 + T-070 — Bubble tool + inline editor + speaker picker | Dialogue authoring |
| 19 | T-086 + T-087 — Monaco code view + bidirectional sync | Power-user escape hatch; also the safest integration test of the mutation API |
| 20 | T-076 + T-077 — Keyboard shortcuts + multi-select move | Productivity baseline |

**Delivery Unit 5 — Model stack + inpaint (Phase 5, ~3 weeks)**

| Rank | Task(s) | Why next |
|------|---------|----------|
| 21 | T-226 — Server-side age gate | Must land before tier-2 generation goes live |
| 22 | T-124 — SAM 2 auto-mask | Prerequisite for T-122 and T-123 |
| 23 | T-121 — Flux Fill Dev inpainting | Core missing generation feature |
| 24 | T-122 + T-123 — bg-only / fg-only inpaint | Top-2 user pain points; unlocked by T-121 + T-124 |
| 25 | T-128 + T-031 UI — Resolution picker | Replaces hardcoded dims; quick win |
| 26 | T-129 + T-130 — Cross-provider fallback router + circuit breaker | Provider independence |
| 27 | T-158 — Hot-swap model per panel | User control; trivial once T-129 exists |

**Delivery Unit 6 — Publish + Play (Phases 9–10, ~2 weeks)**

| Rank | Task(s) | Why next |
|------|---------|----------|
| 28 | T-055 (runtime part 1) + T-057 (save state) — Basic interpreter + save serialization | Playtest and cloud saves both depend on this |
| 29 | T-206 + T-207 — Convex cloud save mutations + runtime writes | Closes the localStorage-only save gap |
| 30 | T-115 + T-116 + T-117 — Pre-publish playtest + checklist + unreachable branch check | No broken games ship |

**What comes after:** T-194 (global slugs), T-199 (game versions), T-231 (real marketplace),
T-235 (reviews), T-237 (Ren'Py export), then Phases 12–14.

---

## 10. Summary of Changes from Original Roadmap

| Change type | Count |
|-------------|-------|
| Critical consistency errors requiring resolution before execution | 6 |
| Redundant task pairs to merge or explicitly link | 5 |
| Tasks moved to an earlier phase for safety | 4 |
| Tasks downgraded from P0 | 4 |
| Tasks deferred / pruned from MVP critical path | 17 |
| Effort estimates revised upward | 7 |
| Missing-context flags (require real-code verification) | 15 |
| Open-question decisions recommended | 6 |

The critical path to a **usable, safe, publishable MVP** compresses to approximately **14 weeks**
of focused execution across Delivery Units 1–6 above, assuming the FableForge codebase is otherwise
in the state the roadmap's audit describes.

---

---

## 11. Post-Spec Cross-Check Findings (2026-04-23)

After producing the Panel schema spec, mutation API spec, and linter design spec, three
additional pruning and scoping decisions emerged.

**T-059 (dry-run symbolic executor) is largely subsumed by the linter.** The linter's default
`ff/flow/unreachable-node` rule (part of T-046) performs a BFS from the start node and reports
any unreachable panel. The `ff/flow/infinite-loop` rule adds cycle detection. Together these
cover the functional requirement stated for T-059. **Prune T-059 as a standalone task; fold its
publish-gate requirement (T-117) into the linter's publish integration (T-048).**

**T-035 through T-039 (individual linter rules) are already sub-items of T-046.** The roadmap
lists them as five separate tasks in Phase 1. They are default rules in the linter catalogue —
not independent engineering efforts. **Consolidate T-035–T-039 into T-046's acceptance
criteria. Remove them from the Phase 1 task count.** This saves ~5 days of apparent task
overhead that was never real.

**T-041 mutation API covers 15+ methods, not 12.** The spec expanded the surface to include
`updatePanel`, `updatePlacement`, `removePlacement`, `movePanelToScene`, `setBubbleStyle`,
`setBubbleReadingOrder`, and `batch()`. The `batch()` method directly implements the core logic
needed by T-077 (multi-select move) and T-078 (bulk regenerate) — both reduce to
`doc.batch(label, fn)` calls. **Those tasks require UI wiring but no new mutation primitives.**

**Minor schema addition:** the linter's `ff/background/missing-background` rule needs a
`buildType: "placeholder" | "draft" | "published"` field at the FFScript document root to
distinguish authoring-time placeholder panels from publishable panels with missing backgrounds.
Add this field to T-021's schema and the v0.1 → v0.2 migration.

**Updated total pruned task count:** 19 (was 17; adds T-059, and the 5 linter sub-tasks
T-035–T-039 as standalone items are collapsed into T-046).

---

*Audit conducted by Claude (Anthropic) on 2026-04-23. FableForge codebase was not directly
accessible; all findings are grounded in the roadmap document's internal evidence.
Real-code verification is required for all items in §6 before implementation begins.*
