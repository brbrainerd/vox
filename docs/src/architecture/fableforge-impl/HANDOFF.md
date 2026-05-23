---
title: "FableForge — Developer Handoff Brief"
description: "FableForge Handoff Notes"
category: "Architecture SSOTs"
---
# FableForge — Developer Handoff Brief

**Produced:** 2026-04-23  
**Status:** Ready for implementation. All planning work is complete.  
**Blockers:** None architectural. One operational: field-name verification against the real codebase (15 items, ~2 hours).

---

## What this folder contains

| File | Addresses tasks | Target destination in FableForge repo |
|------|----------------|---------------------------------------|
| `panel-schema.ts`           | T-021, T-022, T-025, T-031 | `src/lib/ffscript/panel-schema.ts` |
| `mutations-doc.ts`          | T-041–T-045, T-051–T-054   | `packages/ffscript/src/mutations/doc.ts` |
| `linter-engine.ts`          | T-046–T-050, T-035–T-039   | `packages/ffscript/src/linter/engine.ts` |
| `cascade-delete.ts`         | T-191–T-193, T-195, T-205  | `convex/games/mutations.ts` (merge), `convex/lib/auth/requireGameOwner.ts` (new), `tests/integration/auth/game-siloing.test.ts` (new) |
| `generation-orchestrator.ts`| T-004, T-007, T-129, T-130 | `convex/lib/generation/orchestrator.ts` |
| `compiler.ts`               | T-023, T-105               | `src/lib/studio/compiler.ts` |
| `publish-gate.ts`           | T-115–T-117, T-206–T-207, T-209 | `convex/games/studio/actions.ts` (publishDraft), `convex/runtime/saves.ts` (new) |

All spec documents are in `docs/src/architecture/`:
- `fableforge-roadmap-audit-2026-04-23.md` — master audit + re-ranked task list
- `ffscript-panel-schema-spec-2026.md` — schema decisions + version contract
- `ffscript-mutation-api-spec-2026.md` — full class interface + Convex wiring pattern
- `ffscript-linter-design-2026.md` — rule catalogue + publish-gate integration

---

## Step 1: Verification results (2026-04-23 — verified against real repo)

All 15 claims have been checked against the live codebase. Items marked ✅ were correct;
items marked ❌ were wrong and the implementation files have already been patched.

| # | Verified result | Action taken / still needed |
|---|----------------|------------------------------|
| 1 ✅ | `CreateWizardPage.tsx` has zero Convex imports — confirmed mock | Delete it (T-001) |
| 2 ❌ | Index names differ per table. `characters` → `by_game_id`. `characterAnchors` → `by_character` (prefix). `locationAnchors` → `by_location` (prefix). `builds` → `by_game_id`. `saves` → `by_user_and_game` (filter needed). | **Patched** in `cascade-delete.ts`: CASCADE_TABLES now uses per-table `{ table, index }` pairs; saves handled via filter. |
| 3 ❌ | `gameDrafts` is missing `selectedProvider`, `coverImageKey`, `contentRating`. Only `ffscriptKey`, `title`, `userId` confirmed present. | **Patched** in `publish-gate.ts` + `generation-orchestrator.ts` with `/* SCHEMA ADDITION REQUIRED */` blocks. Add the three fields to `gameDrafts` in PR 3 (selectedProvider) and PR 5 (coverImageKey, contentRating). |
| 4 ❌ | `processBeatJob` does NOT hardcode `"fal"`. It delegates to `generateBeatAssets`, which handles routing internally. The real hardcoding issue is `studio/actions.ts:101` (LLM_MODELS.gpt4o — T-013, separate 1-line fix). | **Patched** in `generation-orchestrator.ts` comment block. |
| 5 ✅ | Version regex in `schema.ts` is `/^0\.\d+\.\d+$/` — confirmed | Change to `/^\d+\.\d+\.\d+$/` in `src/lib/ffscript/schema.ts` (line ~3043) as part of PR 2 |
| 6 ✅ | FFScript default version is `"0.1.0"` — confirmed in `defaults.ts:91` | Migration target in `panel-schema.ts` is correct |
| 7 ✅ | `studio/actions.ts:101` pins `LLM_MODELS.gpt4o` — confirmed exact line | Fix T-013: replace with `selectModelForTask({ purpose: "story_outline" })` |
| 8 ✅ | `deleteGame` skips: `saves`, `characterAnchors`, `locationAnchors`, `userGameLibrary`, `characterLibrary`, `backgroundLibrary` — confirmed | `cascade-delete.ts` handles all of these |
| 9 ❌ | `generatedAssets` has `assetKey` + `r2Url`, NOT `storageKey` | **Patched** in `cascade-delete.ts`: R2 key collection now uses `r.assetKey` |
| 10 ✅ | `users` has `by_clerk_id` index and `role` field — confirmed | No change needed |
| 11 ✅ | `timeline-save.ts` is 100% localStorage — no Convex calls | Safe to delete after PR 6 cloud saves go live |
| 12 ❌ | `BrowseClientPage.tsx` is NOT a stub — it's a real implementation with `useQuery`, `FeaturedGamesSection`, routing, etc. | T-231 is a UI redesign, not deletion. No change to implementation files. |
| 13 ✅ | `generation.ts` has silent `catch (e) { /* ignore */ }` at line 166 | Fix T-006: remove or surface this catch |
| 14 ❌ | `faceExtraction.ts` has NO silent catches — all catches return `err()` properly | No T-006 action needed for faceExtraction |
| 15 ❌ | `providers/router.ts` catch at line 55 logs via `logger.error()` — not silent | No T-006 action needed for this router |

