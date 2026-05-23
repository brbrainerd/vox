---
title: "Vox bell-curve strategy"
description: "Program SSOT for the narrow app-software scope, product lanes, ranking model, and rollout status used by the Vox bell-curve work."
category: "Architecture SSOTs"
last_updated: "2026-03-28"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Vox bell-curve strategy

## Program status

- `status`: `in_progress`
- `scope`: center-of-bell-curve app software
- `design_center`: common app software first, with strong AI-generation ergonomics and explicit escape hatches

## Target software categories

Vox is optimizing for:

1. CRUD and line-of-business web apps
2. internal tools and operator consoles
3. content, admin, and research workflow apps
4. API-backed dashboards and portals
5. automation and background job systems
6. AI-assisted application scaffolding, repair, and orchestration

## Non-goals

Vox is not currently trying to become:

- a universal systems language
- a framework-neutral frontend platform
- a first-class host for arbitrary Rust or JS APIs
- a scientific-computing language
- a multi-frontend-target language before WebIR owns the current web path

## Product lanes

Use these lane ids in contracts, docs, command metadata, examples, and future dashboards:

| `product_lane` | Meaning | Typical surfaces |
|----------------|---------|------------------|
| `app` | typed web app construction | `build`, `run`, `island`, WebIR, AppContract |
| `workflow` | background work, automation, durable-ish task flows | `script`, `populi`, workflow runtime |
| `ai` | model generation, eval, review, orchestration, speech | `mens`, `review`, `dei`, `oratio` |
| `interop` | approved integration surfaces and escape hatches | `openclaw`, `skill`, bindings, wrappers |
| `data` | database and publication workflows | `db`, `codex`, `scientia` |
| `platform` | packaging, install, compliance, diagnostics, secrets | `pm`, `ci`, `clavis`, `doctor` |

## Ranking model

Every bell-curve addition should score against the same dimensions:

| Dimension | Weight | Question |
|-----------|--------|----------|
| `bellCurveReach` | 30 | How many common app tasks does this unlock? |
| `llmLeverage` | 25 | How much prompt/repair burden does it remove? |
| `surfaceStability` | 20 | Does it fit current IR, registry, and runtime boundaries cleanly? |
| `implementationRisk` | 15 | What compiler/runtime/docs migration risk does it introduce? |
| `driftReduction` | 10 | Does it eliminate duplicate semantics or conflicting docs/code? |

## Proposal template

Use this checklist for stdlib, interop, workflow, and measurement proposals:

| Field | Required content |
|-------|------------------|
| lane | one `product_lane` from the table above |
| user_problem | narrow statement of the common task being improved |
| preferred_boundary | `WebIR`, `AppContract`, `RuntimeProjection`, builtin registry, approved binding, or docs-only |
| fallback_escape_hatch | how uncommon cases work without broadening the main surface |
| ranking | score all five ranking dimensions |
| semantics_state | `implemented`, `partially_implemented`, `planned`, or `docs_only` |
| drift_risk | what could diverge if the proposal lands incompletely |
| acceptance | tests, docs, and contract gates needed before release |

## Promise language

All docs in this program should explicitly label one of these states when a surface is easy to over-claim:

- `implemented semantics`
- `planned semantics`
- `language intent`
- `escape hatch`

This is especially important for workflows, frontend emission ownership, and interop claims.


