---
title: "Vox V0.5 Crate Reorganization and Stability SSOT (2026)"
description: "Authoritative tier classification for all 64 workspace crates, React-as-primary-target declaration, and the strategy for surfacing maturity warnings. Prerequisite for V1.0 scope discipline."
category: "Architecture SSOTs"
status: "current"
domain: "P"
last_updated: "2026-04-18"
training_eligible: false
training_rationale: "Defines the governed scope of the V0.5 workspace and the maturity contract LLMs must reason about when generating Vox code."
schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Vox V0.5 Crate Reorganization and Stability SSOT (2026)

> [!IMPORTANT]
> This document supersedes any prior crate-tiering proposals. It is the
> single source of truth for crate stability classification, React-first
> targeting policy, and the mechanism for surfacing underdevelopment
> warnings across the monorepo.

## 1. Context and framing

The workspace is at **v0.4.0** with 64 crates, 280+ architecture docs, and
an audit verdict of "best governance at this tier, zero empirical evidence
for its headline claim, and too sprawled to ship." This document does not
retire the sprawl — it **governs** it by assigning every crate a maturity
tier and encoding that tier as machine-readable metadata.

The goal is **V0.5**: a state where:

- The core path (compiler → runtime → web → deploy) is reliable enough
  for a solo dev to ship a real app.
- Every crate outside the core path is explicitly labeled so LLMs,
  agents, and contributors know what is stable.
- Gamification (`vox-ludus`) is active and maintained.
- All other research and experimental crates remain in the monorepo but
  carry compiler-surfaced warnings when consumed by production builds.
- React is the declared and only supported web target. Solid, Svelte,
  and Next.js adapter work is shelved.

---

## 2. Tier definitions

| Tier | Label | Meaning | Build guarantee |
|------|-------|---------|----------------|
| **Core** | `stability = "core"` | V1-track. Breaking changes require an ADR and migration hint in every error. Must compile clean on all features. | Always green on `main` |
| **Active** | `stability = "active"` | Under active development. API unstable; breaking changes expected between minor versions. Works but may have gaps. | Best-effort on `main` |
| **Incubating** | `stability = "incubating"` | Early-stage or research-grade. Not recommended for production use. May have significant gaps or unimplemented stubs. | CI may be optional |
| **Frozen** | `stability = "frozen"` | Compile-only — no new features, no migration path yet. Kept for dependency completeness. | Must compile; no new features |
| **Decision-pending** | `stability = "decision-pending"` | Requires explicit keep/archive/merge decision before V0.6. Blocked from new feature work. | Compile required |

archived_date: 2026-04-18
---

## 3. Crate tier assignments

All 64 crates from `Cargo.toml` workspace members (v0.4.0, April 2026).

### 3.1 Core tier (10 crates)

These crates are on the V1.0-track. Stability guarantees apply.
Zero P0 issues open against any of these is a V0.5 exit criterion.

| Crate | Role | V0.5 gate |
|-------|------|-----------|
| `vox-compiler` | Lexer/parser/HIR/IR/codegen (Rust + TS) | Parse all golden examples; `cargo check --all-features` green |
| `vox-cli` | User-facing entry point, template scaffold | `vox new web` → runnable Vite+React app in < 10 min |
| `vox-runtime` | App runtime, Axum host, SSR orchestration | Zero crash in 30-day soak on reference app |
| `vox-db` | `@table` → SQL → typed client + server | `@table` → typed query round-trip golden test |
| `vox-clavis` | Secret resolution (all API keys / tokens) | `vox clavis doctor` passes on fresh checkout |
| `vox-orchestrator` | DEI agent dispatch, A2A, task lifecycle | `vox dei start` stable; OOPAV state machine encoded |
| `vox-primitives` | Shared types, errors, spans, IDs | No external deps; must be dep-free |
| `vox-config` | Layered config, precedence rules | Config SSOT parity test green |
| `vox-crypto` | ChaCha20Poly1305 AEAD, pure-Rust only | No AEGIS, no ring, no cmake/nasm |
| `vox-toestub` | Governance: God Object, TOESTUB, line limits | CI gate never bypassed |

### 3.2 Active tier (28 crates)