---

## Step 2: PR sequence (ordered by dependency)

Open these PRs in order. Each is self-contained and passes existing tests before the next lands.

### PR 1 — Data safety (no UI dependencies, highest risk if delayed)
**Addresses:** T-001, T-006, T-191, T-192, T-193, T-195, T-205

**Changes:**
1. Delete `src/components/CreateWizardPage.tsx`; add redirect to `StudioWizard`
2. Remove the one confirmed silent catch: `generation.ts:166` (`catch (e) { /* ignore */ }`) — surface as a logged warning or `err()` return. (`faceExtraction.ts` and `providers/router.ts` were not silent — no action needed there.)
3. Add `deletedAt` field + `by_deleted_at` index to `games` table in schema
4. No generic `by_game` index additions needed — `cascade-delete.ts` has already been updated with per-table index names matching the real schema
5. Copy `cascade-delete.ts` → `convex/lib/auth/requireGameOwner.ts` (auth guard only)
6. Replace the body of `deleteGame` in `convex/games/mutations.ts` with `softDeleteGame` + `hardDeleteGame` from `cascade-delete.ts`
7. Add `purgeExpiredGames` scheduled function
8. Uncomment and wire the siloing test suite from `cascade-delete.ts`

**Test gate:** `pnpm test tests/integration/auth/game-siloing.test.ts` — all sibling-user access must throw FORBIDDEN.

---

### PR 2 — Panel schema + migration
**Addresses:** T-021, T-022, T-025, T-031

**Changes:**
1. Copy `panel-schema.ts` → `src/lib/ffscript/panel-schema.ts`
2. In `src/lib/ffscript/schema.ts`:
   - Change version regex to `/^\d+\.\d+\.\d+$/`
   - Extend the schema with `version: z.literal("0.2.0")`, `buildType`, `panels`, `scenes`
3. Write + run migration: `scripts/ffscript/migrate-all.ts` calls `migrateV01toV02` on every game row
4. Add `panels` denormalized cache table to `convex/schema.ts` (from T-027 spec)

**Test gate:** Every existing game validates as v0.2.0 post-migration. No saves break.

---

### PR 3 — Provider routing fix (T-004 quickfix)
**Addresses:** T-004, T-013

**Changes:**
1. In `games/studio/actions.ts:101`, replace `model: LLM_MODELS.gpt4o` with `selectModelForTask({ purpose: "story_outline" })`
2. Add `selectedProvider` field to `gameDrafts` schema (verified absent — required by `generation-orchestrator.ts`):
   ```ts
   selectedProvider: v.optional(v.union(v.literal("fal"), v.literal("comfyui"), v.literal("replicate")))
   ```
3. In `convex/actions/batchGeneration.ts`, replace the hardcoded FAL call with:
   ```ts
   const orchestrator = new ImageOrchestrator({ ... });
   const result = await orchestrator.generate({
     preferredProvider: draft.selectedProvider ?? null,
     ...
   });
   ```
   The full `ImageOrchestrator` from `generation-orchestrator.ts` goes in `convex/lib/generation/orchestrator.ts`.
3. Add `providerHealth` + `generationAttempts` tables to schema (from `generation-orchestrator.ts` schema comments).

