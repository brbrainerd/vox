---
title: "Boilerplate Reduction — Gap Analysis Against Vox's Existing Backlog (2026)"
description: "Code-audited row-by-row reconciliation of the 25 categories in the boilerplate-reduction design brief against Vox's actual implementation state. Distinguishes Shipped / Spec'd-Not-Shipped / Partially Built / No-Spec-Yet / Out-of-Scope. Each graft (GA-01..GA-23) is filed as a self-contained Sonnet-4.6-followable task block with preconditions, files-to-read, files-to-modify, acceptance criteria, verification commands, and P-stack rubric."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-05-09"
training_eligible: true
training_rationale: "Reconciles outside-in framing with the inside-out backlog and with the actual code state. Verdicts are evidence-backed, not inferred. Useful as the authoritative answer to 'is this category shipped, spec'd, or unaddressed?' for each of the 25 industry-shaped categories."
---

# Boilerplate Reduction — Gap Analysis Against Vox's Existing Backlog (2026)

## §0 — How this document is structured

The companion document — [Boilerplate Reduction Design Brief (2026)](boilerplate-reduction-design-brief-2026.md) — proposes 25 ranked categories of language-level boilerplate reduction. This document maps each category onto Vox's *actual* implementation state, not just its design surface.

- §1 defines the verdict tiers used in the table.
- §2 is the 25-row reconciliation table, with code-audit evidence for every verdict.
- §3 records the audit findings — including corrections to verdicts that an earlier draft of this document got wrong.
- §4 lists the **graft tasks (GA-01..GA-23)** as **self-contained Sonnet-4.6-followable task blocks**. Each block is written so that a Claude Sonnet 4.6 agent picking up the file cold has everything required to execute the graft without re-reading the brief or chasing context.
- §5 lists anti-recommendations (framings that should *not* graft into the backlog).
- §6 lists cross-references.

## §1 — Verdict tiers

The brief's original gap analysis used a 4-tier scheme that conflated "spec'd in a CC-XX item" (which means *named gap to build*) with "Already Covered." The 2026-05-09 code audit forced a corrected 5-tier scheme:

| Tier | Symbol | Meaning |
|---|---|---|
| Shipped | ✅ | Implementation is in the codebase and exercised by tests. The brief's category is solved at the design level the brief asks for, modulo polish. |
| Spec'd, Not Shipped | 📋 | A design document exists (CC-XX item, Phase spec, SSOT addendum) but **no parser, runtime, or codegen entries** for it exist yet. The "graft" for these categories is the *implementation work itself*. |
| Partially Built | 🟡 | Some pieces shipped, others remain. A concrete extension task is named below. |
| No Spec Yet (Genuine Delta) | 🔵 | No design document. Needs a new addendum to the closest existing SSOT. |
| Out of Scope for Language Layer | ⚫ | Real category, but better addressed at the framework / runtime / library tier per Vox's design priorities. |

> **Important.** A 📋 verdict means the category looks "covered" if you only read the spec docs, but **is not covered in the running code**. Treating 📋 rows as if they were ✅ would mis-lead any roadmap call.

## §2 — Reconciliation Table (code-audited, 25 rows)

> Audit conducted 2026-05-09 against the worktree at `cc_bdesktop2/goofy-yonath-db8222`. The "Audit evidence" column cites the file path or test that supports each verdict.

| # | Brief category | Verdict | Audit evidence | Existing surface | Net new work |
|---|---|---|---|---|---|
| 1 | Async/data-fetching state (loading/error/empty/optimistic/stale) | 🟡 Partially Built | Reactive `state`/`derived`/`effect` shipped (see row 7). `@loading` token exists and parses on `fn` declarations; HIR carries it (`HirLoading`); **no codegen**. `@cancellable` exists for lambda/fn expressions (sets a flag, not used downstream). The full `Async[T]` tagged-union value with exhaustive view-arm matching is not yet a type. A1-04 (loading/empty slot in `component`) is roadmap-only. | `@loading` parser+HIR (no codegen); `@cancellable`; existing reactive members; A1-01/A1-04 specs. | **GA-01** (§4) |
| 2 | Cross-stack type & contract duplication | 🟡 Partially Built | `@table` and `@endpoint(kind: query\|mutation)` parse and round-trip (`crates/vox-compiler/src/lexer/token.rs:AtTable, AtEndpoint`; goldens `crud_api.vox`, `blog_fullstack.vox`). [Wire Format v1 SSOT](wire-format-v1-ssot.md) is honored by codegen via hand-written rules in `crates/vox-codegen/src/codegen_rust/emit/types.rs`, but **no `vox-wire-format-validator` crate enforces the spec at build time.** [Frontend Convergence Findings §Contract IR](frontend-convergence-findings-2026.md) is a *proposal*, not an implementation. | Wire Format v1 SSOT + Phase 1 build-target split (shipped — `crates/vox-cli/src/cli_args.rs:BuildTargetArg`) + Phase 2 OpenAPI emit. | **GA-02** (§4) |
| 3 | Form state, validation, and submission | 🟡 Partially Built | `@form` has end-to-end implementation: lexer (`Token::AtForm`), parser (`crates/vox-compiler/src/parser/descent/decl/head.rs`), HIR lowering (`hir/lower/decl.rs:lower_form`), and React codegen (`crates/vox-codegen/src/codegen_ts/form_emit.rs`). What's missing per the brief's #3: P0 label-required structural check; client/server validator mirroring via Contract IR (GA-02); debounced async validator with cancellation; multi-step state-machine compilation; symmetric `vox/form/missing-label` diagnostic. | `@form` parser+codegen; A1-02; phase-3 ergonomics. | **GA-03** (§4) |
| 4 | Auth / authz | 🟡 Partially Built | **Three layers, partially aligned.** (a) `@require` token exists and parses on `fn` declarations; HIR carries the precondition expression list; **no codegen wires it**. (b) Runtime side: `crates/vox-actor-runtime/src/auth.rs` (120 lines) and `route_capability_policy.rs` (65 lines) ship a working capability-policy layer. (c) Proven precedent at the VCS layer: `crates/vox-orchestrator-types/src/vcs_capability.rs` (`WorkspaceId` / `BranchName` / `BranchCreate` / `WorkingTreeWrite`), enforced by `crates/vox-orchestrator-mcp/src/git_exec.rs`. **Missing:** OAuth/OIDC `@auth(provider:)` decorator; capability-leak typecheck rule; codegen that lowers `@require` to Tower middleware reading the actor-runtime policy; menu-gating derivation; symmetric `vox/auth/capability-leak` diagnostic. | `@require` parser+HIR; actor-runtime auth + route policy; VCS capability tokens. | **GA-04** (§4) |
| 5 | Effect/IO for external services | 📋 Spec'd, Not Shipped | [Vox Language Rules Phase 5](vox-language-rules-phase5-effects-determinism-2026.md) is the design; `@uses(net | fs | time | random | secret)` is **not parsed** today. Phases 1–4 of the language-rules plan are also pre-shipping. | Phase 5 effect-system spec. | **GA-05** (§4) |
| 6 | Request validation at trust boundaries | 📋 Spec'd, Not Shipped | [Phase 3 HTTP Ergonomics Spec](phase3-http-ergonomics-spec-2026.md) is design only; **no `@cors`, `@auth(scheme:)`, `@role`, `@rate_limit` tokens** parse. CC-13/CC-15 are also roadmap items. | Phase 3 spec; A1-02 / A1-04 view side. | **GA-06** (§4) |
| 7 | State management / reactive sync | ✅ Shipped (mostly) | `state`/`derived`/`effect` reactive members parse and lower in `crates/vox-compiler/src/parser/descent/decl/head.rs:parse_reactive_body_decl`; codegen targets TS/React via `crates/vox-codegen/src/web_ir/`; goldens exercise round-trip. M2/M5 (reactivity outside `component { }`, cost-bounded auto-dep) are the named extensions per [Svelte vs React Research](svelte-vs-react-frameworks-research-2026.md). | Existing reactive members; M2/M5 in [Svelte-Mineable Plan](svelte-mineable-features-implementation-plan-2026.md). | None at design level — track M2/M5 inside the Svelte-mineable plan. |
| 8 | i18n (string extraction, plural, gender, ICU, RTL, date/timezone) | 🔵 No Spec Yet (Genuine Delta) | No CC entry; no archetype lists this as a blocker; no parser entries. | None. | **GA-08** (§4) |
| 9 | Routing / deep-linking (web + mobile) | 🟡 Partially Built | Web `routes` parse and emit. `@deep_link` token exists and parses on the root App declaration with `scheme` / `universal_link` / `on_link` fields; lowers to `HirDeepLink`; **no codegen.** Sitemap.xml emit, typed `href` (M6), and the platform-specific manifest emit (Android `intent-filter`, iOS `apple-app-site-association`) are roadmap items. | `routes`; `@deep_link` parser+HIR; A2-06; M6. | **GA-09a** (§4); **GA-09b** (§4) deferred until native emit lands. |
| 10 | Observability (logs, metrics, traces, audit) | 🟡 Partially Built | `crates/vox-telemetry/` ships the L1 facade (`recorder.rs`, `span.rs`, `aggregator.rs`); per [Telemetry Unification Design](telemetry-unification-design-2026.md) the runtime architecture is in flight (status: roadmap). [Populi Mesh Local Observability Spec](populi-mesh-local-observability-spec-2026.md) defines the `vox.mesh.*` namespace but `traceparent` propagation is forward-compat-only in S1. PII redaction (`@secret`) is on [Phase 4 monitors](vox-language-rules-phase4-runtime-monitors-2026.md), pre-shipping. | `vox-telemetry` facade; CC-09 audit-log spec; mesh observability S1 spec. | None at design level — implementation is the work. |
| 11 | Background jobs, queues, scheduled tasks | 🟡 Partially Built | `@scheduled` and `@durable` parse and lower to `DurabilityKind` in HIR (`crates/vox-compiler/src/hir/lower/mod.rs:322-326`), but **no codegen, no scheduler loop, no retry queue** — verdict from [Durability Runtime Audit](durability-runtime-audit-2026.md) is "zero runtime across all features." `crates/vox-workflow-runtime/` and `crates/vox-actor-runtime/` exist as scaffolds. | Parse-only `@scheduled`/`@durable`; CC-17/CC-18 specs; durability audit. | **GA-11** (§4) |
| 12 | File uploads | 📋 Spec'd, Not Shipped | CC-01 `Upload[T]` is roadmap-only; no token parses; no upload runtime. | CC-01 spec. | **GA-12** (§4) |
| 13 | Real-time / WebSocket / SSE | 📋 Spec'd, Not Shipped | CC-00 typed `Channel[Send, Recv]` and CC-02 `Stream[T]` are roadmap-only; no tokens parse; no Axum WS extractor wired. | CC-00 + CC-02 specs. | **GA-13** (§4) |
| 14 | Push notifications / platform capability | 🟡 Partially Built | `@push`, `@deep_link`, `@back_button` tokens exist; all parse on the root App declaration and lower to `HirPush` / `HirDeepLink` / `HirBackButton` respectively (with `on_register` / `on_notification` / `on_action` / `on_link` / `on_press` callback fields). **Missing:** any codegen target; APNs/FCM/Web-Push adapters in runtime; `Notify { channel: ... }` value type. | `@push` + `@deep_link` + `@back_button` parser+HIR. | **GA-14** (§4) |
| 15 | Offline-first / CRDT | 📋 Spec'd, Not Shipped | CC-22 `@offline_capable`, CC-20 `@collaborative` are roadmap-only; no service-worker emit; no Yrs integration. | CC-22 + CC-20 specs. | **GA-15** (§4) |
| 16 | Webhook receivers (signature verification + idempotency) | 🟡 Partially Built | `crates/vox-webhook/` and `crates/vox-plugin-webhook/` exist as runtime scaffolds; `@webhook` decorator token is **not parsed** in `crates/vox-compiler/src/lexer/token.rs`. CC-04 spec exists. | `vox-webhook` runtime scaffold; CC-04 spec. | **GA-16** (§4) |
| 17 | Pagination, infinite scroll, search debouncing | 📋 Spec'd, Not Shipped | A1-03 `std.ui.paginated_list` is roadmap-only. No stdlib directory exists in the repo to host it. Cursor-codec at the wire-format level is not specified. | A1-03 spec. | **GA-17** (§4) |
| 18 | Cache invalidation tags | 🔵 No Spec Yet (Genuine Delta) | Reactive runtime + endpoint freshness covers it implicitly; no `@invalidates`/`@reads` decorators specified. | None. | **GA-18** (§4) |
| 19 | a11y primitives (focus management, ARIA, keyboard, modals) | 📋 Spec'd, Not Shipped | CC-23 design tokens are roadmap-only. Semantic `menu`/`dialog`/`listbox`/`combobox`/`tabs` primitives with WAI-ARIA wiring are not specified. | CC-23 partial; needs CC-25 addendum. | **GA-19** (§4) |
| 20 | Theming / design tokens / dark mode | 📋 Spec'd, Not Shipped | CC-23 is roadmap-only; no `Token` type parses; no compile-time contrast violation rule exists. | CC-23 spec. | **GA-20** (§4) |
| 21 | LLM integration plumbing (prompts, tools, structured output, streaming) | 🟡 Partially Built | Substantial existing scaffolding: `@ai` token parses on `fn` decls (sets `is_llm`, optional model name); HIR carries it; **no codegen**. `@tool` token parses (`Token::AtTool`). `crates/vox-actor-runtime/src/prompt_canonical.rs` (316 LoC) + `llm/{chat,embed,stream,wire,types}.rs` ship a runtime LLM surface. CC-21 (provider-agnostic tool-call routing) is in flight in `crates/vox-orchestrator-mcp/`. **Missing:** typed-prompt declaration that compile-checks variable substitution; `structured T` return-type validation against the model schema (re-prompt-on-failure); auto-streaming with backpressure as a typed effect. [Unified Task Hopper Research](unified-task-hopper-research-2026.md) is *research*, not design. | `@ai` + `@tool` parser+HIR; `prompt_canonical.rs`; `vox-actor-runtime/llm/`; CC-21. | **GA-21** (§4) |
| 22 | Agentic workflow orchestration | 🟡 Partially Built | [Agentic VCS Phase 1 shipped](agentic-vcs-automation-impl-plan-phase1-2026.md) (capability tokens, GitExec wrapper, secret scanner, `vox_commit_create` MCP tool, telemetry contract; 131 lib tests passing). Phases 2–5 are TDD plans, not yet implemented. [Populi mesh north-star](populi-mesh-north-star-2026.md) S1/S2/S3 slices in flight. [Hopper Research](unified-task-hopper-research-2026.md) is research-stage. | Agentic VCS Phase 1; mesh slices; Hopper research. | None at language level. The brief's `agent { ... }` keyword should *not* be added (see anti-recommendation §5). |
| 23 | Consent / taint / audit for AI features | 📋 Spec'd, Not Shipped | CC-09 audit-log spec; [Phase 4 runtime monitors](vox-language-rules-phase4-runtime-monitors-2026.md) `@secret` redaction is design-only; Phase 5 effects is design-only. | CC-09 + Phase 4 + Phase 5 specs. | **GA-23** (§4) |
| 24 | Vector search / embeddings / RAG | 📋 Spec'd, Not Shipped | CC-16 `Vector[N]` value type and `@embed(model:)` decorator are roadmap-only; not parsed. `crates/vox-search/` exists as a search execution layer (per [Search & Retrieval SSOT](search-retrieval-ssot-2026.md)) but does not yet expose vector search at the language level. | CC-16 spec; `vox-search` runtime. | **GA-24** (§4) |
| 25 | Real-time multiplayer / collaborative editing | 📋 Spec'd, Not Shipped | CC-20 `@collaborative`, CC-12 `RichText`, CC-00 channels — all roadmap-only. | CC-20 + CC-12 + CC-00 specs. | **GA-25** (§4) |