Working, in active development, API unstable. A `#[stability = active]`
Cargo metadata annotation is emitted; `vox doctor` warns when these are
consumed in a "production" build profile.

| Crate | Domain | Primary gap |
|-------|--------|-------------|
| `vox-ludus` | Gamification (XP, achievements, quests) | Stable; maintained; V1.1 candidate |
| `vox-mens` | ML training pipeline (QLoRA, Candle) | Full-graph backward pass (MENS Gap A) |
| `vox-tensor` | Tensor ops, Burn 0.19, autodiff | Nested LoRA serving (MENS Gap D) |
| `vox-populi` | GPU mesh, NVML probes, lease semantics | Lease semantics (ADR 017) not yet shipped |
| `vox-lsp` | Language server | Decoupled from live HIR; stale diagnostics |
| `vox-workflow-runtime` | Durable task workflows | Interp/generated replay determinism (ADR 021) |
| `vox-orchestrator`* | *(Also Core — see §3.1)* | — |
| `vox-scientia-core` | RAG core, BM25+FTS5+vector+RRF | Symbol-proximity detector active; CRAG partial |
| `vox-scientia-api` | Scientia REST API | Worthiness gate wiring incomplete |
| `vox-scientia-ingest` | Document ingest, chunking | Semantic chunking done; auto-classify partial |
| `vox-scientia-runtime` | Scientia serving runtime | Socrates gating partial |
| `vox-scientia-social` | Social publishing, Bluesky/Mastodon | `social_retry` wired; channel allow-list gaps |
| `vox-search` | Tantivy FTS, vector, RRF fusion | `vox ci proximity-drift` gate not yet enforced |
| `vox-grammar-export` | GBNF / XGrammar-2 export | ~30-line stub; CVE-2026-2069 downstream path |
| `vox-constrained-gen` | Grammar-constrained decoding | XGrammar-2 not adopted |
| `vox-corpus` | Training corpus builder, JSONL pipeline | < 100 k organic threshold not yet reached |
| `vox-eval` | Model evaluation harness | HumanEval-Vox not yet published |
| `vox-skills` | ARS skill dispatch | Socrates gating partial |
| `vox-schola` | Learning / curriculum system | Dependency on Scientia; partial |
| `vox-oratio` | Speech-to-code, ASR pipeline | Sherpa-ONNX wired; MENS integration partial |
| `vox-mcp-registry` | MCP tool registry (102 tools) | Tier segmentation (core ≤ 20) not yet shipped |
| `vox-mcp-meta` | MCP metadata, manifests | Stable metadata shape; no tiering yet |
| `vox-git` | Git / JJ VCS integration | JJ optional dep; Windows file-lock stable |
| `vox-repository` | Repository-scoped operations | Scoped orchestrator bridge stable |
| `vox-identity` | User identity, session management | JWT + ed25519 wired; MCP OAuth 2.1 pending |
| `vox-protocol` | A2A protocol types | Stable wire format; trust-tier RBAC pending |
| `vox-doc-pipeline` | Doc processing, doctest runner | Build break fixed (April 2026); gap: idempotent `vox fmt` |
| `vox-doc-inventory` | Doc inventory, drift detection | `vox ci ssot-drift` lane works; auto-archive not yet wired |

> [!NOTE]
> `vox-ludus` is explicitly kept **Active** (not Incubating). Gamification
> is a maintained feature; it integrates with the workflow visualizer for
> XP and achievement encoding and is in steady development.

### 3.3 Incubating tier (13 crates)

Research-grade or early-stage. `vox doctor` emits a `WARN` when these
are depended upon outside their own test suite or a sibling incubating
crate. New feature work is allowed but no stability promises.