**Test gate:** Changing `selectedProvider` on a draft to `"replicate"` causes the next generation to route through Replicate (observable in logs).

---

### PR 4 — FFScript mutation API
**Addresses:** T-041–T-045, T-051–T-054

**Changes:**
1. Copy `mutations-doc.ts` → `packages/ffscript/src/mutations/doc.ts`
2. Export from `packages/ffscript/src/index.ts`
3. Wire keyboard shortcuts in `src/components/editor/`: Cmd+Z → `doc.undo()`, Cmd+Shift+Z → `doc.redo()`
4. Run snapshot tests from the spec's test file list

**Test gate:** `pnpm test tests/unit/ffscript/mutations/` — all mutation round-trips pass.

---

### PR 5 — Linter + publish gate
**Addresses:** T-046–T-050, T-035–T-039, T-115–T-117

**Changes:**
1. Copy `linter-engine.ts` → `packages/ffscript/src/linter/engine.ts`
2. Wire linter into `updateDraft` mutation (health bar via `gameDrafts.lintReport`)
3. Wire `publishDraft` from `publish-gate.ts` (replace existing publish action)
4. Add `playtestToken` generation to the playtest UI flow
5. Wire `pnpm ffscript:lint` CLI
6. Add `coverImageKey` and `contentRating` fields to `gameDrafts` schema (verified absent — required by `publish-gate.ts`):
   ```ts
   coverImageKey:  v.optional(v.string()),
   contentRating:  v.optional(v.union(v.literal("sfw"), v.literal("pg13"), v.literal("r18")))
   // translate sfw/pg13 → "general", r18 → "mature" when writing to games table
   ```

**Test gate:** A game with a duplicate panel ID is rejected at publish. A clean game publishes successfully.

---

### PR 6 — Compiler + cloud saves
**Addresses:** T-023, T-105, T-206, T-207, T-209

**Changes:**
1. Copy `compiler.ts` → `src/lib/studio/compiler.ts` (replacing current implementation)
2. Copy `publish-gate.ts` save mutations → `convex/runtime/saves.ts`
3. Add `saves` table to schema (from `publish-gate.ts` schema comments)
4. In `FFScriptPlayer`, add the autosave hook from the `publish-gate.ts` comment
5. Wire `listSlots` to the load-game UI (T-210)

**Test gate:** Playing a game for 5 panels triggers an autosave record in Convex. Refreshing the browser offers "resume".

---

## Step 3: What is NOT in these files (needs separate work)

| Capability | Roadmap tasks | Estimated effort |
|------------|--------------|-----------------|
| Storyboard Panel Grid UI (`PanelGrid.tsx`) | T-061–T-100 | L–XL per component; needs a UI designer |
| Speech bubble canvas tool | T-067, T-069 | L; Pixi.js drag-drop |
| Monaco bidirectional sync | T-086, T-087 | XL; race condition risk |
| Flux Fill inpainting workflow | T-121–T-125 | L; ComfyUI workflow file needed |
| SAM 2 auto-masking | T-124 | L; ComfyUI node or fal endpoint |
| Mask brush UI | T-125 | L; HTML5 canvas |
| Real `/browse` marketplace | T-231 | L; full page rewrite |
| Age gate server enforcement | T-226 | M; Convex auth middleware |
| Game versions + rollback | T-199 | M; new table + UI |

These are all GUI or model-workflow work that require direct access to the FableForge repo.
The implementation files here cover the backend data model and business logic only.

---

## State after this planning phase

**Tasks addressed by implementation files in this folder:**

| Category | Tasks covered |
|----------|--------------|
| Schema + types | T-021, T-022, T-025, T-031, T-034, T-035 |
| Mutation API | T-041–T-045, T-051–T-054 |
| Linter | T-046–T-050, T-035–T-039 (collapsed) |
| Data safety | T-191–T-193, T-195, T-205 |
| Generation routing | T-004, T-007, T-013, T-129, T-130 |
| Compiler | T-023, T-105 |
| Publish gate | T-115–T-117 |
| Cloud saves | T-206–T-207, T-209 |

**Tasks remaining (need UI or model workflow work):**
T-061 through T-100 (storyboard), T-121–T-128 (inpaint/masks), T-151 (music), T-194 (global slugs), T-199 (versions), T-226 (age gate), T-231 (marketplace), T-237 (Ren'Py export), plus all Phase 12–14 items.

**Planning work is complete. No more specs are needed. Open PR 1.**