---

## §2.1 — Tier counts (audit-corrected; final)

| Tier | Count | Rows |
|---|---|---|
| ✅ Shipped | **1** | #7 (state management, via runes-style reactive members) |
| 🟡 Partially Built | **11** | #1, #2, #3, #4, #9, #10, #11, #14, #16, #21, #22 |
| 📋 Spec'd, Not Shipped | **11** | #5, #6, #12, #13, #15, #17, #19, #20, #23, #24, #25 |
| 🔵 No Spec Yet (Genuine Delta) | **2** | #8, #18 |
| ⚫ Out of Scope for Language Layer | **0** | — |

> **Reading the corrected tally.** The 2026-05-09 token-by-token re-audit (after the initial verdict pass) found that **`@form`, `@require`, `@loading`, `@push`, `@deep_link`, `@back_button`, `@cancellable`, `@ai`, `@tool` already exist in the lexer** — most parse to HIR and several have full codegen. The surface area is meaningfully larger than the initial pass credited. **However**, codegen wiring stops at HIR for everything except `@form`, so a "category is partially built" finding does **not** mean the brief's UX outcome is delivered today. The brief's value remains as a vocabulary bridge — and as a checklist of P0 levers (label-required, capability-leak refusal, taint-tracking) that are *not* yet enforced even where parsers exist. *Specs are not features; parsers without codegen are also not features.*

## §3 — Audit findings (what changed in this document)

This document went through **two correction passes** on 2026-05-09. Both are listed so reasoning is fully auditable.

### Pass A — "Spec'd ≠ Shipped" correction

The first draft used a 4-tier verdict scheme that wrote `✅ Already Covered` for any category whose closest existing surface was a CC-XX item in the [Web App Archetype Coverage Map](web-app-archetype-coverage-2026.md). But CC-XX items are **named gaps**, not shipped features. After the 2026-05-09 code audit (`crates/vox-compiler/`, `crates/vox-codegen/`, `crates/vox-actor-runtime/`, integration tests) the verdicts were re-graded:

| Row | Pass-A previous verdict | Pass-A corrected verdict | Reason |
|---|---|---|---|
| #2 Cross-stack types | ✅ Already Covered | 🟡 Partially Built | Wire-format SSOT exists, but no validator crate enforces it. Contract IR is a proposal, not built. |
| #4 Auth / authz | ✅ Already Covered | 📋 Spec'd, Not Shipped | CC-05..CC-08 are roadmap items only. No `@auth`, `@require`, `@role` tokens parse. The capability-token pattern is shipped only at the VCS orchestrator layer, not as a Vox-language surface. |
| #6 Request validation | ✅ Already Covered | 📋 Spec'd, Not Shipped | Phase 3 HTTP Ergonomics decorators (`@cors`, `@auth(scheme:)`, `@role`, `@rate_limit`) do not parse. Spec is design-only. |
| #10 Observability | ✅ Already Covered | 🟡 Partially Built | `vox-telemetry` L1 facade ships; runtime architecture / `traceparent` propagation are still in flight. Spec status flag itself is "roadmap." |
| #12 File uploads | ✅ Already Covered | 📋 Spec'd, Not Shipped | CC-01 / CC-19 are roadmap-only. No upload runtime. |
| #13 Real-time / WS | ✅ Already Covered | 📋 Spec'd, Not Shipped | CC-00 / CC-02 are roadmap-only. No WS extractor wired. |
| #15 Offline / CRDT | ✅ Already Covered | 📋 Spec'd, Not Shipped | CC-22 / CC-20 are roadmap-only. No service-worker emit, no Yrs integration. |
| #16 Webhook | ✅ Already Covered | 🟡 Partially Built | `vox-webhook` crate exists; `@webhook` decorator token does not parse. |
| #20 Theming / tokens | ✅ Already Covered | 📋 Spec'd, Not Shipped | CC-23 is roadmap-only. No `Token` type parses. |
| #22 Agentic orchestration | ✅ Already Covered | 🟡 Partially Built | Agentic VCS Phase 1 shipped; Phases 2–5 are plans. Hopper is research-stage, not "well past design." |
| #24 Vector search | ✅ Already Covered | 📋 Spec'd, Not Shipped | CC-16 is roadmap-only. `vox-search` does not yet expose vector search at the language level. |
| #25 Multiplayer | ✅ Already Covered | 📋 Spec'd, Not Shipped | CC-20 / CC-12 / CC-00 all roadmap-only. |

### Pass B — Token-by-token re-audit ("Vox ships more than I credited")

Pass A under-credited Vox by relying on a partial mental model of which `At*` tokens existed. A targeted re-audit of `crates/vox-compiler/src/lexer/token.rs` enumerated **every** `At*` token (52 in total) and traced parse / lower / codegen for the relevant ones. Findings forced these further upgrades:

| Row | Pass-A verdict | Pass-B corrected verdict | Trigger |
|---|---|---|---|
| #1 Async | 🟡 (still 🟡) | 🟡 (refined) | `@loading` and `@cancellable` tokens exist; both parse and lower; neither has codegen. Decision implication: GA-01 must decide whether to deprecate `@loading` in favour of typed `Async[T]`, or fold them. |
| #3 Forms | 📋 Spec'd, Not Shipped | 🟡 Partially Built | `@form` ships **end to end**: lexer + parser + HIR `lower_form` + React codegen at `crates/vox-codegen/src/codegen_ts/form_emit.rs`. GA-03 is now "add P0 label rigor and Contract-IR mirroring," not "build form decorator." |
| #4 Auth | 📋 Spec'd, Not Shipped | 🟡 Partially Built | `@require` parses + lowers (no codegen); `crates/vox-actor-runtime/src/auth.rs` (120 LoC) and `route_capability_policy.rs` (65 LoC) ship a working capability-policy runtime layer. GA-04 is now "wire `@require` to the runtime layer + add `@auth(provider:)`," not "design from scratch." |
| #14 Push | 📋 Spec'd, Not Shipped | 🟡 Partially Built | `@push` parses on root App with `on_register` / `on_notification` / `on_action` callbacks; lowers to `HirPush`; no codegen. `@deep_link` and `@back_button` similarly lower but no codegen. |
| #21 LLM | 🟡 (still 🟡) | 🟡 (refined) | `@ai` and `@tool` tokens both exist and parse + lower; `crates/vox-actor-runtime/src/prompt_canonical.rs` (316 LoC) and the entire `llm/` submodule (`chat.rs`, `embed.rs`, `stream.rs`, `wire.rs`, `types.rs`) ship a working LLM runtime. GA-21 is now "wire `@ai`/`@tool` codegen + structured-output validation," not "introduce LLM primitives from scratch." |

### Pass C — Crate-path corrections

The first draft of the graft blocks referenced `crates/vox-runtime/` as a target for many new modules. **That crate does not exist.** The audit confirmed the existing landing surface for runtime modules is `crates/vox-actor-runtime/`, which already ships `auth.rs`, `rate_limit.rs`, `scheduler.rs`, `mailbox.rs`, `state_machine.rs`, `prompt_canonical.rs`, `route_capability_policy.rs`, `subscription.rs`, `transport.rs`, `llm/`, `storage/`, etc. All `crates/vox-runtime/` references in §4 graft blocks were rewritten to point at `crates/vox-actor-runtime/` (extending existing modules where possible) or to a clearly-named *new* crate when no existing home fits.

The previous draft's "anti-recommendation" against the brief's `external pure | retryable | streaming` keyword family and the `agent { ... }` keyword still stands; both would be P-stack regressions per [LANGUAGE_DESIGN_PRIORITIES.md](../../../LANGUAGE_DESIGN_PRIORITIES.md) C2/C4.

---

## §4 — Graft tasks (Sonnet-4.6-followable blocks)

### §4.0 — Pre-flight checklist (run before *any* graft)

A Sonnet 4.6 agent picking up a graft from this document must complete all of the following before writing code. Skip steps at your peril — most "graft drifted into something nobody wanted" failures trace to skipped pre-flight.

**1. Required reading (in this order):**
   1. [AGENTS.md](../../../AGENTS.md) — cross-tool policy. Shows you what *not* to do.
   2. [LANGUAGE_DESIGN_PRIORITIES.md](../../../LANGUAGE_DESIGN_PRIORITIES.md) — the P0–P5 / C1–C5 rubric every graft is scored against.
   3. [docs/src/architecture/where-things-live.md](where-things-live.md) — flat lookup of "this concept lives in this crate." Prevents grep-and-guess.
   4. The named "Files to read first" entries in the specific GA-* block.
   5. [LANGUAGE_DESIGN_PRIORITIES.md](../../../LANGUAGE_DESIGN_PRIORITIES.md) again *after* reading the GA block. Ask yourself: which P-lever does this task lean on, and is that scoring honest?

**2. State-drift verification.** This document was audited on 2026-05-09. Code may have changed since. Run these commands to confirm the GA's premises still hold before starting:

```pwsh
# Confirm the lexer tokens the GA assumes still exist (or do not)
cargo run -p vox-cli -- ci grep-token AtForm AtRequire AtAi AtTool AtPush AtDeepLink AtCancellable AtLoading
# Or directly:
rg -n "Token::At(Form|Require|Ai|Tool|Push|DeepLink|Cancellable|Loading)" crates/vox-compiler/src/lexer/token.rs

# Confirm vox-actor-runtime modules still in the layout the GA assumes
ls crates/vox-actor-runtime/src/

# Confirm the doc cross-links the GA cites still exist
cargo run -p vox-cli -- ci docs-quality

# Confirm baseline test suite is green BEFORE starting
cargo test -p vox-compiler
cargo test -p vox-codegen
cargo run -p vox-arch-check
```

**3. Branch hygiene.** Per [Agentic VCS Phase 1](agentic-vcs-automation-impl-plan-phase1-2026.md), every graft creates a new branch via `vox_branch_create`. **Do not** work directly on `main` or on a branch shared with other agents. The branch name must encode the graft id, e.g. `ga-04-auth-capability-codegen`.

**4. Diagnostic ID convention.** Every new compile-error or warning the graft adds gets a stable id of the form `vox/<area>/<rule>` (e.g. `vox/async/missing-arm`, `vox/auth/capability-leak`, `vox/form/missing-label`). Per [Phase 1 SSOT Collapse](vox-language-rules-phase1-ssot-collapse-2026.md), ids are append-only with deprecation aliases; **never reuse** an id with different semantics. Each id ships with a `--explain` page in the diagnostic catalog.

**5. Generated-output discipline.** Per the [auto-generated-docs feedback](../../../AGENTS.md), tools regenerate `SUMMARY.md`, `*.generated.md`, `architecture-index.md`, `feed.xml`, `.cursorignore`. **Never hand-edit those files.** Re-run the relevant `xtask` or `vox ci` generator and commit the regenerated output.

**6. .vox script discipline.** Per [AGENTS.md §VoxScript-First Glue Code](../../../AGENTS.md), automation scripts are `.vox` files invoked via `vox run scripts/foo.vox`. **Do not** generate `.ps1`, `.sh`, or `.py` glue scripts.

**7. PR convention.** Per repo style, commit messages follow `<area>: <imperative>` (e.g. `compiler: add @form label-required typecheck`); PR title and first commit are the same. Reference the GA id in the PR body so the audit trail is searchable.

### §4.1 — Sequencing / dependency map

The grafts are not all independent. The DAG below shows which grafts unblock which. An agent picking the next task to work on should walk the DAG from roots toward leaves.

```
Foundations (independent, parallel-safe):
   GA-02  Wire-format validator + Contract IR
   GA-05  Effect annotations (@uses)
   GA-20  Design tokens
   GA-08  i18n catalog (doc-only addendum)

Tier 1 (depend on foundations):
   GA-01  Async[T] arm-matcher              ← needs nothing strictly, but composes with GA-02
   GA-03  @form rigor                        ← needs GA-01 (Async submit state), GA-02 (Contract IR)
   GA-04  Capability/principal types         ← composes with GA-06; can begin parser-only in parallel
   GA-06  Phase 3 ergonomics decorators      ← composes with GA-04
   GA-09a Routes-as-types (typed href)       ← independent
   GA-11  Durable runtime impl               ← benefits from GA-05 (effect surface)
   GA-19  Semantic UI primitives             ← needs GA-20 (tokens) for label/dialog interplay
   GA-23  @pii taint companion               ← needs GA-05

Tier 2 (depend on Tier 1):
   GA-12  Upload[T]                          ← benefits from GA-02
   GA-13  Channel[Send,Recv] + SSE           ← benefits from GA-02 + GA-05
   GA-14  @push/@deep_link codegen           ← benefits from GA-09a (typed routes for payloads)
   GA-16  @webhook decorator                 ← benefits from GA-15-style HMAC primitives
   GA-17  Paginated[T]                       ← needs GA-01 + GA-02
   GA-21  @prompt + structured output        ← benefits from GA-02 + GA-05; aligns with GA-04
   GA-24  Vector[N] + @embed                 ← benefits from GA-02

Tier 3 (compositions):
   GA-15  Offline + CRDT                     ← needs GA-13 + RichText (CC-12)
   GA-25  Multiplayer                        ← composes GA-13 + GA-15
   GA-26  Layered layout discipline          ← composes GA-19 (semantic UI) + GA-20 (tokens)

Deferred (no graft host until prerequisite lands):
   GA-04b Multi-tenancy @scoped_by(tenant)  ← deferred, no native-emit need
   GA-09b iOS/Android deep-link emit        ← deferred, no native emit roadmap
   GA-18  @invalidates/@reads cache tags    ← deferred, no user-pain signal yet
```

### §4.2 — How an agent picks the next graft

Apply these in order and stop at the first match:

1. **Is there an open commitment in [the orchestrator state](../../../README.md) the user has assigned?** If yes, do that. Stop.
2. **Is the user's request directly invoking a specific GA-id (e.g. "execute GA-04")?** Do that. Stop.
3. **Is one of the foundational grafts (GA-02, GA-05, GA-08, GA-20) unstarted?** Pick the smallest one (S/M lift) you can complete in one session. GA-02 has highest leverage (unblocks 7 downstream). GA-05 has highest P-stack score (Phase 5 promise). GA-08 is doc-only (S lift, low risk).
4. **If foundations are landed, pick the Tier-1 graft with the highest *(P0-lever-count × archetype-coverage)* score**. As of the 2026-05-09 audit:
   - GA-04 (auth capability-leak refusal) — P0, blocks 6+ archetypes.
   - GA-19 (semantic UI primitives) — P0 a11y wedge, blocks 8+ archetypes.
   - GA-03 (form label rigor) — P0, blocks 12+ archetypes.
5. **If you cannot identify a clear next graft, do not invent one.** Stop and report to the user with a one-paragraph summary of the DAG state and which 2–3 grafts you'd recommend next.

> **Anti-pattern.** Picking a Tier-2 or Tier-3 graft without confirming its dependencies have landed will produce code with stub interfaces that future agents will struggle to evolve. The DAG is binding.

### §4.3 — Conventions used in every block

> Each block is self-contained. A Claude Sonnet 4.6 agent loaded with this file alone has every input it needs to execute the task, including: the goal, dependencies, files to read first, files to modify or create, acceptance criteria, verification commands, and the design-priority rubric the change should be scored against. **Do not** start a graft without first running the §4.0 pre-flight.
>
> **Conventions used in every block:**
> - **Goal**: one sentence stating the user-visible outcome.
> - **Status precondition**: which other GA-* must land first.
> - **Files to read first**: required reading before code changes (paths relative to repo root unless noted). Always *additional* to §4.0's required reading.
> - **Files to modify or create**: target paths. New files require frontmatter only if they are documentation; Rust files take normal Cargo conventions.
> - **Acceptance criteria**: testable claims. Each must be verifiable.
> - **Verification commands**: exact PowerShell-compatible shell. Use `cargo test -p <crate>` form, not `cargo test` at the workspace root, to keep iteration cheap.
> - **P-stack rubric**: which Vox priority lever the task leans on, and which decision-point reductions it claims.
> - **Out of scope for this task**: explicit list of things the agent should *not* do, to prevent drift.
> - **Estimated lift**: T-shirt size (S = ≤300 LoC, M = 300–1000, L = 1000–3000, XL = 3000+). Lift is implementation only — does not include design discussion or PR review cycles.

### GA-01 — Typed `Async[T]` value with exhaustive view-arm matching

**Goal.** Make loading/error/empty/optimistic states unrepresentable-as-untreated in views: any `component` that reads an `Async[T]` value must structurally handle `fetching`, `empty`, `error`, and `ok` arms or fail compilation.

**Existing surface to align with.** `Token::AtLoading` and `Token::AtCancellable` already parse and lower to HIR (`HirLoading`, `cancellable` flag on lambdas) but have no codegen. **Decision required up front:** does GA-01 (a) deprecate `@loading` in favour of typed `Async[T]` (P1 / C4 — one canonical shape), or (b) keep `@loading` as a function-level annotation that *produces* an `Async[T]` value? Recommend (a) per C4 (no cosmetic alternatives), with `@loading` being deprecated via the diagnostic catalog's append-only-with-deprecation-aliases policy. Confirm this decision in an ADR before code changes.

**Status precondition.** Independent. Does not block on other GA-*. (Recommended ADR before starting; otherwise GA-01 will be re-litigated mid-implementation.)

**Files to read first.**
- [docs/src/architecture/web-app-archetype-coverage-2026.md](web-app-archetype-coverage-2026.md) §A1-04 (loading/empty/error structural slots — the pre-existing roadmap entry this graft formalises).
- [docs/src/architecture/svelte-mineable-features-implementation-plan-2026.md](svelte-mineable-features-implementation-plan-2026.md) Phase E (cost-bounded auto-dep tier) — for prior art on view-tree analysis.
- `crates/vox-compiler/src/parser/descent/decl/head.rs` — function `parse_reactive_body_decl` (existing reactive-member parser, the model for arm-matching syntax).
- `crates/vox-compiler/src/lexer/token.rs` — for token-naming conventions before adding `When`/`Fetching`/`Empty`/`Ok` tokens.
- `crates/vox-compiler/src/hir/lower/mod.rs` — for the lowering pattern to follow.
- [docs/src/architecture/wire-format-v1-ssot.md](wire-format-v1-ssot.md) §discriminated-union encoding — for how `Async[T]` should serialize.

**Files to modify or create.**
- `crates/vox-compiler/src/lexer/token.rs` — add `Async`, `When`, `Fetching`, `Empty`, `Ok` (or reuse `Ok`), `Error` view-arm tokens. Per C2/C4, do **not** add cosmetic alternatives.
- `crates/vox-compiler/src/parser/descent/expr/match_async.rs` (new) — parser for `when fetching => ... when empty => ... when error e => ... ok x => ...` arm syntax inside view blocks.
- `crates/vox-compiler/src/typecheck/exhaustiveness.rs` (or extend the existing pattern-matching exhaustiveness pass) — refuse compile when any arm is missing.
- `crates/vox-codegen/src/web_ir/async_state.rs` (new) — emit TS that narrows on `_tag` and renders the matching arm.
- `examples/golden/async_state_basic.vox` (new) — round-trip golden.
- `examples/golden/async_state_basic.expected.tsx` (new) — emitted output snapshot.
- `docs/src/architecture/web-app-archetype-coverage-2026.md` — flip A1-04 status from "roadmap" to "shipped" once landed.

**Acceptance criteria.**
1. A `.vox` source containing a `view` block that reads an `Async[User]` value and omits the `error` arm fails to compile with a stable diagnostic id (`vox/async/missing-arm`); diagnostic id added to the diagnostic catalog per [Phase 1 SSOT Collapse](vox-language-rules-phase1-ssot-collapse-2026.md).
2. A `.vox` source with all four arms compiles and emits a TS component that, given `{_tag: "fetching"} | {_tag: "empty"} | {_tag: "error", error} | {_tag: "ok", value}`, renders the correct subtree.
3. The wire format encodes `Async[T]` per the v1 SSOT discriminated-union convention (`_tag` field, `_kind` not used); test added to `crates/vox-codegen/src/web_ir/async_state.rs::tests`.
4. The exhaustiveness check survives the `vox check --for-llm` JSON output mode added in [Phase 2 Lint Extension](vox-language-rules-phase2-lint-extension-2026.md) — i.e., the diagnostic carries a minimal-repro per the JSON schema there.
5. `cargo run -p vox-arch-check` passes.

**Verification commands.**
```pwsh
cargo test -p vox-compiler async_state
cargo test -p vox-codegen async_state
cargo run -p vox-cli -- check examples/golden/async_state_basic.vox
cargo run -p vox-cli -- build --target=fullstack examples/golden/async_state_basic.vox
diff examples/golden/async_state_basic.expected.tsx (cargo run -p vox-cli -- emit client examples/golden/async_state_basic.vox)
cargo run -p vox-arch-check
```

**P-stack rubric.**
- **P0 lever:** wrong programs (forgetting the error arm) become structurally unrepresentable.
- **P1 lever:** removes 4–6 manual narrowing branches per fetch site.
- **C2/C4 hygiene:** *one* canonical arm-matcher syntax. Do not also expose `if loading { ... }` as an alternative — that would be a cosmetic alternative per C4.
- **Decision-point delta:** `−4` per fetch site (the four arms become structural, not chosen).

**Out of scope for this task.**
- Optimistic-update primitive (separate; not part of GA-01).
- Server-side `Async[T]` for SSR — file as GA-01 follow-up.
- Mutation/rollback semantics (those slot under GA-11 + GA-18).