| Crate | Why incubating | Path to Active |
|-------|---------------|----------------|
| `vox-ssg` | Static site generation; no golden coverage | Define SSG use-case and golden |
| `vox-pm` | Package manager; core model unspecified | ADR required |
| `vox-publisher` | Social publishing; multi-channel gaps (13-col gap table) | Complete scientia-pipeline-ssot G1–G10 |
| `vox-forge` | Build/forge orchestration; role overlaps cli | Merge or scope boundary ADR |
| `vox-socrates-policy` | Confidence gating; partial wiring to skills | Wire into `@island` / `@server` generation |
| `vox-scaling-policy` | Scaling rules; runtime projection gap | Link to RuntimeProjection boundary |
| `vox-openai-sse` | OpenAI SSE streaming; no public consumer yet | Wire into `@llm` dispatch |
| `vox-openai-wire` | OpenAI wire protocol types | Merge with `vox-openai-sse` or keep as types-only |
| `vox-webhook` | Inbound webhook handling | Needs runtime route registration |
| `vox-audio-ingress` | Audio capture, resampling (rubato/symphonia) | Depends on Oratio stabilization |
| `vox-browser` | Browser automation, Playwright glue | No V0.5 consumer; park until Visus lane |
| `vox-project-scaffold` | Project template logic | Merge into `vox-cli` templates or keep isolated |
| `vox-codex-api` | Codex SSE bridge to orchestrator | Agentic planning V2 depends on it; stabilize there |

### 3.4 Frozen tier (7 crates)

Compile-only. No new features. No new dependents without an ADR.
Kept in workspace for dependency graph completeness.

| Crate | Reason frozen |
|-------|---------------|
| `vox-bounded-fs` | Landlock/Win32Job sandbox; correct but niche |
| `vox-build-meta` | Build-time constants; stable, no changes expected |
| `vox-checksum-manifest` | BLAKE3 manifest integrity; correct and complete |
| `vox-install-policy` | Platform install rules; stable |
| `vox-jsonschema-util` | JSON schema helpers; depends on `schemars`/`jsonschema` |
| `vox-reqwest-defaults` | HTTP client defaults; migration to this crate complete |
| `vox-capability-registry` | Capability contract rows; driven by CI, not code |

### 3.5 Decision-pending tier (5 crates)

Must have an explicit keep / archive / merge decision before V0.6 branch.
No new feature work allowed. Blocked on human decision.

| Crate | Situation | Decision options |
|-------|-----------|-----------------|
| `vox-dei` | Stub `lib.rs`. Active CI import-ban guard. 0 real code. | (a) Resurrect with real agent-daemon code. (b) Delete and update CI guard to reference `vox-orchestrator` directly. |
| `vox-container` | Docker/OCI layer; no clear V0.5 consumer path | (a) Keep for deploy story. (b) Archive until deploy lane. |
| `vox-bootstrap` | Thin cargo-run launcher; overlap with scripts/ | (a) Formalize as the only bootstrap. (b) Merge logic into `vox-cli` install command. |
| `vox-tools` | Catch-all tools; unclear boundary | (a) Enumerate owned tools and split to correct crates. (b) Archive. |
| `workspace-hack` | cargo-hakari workspace-hack | Keep; verify in CI (currently not verified). |

---

## 4. React-as-primary-target declaration

> [!IMPORTANT]
> **React 19 is the sole declared web codegen target for V0.5 and V1.0.**

This decision, confirmed April 2026, collapses the following:

### 4.1 What is now canonical

- **Output:** Named-export React TSX + `routes.manifest.ts` + `vox-client.ts`
  (typed fetch). This is the React interop charter (see
  [react-interop-migration-charter-2026.md](./react-interop-migration-charter-2026.md)).
- **Runtime glue:** Vite 8 + user-owned `App.tsx`. The compiler stops at
  manifest + components + client. Frameworks own router construction.
- **Component model:** `component Name() { }` blocks, lowered to React hooks
  by the compiler. `@component fn` is a hard error with migration hint.
- **State:** `state`, `derived`, `effect` compiler-owned keywords — no raw
  `use_state` import from `.vox` source. JSX leakage to be progressively
  eliminated (see §4.3).
- **CSS:** Compiler-owned `style:` blocks → emitted CSS modules or inline
  styles. Authors do not write raw `className="…"` strings.

### 4.2 What is now shelved (do not implement until V1.1 review)

| Shelved item | Reason |
|---|---|
| Solid.js / Svelte 5 / Next.js adapter | Multi-backend rejected; React-only focus |
| Vox-native reactivity DSL (Path C) | User confirmed "React-embedded forever" |
| Next.js wedge adapter (`@vox/next`) | Premature ecosystem play |
| TanStack router tree emission | Replaced by manifest-first routing |
| `createServerFn` codegen | Retired in charter; no new use |

### 4.3 JSX leakage remediation (V0.5 work item)

