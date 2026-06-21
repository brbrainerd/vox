---
title: "FableForge Combat Knowledge Graph"
description: "A directed knowledge graph spanning every layer of the Dystopia/FableForge combat engine, from legacy C to TypeScript."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
---

# FableForge Combat Knowledge Graph

Last modified: 2026-06-17T16:35:00-04:00

## Purpose

A directed knowledge graph (`graphify-out/`) spanning every layer of the Dystopia/FableForge 
combat engine, from the original GodWars C source through the TypeScript/Convex implementation.
Used for semantic search to continuously audit the C→TS migration and plan the generic combat engine.

## Graph Stats

- **4,838 nodes · 13,683 edges · 198 communities**
- Extraction: 78% EXTRACTED · 22% INFERRED · 0% AMBIGUOUS
- Built from 728 files (~712k words), AST only (no LLM tokens consumed)

## Files

| File | Purpose |
|------|---------|
| `graphify-out/graph.html` | Interactive vis.js browser graph |
| `graphify-out/graph.json` | GraphRAG-ready JSON |
| `graphify-out/GRAPH_REPORT.md` | Audit report with all community labels |
| `graphify-out/manifest.json` | File manifest for `--update` incremental runs |
| `graphify-out/.graphify_labels.json` | Community ID → name map |
| `graphify-out/.graphify_python` | Resolved interpreter path (Python 3.12) |
| `graphify-out/.graphify_root` | Scan root for `--update` |

## Corpus Sources

1. `C:\Users\Owner\DystopiaGold-main\dystopia-legacy\src\` — 75 legacy C/H files
   - Note: excluded by `.gitignore` (`dystopia-legacy/`) so manually injected
2. `packages/dystopia-core/src/` — 445 TypeScript files (combat SSOT)
3. `convex/dystopia/` — 34 Convex server files
4. `convex/village_game/combat/` — 2 files (mutations, consequences)
5. `packages/fableforge-combat/src/` — 1 file
6. `packages/game-systems/src/` — 53 files
7. `packages/ffscript/src/` + `packages/ffscript-core/src/` — 25 files
8. `packages/shared-config/src/` — 93 files

## Key God Nodes (Most Interconnected)

| Node | Degree | Layer | Notes |
|------|--------|-------|-------|
| `one_argument()` | 304 | Legacy C | String parsing utility — called everywhere in legacy C |
| `number_range()` | 233 | Legacy C | RNG — central to all combat math |
| `is_affected()` | 167 | Legacy C | Status effect check — foundation of affect system |
| `str_cmp()` | 144 | Legacy C | String compare — C command dispatch workhorse |
| `get_char_room()` | 142 | Legacy C | Room lookup — core of C combat targeting |
| `CHAR_DATA` | 132 | Legacy C | Core character struct — everything touches it |
| `damage()` | 93 | Legacy C | Main damage fn — bridges 7+ communities |
| `deriveCombatMetrics()` | high-betweenness | TypeScript | Bridges TS Combat Math, Damage Formulas, Class Skills |

## Key Community Labels (Combat-Focused)

| ID | Label | Key Files |
|----|-------|-----------|
| 0 | Legacy C Combat Core | magic.c, fight.c, damage(), dice() |
| 3 | Legacy C Magic System | magic.c, CHAR_DATA, affect_to_char |
| 15 | Legacy C Fight Resolution | fight.c, kav_fight.c, one_hit, darkheart |
| 7 | Legacy C Arena & Movement | arena.c, act_move.c, do_recall |
| 35 | Legacy C Stealth Classes | assassin.c, do_ambush, do_assassinate |
| 14 | Legacy C Vampire & Lich | vamp.c, lich.c, set_fighting, callgolems |
| 39 | Legacy C Ninja & Samurai | ninja.c, samurai.c, tanarri.c, one_hit |
| 41 | TS Arena Engine Core | arenaEngine.ts, CombatResolver, tests |
| 25 | TS Damage Formulas | damageFormulas.ts, resolveOneHit, kekkaishi |
| 28 | TS Combat Math & Tests | combatMath.test, resolveOneHit, specCastMage |
| 10 | TS Combat Command Pipeline | extendedCombat, combatInputQueue, LimbSlot |
| 23 | TS Combat Input Queue | arenaWorkflow, queueCommands, run.ts |
| 18 | TS Affect / Status System | affects.ts → AFFECT_DATA port |
| 43 | TS Religion Engine | religionEngine.ts → religion.c port |
| 26 | FFScript State Bridge | ffscriptAlignment.ts, MudStateSnapshot |
| 50 | Generic Combat Adapters | ADAPTER_REGISTRY, listRegisteredSystems |

## Import Cycles Detected (Action Required)

Critical cycles in TS combat layer:
- `arenaEngine.ts → CombatResolver.ts → miracles.ts → arenaEngine.ts` (3-cycle)
- `championFlags.ts → skills.ts → world/types.ts → championFlags.ts` (3-cycle)
- `commands/combat.ts → combatAction.ts → run.ts → index.ts → combat.ts` (4-cycle)
- Multiple `*Skills.ts → skills.ts → *Skills.ts` 2-cycles (skills registry pattern)

## Surprising Connections Found

- `update_handler()` → `highlander_update()` [INFERRED] — update tick calls Highlander-specific logic
- `interpret()` → `log_string2()` [INFERRED] — command interpreter touches jobo logging
- `damage()` bridges 7 legacy C communities — true god function, must be faithfully ported

## Migration Gap Analysis

The 1,383 isolated nodes (≤1 connection) mostly represent:
- Legacy C struct fields (OBJ_DATA, EXIT_DATA, CHAR_DATA sub-fields)
- These indicate areas where C mechanics have NO TS counterpart yet → migration backlog

## How to Query

```powershell
# From fableforge repo root:
& "C:\Users\Owner\AppData\Local\Programs\Python\Python312\python.exe" -m graphify query "How does damage() work?"
# Or run the pipeline again with --update for incremental refresh:
# (re-run Step 3+ of the graphify pipeline using the existing detect/manifest)
```

## Rebuild Command

To update after code changes:
```powershell
# Re-run AST extraction + graph build (skips semantic for code-only corpus)
& "C:\Users\Owner\AppData\Local\Programs\Python\Python312\python.exe" graphify-out/label_communities.py
```