**Estimated lift.** M (700–1200 LoC across compiler + codegen + tests + golden).

---

### GA-02 — Wire-format validator crate + Contract IR introduction

**Goal.** Make the [Wire Format v1 SSOT](wire-format-v1-ssot.md) machine-checked: any divergence between the SSOT and emitted codecs becomes a build-time error, and a new Contract IR layer subsumes the duplicated rules in `crates/vox-codegen/`.

**Status precondition.** Independent of other GA-*. Should land before GA-12, GA-13, GA-15, GA-17, GA-24, GA-25 (all of which add new wire-format encodings).

**Files to read first.**
- [docs/src/architecture/wire-format-v1-ssot.md](wire-format-v1-ssot.md) — full document.
- [docs/src/architecture/frontend-convergence-findings-2026.md](frontend-convergence-findings-2026.md) §Contract IR — the proposal this graft implements.
- `crates/vox-codegen/src/codegen_rust/emit/types.rs` — current hand-written rules; this graft factors them out.
- [docs/src/architecture/external-frontend-interop-plan-2026.md](external-frontend-interop-plan-2026.md) Phase 2 — host phase for this graft.

**Files to modify or create.**
- `crates/vox-wire-format/` (new crate) — single Rust library defining the SSOT as Rust types; codegen depends on this, no parallel hand-written rules.
- `crates/vox-wire-format/Cargo.toml` — add to workspace `members`.
- `crates/vox-wire-format-validator/` (new crate) — `vox ci wire-format-parity` subcommand; runs against codegen outputs and SSOT, fails on drift.
- `crates/vox-codegen/src/codegen_rust/emit/types.rs` — refactor to call into `vox-wire-format` instead of hand-rolling.
- `crates/vox-codegen/src/web_ir/types.rs` — same refactor for TS emit.
- `.github/workflows/ci.yml` — add `vox ci wire-format-parity` step (or its xtask equivalent).
- `docs/src/architecture/wire-format-v1-ssot.md` — add `@generated-hash` blake3 header per [Phase 1 SSOT Collapse](vox-language-rules-phase1-ssot-collapse-2026.md) so the SSOT itself is part of the drift-check surface.

**Acceptance criteria.**
1. A round-trip golden (`examples/golden/wire_format_round_trip.vox`) compiles to identical bytes between Rust server emit and TS client emit for every primitive in the SSOT (Decimal, BigInt, Date, Option absent-key, discriminated unions with `_tag`).
2. Editing the SSOT without regenerating produces a `vox/wire-format/spec-drift` diagnostic at build time.
3. Editing the codegen rules out from under the SSOT produces the same diagnostic.
4. The Contract IR exposes a single Rust API (`vox_wire_format::TypeShape`) that codegen consumes; there is exactly one module per encoding decision (no two rules for "how to emit Decimal").
5. `cargo run -p vox-arch-check` confirms `crates/vox-codegen` no longer references the old `emit/types.rs` hand-rolled paths.

**Verification commands.**
```pwsh
cargo test -p vox-wire-format
cargo test -p vox-wire-format-validator
cargo test -p vox-codegen wire_format_round_trip
cargo run -p vox-cli -- ci wire-format-parity
cargo run -p vox-arch-check
```

**P-stack rubric.**
- **P0 lever:** spec drift becomes structurally impossible (the SSOT *is* the code).
- **C1 lever:** the fine-tuning corpus can train against the SSOT directly.
- **Decision-point delta:** the codegen author no longer chooses between "follow the spec" and "improvise" — there is no improvising.

**Out of scope for this task.**
- New encodings (Date with timezone, CRDT envelopes) — those slot under GA-08 / GA-15 / GA-17 / GA-25.
- AsyncAPI emit (slot under GA-13).

**Estimated lift.** L (1500–2500 LoC across two new crates + refactor).

---

### GA-03 — `@form` rigor: P0 a11y check + Contract-IR cross-stack mirror

**Goal.** Tighten the *existing* end-to-end `@form` implementation (lexer + parser + HIR `lower_form` + React codegen at `crates/vox-codegen/src/codegen_ts/form_emit.rs`) with the structural correctness levers the brief's #3 calls out: P0 label-required check, Contract-IR-driven server/client validator mirroring, debounced async validator with cancellation, multi-step state-machine compilation.

**Existing surface to align with.** **`@form` is shipped end-to-end.** Lexer: `Token::AtForm`. Parser: `crates/vox-compiler/src/parser/descent/decl/head.rs`. HIR lowering: `crates/vox-compiler/src/hir/lower/decl.rs:lower_form`. React codegen: `crates/vox-codegen/src/codegen_ts/form_emit.rs`. **Do not** reintroduce a parallel `@form` parser; *extend* the existing one. Per C4, exactly one form-binding shape.