Raw JSX in `.vox` source (`<div className="…" onClick={…}>`) contaminates
the MENS corpus with React idioms and breaks the K-complexity thesis.

The phased plan:

1. **Now:** `DeprecatedUsageDetector` warns on raw JSX strings in `vox check`
   output (not just CI). `@component fn` is already an error.
2. **V0.5:** Compiler-owned `view { }` block replaces raw JSX in new goldens.
   Old goldens frozen in place with `// vox:corpus-legacy` annotation.
3. **V1.0:** Legacy JSX paths deleted from codegen; all goldens migrated.

archived_date: 2026-04-18
---

## 5. How to identify underdeveloped areas: the warning surface strategy

Rather than retiring code, the V0.5 strategy is to **encode maturity in
machine-readable metadata and surface warnings at the right moment**.

### 5.1 `[package.metadata.vox]` Cargo annotation (SSOT)

Every crate's `Cargo.toml` must carry:

```toml
[package.metadata.vox]
stability = "core"          # core | active | incubating | frozen | decision-pending
product_lane = "app"        # app | workflow | ai | interop | data | platform
v1_track = true             # only for Core tier
known_gaps = [              # free-form; consumed by vox doctor and CI
  "correction_hint missing in 30/31 HIR validation sites",
]
```

This is the single source of truth for tier information. All derived
surfaces (doctor output, CI reports, docs) read from here.

### 5.2 `vox doctor` maturity warnings

`vox doctor` gains a **maturity lane** that:

1. Reads `[package.metadata.vox.stability]` from every crate in the
   dependency closure of the current project.
2. Emits a `WARN` for each `incubating` or `decision-pending` crate
   reached from a production build target (i.e., not `[dev-dependencies]`).
3. Emits an `ERROR` if a `decision-pending` crate is depended on without
   an explicit `[package.metadata.vox.decision_approved_by]` field.
4. Lists `known_gaps` from each `active` crate in a collapsible section.

### 5.3 CI maturity gate

The `vox ci maturity-gate` command:

- Enforces that every crate has a `[package.metadata.vox]` block.
- Fails if a `Core` crate declares `known_gaps` items marked `[P0]`.
- Emits a Markdown report of all `incubating` crates and their gap counts.
- Runs on every PR touching `crates/`.

### 5.4 Docs front-matter `stability` echo

Every `docs/src/` page that corresponds to a crate must echo the crate's
tier in its YAML front-matter:

```yaml
crate_stability: "active"   # mirrors [package.metadata.vox.stability]
```

The `vox ci ssot-drift` gate checks that these two agree.

---

## 6. V0.5 exit criteria (the minimum bar)

V0.5 is done when **all three** of the following are true:

| # | Criterion | Measurable signal |
|---|-----------|------------------|
| 1 | **Core path is reliable** | All 10 Core crates: `cargo check --all-features` green; zero P0 issues open; 43 golden examples parse and emit. |
| 2 | **Golden path works end-to-end** | `vox new web` → `vox run` → browser renders app; `vox db migrate` → typed query works. Sub-10-minute on a fresh machine. |
| 3 | **Maturity metadata is complete** | Every crate has `[package.metadata.vox]`; `vox ci maturity-gate` passes on `main`. |

V0.5 does **not** require:

- MENS training pipeline green (Active tier, V1.1)
- HumanEval-Vox published (shelved)
- Grammar-constrained decoding complete (Active tier)
- Marquee production app shipped (V1.0 gate)

archived_date: 2026-04-18
---

## 7. What stays in the monorepo

Everything stays. No crates are evicted. The monorepo is the right shape
for a solo + AI-agents team at this scale. The tiers govern **behavior**,
not **location**:

- Core crates: full CI, full docs, full test coverage required.
- Active crates: best-effort CI; `vox check` must pass; `known_gaps` tracked.
- Incubating crates: must compile; tests optional; zero stability promise.
- Frozen crates: compile-only; no-merge-without-ADR rule.
- Decision-pending: human decision required before V0.6.

---

## 8. Priority action list (sequenced for V0.5)

Derived from the April 2026 audit v2, reshaped for the solo-dev-to-
production posture and the React-primary decision.

### Week 1 — stop the bleeding, make scope honest

