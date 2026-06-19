---
title: "Village Narrative Architecture Research Audit"
description: "A research audit on game structure for facilitating narrative, focusing on FableForge village."
category: "architecture"
status: "current"
training_eligible: true
---

# KI: Village Narrative Architecture — Research Audit

**Last modified: 2026-06-17T09:14 EDT**
**Source conversation:** 3d92046d-6bbc-4be3-b028-1eeb8fde9ca9
**Full artifact:** `C:\Users\Owner\.gemini\antigravity\brain\3d92046d-6bbc-4be3-b028-1eeb8fde9ca9\narrative_audit.md`

## Summary

A comprehensive research audit was conducted on how games should be structured to facilitate good narrative, with specific application to FableForge village. Two research agents ran in parallel: one on game design theory (38 findings, 40+ sources) and one on LLM narrative systems (45 findings, 30+ papers).

## Five-Layer Narrative Framework (Research Consensus)

1. Authored Core — fixed story beats / dramatic milestones (pearls on the string)
2. Narrative Architecture — locations as story containers with embedded history
3. Simulation/System — NPCs with goals, consequence systems, faction dynamics  
4. Drama Management — active curation of pacing and tension
5. LLM/Procedural Language — LLM as narrator ONLY, never state owner

## Critical LLM Constraints

- "Lost in the Middle" (Stanford/ACL): U-shaped attention — critical instructions at beginning/end of context only
- LLMs cannot plan ahead — arc/chapter level must be human-authored
- LLMs are narrators, not simulators — Convex DB owns world state

## Bugs Identified (High Severity)

- B1: `hearRumor` always generates fresh LLM rumors; never queries `worldRumors` table
- B2: `getPacingRecommendation` hardcoded "maintain" — never reads `storyBeats`
- B3: `recordStoryBeat` inserts to DB but nothing reads/consumes `storyBeats`
- B4: No `record_story_beat` FFScript node — quest scripts are narratively silent
- B7: No `location_narrative_op` FFScript node — locations don't record events
- B12: No `narrativeHistory`/`narrativeTags`/`narrativeWeight` on locations

## Preliminary Recommendations (Not Yet Implemented)

- R1: Narrative-Aware Location Schema
- R2: Fix Rumor Loop (query before generate)
- R3: Activate Drama Manager (read storyBeats, return actionable recommendations)
- R4: 5 new FFScript narrative node types
- R5: Dual Timeline (World Chronicle + Player Chronicle)
- R6: NPC Memory Types expansion

## Implementation Status

Research only. Implementation plan pending user approval of direction.

## Key Academic References

- Jenkins (2004): Four modes of spatial storytelling
- Mateas & Stern, Facade (2005): Drama manager
- "Lost in the Middle" (Stanford/ACL): U-shaped context attention
- SWAG (EMNLP 2024): Story generation as search (arXiv:2402.03483)
- DOME (NAACL 2025): Dynamic hierarchical outline + temporal KG (arXiv:2412.13575)
- NarrativeGenie (AIIDE 2024): Partially ordered event graphs
- James Ryan (UCSC PhD): Story sifting / curationism