**Status precondition.** Lands cleanest after GA-01 (uses `Async[T]` for the submit state) and GA-02 (uses Contract IR for cross-stack validators). The label-required check (#1 below) is independent and can land first as a small standalone PR.

**Files to read first.**
- `crates/vox-compiler/src/parser/descent/decl/head.rs` — `parse_form` (or wherever `@form` parsing lives — confirm via grep).
- `crates/vox-compiler/src/hir/lower/decl.rs` — function `lower_form`.
- `crates/vox-codegen/src/codegen_ts/form_emit.rs` — current React emit, the model for the additions.
- [docs/src/architecture/web-app-archetype-coverage-2026.md](web-app-archetype-coverage-2026.md) §A1-02 `@form`.
- [docs/src/architecture/phase3-http-ergonomics-spec-2026.md](phase3-http-ergonomics-spec-2026.md) — host phase for the server-side mirror.

**Files to modify or create.**
- `crates/vox-compiler/src/typecheck/form.rs` (new) — refuse compile when a `@form` field lacks a label (P0). Diagnostic id: `vox/form/missing-label`.
- `crates/vox-codegen/src/codegen_ts/form_emit.rs` (extend) — emit debounced async validator (composes with `@cancellable`) and focus-on-error.
- `crates/vox-codegen/src/codegen_rust/emit/form_endpoint.rs` (new) — emit server-side validator from the same Contract IR (GA-02 dependency). The validator's structured-error shape must round-trip identically to the client's.
- `crates/vox-compiler/src/typecheck/form_state_machine.rs` (new) — compile multi-step `@form` declarations into a state-machine type rather than nested context providers.
- `examples/golden/form_basic.vox` (new) + expected emit.
- `examples/golden/form_multi_step.vox` (new) + expected emit.

**Acceptance criteria.**
1. A `@form` declaration without a `label` on any field fails compile with `vox/form/missing-label` (P0).
2. Validation rules expressed once produce both server-side and client-side validators using GA-02's Contract IR. A client-only edit cannot diverge from server.
3. Submit returns an `Async[Result[T, FieldErrors]]` (composes with GA-01).
4. Multi-step forms compile from a declarative state-machine, not nested context providers; emit a state-machine type that the runtime threads.
5. Per C4, there is exactly one form-binding shape — no `Form` value alongside `@form` decorator.

**Verification commands.**
```pwsh
cargo test -p vox-compiler form
cargo test -p vox-codegen form
cargo run -p vox-cli -- check examples/golden/form_basic.vox
cargo run -p vox-cli -- build --target=fullstack examples/golden/form_basic.vox
```

**P-stack rubric.**
- **P0 levers:** label-required (a11y); validation-divergence-impossible.
- **P1 lever:** removes ~8 decisions per form (field declarations, validators, submit handler, focus, optimistic, error-mapping, debounce, draft).
- **Decision-point delta:** `−8` per form site.

**Out of scope for this task.**
- Draft persistence — file as GA-03 follow-up.
- Optimistic submit (slot under GA-11 once durable functions land).

**Estimated lift.** M (800–1500 LoC).

---

### GA-04 — Wire `@require` to capability typecheck + add `@auth(provider:)` for OAuth flows

**Goal.** Make the *existing* `@require` decorator (which today parses and lowers but has no codegen) actually enforce capability checks at compile and runtime. Add `@auth(provider:)` for OAuth/OIDC flows. Refuse compile when an endpoint response leaks a capability-gated field.

**Existing surface to align with.**
- Lexer `Token::AtRequire` parses on `fn` declarations. HIR carries the precondition expression list. **No codegen.**
- `crates/vox-actor-runtime/src/auth.rs` (120 LoC) ships a working session/principal layer — `@require` codegen must call into this, not bypass it.
- `crates/vox-actor-runtime/src/route_capability_policy.rs` (65 LoC) ships a per-route capability-policy decision layer.
- Proven precedent at the VCS layer: `crates/vox-orchestrator-types/src/vcs_capability.rs` (capability tokens) + `crates/vox-orchestrator-mcp/src/git_exec.rs` (runtime enforcement).

**Status precondition.** The `@require` codegen path can ship first as a small standalone PR. `@auth(provider:)` (OAuth) is independent and lands later. GA-06 (Phase 3 ergonomics) lands the rate-limit / CORS sidecars; do not duplicate.

**Files to read first.**
- `crates/vox-compiler/src/lexer/token.rs` — confirm `AtRequire` parses today.
- `crates/vox-compiler/src/parser/descent/decl/head.rs` — parser for `@require`.
- `crates/vox-actor-runtime/src/auth.rs` and `crates/vox-actor-runtime/src/route_capability_policy.rs` — the runtime surface to lower into.
- `crates/vox-orchestrator-types/src/vcs_capability.rs` — proven pattern.
- [LANGUAGE_DESIGN_PRIORITIES.md](../../../LANGUAGE_DESIGN_PRIORITIES.md) §C4 — to avoid offering both `@require` and `@role` as cosmetic alternatives.

**Files to modify or create.**
- `crates/vox-compiler/src/lexer/token.rs` — add `AtAuth` (NOT `AtRequire`, which already exists; NOT `AtRole`, which is anti-recommended per C4).
- `crates/vox-compiler/src/parser/descent/decl/auth.rs` (new) — parser for `@auth(provider: oauth(...), redirect: ...)`.
- `crates/vox-compiler/src/typecheck/capability.rs` (new) — refuse endpoint compile when response shape leaks a field whose capability the principal lacks. Diagnostic id: `vox/auth/capability-leak`.
- `crates/vox-codegen/src/codegen_rust/emit/auth.rs` (new) — lower `@require` to a Tower middleware that consults `vox-actor-runtime/route_capability_policy`. Lower `@auth(provider:)` to a working Authorization-Code-Flow + PKCE handler.
- `crates/vox-codegen/src/web_ir/auth.rs` (new) — emit menu-gating helpers from the same capability set.
- `examples/golden/auth_capability.vox` (new) + expected emit.

**Acceptance criteria.**
1. An endpoint whose response includes a `User.email` field, where `email` has capability `Read.Email`, refuses compile under a session principal that lacks `Read.Email`. Diagnostic id: `vox/auth/capability-leak`.
2. The same capability set generated server-side flows to the client; menu-gating compiles from it (no manual duplication).
3. OAuth/OIDC + PKCE + token rotation all expressed in a single `@auth { ... }` block; OAuth-Authorization-Code-Flow golden emits a working Axum handler.
4. Audit-log entries are emitted automatically when a capability is exercised — coupled into [CC-09 audit](web-app-archetype-coverage-2026.md) (test stub OK if CC-09 hasn't landed).

**Verification commands.**
```pwsh
cargo test -p vox-compiler capability
cargo test -p vox-codegen auth
cargo run -p vox-cli -- check examples/golden/auth_capability.vox
cargo run -p vox-cli -- build --target=server examples/golden/auth_capability.vox
```

**P-stack rubric.**
- **P0 lever:** privilege-escalation bugs become structurally unrepresentable (capability-leak refusal).
- **C4 hygiene:** capability subsumes role; no `@role` keyword.
- **Decision-point delta:** `−5` per protected endpoint (no manual middleware, no duplicate menu-gating).

**Out of scope for this task.**
- Multi-tenancy `@scoped_by(tenant)` — file separately as GA-04b (deferred block below; CC-07).
- SSO/SAML — defer until enterprise pull is real.

**Estimated lift.** L (1500–2500 LoC).

---

### GA-04b — Multi-tenancy `@scoped_by(tenant)` (deferred)

**Status: deferred.** [CC-07](web-app-archetype-coverage-2026.md) is roadmap-only. **Trigger to un-defer:** the first multi-tenant SaaS user-journey enters the [archetype coverage map](web-app-archetype-coverage-2026.md) §1 archetypes as a Tier-1 candidate, OR an enterprise commit lands. **Sketch when un-deferred:** `@table(scoped_by: tenant)` decorator; every `db.*` call against a scoped table requires a `Tenant` value in scope (compile error if missing); codegen emits Postgres RLS + SQLite where-clause injection. Composes with GA-04 capabilities (a tenant id flows alongside the principal). Do not start before the trigger.

---

### GA-05 — Effect annotations + retry/backoff/idempotency-key decorator forms on `@uses(net)`

**Goal.** Land Phase 5 effects (`@uses(net | fs | time | random | secret)`) in a form that allows external-service policy (retry, backoff, timeout, idempotency-key) to ride as decorator parameters, *without* introducing the brief's `external pure | retryable | streaming` bare-keyword family.

**Status precondition.** Phase 4 monitors must be partial (panic-trap boundary, `@secret` redactor); GA-05 lands `@uses(...)` static.

**Files to read first.**
- [docs/src/architecture/vox-language-rules-phase5-effects-determinism-2026.md](vox-language-rules-phase5-effects-determinism-2026.md) — full document.
- [docs/src/architecture/vox-language-rules-phase4-runtime-monitors-2026.md](vox-language-rules-phase4-runtime-monitors-2026.md) — for `@secret` precedent.
- [LANGUAGE_DESIGN_PRIORITIES.md](../../../LANGUAGE_DESIGN_PRIORITIES.md) §C2/§C4.

**Files to modify or create.**
- `crates/vox-compiler/src/lexer/token.rs` — add `AtUses`. Per C2/C4, do **not** add `External`, `Retryable`, `Streaming`.
- `crates/vox-compiler/src/parser/descent/decl/effects.rs` (new).
- `crates/vox-compiler/src/typecheck/effects.rs` (new) — propagate `@uses` transitively; reject `@pure` callers of `@uses(net)`.
- `crates/vox-actor-runtime/src/effect_policies.rs` (new module in existing crate) — implements `retry: exp_backoff`, `idempotency: auto`, `timeout: 30s` from the `@uses(net(...))` parameters. Coordinate with the existing `resilient_http.rs` (don't duplicate retry logic — fold into one path).
- `crates/vox-codegen/src/codegen_rust/emit/effects.rs` (new).

**Acceptance criteria.**
1. `@uses(net(retry: exp_backoff(5), idempotency: auto, timeout: 30s))` parses and the runtime applies the named policy with no further code.
2. A function annotated `@pure` that transitively calls a `@uses(net)` function fails compile with `vox/effect/pure-violation`.
3. A function calling `net.fetch(...)` *without* `@uses(net)` fails compile with `vox/effect/missing-net-decl`.
4. An idempotency key auto-derived from the call signature is stable across retries (test by replaying the same call; runtime returns the cached response).
5. The symmetric `vox/effect/unjustified-net-decl` warning fires when `@uses(net)` is declared on a function that demonstrably does not perform net I/O.

**Verification commands.**
```pwsh
cargo test -p vox-compiler effects
cargo test -p vox-actor-runtime effect_policies
cargo test -p vox-actor-runtime resilient_http  # confirm no behavior regression
cargo run -p vox-cli -- check examples/golden/effect_net_retry.vox
```

**P-stack rubric.**
- **P0 lever:** missing-effect-declaration becomes a compile error (Phase 5 promise).
- **C2/C4 hygiene:** no bare keyword family; everything rides on `@uses(...)`.
- **Decision-point delta:** `−3` per external call site (retry/timeout/idempotency become structural).

**Out of scope for this task.**
- LLM-specific `@prompt` (slot under GA-21).
- Streaming effect (slot under GA-13).

**Estimated lift.** L (1500–2500 LoC across compiler + runtime).

---

### GA-06 — Phase 3 HTTP ergonomics decorators (`@cors`, `@auth(scheme:)`, `@rate_limit`)

**Goal.** Land [Phase 3 HTTP Ergonomics Spec](phase3-http-ergonomics-spec-2026.md) — explicit `method`/`path` on `@endpoint`, plus `@cors`, `@auth(scheme: bearer)`, `@rate_limit`. Emits Tower middleware in generated Axum crates; OpenAPI 3.1 reflects all.

**Status precondition.** Independent at the parser layer; integrates cleanly with GA-04 (capability tokens) once both ship.

**Files to read first.**
- [docs/src/architecture/phase3-http-ergonomics-spec-2026.md](phase3-http-ergonomics-spec-2026.md).
- `crates/vox-compiler/src/lexer/token.rs:AtEndpoint` — existing pattern.
- `crates/vox-codegen/src/codegen_rust/emit/types.rs` — for OpenAPI cross-reference.

**Files to modify or create.**
- `crates/vox-compiler/src/lexer/token.rs` — add `AtCors`, `AtRateLimit`. (Note: `@auth` is added by GA-04; coordinate to avoid collision.)
- `crates/vox-compiler/src/parser/descent/decl/http_ergonomics.rs` (new).
- `crates/vox-codegen/src/codegen_rust/emit/http_ergonomics.rs` (new) — emit Tower layers.
- `crates/vox-codegen/src/openapi/mod.rs` (or extend existing) — reflect rate-limit and CORS into the OpenAPI 3.1 doc.

**Acceptance criteria.**
1. `@rate_limit(by: ip, per: 1m, max: 100)` produces a Tower layer that returns 429 with a `Retry-After` header at the 101st request inside the same minute from the same IP.
2. `@cors(origins: [...])` with a missing origin produces a 403 (not a silent allow).
3. The OpenAPI 3.1 doc generated for an annotated endpoint includes `x-rate-limit` and `x-cors-origins` extensions.
4. Per C4, there is no parallel "raw `Tower::Layer`" escape hatch in user code unless `// vox:skip`-ed.

**Verification commands.**
```pwsh
cargo test -p vox-compiler http_ergonomics
cargo test -p vox-codegen http_ergonomics
cargo run -p vox-cli -- check examples/golden/http_ergonomics.vox
```

**P-stack rubric.**
- **P1 lever:** removes 3–4 decisions per public endpoint (CORS, rate, auth scheme).

**Out of scope for this task.**
- `@require(can: ...)` (slot under GA-04).

**Estimated lift.** M (700–1200 LoC).

---

### GA-08 — i18n message catalog as types (proposed `CC-24`)

**Goal.** Add a typed `t"..."` template, project-root locale config, plural arms checked at compile, and timezone-carrying date types. Defer until at least one user-research signal flags i18n as a real blocker.

**Status precondition.** Independent.

**Files to read first.**
- [docs/src/architecture/web-app-archetype-coverage-2026.md](web-app-archetype-coverage-2026.md) §2 — for the CC-XX addition convention.

**Files to modify or create.**
- `docs/src/architecture/web-app-archetype-coverage-2026.md` — append `CC-24. i18n message catalog as types` (4 facets: design, runtime, codegen, eval).
- *Implementation deferred.* The graft is the spec addendum; the implementation graft is to be authored once the spec is approved.

**Acceptance criteria.**
1. CC-24 added to the archetype coverage map with all four facets (design / runtime / codegen / eval).
2. Tier-1 archetype list re-checked: any archetype that should now list CC-24 as a cross-cutting link has it added.
3. Spec-only PR — no parser changes.

**Verification commands.**
```pwsh
cargo run -p vox-cli -- ci docs-quality
```

**P-stack rubric.**
- **P0 lever:** missing-translation becomes structurally unrepresentable for declared locales.
- **Decision-point delta:** `−2` per UI string (no key extraction, no plural-arm omission).

**Out of scope for this task.**
- Parser/runtime work — split into a follow-up GA once CC-24 spec is reviewed.

**Estimated lift.** S (200–400 LoC, doc only).

---

### GA-09a — Routes-as-types: typed `href` unifying URL + sitemap entry + analytics slug

**Goal.** Lift M6 (typed `href`) from the Svelte-mineable plan to "routes-as-types": a single declaration produces the web URL, the sitemap entry, and the analytics-event slug.

**Status precondition.** Coordinate with [Svelte-Mineable Features Implementation Plan](svelte-mineable-features-implementation-plan-2026.md) Phase F.

**Files to read first.**
- [docs/src/architecture/svelte-mineable-features-implementation-plan-2026.md](svelte-mineable-features-implementation-plan-2026.md) §M6.
- `crates/vox-compiler/` — for `routes` block parser.

**Files to modify or create.**
- `crates/vox-compiler/src/parser/descent/decl/routes.rs` (or extend existing) — emit a `RouteId` typed value per route.
- `crates/vox-codegen/src/web_ir/href.rs` (new) — emit typed-href helper for client.
- `crates/vox-codegen/src/sitemap/mod.rs` (new) — emit sitemap.xml from route declarations.
- `crates/vox-codegen/src/analytics/route_slug.rs` (new) — emit analytics slugs.

**Acceptance criteria.**
1. `<a href={Route.UserProfile(id: u.id)}>` compiles and emits the correct URL; passing a string literal `<a href="/users/123">` emits a `vox/route/stringly-typed` warning.
2. `sitemap.xml` reflects every public route at build time.
3. Analytics-event slugs match the route names (no manual mapping).

**Verification commands.**
```pwsh
cargo test -p vox-compiler routes
cargo test -p vox-codegen href sitemap
cargo run -p vox-cli -- build --target=fullstack examples/golden/routes_typed.vox
```

**P-stack rubric.**
- **P0 lever:** wrong-URL becomes structurally unrepresentable.
- **C4 hygiene:** one route declaration drives all derivations.

**Out of scope for this task.**
- iOS Universal Link / Android App Link emit (GA-09b, deferred).
- Push-notification deep-link payload (GA-14).

**Estimated lift.** M (600–1000 LoC).

---

### GA-09b — Native deep-link emit for iOS/Android

**Status: deferred.** No graft host until a native emit roadmap lands. Re-open when Vox commits to native-client emit. Cross-link this row in any future native-emit roadmap.

---

### GA-11 — Implement single-node tokio scheduler for `@scheduled` + `@durable`

**Goal.** Close the [Durability Runtime Audit](durability-runtime-audit-2026.md) — make `@scheduled` and `@durable` actually run. Single-node tokio scheduler for v1; cluster-aware via Populi mesh in v1.5.

**Status precondition.** Independent at the runtime layer; benefits from GA-05 (effect system) for declaring the durable function's effect surface, but does not block on it.

**Files to read first.**
- [docs/src/architecture/durability-runtime-audit-2026.md](durability-runtime-audit-2026.md) — full document.
- [docs/src/architecture/web-app-archetype-coverage-2026.md](web-app-archetype-coverage-2026.md) §CC-17, §CC-18.
- `crates/vox-workflow-runtime/` and `crates/vox-actor-runtime/` — existing scaffolds.
- `crates/vox-compiler/src/hir/lower/mod.rs:322-326` — where `DurabilityKind` is currently lowered (parse-only today).

**Files to modify or create.**
- `crates/vox-workflow-runtime/src/scheduler.rs` (new) — tokio scheduler with cron parsing, missed-run policy as a structural enum (run-now / skip / catch-up).
- `crates/vox-workflow-runtime/src/durable.rs` (new) — at-least-once persistence of inputs, outputs, retry queue, dead-letter table.
- `crates/vox-codegen/src/codegen_rust/emit/durability.rs` (new) — emit registration of `@durable`/`@scheduled` functions to the runtime registry at startup.
- `crates/vox-db/src/schema/manifest.rs` — bump `BASELINE_VERSION` (per the canonical migration policy referenced by the mesh-canon docs); add `durable_jobs`, `dead_letter_jobs`, `scheduled_runs` tables.

**Acceptance criteria.**
1. A `@scheduled("0 * * * *") fn report() { ... }` runs once per hour from a single-node `vox` server.
2. A `@durable fn email_user(u: User) { ... }` survives a server crash mid-execution: re-running the server resumes from the last persisted step.
3. Retry-with-jitter respects the dead-letter cap; failures past the cap land in `dead_letter_jobs` and are queryable via stdlib.
4. Missed-run policy is structural: a `@scheduled(missed: skip)` function does not catch up; `missed: catch_up` runs every missed bucket; `missed: run_now` runs the latest one only. No flag magic — must be a typed enum on the decorator.

**Verification commands.**
```pwsh
cargo test -p vox-workflow-runtime scheduler durable
cargo test -p vox-codegen durability
cargo run -p vox-cli -- check examples/golden/durable_email.vox
```

**P-stack rubric.**
- **P0 lever:** lost-job-on-crash becomes structurally unrepresentable.
- **C4 hygiene:** structural missed-run policy, not flags.
- **Decision-point delta:** `−6` per durable function (no manual queue, no manual retry, no manual DLQ).

**Out of scope for this task.**
- Cluster-aware scheduling — slot under v1.5 / Populi mesh follow-up.
- LLM-specific durable wrappers (slot under GA-21).

**Estimated lift.** L (2000–3000 LoC across scheduler + DB schema + codegen).

---

### GA-12 — `Upload[T]` typed primitive + S3-compatible blob store

**Goal.** Implement [CC-01](web-app-archetype-coverage-2026.md). Streaming multipart, progress as a reactive value, image-derivative pipeline (CC-19 hook).

**Status precondition.** Benefits from GA-02 (Contract IR) for cross-stack typing. Otherwise independent.

**Files to read first.**
- [docs/src/architecture/web-app-archetype-coverage-2026.md](web-app-archetype-coverage-2026.md) §CC-01, §CC-19.

**Files to modify or create.**
- `crates/vox-compiler/src/lexer/token.rs` — add `Upload` type-keyword.
- `crates/vox-compiler/src/parser/descent/type_/upload.rs` (new).
- `crates/vox-blob-store/` (new crate, with workspace member entry) — `BlobStore` trait + S3-compatible (R2/B2) and local-disk impls. New crate (not in `vox-actor-runtime`) because uploads are a build-target-shaped concern, not an actor concern, and we want the dependency graph clean. Modules: `src/{lib,trait_,s3,local}.rs`.
- `crates/vox-codegen/src/codegen_rust/emit/upload.rs` (new) — Axum multipart handler.
- `crates/vox-codegen/src/web_ir/upload.rs` (new) — TS client with typed `upload(file: File)`.

**Acceptance criteria.**
1. `Upload[Image]` type structurally bounds size and content-type at the type level (a 10MB image upload of a non-image MIME refuses at the codec, not at the handler).
2. Streaming, no full-buffer.
3. Progress is a reactive value (composes with row 7).
4. CC-19 image-derivative pipeline (AVIF/WebP/JPEG at declared widths) is invokable from the upload's completion event.

**Verification commands.**
```pwsh
cargo test -p vox-blob-store
cargo test -p vox-codegen upload
cargo run -p vox-cli -- build --target=fullstack examples/golden/file_upload.vox
```

**P-stack rubric.**
- **P0 lever:** type-spoofing on uploads becomes structurally unrepresentable.

**Out of scope for this task.**
- Virus scan — stub trait, real impl deferred.

**Estimated lift.** M (1000–1500 LoC).

---

### GA-13 — Typed `Channel[Send, Recv]` + Axum WS extractor + AsyncAPI emit

**Goal.** Implement [CC-00](web-app-archetype-coverage-2026.md) and [CC-02](web-app-archetype-coverage-2026.md). Typed channels with reconnect sequence numbers; SSE stream type with `text/event-stream` content negotiation.

**Status precondition.** Benefits from GA-02 (Contract IR for envelope schema) and GA-05 (effect-tracked streaming).

**Files to read first.**
- [docs/src/architecture/web-app-archetype-coverage-2026.md](web-app-archetype-coverage-2026.md) §CC-00, §CC-02.
- [docs/src/architecture/wire-format-v1-ssot.md](wire-format-v1-ssot.md) — for envelope-schema convention.

**Files to modify or create.**
- `crates/vox-compiler/src/parser/descent/type_/channel.rs` (new).
- `crates/vox-actor-runtime/src/channel/{mod,reconnect,backpressure}.rs` (new modules in existing crate). Compose with the existing `subscription.rs` and `transport.rs` rather than duplicate.
- `crates/vox-codegen/src/codegen_rust/emit/channel.rs` (new).
- `crates/vox-codegen/src/web_ir/channel.rs` (new) — typed TS subscriber with `AsyncIterable<T>`.
- `crates/vox-codegen/src/asyncapi/mod.rs` (new) — emit AsyncAPI alongside OpenAPI.

**Acceptance criteria.**
1. A `Channel[OrderEvent, Ack]` declaration produces a typed Rust server handler and a typed TS subscriber; both reuse Contract IR types.
2. Killing the connection mid-stream and reconnecting replays missed envelopes via sequence numbers (golden harness exercises this).
3. SSE returns `text/event-stream` when `Accept` requests it, NDJSON otherwise; both decode to the same TS `AsyncIterable<T>` from the client.
4. Backpressure: bounded mpsc; no unbounded queues.

**Verification commands.**
```pwsh
cargo test -p vox-actor-runtime channel
cargo test -p vox-codegen channel asyncapi
```

**P-stack rubric.**
- **P0 lever:** raw-bytes channels become unrepresentable; envelope schema is structural.
- **C4 hygiene:** one channel shape, not two (no parallel "raw WebSocket" escape hatch).

**Out of scope for this task.**
- Yjs / CRDT integration (GA-25).

**Estimated lift.** L (1500–2500 LoC).

---

### GA-14 — `Notify { channel: Email | SMS | WebPush }` + wire existing `@push`/`@deep_link` HIR to codegen

**Goal.** Implement [CC-03](web-app-archetype-coverage-2026.md) for the deliverables that don't require native client emit. **Specifically use the existing `@push` and `@deep_link` HIR nodes** rather than introducing new tokens. APNs/FCM are deferred until native emit lands.

**Existing surface to align with.**
- `Token::AtPush` parses on the root App declaration with `on_register` / `on_notification` / `on_action` callback fields. Lowers to `HirPush`. **No codegen.**
- `Token::AtDeepLink` parses on root App with `scheme` / `universal_link` / `on_link`. Lowers to `HirDeepLink`. **No codegen.**

**Status precondition.** Independent. Composes with GA-09a (typed `href` for deep-link payload type-safety).

**Files to read first.**
- `crates/vox-compiler/src/parser/descent/decl/head.rs` — parsers for `@push` and `@deep_link`.
- `crates/vox-compiler/src/hir/lower/mod.rs` — app-level lowering of `HirPush` / `HirDeepLink`.
- [docs/src/architecture/web-app-archetype-coverage-2026.md](web-app-archetype-coverage-2026.md) §CC-03.

**Files to modify or create.**
- `crates/vox-compiler/src/parser/descent/type_/notify.rs` (new) — `Notify { channel: ..., recipient: ..., template: ... }` value type.
- `crates/vox-notify/` (new crate, with workspace member entry) — `Notify` value type runtime + delivery adapters. Modules: `src/{lib,trait_,email_ses,email_resend,email_postmark,sms_twilio,push_web}.rs`. New crate to keep adapter dependencies (Twilio, Resend, etc.) out of the actor-runtime dependency graph.
- `crates/vox-codegen/src/codegen_ts/push_app.rs` (new) — wire the existing `HirPush` to a Web-Push service-worker subscription on the client side.
- `crates/vox-codegen/src/codegen_rust/emit/push_endpoint.rs` (new) — server-side push-target endpoint reading from `vox-notify`.
- `crates/vox-codegen/src/codegen_ts/deep_link.rs` (new) — wire the existing `HirDeepLink` to client-side URL parser + state navigation.
- `crates/vox-codegen/src/codegen_rust/emit/notify.rs` (new).

**Acceptance criteria.**
1. Recipient type structurally constrains channel — `Notify { channel: Email, recipient: PhoneNumber }` refuses compile.
2. Delivery events stored in the stdlib `DeliveryEvent` table; status queryable.
3. Bounce/open webhooks slot under GA-16 (stub trait OK if GA-16 hasn't shipped).

**Verification commands.**
```pwsh
cargo test -p vox-notify
```

**P-stack rubric.**
- **P0 lever:** SMS-to-email-address structurally unrepresentable.

**Out of scope for this task.**
- APNs / FCM (deferred until native emit lands).
- Rich-notification rendering (deferred).

**Estimated lift.** M (800–1200 LoC).

---

### GA-15 — `@offline_capable` SW emit + `@collaborative` Yjs binding

**Goal.** Implement [CC-22](web-app-archetype-coverage-2026.md) and [CC-20](web-app-archetype-coverage-2026.md). Service-worker emit at build time; CRDT-typed values for collaborative editing.

**Status precondition.** Depends on GA-13 (channels for Yjs transport) and `RichText` from GA-19/GA-20 prerequisites.

**Files to read first.**
- [docs/src/architecture/web-app-archetype-coverage-2026.md](web-app-archetype-coverage-2026.md) §CC-22, §CC-20, §CC-12.

**Files to modify or create.**
- `crates/vox-compiler/src/parser/descent/decl/offline_capable.rs` (new).
- `crates/vox-codegen/src/service_worker/mod.rs` (new) — SW + manifest.webmanifest emit; strategy declared structurally per route.
- `crates/vox-crdt/` (new crate) — Yrs-backed CRDT runtime; modules `src/{lib,yjs}.rs`. New crate to keep `yrs` dependency isolated.
- `crates/vox-codegen/src/web_ir/collaborative.rs` (new).

**Acceptance criteria.**
1. `@offline_capable(strategy: stale_while_revalidate)` produces a working SW; Lighthouse PWA gate passes in goldens.
2. A `@collaborative` `RichText` field round-trips concurrent edits via Yrs; concurrent-edit fuzzer in `crates/vox-eval/` passes.
3. Conflict-free merge is automatic; user code does not see two values to reconcile.

**Verification commands.**
```pwsh
cargo test -p vox-codegen service_worker
cargo test -p vox-crdt
```

**P-stack rubric.**
- **P0 lever:** merge conflict surfacing to user code is structurally unrepresentable.

**Out of scope for this task.**
- Presence (cursors / awareness) — slot under follow-up.

**Estimated lift.** L (1500–2500 LoC).

---

### GA-16 — `@webhook(provider: ...)` decorator + verified-idempotent endpoint

**Goal.** Lift `crates/vox-webhook/` from runtime scaffold to a parser-backed surface. HMAC-SHA256 verification before body parse; replay-window enforcement; raw-byte preservation; idempotency-key write.

**Status precondition.** Independent.

**Files to read first.**
- [docs/src/architecture/web-app-archetype-coverage-2026.md](web-app-archetype-coverage-2026.md) §CC-04.
- `crates/vox-webhook/` and `crates/vox-plugin-webhook/` — existing scaffolds.

**Files to modify or create.**
- `crates/vox-compiler/src/lexer/token.rs` — add `AtWebhook`.
- `crates/vox-compiler/src/parser/descent/decl/webhook.rs` (new).
- `crates/vox-codegen/src/codegen_rust/emit/webhook.rs` (new) — emit verified handler that calls into `vox-webhook`.
- `crates/vox-webhook/src/verifiers/{stripe,github,slack,custom}.rs` — provider-specific HMAC verifiers.

**Acceptance criteria.**
1. A tampered signature produces a 401 *before* the body is parsed (golden test asserts the user code is never reached).
2. Replay of an already-seen idempotency key returns the cached response.
3. Body type is inferred from the provider's schema; mismatch is a compile error.
4. Per C4, no `@webhook` + manual HMAC pair — there is one canonical shape.

**Verification commands.**
```pwsh
cargo test -p vox-webhook verifiers
cargo test -p vox-codegen webhook
```

**P-stack rubric.**
- **P0 lever:** unverified webhook becomes unrepresentable in user code.

**Out of scope for this task.**
- Outbound webhooks (CC-04-C, deferred).

**Estimated lift.** M (700–1200 LoC).

---

### GA-17 — `Paginated[T]` typed value + cursor codec at wire-format level

**Goal.** Lift A1-03 from "stdlib component" to a typed `Paginated[T]` value with cursor codec at the wire-format SSOT level. Composes with GA-01 for status states.

**Status precondition.** Lands cleanest after GA-01 + GA-02.

**Files to modify or create.**
- `docs/src/architecture/wire-format-v1-ssot.md` — add cursor-codec entry (HMAC-signed; replay-resistant).
- `crates/vox-compiler/src/parser/descent/type_/paginated.rs` (new).
- `crates/vox-actor-runtime/src/pagination/cursor.rs` (new module in existing crate) — HMAC-signed cursor encoder.
- `crates/vox-codegen/src/web_ir/paginated.rs` (new) — Intersection-Observer wiring; debounced search input; virtualization.

**Acceptance criteria.**
1. `Paginated[Order]` over a `db.filter(...)` query produces both a server endpoint and a TS client method that handles cursor encoding, prefetch, virtualization, and debounced refetch.
2. Cursor signing uses HMAC; tampered cursors return a structured `vox/pagination/invalid-cursor` error.
3. Wire format SSOT documents the cursor codec; GA-02's drift-checker enforces it.

**Verification commands.**
```pwsh
cargo test -p vox-actor-runtime pagination
cargo test -p vox-codegen paginated
```

**P-stack rubric.**
- **P0 lever:** cursor forgery becomes structurally unrepresentable.

**Estimated lift.** M (700–1200 LoC).

---

### GA-18 — `@invalidates`/`@reads` cache-invalidation tags

**Goal.** Make cache-invalidation tags a typed primitive: a mutation declares what it invalidates; a query declares what it reads; the runtime computes staleness.

**Status precondition.** Defer until at least one user-pain signal arrives. No archetype currently flags this as a blocker.

**Files to modify or create.**
- *Doc-only graft until pain signal.* When ready: `crates/vox-compiler/src/parser/descent/decl/invalidates.rs` (new) and an addendum to [External Frontend Interop Plan](external-frontend-interop-plan-2026.md) Phase 4.

**Acceptance criteria.** *Deferred.*

**Estimated lift.** *Deferred.*

---

### GA-19 — Semantic UI primitives (proposed `CC-25`)

**Goal.** Add `menu`/`dialog`/`listbox`/`combobox`/`tabs` as built-in semantic UI primitives with WAI-ARIA wiring, focus management, keyboard nav as defaults. Compile-time check: cannot ship a `dialog` without a label.

**Status precondition.** Sequence after CC-23 design-tokens (GA-20) lands.

**Files to read first.**
- [docs/src/architecture/web-app-archetype-coverage-2026.md](web-app-archetype-coverage-2026.md) §CC-23 (precondition) and §1.x archetypes A6/A14 that need it.

**Files to modify or create.**
- `docs/src/architecture/web-app-archetype-coverage-2026.md` — append `CC-25. semantic UI primitives`.
- `crates/vox-compiler/src/parser/descent/decl/semantic_ui.rs` (new) — keywords for the five primitives.
- `crates/vox-codegen/src/web_ir/semantic_ui/{menu,dialog,listbox,combobox,tabs}.rs` (new).

**Acceptance criteria.**
1. A `dialog { ... }` without a label refuses compile with `vox/a11y/dialog-missing-label`.
2. Focus-trap and restore-focus-on-close are emitted by default; cannot be opted out without `// vox:skip`.
3. Keyboard nav matches the WAI-ARIA Authoring Practices Guide for each primitive.

**Verification commands.**
```pwsh
cargo test -p vox-compiler semantic_ui
cargo test -p vox-codegen semantic_ui
```

**P-stack rubric.**
- **P0 lever:** focus-trap omission and missing-label become structurally unrepresentable. C2 wedge: this is the GUI proof-of-thesis.

**Estimated lift.** L (1200–2000 LoC across five primitives).

---

### GA-20 — Design tokens as types (`CC-23`)

**Goal.** Implement [CC-23](web-app-archetype-coverage-2026.md). Tokens at the project root; components consume by name; compile-time contrast violation refusal; `@light`/`@dark` required pairs.

**Status precondition.** Independent. Precondition for GA-19.

**Files to modify or create.**
- `crates/vox-compiler/src/parser/descent/decl/tokens.rs` (new) — `Token { color, spacing, radius, shadow, font }` declaration syntax.
- `crates/vox-compiler/src/typecheck/contrast.rs` (new) — refuse compile when two named tokens used together violate contrast (P0).
- `crates/vox-codegen/src/web_ir/tokens.rs` (new) — emit CSS variables + typed TS export.

**Acceptance criteria.**
1. A raw hex value `#ff0000` inlined in a styled component refuses compile (`vox/tokens/raw-color`).
2. A `Color.Surface.Primary` paired with a `Color.Text.Primary` whose contrast ratio is below 4.5:1 refuses compile.
3. `@dark` variant required for every token that has a `@light` (and vice versa).

**Verification commands.**
```pwsh
cargo test -p vox-compiler tokens contrast
```

**P-stack rubric.**
- **P0 lever:** contrast violation structurally unrepresentable. C2 wedge.

**Estimated lift.** M (800–1200 LoC).

---

### GA-21 — Wire existing `@ai` / `@tool` HIR to codegen + add structured-output validation

**Goal.** Land typed prompts and structured-output enforcement on top of the *existing* `@ai` and `@tool` decorators (which today parse + lower but have no codegen) and the *existing* `prompt_canonical.rs` runtime. Per C2/C4, **do not** introduce a new `@prompt` keyword — `@ai` already plays this role.

**Existing surface to align with.**
- `Token::AtAi` parses on `fn` decls with optional model name; lowers to HIR with `is_llm` flag. **No codegen.**
- `Token::AtTool` parses (`@tool`/`@mcp.tool`); used for MCP tool registration in `crates/vox-orchestrator-mcp/`.
- `crates/vox-actor-runtime/src/prompt_canonical.rs` (316 LoC) ships canonical-prompt construction.
- `crates/vox-actor-runtime/src/llm/{chat,embed,stream,wire,types}.rs` ship a working LLM client surface.
- CC-21 (provider-agnostic tool-call routing) is in flight in `crates/vox-orchestrator-mcp/`.

**Status precondition.** Lands cleanest after GA-02 (Contract IR for output validation) and GA-05 (effect system for `@uses(net)`). The `@ai` codegen path can ship before structured-output validation as a thin wrapper that calls `prompt_canonical`.

**Files to read first.**
- `crates/vox-compiler/src/lexer/token.rs` — confirm `AtAi` and `AtTool` parse today.
- `crates/vox-compiler/src/parser/descent/decl/head.rs` — parsers for `@ai`/`@tool`.
- `crates/vox-actor-runtime/src/prompt_canonical.rs` — runtime prompt construction.
- `crates/vox-actor-runtime/src/llm/` — entire submodule.
- [docs/src/architecture/web-app-archetype-coverage-2026.md](web-app-archetype-coverage-2026.md) §CC-21.
- [docs/src/architecture/unified-task-hopper-research-2026.md](unified-task-hopper-research-2026.md) — typed-job contract this should be consistent with.
- [docs/src/architecture/populi-mesh-improvement-backlog-2026.md](populi-mesh-improvement-backlog-2026.md) — mesh-side coordination.

**Files to modify or create.**
- `crates/vox-compiler/src/typecheck/structured_output.rs` (new) — typecheck `@ai fn name(args) -> SomeStruct` so that `SomeStruct` round-trips through GA-02's Contract IR. Diagnostic id `vox/ai/return-shape-not-codec'd` if `SomeStruct` lacks a wire codec.
- `crates/vox-actor-runtime/src/llm/structured_output.rs` (new module in existing `llm/`) — runtime validator that the LLM's structured output matches the declared `SomeStruct`; on mismatch, return a typed mismatch error.
- `crates/vox-actor-runtime/src/llm/reprompt.rs` (new module) — automatic re-prompt up to declared `max_iterations` on schema failure; coordinate with existing `prompt_canonical.rs`.
- `crates/vox-codegen/src/codegen_rust/emit/ai_fn.rs` (new) — emit the body of an `@ai`-annotated function as a call into `vox-actor-runtime::llm::chat` + `structured_output::validate` + `reprompt::with_retries`.
- Diagnostic catalog entries: `vox/ai/return-shape-not-codec'd`, `vox/ai/missing-uses-net`, `vox/ai/structured-output-divergence`.

**Acceptance criteria.**
1. `@ai fn plan(user: User, goal: Goal) -> Plan` is callable from user code; codegen emits a body that consults `vox-actor-runtime::llm::chat` and validates the structured output against `Plan` using GA-02's Contract IR.
2. A schema-validation failure triggers automatic re-prompt up to the declared `max_iterations`; if exceeded, returns a typed `LlmError::StructuredOutputDivergence`.
3. HITL approval for dangerous tool calls is wired to GA-04's capability check via `@require(can: ...)` — stub if GA-04 hasn't shipped, but the trait surface must already accept a capability check so the wiring is non-breaking later.
4. Per C2/C4: no new `@prompt`, `agent` bare keywords. The existing `@ai` and `@tool` are the only surfaces; `agent { ... }` is anti-recommended (§5).
5. `@ai` requires `@uses(net)` on the same function (after GA-05 lands) — diagnostic `vox/ai/missing-uses-net`.

**Verification commands.**
```pwsh
cargo test -p vox-actor-runtime llm
cargo test -p vox-compiler prompt
```

**P-stack rubric.**
- **P0 lever:** schema-divergent LLM output structurally unrepresentable in user code.
- **C2/C4 hygiene:** decorator only; no new bare keywords.

**Out of scope for this task.**
- Extending `@tool` semantics beyond what `crates/vox-orchestrator-mcp/` already provides — file as a separate follow-up.
- Agentic loop control flow (the brief's `agent { ... }` is anti-recommended; see §5).
- Streaming-as-an-effect — slot under GA-13 (channel/stream typing) once that lands.

**Estimated lift.** L (1500–2500 LoC).

---

### GA-23 — `@pii` taint companion to `@secret`

**Goal.** Add static taint propagation: a value carrying `@pii` cannot reach an external-call site without redaction or recorded consent.

**Status precondition.** Depends on GA-05 (effect system) and Phase 4 `@secret` redactor having shipped at least at the runtime level.

**Files to read first.**
- [docs/src/architecture/vox-language-rules-phase4-runtime-monitors-2026.md](vox-language-rules-phase4-runtime-monitors-2026.md) — §`@secret` redactor.
- [docs/src/architecture/vox-language-rules-phase5-effects-determinism-2026.md](vox-language-rules-phase5-effects-determinism-2026.md) — §effect taint.

**Files to modify or create.**
- `crates/vox-compiler/src/lexer/token.rs` — add `AtPii`.
- `crates/vox-compiler/src/typecheck/taint.rs` (new) — static `@pii` propagation through assignments and function calls; refuse compile when a tainted value reaches a `@uses(net)` call site without `redact()` or `consent_recorded()`.

**Acceptance criteria.**
1. A `User.email: @pii String` reaching `llm.complete(prompt)` without `email.redact()` refuses compile with `vox/taint/pii-leak`.
2. `consent_recorded(user, "email")` clears the taint at the call site; the runtime emits an audit-log entry.
3. The symmetric `vox/taint/unjustified-pii` warning fires when `@pii` is declared on a value that demonstrably does not carry PII.

**Verification commands.**
```pwsh
cargo test -p vox-compiler taint
```

**P-stack rubric.**
- **P0 lever:** PII leak structurally unrepresentable.

**Out of scope for this task.**
- Right-to-erasure cascade through embedding stores (defer).

**Estimated lift.** M (700–1100 LoC).

---

### GA-24 — `Vector[N]` + `@embed(model:)` decorator + hybrid search

**Goal.** Implement [CC-16](web-app-archetype-coverage-2026.md). Statically dimensioned `Vector[N]`, embedding generation as a typed effect, hybrid (BM25 + vector) search via `db.search`.

**Status precondition.** Coordinate with [Search & Retrieval SSOT](search-retrieval-ssot-2026.md) so the language surface and `vox-search` runtime are aligned.

**Files to modify or create.**
- `crates/vox-compiler/src/parser/descent/type_/vector.rs` (new).
- `crates/vox-compiler/src/lexer/token.rs` — add `AtEmbed`.
- `crates/vox-search/src/vector/{mod,pgvector,sqlite_vss,in_memory}.rs` (new or extend).
- `crates/vox-codegen/src/codegen_rust/emit/embed.rs` (new).

**Acceptance criteria.**
1. `Vector[768]` and `Vector[1536]` are distinct types; passing one to a function expecting the other refuses compile.
2. `@embed(model: "text-embedding-3-small")` on a `@table` field auto-generates embeddings on insert/update.
3. `db.search(by: similarity(query_vec, top_k: 10))` returns a typed `[SearchResult[T]]`.

**Verification commands.**
```pwsh
cargo test -p vox-search vector
cargo test -p vox-compiler vector embed
```

**P-stack rubric.**
- **P0 lever:** dimension mismatch structurally unrepresentable.

**Estimated lift.** L (1200–1800 LoC).

---

### GA-25 — Real-time multiplayer (composes GA-13 + GA-15)

**Goal.** Real-time multiplayer is the composition of channels (GA-13) + CRDT (GA-15) + presence. No new top-level primitive — the user writes `@collaborative` `RichText` over a `Channel[...]` and gets Liveblocks-class functionality emergent from the language.

**Status precondition.** GA-13 and GA-15 must ship first.

**Files to modify or create.**
- `crates/vox-actor-runtime/src/presence.rs` (new module in existing crate) — cursor presence over an existing `Channel`. Extend the existing `subscription.rs` for awareness fan-out.
- `examples/golden/multiplayer_doc.vox` (new) — composes `@collaborative` + `Channel` + presence.

**Acceptance criteria.**
1. Two clients see each other's cursors within one round-trip of a `Channel` heartbeat.
2. Concurrent-edit fuzzer in `vox-eval` passes.
3. Per C4, no new `multiplayer` keyword; functionality is emergent.

**Estimated lift.** M (500–800 LoC, mostly composition + golden).

---

### GA-26 — Layered layout discipline: typed Z-tiers + partitioning containers + `Mark<T>`

**Goal.** Make Z-fighting, accidental occlusion, and tier inversion structurally unrepresentable in VUV (the Vox view-tree language). Adopt the `wlr-layer-shell` four-tier model (extended to seven for app UIs) plus i3/Sway's tree-of-partitioning-containers discipline. See companion memo: [`vuv-layered-layout-discipline-2026.md`](vuv-layered-layout-discipline-2026.md).

**Existing surface to align with.** WebIR (`crates/vox-codegen/src/web_ir/`) already represents view trees as `DomNode` arenas with `view_roots`, plus dedicated `validate_a11y`, `validate_keys`, and `validate_overlay` passes. A `validate_overlay.rs` module exists but does NOT enforce a closed Z-tier enum today — so this graft is *strengthening an existing validation seam*, not adding a parallel one. Do not duplicate into a new `validate_z.rs`; extend `validate_overlay.rs`.

**Status precondition.** Independent at the parser layer; benefits from GA-19 (semantic UI primitives — dialog/tooltip/menu naturally bind to specific tiers) and GA-20 (design tokens — z-tier *visual* affordances like elevation shadows live there).

**Files to read first.**
- [`vuv-layered-layout-discipline-2026.md`](vuv-layered-layout-discipline-2026.md) — the design memo this graft implements.
- [i3 User's Guide §Tree](https://i3wm.org/docs/userguide.html#_tree) and [§VIM-like marks](https://i3wm.org/docs/userguide.html#vim_like_marks) — for tree partitioning + marks model.
- [`zwlr_layer_shell_v1` spec](https://wayland.app/protocols/wlr-layer-shell-unstable-v1) — for the four-tier `background < bottom < top < overlay` precedent.
- `crates/vox-codegen/src/web_ir/validate_overlay.rs` — existing seam to strengthen.
- `crates/vox-codegen/src/web_ir/mod.rs` — `DomNode` schema; new variants live here.

**Files to modify or create.**
- `crates/vox-compiler/src/lexer/token.rs` — add `AtLayer` (decorator on a component declaring its z-tier).
- `crates/vox-compiler/src/hir/nodes/layer.rs` (new) — `LayerTier` closed enum (`Background`, `Content`, `Chrome`, `Popover`, `Modal`, `Toast`, `SystemOverlay`); `HirMarkRef` for typed cross-tree handles.
- `crates/vox-compiler/src/typeck/layer.rs` (new) — refuse compile when:
  - a `Tooltip` is rendered as a parent of a `Modal` (`vox/layer/tier-inversion`);
  - two siblings inside a partitioning container declare overlapping geometry (`vox/layer/sibling-overlap`);
  - a `Mark<"k">` is declared twice in the same view-tree scope (`vox/layer/duplicate-mark`);
  - a `Mark<T>`-typed jump target references a name that no surface declares (`vox/layer/dangling-mark`).
- `crates/vox-codegen/src/web_ir/validate_overlay.rs` (extend) — wire the new diagnostics through the existing overlay pass.
- `crates/vox-codegen/src/web_ir/layer_emit.rs` (new) — emit CSS portals + a single `<div data-vox-layer="modal">` per active tier; tier ordering is enforced via a generated stylesheet, not user-authored z-index.

**Acceptance criteria.**
1. A `component MyTooltip @layer(tier: Modal)` refuses compile with `vox/layer/tier-inversion` (a Tooltip in Modal tier is a category mismatch).
2. A `Modal` declared inside a `Toast` parent's view tree refuses compile with `vox/layer/tier-inversion`.
3. A `mark "primary-action"` declared on two sibling view roots refuses compile with `vox/layer/duplicate-mark`.
4. A `Tooltip::for(target: Mark<"missing">)` where no surface declares that mark refuses compile with `vox/layer/dangling-mark`.
5. Raw `style={{ zIndex: 9999 }}` in a generated TSX file refuses lint (lifted in GA-20 if `vox/tokens/raw-color` already does the analogous check for color); diagnostic id `vox/layer/raw-z-index`.
6. The generated CSS embeds exactly seven layered `data-vox-layer="…"` selectors with a fixed z-index ladder; the ladder is **not user-extensible from `.vox` source** (per C4, one canonical ordering).

**Verification commands.**
```pwsh
cargo test -p vox-compiler layer
cargo test -p vox-codegen layer_emit overlay
cargo run -p vox-cli -- check examples/golden/layer_modal_tooltip.vox
```

**P-stack rubric.**
- **P0 levers:** Z-fighting, tier inversion, dangling-mark are all structurally unrepresentable.
- **C2/C4 hygiene:** one canonical tier ladder; one canonical mark namespace; partitioning layouts (Row/Col/Tabs/Stack) are the *only* parents that can hold non-overlay siblings — no `position: absolute` escape hatch except via `@layer(tier: …)`.
- **Decision-point delta:** `−4` per overlay site (no z-index choice, no portal-target choice, no tier-name choice, no de-collision via stacking-context kludges).

**Out of scope for this task.**
- Native (iOS/Android) layer-shell binding — defer with GA-09b.
- Compositing-layer hints (`will-change`, `contain: layout`) — performance follow-up.
- Floating-window discipline as a runtime escape hatch — initial graft is structural-only at compile time.

**Estimated lift.** L (1500–2500 LoC across HIR + typeck + WebIR + emit + golden).

---

## §5 — Anti-recommendations

- **The brief's `external pure | external retryable | external streaming` bare-keyword family.** Use decorator parameters on the existing `@uses(net)` effect (GA-05). Bare keywords are a C2/C4 cost. **Specifically refuse**: any PR that adds `External`, `Retryable`, `Streaming` to `crates/vox-compiler/src/lexer/token.rs`.
- **The brief's `agent { steps, tools, max_iterations, stop_when }` primitive control flow.** Duplicates the Hopper / Populi mesh / Agentic VCS spine. The "agent is just a function" framing the brief itself proposes (in #22) is the right end state — built atop typed effects + capabilities + durable functions, not a new keyword. **Specifically refuse**: any PR that adds `Agent` as a bare keyword token, or proposes an `agent { ... }` block syntax.
- **The brief's "Phase 1 = build async/effect type + cross-stack types + reactive primitives" framing.** Reactive primitives are already shipped (`state`/`derived`/`effect` parse + lower + emit; see row #7). The brief reads as if Vox were a blank slate. The [archetype coverage map §4 Pareto sequence](web-app-archetype-coverage-2026.md) remains authoritative over the brief's Phase 1–4.
- **The brief's "Phase 4 cross-cutting polish" lumping.** Heterogeneous group: theming/tokens (CC-23, GA-20) and a11y primitives (GA-19) are P0 wedge items, not polish; webhook receivers (GA-16) are partially built; i18n (GA-08) is a genuine delta. Do not plan as a single phase.
- **The brief's "Phase 1: 1. Async/effect type … 2. Cross-stack structural types … 3. Reactive primitives"** ordering. Per the dependency map (§4.1), GA-02 (cross-stack/Contract IR) is the unblocker for 7+ downstream grafts and should land *first*. Reactive primitives are already shipped. GA-01 (`Async[T]`) sequences after GA-02. The brief's order would invert the leverage.
- **Adding a new `@prompt` keyword for LLM calls.** `@ai` already plays this role and parses today (`Token::AtAi`). Adding `@prompt` as a synonym would be a C4 cosmetic-alternative violation; rejecting `@ai` to introduce `@prompt` would invalidate every existing call site in the corpus. GA-21 explicitly extends `@ai`, not replaces.
- **Adding `@role` alongside `@require`.** The brief's #4 sketches RBAC. Per C4, `@require(can: capability_fn(args))` already covers role-shape policies (a role is a capability function); adding `@role(...)` would be a cosmetic alternative.

## §6 — Audit re-run / drift-check commands

This document was code-audited on 2026-05-09 against the worktree at `cc_bdesktop2/goofy-yonath-db8222`. To verify the document has not drifted from current code before relying on it, run:

```pwsh
# 1. Confirm lexer tokens still match the audit's assumptions
rg -nE "Token::(AtForm|AtRequire|AtAi|AtTool|AtPush|AtDeepLink|AtCancellable|AtLoading|AtBackButton|AtScheduled|AtTable|AtEndpoint|AtPure|AtMcpTool|AtMcpResource)" crates/vox-compiler/src/lexer/token.rs

# 2. Confirm vox-actor-runtime modules still in the layout the GA blocks assume
ls crates/vox-actor-runtime/src/
ls crates/vox-actor-runtime/src/llm/

# 3. Confirm the doc cross-links resolve
cargo run -p vox-cli -- ci docs-quality

# 4. Confirm baseline test suite green
cargo test -p vox-compiler
cargo test -p vox-codegen
cargo test -p vox-actor-runtime
cargo run -p vox-arch-check

# 5. Confirm form codegen still has end-to-end coverage (precondition for GA-03)
ls crates/vox-codegen/src/codegen_ts/form_emit.rs
cargo test -p vox-codegen form

# 6. Confirm the durability audit's "zero runtime" verdict still holds (precondition for GA-11)
rg -n "vox.scheduler|tokio_cron" crates/vox-actor-runtime/src/scheduler.rs
# If matches grow significantly, the audit is stale; refresh GA-11.
```

**If any of the above commands return unexpected output, treat the gap analysis as stale and re-audit before starting any GA-* graft.** The document carries `last_updated: 2026-05-09` — if more than ~30 days have elapsed, prefer a fresh audit over trusting the verdicts.

## §7 — Cross-references

- [Boilerplate Reduction Design Brief (2026)](boilerplate-reduction-design-brief-2026.md) — the source brief filed verbatim with reviewer notes; cross-reference table there now carries audit-verdict column.
- [where-things-live.md](where-things-live.md) — flat lookup of "this concept lives in this crate"; required reading per §4.0 pre-flight.
- [Web App Archetype Coverage Map (2026)](web-app-archetype-coverage-2026.md) — Vox's authoritative inside-out backlog spine.
- [External Frontend Interop Plan (2026)](external-frontend-interop-plan-2026.md) — host phase for many grafts.
- [Vox Language Rules & Enforcement Plan (2026)](vox-language-rules-and-enforcement-plan-2026.md) — Phase 4/5 host for effect-system grafts.
- [Populi Mesh North-Star (2026)](populi-mesh-north-star-2026.md) + [Populi Mesh Improvement Backlog (2026)](populi-mesh-improvement-backlog-2026.md) + [Unified Task Hopper Research (2026)](unified-task-hopper-research-2026.md) — host for the agentic / hopper / mesh grafts.
- [Frontend Convergence Findings (2026-05-08)](frontend-convergence-findings-2026.md) — Contract IR proposal subsumes brief #2.
- [Wire Format v1 SSOT](wire-format-v1-ssot.md) — host for GA-02, GA-17 cursor codec.
- [Telemetry Unification Design (2026)](telemetry-unification-design-2026.md) — covers brief #10 in part; runtime architecture in flight.
- [Durability Runtime Audit (2026)](durability-runtime-audit-2026.md) — current state of `@durable`/`@scheduled`; GA-11 closes this audit.
- [Svelte-Mineable Features Implementation Plan (2026)](svelte-mineable-features-implementation-plan-2026.md) — host for GA-09a (M6 typed `href`).
- [Plugin System Redesign (2026)](plugin-system-redesign-2026.md) — answers stdlib-vs-core-vs-plugin for any graft.
- [Search & Retrieval SSOT (2026)](search-retrieval-ssot-2026.md) — coordination point for GA-24.
- [Agentic VCS Phase 1 Implementation Plan (2026)](agentic-vcs-automation-impl-plan-phase1-2026.md) — proven precedent for capability tokens (relevant for GA-04).
- [LANGUAGE_DESIGN_PRIORITIES.md](../../../LANGUAGE_DESIGN_PRIORITIES.md) — P0–P5 / C1–C5 decision rubric every graft is scored against.
- [AGENTS.md](../../../AGENTS.md) — cross-tool policy; required reading before any code change.