| # | Task | Crate(s) | Audit ref |
|---|------|----------|-----------|
| 1 | Add `[package.metadata.vox]` block to all 64 crates | all | §5.1 |
| 2 | Wire `vox ci maturity-gate` CI lane | `vox-toestub`, CI | §5.3 |
| 3 | Resolve `vox-dei` decision: resurrect or delete | `vox-dei` | §3.5 |
| 4 | Delete `spec.rs.bak` (199k lines) and add to `.gitignore` | root | P3/A.14 |
| 5 | Delete stale `check.log`, `parser_errors.txt` from repo root | root | P0/A.3 |
| 6 | Make `VOX_EXAMPLES_STRICT_PARSE=1` default in CI | `vox-compiler` | P2/A.13 |

### Weeks 2–3 — the product is `vox new web`

| # | Task | Crate(s) | Audit ref |
|---|------|----------|-----------|
| 7 | Script-mode parser: `parse_script` wrapping top-level statements in implicit `fn main()` | `vox-compiler` | P0/A.1 |
| 8 | `vox new web` → Vite + React 19 + `vox run` single command; golden path in docs | `vox-cli`, `vox-project-scaffold` | P0/B.28 |
| 9 | Eliminate raw JSX from `.vox` goldens; warn in `vox check` | `vox-compiler` | P1/B.16 |
| 10 | Wire `DeprecatedUsageDetector` into default `vox check` output | `vox-compiler` | P2/A.10 |
| 11 | Default `vox run` to SSR-orchestrated mode without env flag | `vox-runtime`, `vox-cli` | P1/B.20 |

### Weeks 4–6 — first real user, keep app alive

| # | Task | Crate(s) | Audit ref |
|---|------|----------|-----------|
| 12 | Runtime crash recovery: supervisor trees, graceful degradation | `vox-runtime`, `vox-orchestrator` | P1/E.49–51 |
| 13 | Structured logs + error reporting + uptime probe | `vox-runtime`, `vox-orchestrator` | P1/F.62–64 |
| 14 | `vox fmt` parse-print-reparse idempotence CI gate | `vox-compiler` | P2/A.12 |
| 15 | `correction_hint` in every HIR validation site (31 sites) | `vox-compiler` | P1/A.5 |
| 16 | `vox doctor` maturity lane (reads `[package.metadata.vox]`) | `vox-cli` | §5.2 |

### Months 2–3 — quality ratchet

| # | Task | Crate(s) | Audit ref |
|---|------|----------|-----------|
| 17 | Parallel error collection in parser (replace first-error-bail) | `vox-compiler` | P0/A.2 |
| 18 | Route loader auto-type from `@query` signatures | `vox-compiler` | P1/B.19 |
| 19 | `vox lsp` connected to live HIR validator | `vox-lsp` | P1/F.60 |
| 20 | `workspace-hack` verified in CI (cargo-hakari) | `workspace-hack` | P2/H.83 |

archived_date: 2026-04-18
---

## 9. Deferred scope (do not start before third production user)

The following are valid long-term work but are not V0.5 or V1.0 scope:

- HumanEval-Vox / SWE-bench-Lite-Vox (labs are not the audience)
- GBNF / XGrammar-2 completion (research pillar)
- MENS training pipeline hardening (research pillar)
- Scientia/RAG hardening beyond current state
- SSoT generator consolidation (solo team pain is manageable)
- Foundation governance / Open Collective bylaws
- Weekly public status posts
- GitHub Linguist submission
- Public MCP endpoint hosting

---

## 10. Relation to other SSOT documents

| This doc governs | See also |
|---|---|
| Crate tier assignments | [feature-growth-boundaries.md](./feature-growth-boundaries.md) |
| React-primary declaration | [react-interop-migration-charter-2026.md](./react-interop-migration-charter-2026.md) |
| Scope discipline | [vox-bell-curve-strategy.md](./vox-bell-curve-strategy.md) |
| `vox-dei` decision | [AGENTS.md](../../../AGENTS.md) §Retired Surfaces |
| `vox-ludus` (gamification) | [Vox Ludus KI](../../../crates/vox-ludus/) |
| CI governance | [governance.md](../../agents/governance.md) |
| Classification taxonomy | [classification-ssot-2026.md](./classification-ssot-2026.md) |


