---
title: "Web App Archetype Coverage Map (2026)"
description: "Coverage map of 21 web-app archetypes against Vox's current substrate. Every blocker traced to a named language, runtime, codegen, or DevEx gap. Prioritization input for the next slate of high-value, low-debt improvements."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-05-02"
training_eligible: true
training_rationale: "Strategic prioritization input; canonical archetype-to-blocker mapping. Anchors the next slate of language, runtime, and orchestrator improvements to concrete user-journey friction."
---

# Web App Archetype Coverage Map (2026)

## Premise

Vox today supports a narrow band of web-app shapes well: full-stack CRUD with reactive components, typed endpoints, and a generated TS/React frontend. The five-phase [external frontend interop plan](external-frontend-interop-plan-2026.md) widens the band — backend-only mode, OpenAPI emit, HTTP decorators, auth stdlib, bidirectional component interop. This document inverts the lens: instead of "what should the language add next?", ask "what does a user trying to ship $ARCHETYPE actually hit, and which gaps unblock the most archetypes per unit of design surface?"

This document is a *backlog spine*, not a wishlist. Every entry is anchored to one of:

- An observed gap in the codebase (named crate / module / decorator / runtime primitive missing)
- An item already called out in an existing audit (e.g. [`model-orchestration-ssot-audit-2026.md`](model-orchestration-ssot-audit-2026.md), [`orchestrator-companion-audit-findings-2026.md`](orchestrator-companion-audit-findings-2026.md), [`nextgen-orchestrator-research-2026.md`](nextgen-orchestrator-research-2026.md))
- A user-journey friction point that empirically blocks an archetype the codebase already scaffolds (`vox init --kind=...`)

Items are written as 3–4 line stubs: **what's missing**, **why it blocks**, **where it slots** (existing phase, or new initiative).

## Scoring rubric

Every blocker is judged on four axes. The recommended sequence at the bottom is a Pareto frontier on these:

- **P-stack alignment** — does the item advance P0 (structural prevention) or P1 (decision minimization), or just add a feature? Items that *remove* sampling decisions outrank items that add them. See [LANGUAGE_DESIGN_PRIORITIES.md](../../../LANGUAGE_DESIGN_PRIORITIES.md).
- **Archetype coverage leverage** — how many of the 21 archetypes does shipping this unblock? A WebSocket primitive unblocks 6 archetypes; a Stripe integration unblocks 4. Coverage > novelty.
- **Maintenance footprint** — does this enlarge the surface that future agents must reason about? Decorators on existing keywords cost less than new bare keywords. Adapters over a single SSOT cost less than parallel emit paths.
- **Decision-point delta** — does the item *add* configuration knobs the model has to choose between, or *remove* them? Per C4, two cosmetically equivalent expressions is a bug, not a feature.

A "high value" item scores well on all four. An item that scores well on coverage but adds a bare keyword is a P-stack regression. An item that removes decisions but only unblocks one archetype is leverage-poor — keep it but sequence it later.

## How to read the archetype sections

Each archetype lists:

- **What works today** — the shape of an MVP that actually compiles and runs end-to-end against current goldens.
- **Blockers** — itemized, with: gap title, what's missing, why it blocks, where it slots.
- **Cross-cutting links** — references to the `CC-NN` items in §Cross-cutting infrastructure spine. The CC items are where leverage compounds.

---

# §1 — Archetypes

## §1.1 Tier 1 — Mostly works today

### A1. Basic CRUD app (todo, contacts, simple inventory)

**Status:** Tier 1 — works.
**What works today:** `@table` types, `db.Table.all/filter/insert/update/delete`, `@endpoint(kind: query|mutation)`, reactive components binding to query results, client routes. End-to-end demonstrated by [`crud_api.vox`](../../../examples/golden/crud_api.vox) and [`blog_fullstack.vox`](../../../examples/golden/blog_fullstack.vox).
**Blockers:**
- **A1-01 No optimistic UI primitive** — there is no `optimistic_update` helper in the component runtime, so writes round-trip to the server before reflecting in the UI. Blocks the snappiness users now expect from CRUD apps. Slot: Phase 4 reactive runtime extension; new `state.optimistic_with(rollback_on_error: ...)` op.
- **A1-02 No `@form` decorator on routes** — form submission is hand-wired through `state` + `mutation`. Forces every CRUD app to repeat boilerplate; high decision-point cost for what is structurally one shape. Slot: Phase 3 HTTP ergonomics, decorator on `component`.
- **A1-03 No standard pagination component** — `pagination.vox` shows the pattern, but each app reimplements it. Re-derives a structurally identical decision on every CRUD page. Slot: stdlib `std.ui.paginated_list` component shipped in v1 stdlib.
- **A1-04 No empty-state / loading-state slot** — components don't have a structural way to express "while query loading" / "if empty" — must be hand-rolled in the view block. Models routinely omit one. Slot: language-level addition to component blocks (`while loading { ... } when empty { ... }`).
- **A1-05 No `vox emit fixture-data`** — seeding a CRUD app for demo or test requires writing a `.vox` script. A `vox seed --table=Foo --count=N` would close the dev-loop. Slot: CLI subcommand on top of existing `db` surface.
- **A1-06 No `@confirm` decorator for destructive mutations** — every delete reimplements a confirmation modal. Blocks UX correctness. Slot: decorator on `@endpoint(kind: mutation)` that requires a typed confirmation token.

**Cross-cutting links:** CC-08 (RBAC), CC-15 (audit log).

### A2. Marketing / blog / docs site

**Status:** Tier 1 — works for the application shell; content authoring is the gap.
**What works today:** `routes`, components, static-ish pages emitted via the full-stack pipeline. Server-rendered output via the existing Vite/SSR path.
**Blockers:**
- **A2-01 No native Markdown component** — there's no `std.content.markdown(source: str)` that renders sanitized MD. Every blog reimplements the same parser bridge. Slot: stdlib component, vendored CommonMark, ships with the runtime.
- **A2-02 No content-collection primitive** — Vox has no equivalent of Astro's content collections (typed frontmatter + filesystem-as-CMS). Forces every content site to model rows in `@table` instead. Slot: new `@content` decorator on a directory; emits typed entries at compile time. Read-only at runtime.
- **A2-03 No RSS / Atom emit** — already has `feed.xml` for the docs site, but there's no language-level emit for application feeds. Blocks marketing-site portability. Slot: stdlib `std.feed.rss(items: ...)`.
- **A2-04 No image optimization pipeline** — markdown image references aren't resized/converted to AVIF/WebP at build time. Blocks Lighthouse-passing sites. Slot: build-time pipeline behind `@asset(image)` decorator.
- **A2-05 No SEO metadata as types** — `<title>`, OG tags, canonical URL are stringly-typed in component view blocks. Models silently omit canonical/OG. Slot: structural `meta { title: ..., og: ..., canonical: ... }` block in `component`.
- **A2-06 No sitemap.xml emit** — site map is not derived from `routes`. Blocks indexability. Slot: build artifact emitted alongside the OpenAPI doc in Phase 2.
- **A2-07 No view-transition primitive** — page-to-page animations are not surfaced; client routing is teleport-by-default. Blocks polished marketing feel. Slot: opt-in decorator on `routes` block.

**Cross-cutting links:** CC-12 (rich text), CC-19 (asset pipeline).

### A3. Read-only admin dashboard (single-tenant)

**Status:** Tier 1 — partial. Tables work; charts and exports don't.
**What works today:** Components with `db.Table.filter()` reactivity demonstrated by [`dashboard_ui.vox`](../../../examples/golden/dashboard_ui.vox). Multi-table layouts, derived state, basic filters.
**Blockers:**
- **A3-01 No charting primitives** — there is no `std.chart.line / bar / area` component. Every dashboard reaches for a TS chart library through the (retiring) `@island` bridge. Slot: stdlib component lib backed by D3 or Recharts; emits via Phase 5 React interop.
- **A3-02 No date-range picker stdlib** — every admin reimplements one. High decision-point burden. Slot: `std.ui.date_range_picker` with structural type for the range. Couples to CC-11 (time-series).
- **A3-03 No CSV / XLSX export decorator** — `@endpoint(format: csv)` doesn't exist; admins hand-roll `text/csv` responses. Slot: response negotiation decorator, server-side serializer in stdlib.
- **A3-04 No `@scheduled` runtime** — decorator parses but does nothing at runtime per [`v1-release-criteria.md`](v1-release-criteria.md). Blocks any dashboard that needs nightly aggregations. Slot: Phase 4 durability runtime.
- **A3-05 No drill-down navigation pattern** — clicking a chart point to filter another panel requires lifting state through a parent. Blocks composable analytics. Slot: a `linked_filter[T]` typed channel between components.
- **A3-06 No saved-view persistence** — admins reimplement "save filter to URL" or "save filter to user prefs" every time. Slot: `@persisted_state` decorator on a component-local state.

**Cross-cutting links:** CC-11 (time-series), CC-08 (RBAC), CC-09 (audit log), CC-19 (asset pipeline).

### A4. Internal API + typed TS client (Phase 1 backend-only)

**Status:** Tier 1 — works once Phase 1 lands. Currently in flight.
**What works today:** `@table`, `@endpoint`, server compilation. The `--target=server` flag and `vox emit client` are scoped in [Phase 1 of the interop plan](external-frontend-interop-plan-2026.md).
**Blockers:**
- **A4-01 Phase 1 not landed in stable channel** — `--target=server` is roadmapped, not shipped. Until then, server-only consumers either depend on the full-stack emit's transient artifacts or hand-write a client.
- **A4-02 OpenAPI emit pending** — the Phase 2 OpenAPI 3.1 emitter is the artifact that unlocks every TS-side consumer (RTK Query, openapi-typescript, Orval). Slot: Phase 2.
- **A4-03 No RPC-mode client** — only REST is emitted; no typed RPC client where method signatures match Vox `fn` signatures (à la tRPC). Slot: extension to Phase 2 client emit, gated on user opting into a single-origin contract.
- **A4-04 No streaming response support** — endpoints can't return `Stream[T]` because the wire format SSOT pins JSON only. Blocks AI chat backends. Slot: CC-02 (SSE) plus a wire-format addendum for `application/x-ndjson`.
- **A4-05 No request context propagation** — there's no language-level `Request` context type carrying `request_id`, `user`, `trace_id`. Forces every endpoint to take these as explicit params. Slot: implicit `ctx: RequestContext` in `@endpoint`-decorated `fn`s.
- **A4-06 No idempotency-key primitive** — POST endpoints can't structurally declare they accept an `Idempotency-Key` header and dedupe. Blocks any API consumed by webhooks. Slot: decorator + storage backend on `@endpoint(kind: mutation)`.
- **A4-07 No API versioning primitive** — adding `v2/` to a route family is hand-coded. Slot: `@version("v2")` decorator emitting a path prefix and changelog entry.

**Cross-cutting links:** CC-04 (webhooks), CC-13 (rate limit), CC-08 (RBAC).

## §1.2 Tier 2 — Plausible with named, finite work

### A5. Multi-tenant B2B SaaS

**Status:** Tier 2.
**What works today:** Manual tenant_id filtering pattern shown in [`multi_tenancy.vox`](../../../examples/golden/multi_tenancy.vox). Sessions table pattern in [`auth_patterns.vox`](../../../examples/golden/auth_patterns.vox).
**Blockers:**
- **A5-01 Tenancy is structural, not enforced** — `tenant_id` filtering is a convention, not a type. A model can write a query that omits it. Direct violation of P0 — exactly the class of bug Vox should structurally prevent. Slot: CC-07 (multi-tenancy primitive).
- **A5-02 No SSO primitives** — OIDC / SAML have no decorator surface; every SaaS reimplements the dance. Slot: CC-05 (OAuth/OIDC).
- **A5-03 No JWT verification stdlib** — `auth_patterns.vox` rolls its own session lookup; no JWT verifier in stdlib. Slot: CC-06 (JWT/sessions).
- **A5-04 No subscription billing primitive** — Stripe Checkout, webhook ingestion, plan-gating logic are user-implemented. Slot: CC-10 (payments).
- **A5-05 No usage metering primitive** — cannot structurally declare "this endpoint consumes 1 unit of plan X." Slot: `@meter(plan_unit: ...)` decorator; couples to billing.
- **A5-06 No team-invite flow primitive** — invite tokens, email delivery, role assignment are all per-app. Slot: stdlib `std.team.invite_flow(...)`; couples to CC-03 (email).
- **A5-07 No audit-log primitive** — every SaaS reimplements "who changed what when." Slot: CC-09 (audit log).
- **A5-08 No row-level encryption decorator** — sensitive PII columns (SSN, address) have no `@encrypted` declaration. Slot: decorator on `@table` field; integrates with `vox-crypto` and vox-secrets-resolved keys.
- **A5-09 No data-residency primitive** — SaaS sold to EU/US customers needs per-tenant region pinning; not expressible. Slot: `@region(allowed: [eu, us])` table-level decorator. Long-tail.
- **A5-10 No impersonation / "view as user" primitive** — support staff debugging require it; reimplemented every time. Slot: `@impersonable` decorator gating session swap; logs to CC-09.

**Cross-cutting links:** CC-05, CC-06, CC-07, CC-08, CC-09, CC-10, CC-03.

### A6. Marketplace (listings + payments + ratings + search)

**Status:** Tier 2.
**What works today:** Listings as `@table`, basic filtering, components for listing pages.
**Blockers:**
- **A6-01 No file-upload primitive for listing photos** — multipart/form-data has no decorator or stdlib path. Slot: CC-01 (file upload + blob storage).
- **A6-02 No Stripe Connect / multi-party payment primitive** — marketplace payouts and platform fee splits are unsupported. Slot: extension to CC-10 (payments) for split tenders.
- **A6-03 No full-text search backend** — buyers can't search listings. `db.filter()` does only exact / prefix matches. Slot: CC-14 (full-text search).
- **A6-04 No rating-aggregate type** — every marketplace re-derives "weighted average rating" with stale-cache pitfalls. Slot: stdlib `std.aggregate.rolling_mean` + materialized view decorator.
- **A6-05 No escrow / hold-funds primitive** — payment intents that release on delivery confirmation are app-coded. Slot: extension to CC-10; long-tail.
- **A6-06 No moderation queue primitive** — flagged listings, user reports, moderator decisions are all bespoke. Slot: stdlib pattern for moderation; couples to CC-09 (audit log).
- **A6-07 No notification fan-out** — "your listing has a new offer" is per-app email/SMS/push code. Slot: CC-03.
- **A6-08 No image content-moderation hook** — uploads have no scan/classify gate. Slot: `@content_scan(provider: ...)` decorator on a file-upload field.
- **A6-09 No geographic search primitive** — distance / radius search has no operator. Slot: `db.filter(within: Point(lat, lon, radius_km))`; PostGIS or sqlite-spatial backend.
- **A6-10 No category taxonomy primitive** — hierarchical category trees are reimplemented. Slot: `@hierarchical` decorator on a `@table`; recursive query helpers in stdlib.

**Cross-cutting links:** CC-01, CC-10, CC-14, CC-03, CC-09.

### A7. Form-builder / survey tool

**Status:** Tier 2.
**What works today:** Static forms expressible via component view blocks.
**Blockers:**
- **A7-01 No dynamic schema** — form fields known only at runtime (user-defined surveys) cannot be typed. Tension with P0; needs a typed `DynSchema` value type with safe access. Slot: language-level addition; couples to a `Json[T]` upgrade path.
- **A7-02 No conditional-logic engine** — "show field B if field A == 'yes'" reimplemented each time. Slot: stdlib `std.form.conditional_visibility(rules: ...)`.
- **A7-03 No file-upload field type** — see CC-01.
- **A7-04 No partial-submit / resume primitive** — long surveys cannot persist mid-completion. Slot: `@autosaved` decorator on form state.
- **A7-05 No CSV export of responses** — see A3-03.
- **A7-06 No payment-gated submit** — form-builders that charge for downloads (form leadgen) blocked on CC-10.
- **A7-07 No drag-drop canvas component** — the form-builder itself (the editor UI) needs a generic DnD primitive. Slot: stdlib component; long-tail.
- **A7-08 No anti-spam primitive** — turnstile / hCaptcha / honeypot have no decorator. Slot: `@bot_check(provider: ...)` on `@endpoint(kind: mutation)`.

**Cross-cutting links:** CC-01, CC-03, CC-10.

### A8. Knowledge base / RAG-driven Q&A

**Status:** Tier 2.
**What works today:** `vox-actor-runtime` has `retrieval.rs` per the substrate audit, but it's not surfaced to user code. MENS embeddings exist in pipeline form.
**Blockers:**
- **A8-01 No vector type / vector-search operator** — `db.filter(by: similarity(...))` doesn't exist. Slot: CC-16 (vector search).
- **A8-02 No `@embed(model: ...)` decorator** — embedding generation on insert is hand-coded. Slot: decorator on a `@table` field, runs at insert/update.
- **A8-03 No chunking stdlib** — every RAG app reimplements text splitting. Slot: stdlib `std.text.chunk(by: ..., overlap: ...)`.
- **A8-04 No reranker primitive** — multi-stage retrieval has no abstraction. Slot: `std.retrieval.rerank(provider: ...)` stdlib.
- **A8-05 No streaming generation in endpoints** — answer-streaming requires SSE. Slot: CC-02.
- **A8-06 No citations type** — `Result[Answer, Error]` does not have a structural slot for "the chunks this came from." Slot: stdlib `Citation[T]` wrapping the answer; serializer pins the format.
- **A8-07 No prompt-template primitive** — prompts hardcoded as strings. Slot: `@prompt_template` declaration with typed slots; couples with eval suites.
- **A8-08 No retrieval-eval harness** — there's no first-class way to evaluate retrieval quality (recall@k, MRR). Slot: extension to `vox-eval`.

**Cross-cutting links:** CC-02, CC-14, CC-16, CC-19.

### A9. Booking / scheduling app

**Status:** Tier 2.
**What works today:** Tables for slots, basic CRUD on bookings.
**Blockers:**
- **A9-01 No interval-tree / availability type** — overlap detection is hand-coded SQL. Off-by-one errors are systemic. Direct P0 candidate. Slot: stdlib `Availability` type with structural overlap operators.
- **A9-02 No timezone-correct datetime ergonomics** — wire format is RFC 3339 UTC ([`wire-format-v1-ssot.md`](wire-format-v1-ssot.md)) but the user-facing Vox `DateTime` doesn't surface zone-aware arithmetic. Slot: stdlib `Time::in_zone(tz)` operations.
- **A9-03 No iCal / .ics export** — bookings can't be added to user calendars. Slot: stdlib `std.calendar.ics(events: ...)` emitter.
- **A9-04 No reminder-scheduling primitive** — "remind me 24h before" needs CC-17 (cron) and CC-03 (email/SMS).
- **A9-05 No "pay to confirm" primitive** — coupling slot reservation to CC-10. Slot: stdlib transaction primitive that holds the slot pending payment, releases on timeout.
- **A9-06 No double-booking transaction shape** — `@table` lacks declarative locking semantics. Slot: extension to `@table` with `@unique_in_range(field, range)` constraint.
- **A9-07 No staff-availability composition** — multi-resource scheduling (room + staff) has no compositional shape. Slot: stdlib `Schedule.intersect(...)` operator on availabilities.

**Cross-cutting links:** CC-03, CC-10, CC-17.

### A10. Webhook hub / integration receiver

**Status:** Tier 2.
**What works today:** `@endpoint(kind: mutation)` accepts JSON; can write to tables.
**Blockers:**
- **A10-01 No `@webhook` decorator** — signature verification (HMAC, Stripe / GitHub / Slack styles) is hand-rolled per integration. Direct P0 candidate; signature mistakes cause auth bypasses. Slot: CC-04 (webhook sign/verify).
- **A10-02 No replay-attack window** — receivers must check timestamp drift; no built-in. Slot: extension to CC-04.
- **A10-03 No idempotency key support** — see A4-06.
- **A10-04 No retry-with-backoff queue** — webhook handlers that fan out to other systems need durable retries. Slot: CC-18 (durable jobs).
- **A10-05 No payload-validation cliff** — JSON Schema validation of incoming webhooks isn't structural. Slot: `@webhook(schema: ...)` ties to the Phase 2 JSON Schema emit.
- **A10-06 No dead-letter queue introspection** — failed webhooks need a UI surface. Slot: dashboard panel on top of CC-18 + CC-09.
- **A10-07 No webhook transform-and-forward primitive** — common integration-hub use case. Slot: stdlib `std.webhook.forward(map: ..., target: ...)`.

**Cross-cutting links:** CC-04, CC-18, CC-09.

### A11. Notification / alerting system

**Status:** Tier 2.
**What works today:** Endpoints that read tables and decide what to send.
**Blockers:**
- **A11-01 No email send primitive** — see CC-03.
- **A11-02 No SMS send primitive** — see CC-03.
- **A11-03 No push-notification primitive** — Web Push, FCM, APNs have no abstraction. Slot: extension to CC-03.
- **A11-04 No template engine** — message bodies are string concatenation. Slot: stdlib `std.template` with typed slots; renders MJML for email.
- **A11-05 No delivery-status table** — bounces, opens, clicks aren't structurally captured. Slot: stdlib `@table DeliveryEvent { ... }` ships with CC-03.
- **A11-06 No throttling per-recipient** — "no more than 1 email per user per hour" is hand-coded. Slot: extension to CC-13 (rate limit) keyed on recipient.
- **A11-07 No escalation rules** — "if not acknowledged in 15m, page on-call" reimplemented. Slot: stdlib state-machine pattern; couples to CC-17 (cron) and `actor`/`workflow` keywords.
- **A11-08 No subscription-management surface** — unsubscribe handling is per-app. Slot: stdlib + auto-emitted `/unsubscribe?token=...` route.

**Cross-cutting links:** CC-03, CC-13, CC-17, CC-18.

### A12. Time-tracking / billing tool

**Status:** Tier 2.
**What works today:** Tables for time entries, basic mutations.
**Blockers:**
- **A12-01 No timer / duration primitive** — start/stop timers are hand-coded. Slot: stdlib `Timer` value type with `start`, `stop`, `pause` operators; couples to CC-18 for durability.
- **A12-02 No invoice generation** — PDF emit is unsupported. Slot: stdlib `std.pdf.invoice(items: ...)`; long-tail, can use Typst or wkhtmltopdf.
- **A12-03 No payment collection** — see CC-10.
- **A12-04 No recurring-invoice primitive** — needs CC-17 (cron) plus CC-10 (subscriptions).
- **A12-05 No tax-rate type** — sales tax / VAT / regional rates are hand-coded. Slot: stdlib `Tax` value type backed by an external rates table; updates via Phase 2 schedule.
- **A12-06 No CSV / QuickBooks export** — see A3-03.
- **A12-07 No team-rollup queries** — "billable hours by team this week" is N+1 SQL. Slot: stdlib `db.aggregate(by: ..., over: ...)` window function ergonomics.

**Cross-cutting links:** CC-10, CC-17, CC-18.

## §1.3 Tier 3 — Blocked on real-time + media spine

### A13. Real-time chat / messaging

**Status:** Tier 3.
**What works today:** Message tables, components rendering them via polling. No live updates.
**Blockers:**
- **A13-01 No WebSocket server primitive** — see CC-00 (WebSocket). Single most blocking gap in this tier.
- **A13-02 No presence primitive** — "user is online" / "user is typing" is a real-time state shape with no abstraction. Slot: stdlib `Presence[UserId]` type, backed by CC-00.
- **A13-03 No message-history pagination** — infinite-scroll with key-based cursors is hand-coded. Slot: stdlib `cursor_paginated[T]` view on top of `@table`.
- **A13-04 No reactive table subscription** — `subscription.rs` exists in `vox-actor-runtime` but isn't surfaced to user code per substrate audit. Slot: surface the existing internal primitive with a `db.Table.subscribe(filter: ...)` API.
- **A13-05 No read-receipt / unread-count primitive** — every chat reimplements the ledger. Slot: stdlib pattern; couples to CC-09.
- **A13-06 No file-attachment in messages** — see CC-01.
- **A13-07 No image-thumbnail generation** — see A2-04.
- **A13-08 No moderation hooks on send** — keyword filters / classifier-on-send have no abstraction. Slot: `@on_send_classify(provider: ...)` decorator on a message-send endpoint.
- **A13-09 No reconnection / message-replay protocol** — clients don't know what they missed during a network blip. Slot: extension to CC-00; sequence-number based replay.

**Cross-cutting links:** CC-00, CC-01, CC-09.

### A14. Project management (Linear/Asana-style)

**Status:** Tier 3.
**What works today:** Issues / tasks as `@table`, basic views, comments.
**Blockers:**
- **A14-01 No real-time updates** — see CC-00.
- **A14-02 No drag-drop kanban primitive** — common UI shape, no stdlib component. Slot: stdlib component; couples to CC-00 for live ordering.
- **A14-03 No comment-thread primitive** — every PM tool re-derives nested comments + mentions + edits. Slot: stdlib `Thread` aggregate with structural mention extraction.
- **A14-04 No mention notification fan-out** — `@mentions` triggering notifications is per-app. Slot: stdlib + CC-03.
- **A14-05 No file-attachment** — see CC-01.
- **A14-06 No saved-filter / saved-view** — see A3-06.
- **A14-07 No webhook-out primitive** — "post to Slack when issue closed" is per-app. Slot: stdlib outbound webhook; couples to CC-04 (signing) for verifiable delivery.
- **A14-08 No GitHub / GitLab integration adapter** — `vox-forge` exists but isn't surfaced as a stdlib bridge for app-level use. Slot: expose forge as `std.forge.{github, gitlab}` adapters.
- **A14-09 No bulk-edit primitive** — "select 50 issues, change status" is per-app. Slot: `@bulk_mutation` decorator that batches under a single transaction.

**Cross-cutting links:** CC-00, CC-01, CC-03, CC-04, CC-09, CC-12.

### A15. Analytics dashboard (charts, drill-downs, exports)

**Status:** Tier 3.
**What works today:** Read-only tables, basic filtering.
**Blockers:**
- **A15-01 No charting primitives** — see A3-01.
- **A15-02 No time-series query ergonomics** — `db.bucket(by: 1h, aggregate: count)` doesn't exist. Slot: CC-11 (time-series).
- **A15-03 No materialized-view primitive** — analytics that pre-aggregate are user-coded with cron + tables. Slot: `@materialized_view(refresh: ...)` decorator.
- **A15-04 No cohort-analysis primitive** — "users who signed up in week N, retention at week N+k" is reimplemented. Slot: stdlib `std.analytics.cohort(...)`.
- **A15-05 No funnel primitive** — multi-step conversion measurement reimplemented. Slot: stdlib `std.analytics.funnel(steps: ...)`.
- **A15-06 No CSV / XLSX export** — see A3-03.
- **A15-07 No scheduled-email reports** — needs CC-17 (cron) + CC-03 (email) + a templated render.
- **A15-08 No drill-down state** — see A3-05.
- **A15-09 No row-count guard on UI tables** — admin pages OOM the browser when a query returns 1M rows; no structural cap. Slot: `<DataTable max_rows=1000>` enforcement.

**Cross-cutting links:** CC-11, CC-17, CC-03, CC-19.

### A16. Document editor / wiki (rich text, attachments)

**Status:** Tier 3.
**What works today:** Markdown stored as text in tables; rendered via the (still-missing) markdown component.
**Blockers:**
- **A16-01 No rich-text type** — `RichText` as a structural value (not a string) doesn't exist. Slot: CC-12 (rich text).
- **A16-02 No block-based document model** — Notion-style block trees are bespoke. Slot: stdlib `BlockTree` with structural validity.
- **A16-03 No collaborative editing** — see CC-20 (CRDT/presence).
- **A16-04 No attachment storage** — see CC-01.
- **A16-05 No revision history** — every wiki rolls its own diff/version table. Slot: `@versioned` decorator on a `@table` field.
- **A16-06 No internal-link / backlink primitive** — `[[wiki link]]` and "what links here" are hand-coded. Slot: stdlib `std.wiki.link_graph(...)`.
- **A16-07 No mention / @user resolution** — see A14-04.
- **A16-08 No table-of-contents auto-derivation** — outline from headings is per-app. Slot: stdlib `std.markdown.toc(content: ...)`.
- **A16-09 No image-paste / drag-upload component** — basic UX, missing. Slot: stdlib component on top of CC-01.

**Cross-cutting links:** CC-01, CC-12, CC-20, CC-19.

### A17. AI chatbot UI with streaming + tool calls

**Status:** Tier 3.
**What works today:** `@mcp.tool` exposes tools to agents per [`mcp_tools.vox`](../../../examples/golden/mcp_tools.vox); `vox-orchestrator` routes to providers.
**Blockers:**
- **A17-01 No SSE / streaming response from `@endpoint`** — see CC-02.
- **A17-02 No `Conversation` value type** — message threads with role/tool-call/tool-result variants are bespoke. Slot: stdlib `Conversation` ADT mirroring the [model orchestration audit](model-orchestration-ssot-audit-2026.md) wire.
- **A17-03 No tool-call schema translation** — `nextgen-orchestrator-research-2026.md` calls out silent failures when tool-call payloads cross provider schemas (Anthropic vs OpenAI). Slot: shared in CC-21 (provider routing).
- **A17-04 No prompt-template primitive** — see A8-07.
- **A17-05 No token-budget primitive at the app level** — apps can't structurally cap conversation length to model context. Slot: `@token_budget(max: ...)` on a `Conversation` value.
- **A17-06 No streaming-aware UI primitive** — partial-response rendering with backpressure is bespoke per app. Slot: stdlib `<StreamingMessage stream={...}>`.
- **A17-07 No human-in-the-loop hooks** — "pause for user approval before tool call" reimplemented. Slot: stdlib `await user_confirm(prompt: ...)` operator.
- **A17-08 No conversation persistence pattern** — rebuilding context on reload is per-app. Slot: `@persisted_conversation(scope: ...)`.
- **A17-09 No safety filter chain** — input/output classifier hooks aren't structural. Slot: stdlib `std.safety.{input,output}_filter(provider: ...)`.

**Cross-cutting links:** CC-02, CC-16, CC-21.

## §1.4 Tier 4 — Substantially out of reach

### A18. Real-time collab editor (Notion / Figma-style)

**Status:** Tier 4 — needs CRDT + presence + low-latency transport.
**What works today:** Nothing reasonable. Editor UI partially expressible; the synchronization layer is absent.
**Blockers:**
- **A18-01 No CRDT primitive** — no `Yjs` / `Automerge` -equivalent value type. Slot: CC-20 (CRDT).
- **A18-02 No multi-cursor presence** — see A13-02 + CC-20.
- **A18-03 No offline-edit + sync-on-reconnect** — needs CRDT + service worker. Slot: CC-20 + CC-22 (PWA).
- **A18-04 No granular permission per block** — "this paragraph is read-only" needs row-level RBAC. Slot: extension to CC-08.
- **A18-05 No conflict-resolution UX primitive** — when CRDT can't auto-merge, the UI surface for resolution is bespoke. Slot: stdlib component pattern; long-tail.
- **A18-06 No undo / redo stack as a structural value** — every editor reimplements a history stack. Slot: stdlib `History[T]` operators.
- **A18-07 No comment-on-selection primitive** — anchored comments survive document edits via OT/CRDT positions. Slot: extension to CC-20.
- **A18-08 No co-edit cursor smoothing transport** — high-frequency ephemeral state needs a different channel than CRDT. Slot: CC-00 ephemeral channel.

**Cross-cutting links:** CC-00, CC-20, CC-08, CC-22.

### A19. Social network / feed-driven forum

**Status:** Tier 4.
**What works today:** Posts + comments tables; basic listing.
**Blockers:**
- **A19-01 No timeline / fanout primitive** — feed assembly (push vs pull, hybrid) is the core problem of this archetype. Slot: stdlib `std.feed.timeline(strategy: ...)`; long-tail.
- **A19-02 No follow-graph primitive** — "who follows whom" with efficient queries is bespoke. Slot: stdlib `Graph[NodeId]` table type with `followers/following/mutual` operators.
- **A19-03 No moderation queue / report primitive** — see A6-06.
- **A19-04 No content classification on post** — see A6-08.
- **A19-05 No notification fan-out** — see A11.
- **A19-06 No real-time updates** — see CC-00.
- **A19-07 No image / video upload + thumbnails** — see CC-01 + A2-04.
- **A19-08 No anti-spam / abuse-detection** — see A7-08.
- **A19-09 No federation primitive** — ActivityPub interop is a stretch goal; would need wire-format extensions. Slot: long-tail; new initiative.

**Cross-cutting links:** CC-00, CC-01, CC-03, CC-09, CC-14.

### A20. E-commerce storefront

**Status:** Tier 4.
**What works today:** Catalog as `@table`. Cart / checkout / fulfillment all bespoke.
**Blockers:**
- **A20-01 No payments primitive** — see CC-10.
- **A20-02 No inventory transaction primitive** — atomic decrement under contention is hand-coded SQL. Slot: stdlib `db.atomic_decrement(field, amount, min: 0)` with clear failure shape.
- **A20-03 No cart persistence** — anonymous carts surviving login are reimplemented. Slot: stdlib `Cart` aggregate with anonymous→user merge operator.
- **A20-04 No shipping-rate calculator** — Shippo / EasyPost integrations are per-app. Slot: stdlib adapter; long-tail.
- **A20-05 No tax calculator** — see A12-05.
- **A20-06 No fulfillment-status workflow** — "order placed → packed → shipped → delivered" is `actor`/`workflow` shaped but no stdlib pattern. Slot: pattern doc + example, no language addition needed.
- **A20-07 No image gallery / zoom component** — see A6-01.
- **A20-08 No SKU search + faceted filtering** — see CC-14.
- **A20-09 No abandoned-cart email** — needs CC-17 (cron) + CC-03 (email) + a templated render.
- **A20-10 No fraud-check hook** — payment fraud screening before charge is per-app. Slot: extension to CC-10.

**Cross-cutting links:** CC-01, CC-03, CC-10, CC-14, CC-17.

### A21. Mobile-first PWA (offline + push + installability)

**Status:** Tier 4.
**What works today:** Components emit React; mobile bridges exist for camera/vibrate per [`mobile_camera.vox`](../../../examples/golden/mobile_camera.vox).
**Blockers:**
- **A21-01 No service-worker emit** — Vox doesn't generate a SW; the app is online-only. Slot: CC-22 (service worker / offline).
- **A21-02 No app-manifest emit** — `manifest.webmanifest` not derived from app metadata. Slot: extension to CC-22.
- **A21-03 No web-push primitive** — see A11-03.
- **A21-04 No background-sync primitive** — failed mutations replayed when online has no abstraction. Slot: extension to CC-22.
- **A21-05 No install-prompt component** — `beforeinstallprompt` event is hand-wired. Slot: stdlib component.
- **A21-06 No offline-first table subscription** — needs CC-20 partial (offline CRDT) or a simpler "stale data ok" mode. Slot: extension to A13-04.
- **A21-07 No share-target primitive** — Web Share Target API not derived from `routes`. Slot: `@share_target(...)` decorator on a route.
- **A21-08 No native-feel transition spec** — see A2-07.

**Cross-cutting links:** CC-00 (optional), CC-20 (optional), CC-22.

---

# §2 — Cross-cutting infrastructure spine

The 21 archetypes above bottleneck on a small set of shared primitives. Building these *first* unblocks 4–10 archetypes per CC item — that's where the leverage is. Each CC item is split into four facets: **design**, **runtime**, **codegen**, **eval**. Some CC items collapse facets (e.g. when no codegen is needed); most have all four.

### CC-00. WebSocket server + bidirectional channels

**Unblocks:** A13, A14, A18, A19, plus partial A15, A21.
**Why now:** Real-time is the biggest single feature gap in archetype coverage; chat alone is one of the top-five use cases for new web projects.
- **CC-00-D Design** — pin one WS shape: typed channels with structurally validated message envelopes (`Channel[Send, Recv]`). Per C4, do not also expose raw bytes. Couples to wire-format SSOT — message envelopes get a `_kind` discriminator. Slot: new SSOT addendum, not a phase yet.
- **CC-00-R Runtime** — Axum WS extractor wired to a `Channel` registry; backpressure via bounded mpsc; reconnection sequence numbers in a stdlib `ChannelMeta` block.
- **CC-00-C Codegen** — emit a typed TS client subscriber from the same envelope schema. Reuse the Phase 2 OpenAPI emitter's type pipeline; add an AsyncAPI emit alongside.
- **CC-00-E Eval** — golden harness that runs both peers, kills the connection mid-stream, asserts replay correctness. Couples to `vox-eval`.

### CC-01. File upload + blob storage

**Unblocks:** A6, A7, A11, A13, A14, A16, A19, A20.
- **CC-01-D Design** — typed `Upload[T]` value (not raw multipart) with content-type and size structurally bounded at the type level. Storage backends abstracted via a `BlobStore` trait surface in `vox-actor-runtime`.
- **CC-01-R Runtime** — multipart handler in Axum integration; backends for local disk + S3-compatible (R2, B2). Streaming uploads (no full-buffer).
- **CC-01-C Codegen** — TS client emits a typed `upload(file: File)` call; OpenAPI emit handles `multipart/form-data` correctly.
- **CC-01-E Eval** — fixture fuzzer for filename / mime-type spoofing; large-file streaming pass; partial-upload resume. Couples to security fuzz.

### CC-02. SSE / streaming responses

**Unblocks:** A4 partially, A8, A17.
- **CC-02-D Design** — endpoints return `Stream[T]` instead of `T`; serializer chooses NDJSON or text/event-stream from `Accept`. Wire-format SSOT addendum needed (per A4-04).
- **CC-02-R Runtime** — Axum SSE response shaping; backpressure via `Stream::poll_next`; client-disconnect detection.
- **CC-02-C Codegen** — TS client returns an `AsyncIterable<T>`; Phase 2 OpenAPI emit handles `text/event-stream` content type.
- **CC-02-E Eval** — slow-client test; abort-mid-stream test; reconnection-with-Last-Event-ID test.

### CC-03. Email / SMS / push delivery

**Unblocks:** A5, A6, A9, A11, A12, A14, A19, A20, A21.
- **CC-03-D Design** — `Notify { channel: Email | SMS | Push, template: TemplateId, recipient: ... }`. Single typed shape; recipient type structurally constrains channel (no SMS to email address). One adapter trait, multiple provider impls.
- **CC-03-R Runtime** — adapters: SES / Resend / Postmark for email; Twilio for SMS; Web Push + FCM for push. Delivery events stored to a stdlib `DeliveryEvent` table for status tracking.
- **CC-03-C Codegen** — none required directly; bounces / opens generate webhook routes via CC-04.
- **CC-03-E Eval** — golden tests for template rendering; delivery-status reconciliation.

### CC-04. Webhook signing / verification

**Unblocks:** A4 (idempotency relevance), A10, A14, A19, A20.
- **CC-04-D Design** — `@webhook(provider: Stripe | GitHub | Slack | Custom { algo, header, secret })` decorator on `@endpoint`. Verification happens before the body is parsed; signature mismatch returns 401 structurally, never hits user code.
- **CC-04-R Runtime** — HMAC-SHA256 verifier; replay-window enforcement (timestamp + nonce table); body raw-byte preservation across the parser boundary.
- **CC-04-C Codegen** — outbound: helper to *send* signed webhooks for our own integrations.
- **CC-04-E Eval** — tampered-signature rejection; replay rejection; clock-skew tolerance.

### CC-05. OAuth / OIDC primitives

**Unblocks:** A5, A6, A14, A17 partial, A19, A20.
- **CC-05-D Design** — `@auth(provider: oauth(client_id, scopes), redirect: ...)` decorator on a route. Provider catalogue: Google / GitHub / Microsoft / generic OIDC. Token storage via vox-secrets. Per-tenant configuration via CC-07.
- **CC-05-R Runtime** — full Authorization Code + PKCE flow; token refresh; userinfo fetch. Session minting integrates with CC-06.
- **CC-05-C Codegen** — TS client gets a `signIn(provider)` helper; redirect URLs emit into the routes manifest.
- **CC-05-E Eval** — golden state-token validation; CSRF rejection; replay rejection; provider-error surfacing.

### CC-06. JWT + session stdlib

**Unblocks:** A5, A6, A14, A19, A20, A21.
- **CC-06-D Design** — `Session` is a structural type, not a row. `@require_session(role: ...)` decorator on `@endpoint`. JWT is one possible representation, opaque session id is another; the decorator picks; the user code reads `Session` regardless.
- **CC-06-R Runtime** — JWT signer/verifier (vox-crypto-backed), opaque-token store, automatic rotation, revocation table.
- **CC-06-C Codegen** — TS client carries the session token transparently; refresh handled in the client class.
- **CC-06-E Eval** — token-tampering rejection; expired-token rejection; revoked-token rejection; rotation-during-request safety.

### CC-07. Multi-tenancy as a primitive

**Unblocks:** A5, A6, A12, parts of A4 / A20.
- **CC-07-D Design** — `@table(scoped_by: tenant)` decorator. Every `db.*` operation against the table requires a `Tenant` value in scope; missing tenant is a compile error, not a runtime check. P0 candidate. The hardest sub-decision is what "scope" looks like in source — implicit context vs explicit param. Recommend: explicit `Tenant` param at every callsite, no implicit context (P3 locality).
- **CC-07-R Runtime** — tenant id injected into the `WHERE` clause at codegen time; row-level security as a backend backstop.
- **CC-07-C Codegen** — emit per-table policies for Postgres RLS; SQLite layer enforces in the query builder.
- **CC-07-E Eval** — fuzz queries that "forget" the tenant; assert all of them fail to compile, not just at runtime.

### CC-08. RBAC / authorization model

**Unblocks:** A3, A5, A14, A17, A18, A19.
- **CC-08-D Design** — capability-based, not role-based. `@require(can: edit_post(post_id))` decorator on `@endpoint`. Capability functions are typed `fn(Session, Resource) -> Bool`. Composable via `and` / `or`. Avoid the role-list anti-pattern (forces a decision for every endpoint).
- **CC-08-R Runtime** — capability evaluation memoized per request; audit trail integration with CC-09.
- **CC-08-C Codegen** — TS client returns 403 Forbidden with a structured error indicating the missing capability; UI can hide affordances based on the capability check, run client-side as a hint, server-side as authority.
- **CC-08-E Eval** — exhaustive endpoint × role matrix; deny-by-default test.

### CC-09. Audit log primitive

**Unblocks:** A5, A6, A10, A11, A14, A18, A19, A20.
- **CC-09-D Design** — `@audited` decorator on `@table` automatically captures who/what/when/old/new. Audit records are append-only and structurally tamper-evident (Merkle chain optional, sign-on-write recommended).
- **CC-09-R Runtime** — interceptor in the db layer; configurable storage backend; retention policy.
- **CC-09-C Codegen** — admin dashboard component for reading audit history; per-table audit view auto-generated.
- **CC-09-E Eval** — golden tests for tamper-detection; retention-policy adherence.

### CC-10. Payments stdlib (Stripe-first)

**Unblocks:** A5, A6, A7, A9, A12, A20.
- **CC-10-D Design** — `Payment` is a typed value with discriminated states (`Pending | Succeeded | Failed | Refunded`). One stdlib per provider, sharing a `PaymentProvider` trait surface. Stripe first because it covers the bulk of TAM. Webhook signing via CC-04 is a precondition.
- **CC-10-R Runtime** — Stripe Checkout + PaymentIntent flows; subscription / metered billing; refunds; disputes.
- **CC-10-C Codegen** — `@endpoint(payment: ...)` emits a checkout-session URL; TS client gets a `redirectToCheckout(...)` helper.
- **CC-10-E Eval** — golden replay of Stripe webhook fixtures; idempotency verification; partial-failure recovery.

### CC-11. Time-series / analytics queries

**Unblocks:** A3, A5, A12, A14, A15, A19, A20.
- **CC-11-D Design** — `db.bucket(by: 1h, aggregate: count)` operator that compiles to native time-bucketing on the backend (Postgres `date_trunc`, SQLite group-by). Window functions surfaced as `db.window(over: ..., order_by: ..., range: ...)`. Per C4, a single canonical shape — not parallel "use raw SQL" escape hatch unless `// vox:skip`-ed.
- **CC-11-R Runtime** — query builder extensions; result set is a typed `TimeSeries[T]` with a `.fill_gaps(zero: ...)` operator.
- **CC-11-C Codegen** — TS client sees `TimeSeries[T]` as `{ buckets: [{t, v}] }`; charting components consume directly.
- **CC-11-E Eval** — golden tests for bucket boundaries; DST transitions; sparse data handling.

### CC-12. Rich text type

**Unblocks:** A6 partial, A14, A16, A19.
- **CC-12-D Design** — `RichText` is a structural type, not a string blob. The on-disk shape is a typed block tree (à la Lexical / Slate). Sanitization / escaping is automatic; no XSS path exists. P0 candidate.
- **CC-12-R Runtime** — block-tree walk + sanitizer; markdown / HTML import paths produce the same internal shape.
- **CC-12-C Codegen** — server-side render to safe HTML; client-side editor component (ships in stdlib, integrates with TipTap or similar but invisibly).
- **CC-12-E Eval** — XSS fuzz; round-trip preservation across markdown/HTML/internal forms.

### CC-13. Rate limiting / quotas

**Unblocks:** A4, A6, A11, A17, A19, A20.
- **CC-13-D Design** — `@rate_limit(by: ip | user | tenant, per: 1m, max: 100)` decorator on `@endpoint`. Storage backends pluggable (in-memory, Redis); single canonical decorator shape. Dovetails with Phase 3 HTTP ergonomics.
- **CC-13-R Runtime** — token-bucket per key; backoff hint in 429 response; integration with CC-09.
- **CC-13-C Codegen** — TS client surfaces rate-limit headers (`Retry-After`) automatically.
- **CC-13-E Eval** — burst behavior; recovery; per-key isolation.

### CC-14. Full-text search

**Unblocks:** A6, A14, A16, A19, A20.
- **CC-14-D Design** — `@searchable(by: [title, body])` decorator on `@table`. `db.search(query: str, on: Table)` operator. Single canonical shape; the backend (Postgres tsvector, SQLite FTS5, external like Meilisearch) is config, not user code.
- **CC-14-R Runtime** — index maintenance on insert/update; tokenizer config per language; ranking surfaced via a typed `SearchResult[T] { item: T, rank: Decimal, highlights: ... }`.
- **CC-14-C Codegen** — TS client search method; admin UI for index health.
- **CC-14-E Eval** — relevance regression tests via a known corpus; tokenizer correctness; index-rebuild safety.

### CC-15. Idempotency keys + transaction shape

**Unblocks:** A4, A6, A10, A20.
- **CC-15-D Design** — `@idempotent(window: 24h)` decorator on `@endpoint(kind: mutation)`. Key extracted from `Idempotency-Key` header. Replay returns cached response; conflict (different body, same key) returns structured error.
- **CC-15-R Runtime** — key/response store with TTL; collision detection.
- **CC-15-C Codegen** — TS client auto-attaches a UUID by default; opt-out via flag.
- **CC-15-E Eval** — golden replay; collision detection; concurrent-request safety.

### CC-16. Vector search / embeddings

**Unblocks:** A8, A17 partial.
- **CC-16-D Design** — `Vector[N]` value type (statically dimensioned). `@embed(model: ...)` decorator on a `@table` field. `db.search(by: similarity(query_vec, top_k: ...))`. Single canonical shape; backend is pgvector / sqlite-vss / in-memory.
- **CC-16-R Runtime** — embedding generation hooks (provider via `vox-ml-cli` for local, or remote provider). Index types (HNSW / IVF) surfaced as decorator config.
- **CC-16-C Codegen** — none distinct from CC-14.
- **CC-16-E Eval** — recall@k harness; embedding-drift detection across model upgrades.

### CC-17. Cron / scheduled jobs runtime

**Unblocks:** A3, A9, A10, A11, A12, A15, A20.
- **CC-17-D Design** — finish the `@scheduled` decorator that currently parses but doesn't run. Per [`v1-release-criteria.md`](v1-release-criteria.md), there's an open ADR. Distributed deployments need leader election; spec it now.
- **CC-17-R Runtime** — single-node tokio scheduler for v1; cluster-aware via Populi mesh in v1.5. Missed-run policy is structural (run-now vs skip vs catch-up), not a flag.
- **CC-17-C Codegen** — none beyond a runtime registration.
- **CC-17-E Eval** — clock-skew tolerance; missed-run recovery; long-running-job preemption.

### CC-18. Durable jobs / queues with retry

**Unblocks:** A6, A10, A11, A12, A20.
- **CC-18-D Design** — `@durable` decorator on `fn` that records inputs, retries on failure with backoff, dead-letters after N attempts. Already pre-discussed in the durability research; this CC item is the impl spec. Couples to `actor` / `workflow` keywords already supported in HIR.
- **CC-18-R Runtime** — job table; worker pool; retry-with-jitter; dead-letter table surfaced via stdlib query.
- **CC-18-C Codegen** — none.
- **CC-18-E Eval** — at-least-once semantics; idempotency with CC-15; poison-pill handling.

### CC-19. Asset pipeline (images, fonts, static)

**Unblocks:** A2, A3, A6, A8, A13, A15, A16, A19.
- **CC-19-D Design** — `@asset(image)` decorator on a string referring to an image at compile-time. Build-time pipeline emits responsive variants (AVIF/WebP/JPEG) at declared widths. Single canonical shape; no per-app pipeline.
- **CC-19-R Runtime** — none at request time; outputs served as static.
- **CC-19-C Codegen** — `<picture>` markup with responsive `srcset`; LQIP placeholder emit.
- **CC-19-E Eval** — golden tests for output-format correctness; reproducibility (same input → byte-identical output) per Phase 1 reproducibility commitments.

### CC-20. CRDT / collaborative state

**Unblocks:** A14, A16, A18.
- **CC-20-D Design** — `@collaborative` decorator on a `RichText` or `BlockTree`-typed field. Backend uses Yjs over WebSocket (CC-00) or Automerge. Conflict-free merge is automatic; the user does not see two values to reconcile. Long-tail; do not ship until CC-00 + CC-12 are stable.
- **CC-20-R Runtime** — Yrs integration; persistence via snapshots + update log; presence channel for cursors.
- **CC-20-C Codegen** — TS client wires the editor binding automatically.
- **CC-20-E Eval** — concurrent-edit fuzzer; offline-edit + reconnect; delete-cursor races.

### CC-21. Provider-agnostic tool-call routing

**Unblocks:** A17, plus the orchestrator stack writ large.
**Why now:** [`nextgen-orchestrator-research-2026.md`](nextgen-orchestrator-research-2026.md) names this as a silent-failure source.
- **CC-21-D Design** — single canonical Vox-side `ToolCall` / `ToolResult` envelope, with adapters to OpenAI / Anthropic / Google formats. Per C4, do not allow user code to see provider-native shapes — they're an internal detail of the router.
- **CC-21-R Runtime** — schema translation in `vox-orchestrator`; failure shape: when a model emits an unsupported shape, the router converts when possible, returns a typed `ToolCallError` when not.
- **CC-21-C Codegen** — none.
- **CC-21-E Eval** — fuzz across providers; ensure no silent 400s.

### CC-22. Service worker / offline / PWA emit

**Unblocks:** A21, A18 (partial offline).
- **CC-22-D Design** — `@offline_capable` declaration on a `routes` block. Build emits a service worker with a strategy declared structurally (cache-first / network-first / stale-while-revalidate per route).
- **CC-22-R Runtime** — generated SW served at root; manifest.webmanifest derived from app metadata.
- **CC-22-C Codegen** — SW + manifest; integrates with Phase 1 / Phase 2 build pipeline.
- **CC-22-E Eval** — Lighthouse PWA score gate in golden builds.

### CC-23. Pre-built design system / token primitives

**Unblocks:** all archetypes — quality-of-output gain.
**Why now:** v0 / Tailwind / shadcn defaults are what users get from MENS today; without a Vox-native design system, the model's output drifts towards whichever framework was most represented in pretraining.
- **CC-23-D Design** — typed `Token { color, spacing, radius, shadow, font }` declarations at the project root. Components consume tokens by name, not by raw value. Per P0, a contrast violation between two token names is refused at compile time. `@light` / `@dark` variants are required pairs.
- **CC-23-R Runtime** — none; all compile-time.
- **CC-23-C Codegen** — emit CSS variables; emit a typed TS export for token names.
- **CC-23-E Eval** — contrast gate; spacing-scale consistency; cross-component token coverage.

### CC-24. i18n message catalog as types

**Unblocks:** A1 (e-commerce locale), A2 (multi-region marketing sites), A9 (payments with locale-aware formatting), A12 (enterprise SaaS multi-locale), A17 (AI assistants with locale-aware response shaping) — 5+ archetypes.
**Why now:** Every multi-locale Vox app hand-rolls ICU-message bridging today; the model re-derives the same extraction pattern each time. A typed `t"..."` template literal would remove the extraction decision entirely and make missing-translation a compile error for declared locales.
- **CC-24-D Design** — typed `t"key"` template literal referencing a project-root `locales/` catalog; plural arms checked structurally at compile time (`t"item_count" when 1 => "...", n => "..."`). Timezone-carrying date type flows through locale-aware format functions. Missing-translation for a declared locale is a `vox/i18n/missing-translation` error. Per C4, one canonical surface — no `i18n.t()` call form alongside `t"..."`.
- **CC-24-R Runtime** — locale bundle loaded at route boundary; no over-fetching. `vox-i18n` sub-crate or module in `vox-actor-runtime` for server-side locale resolution.
- **CC-24-C Codegen** — emit React-Intl-compatible message descriptors from `t"..."` uses; emit per-locale JSON bundles at build time; server-side formatter for SSR text nodes.
- **CC-24-E Eval** — golden with two locales; missing-translation test; plural-arm exhaustiveness test; timezone round-trip.

---

# §3 — MENS + orchestrator journey blockers

These are *meta* gaps — independent of any archetype, they degrade the prompt → repo journey for every archetype. Most are already named in existing audits; this section dedupes them and prioritizes by archetype-coverage impact.

### M1. Prompt → repo archetype templates beyond `vox init` kinds

**What's missing:** `vox init` ships ~5 kinds (chatbot, web, api, mobile-pwa, fullstack) per the substrate scan. None correspond directly to the 21 archetypes above. MENS has no archetype-aware scaffold prompt set.
**Why it blocks:** A user asking "build me a marketplace" gets a generic full-stack scaffold; MENS then has to invent the marketplace shape from scratch every time. High decision-point burden. Higher hallucination rate.
**Slot:** Build per-archetype prompt + scaffold template pairs, owned by `vox-project-scaffold` and consumed by `vox-ml-cli` as system-prompt context.

### M2. Feature-pack composition (no "add auth + payments to existing repo")

**What's missing:** Adding cross-cutting features incrementally (e.g. "add auth to my existing CRUD app") requires MENS to read the repo and patch every file consistently. There's no structured "feature pack" that knows about its own dependencies.
**Why it blocks:** Iteration loops on mature apps. Users can't grow a repo without rewriting it.
**Slot:** Feature-pack manifests under `.vox_modules/` describing pre-built dependencies + integration points; composable in series.

### M3. Codegen output not inspectable without rebuild

**What's missing:** No `vox emit --dry-run` and no dashboard panel showing emitted artifacts. Substrate scan: developers must rebuild to see generated `.ts` or `.rs`.
**Why it blocks:** Tight feedback loops impossible. Especially painful when MENS makes a subtle codegen mistake.
**Slot:** CLI subcommand + dashboard panel reading from a structured emit manifest.

### M4. Live preview while MENS is generating

**What's missing:** MENS generation is a black box from the user perspective. No streamed preview of partial files, no "MENS thinks it's 60% done."
**Why it blocks:** Users abort runs they would have let finish; users let runaway runs continue. Couples to the doom-loop detection gap.
**Slot:** Dashboard streaming view bound to MENS generation events.

### M5. Archetype-specific eval suites

**What's missing:** `vox-eval` runs language-level eval; there's no "does this generated chat app survive a network blip?" or "does this marketplace prevent double-spend?" suite per archetype.
**Why it blocks:** MENS quality gates are language-shape, not application-correctness. A chat app can pass typecheck and still be broken.
**Slot:** Per-archetype eval scripts; couples to fixture-data generator (A1-05).

### M6. Doom-loop detection (audit FIX-11) ✓

**What's missing:** Per [`nextgen-orchestrator-research-2026.md`](nextgen-orchestrator-research-2026.md) §4, the orchestrator does not monitor cost/progress ratio; runaway agents burn budget until the global cap.
**Why it blocks:** Direct user-visible cost incidents.
**Slot:** Already specced in audit; `vox-orchestrator` work, no language change.
**Status (2026-05-02):** Landed. `BudgetManager::doom_loop_cost_check` + `GateResult::DoomLoop` + pre-dispatch hook in `submit_task_with_agent`; `record_task_completion` resets the counter on `complete_task_with_attestation`. Default $2.00 threshold, runtime tunable via `set_doom_loop_cost_threshold`.

### M7. Pre-execution token estimation ✓

**What's missing:** Tasks dispatch, *then* fail budget checks. No proactive token estimation. Audit-named gap.
**Why it blocks:** Failed tasks cost real money and force rework.
**Slot:** Orchestrator-side estimator; couples to model registry token costs.
**Status (2026-05-02):** Landed. `BudgetManager::would_exceed_token_budget` + pre-dispatch estimation in `process_task_submission_logic` returns `OrchestratorError::BudgetExceeded` before dispatch. Conservative heuristic: `description.len()/4 + file_manifest.len()*200`.

### M8. Schema-aware multi-provider routing ✓ (partial)

**What's missing:** Tool-call schemas differ across providers (Anthropic vs OpenAI). Routing across providers can fail silently. Audit FIX in [`nextgen-orchestrator-research-2026.md`](nextgen-orchestrator-research-2026.md).
**Why it blocks:** Users hit "model X doesn't support tool calls correctly" without diagnostics.
**Slot:** CC-21 (above); the same item, surfaced at the user-journey level.
**Status (2026-05-02):** Silent-failure path closed. `HttpInferError.is_capability_gap` + `anthropic_tools_guard` + retry-on-gap in `infer_via_provider_adapter` route Anthropic-direct tool-call requests to the OpenAI-compat fallback adapter. Full bidirectional schema translation (tools support directly in `AnthropicRequest`) still deferred.

### M9. P0 security fixes (HTTP timeouts, origin guard, debug-print env vars) ✓

**What's missing:** Per [`orchestrator-companion-audit-findings-2026.md`](orchestrator-companion-audit-findings-2026.md): no explicit timeout on provider HTTP clients; origin-guard prefix-bypass (`127.0.0.1.attacker.com`); env var leak via `println!`.
**Why it blocks:** Users running dashboards in mixed-trust environments are vulnerable. Until these are fixed, the dashboard cannot be exposed beyond localhost.
**Slot:** Already specced as FIX-J-01 / FIX-K-01 / FIX-K-02; immediate.
**Status (2026-05-02):** Landed. Production clients carry 120s timeout; origin guard properly asserts host boundary; no debug env-var prints. Test-side HTTP client also given a 30s timeout (FIX-J-01 follow-up).

### M10. Nightly model discovery refresh ✓

**What's missing:** Per audit FIX-30, model discovery is one-shot at process start. New OpenRouter models / pricing changes are not picked up.
**Why it blocks:** Cost decisions stale. Users routed to deprecated models.
**Slot:** Orchestrator daemon job; couples to CC-17 once landed.
**Status (2026-05-02):** Already landed. `catalog_refresh.rs` runs a 6-hour `run_catalog_refresh_loop` background task.

### M11. Persistent dashboard state

**What's missing:** Dashboard is stateless. Refreshing loses tab, scroll, selected agent. Substrate scan.
**Why it blocks:** Multi-hour debugging sessions hit reload friction.
**Slot:** Local-storage state; small effort, high DevEx return.

### M12. LSP completion / hover / goto-def

**What's missing:** LSP only provides diagnostics per substrate scan; completion / hover / goto-def stubs only.
**Why it blocks:** Editor experience is materially worse than mainstream languages. Drives drift back to TS.
**Slot:** Standalone LSP work; couples to `vox-compiler` symbol tables (already exist in HIR).

### M13. MCP tool palette in dashboard

**What's missing:** 234+ MCP tools per substrate scan, no in-dashboard discovery surface; users browse YAML registry document.
**Why it blocks:** Tool capabilities are invisible. Users can't compose; agents can't show users what they did.
**Slot:** Dashboard component reading from the canonical registry.

### M14. Agent retrospective / replay surface

**What's missing:** When a task completes (or fails), there's no UI to replay decisions, costs, tool calls. `journey_id` exists internally per audit but isn't surfaced.
**Why it blocks:** Debugging "why did MENS produce this" is impossible.
**Slot:** Dashboard panel + structured event store query.

### M15. Cost-attribution per generated file

**What's missing:** No per-file cost telemetry — users can't see which parts of a generated repo were expensive.
**Why it blocks:** Users can't optimize their prompting toward cheap-to-generate shapes.
**Slot:** Orchestrator telemetry extension; couples to existing `vox.script.*` events.

### M16. Generated-repo provenance markers

**What's missing:** Generated files don't carry a stable "what-prompted-this" reference. When MENS regenerates, it can't surgically update the section that originally came from prompt P.
**Why it blocks:** Re-prompting on a sub-section of a repo. Forces full-file rewrites.
**Slot:** Source-file annotations + orchestrator-side mapping.

### M17. Seamless `vox forge` <-> dashboard integration

**What's missing:** `vox-forge` exists for GitHub/GitLab interactions. Not surfaced in the dashboard for app-level use (e.g. "PR-from-dashboard"). Substrate scan.
**Why it blocks:** Users drop to terminal for git ops. Loses momentum.
**Slot:** Dashboard panel + CC-08 to gate destructive ops.

---

# §4 — Recommended sequence (Pareto frontier)

The ordering optimizes for archetype-coverage gain per unit-of-design-surface. Items earlier unlock items later, in many cases. Each block names which archetypes / cross-cutting items advance.

### Block 1 (immediate, no language change required)

**Land first because they are pure runtime / orchestrator work, no decision-point cost, and unblock other blocks.**

1. **M9. P0 security fixes** — non-negotiable; gates everything that exposes the dashboard.
2. **M6. Doom-loop detection** — direct user-cost protection.
3. **M7. Pre-execution token estimation** — pairs with M6.
4. **M8 / CC-21. Schema-aware tool-call routing** — silent failure source today.
5. **M10. Nightly model discovery** — small, well-bounded, audit-named.

**Cumulative archetype impact:** Quality and reliability of *all* archetypes — these don't unlock new shapes, they make existing shapes shippable.

### Block 2 (Phase 2 of interop plan + adjacent quality)

6. **CC-15. Idempotency primitive** — small, generally useful.
7. **A4-05. Implicit `RequestContext`** — touches Phase 3 surface; foundational.
8. **CC-13. Rate limit decorator** — Phase 3 territory; leverages.
9. **CC-04. Webhook signing** — unblocks A10, A14, A19, A20.
10. **CC-19. Asset pipeline** — pure compile-time work, broad coverage.

**Cumulative archetype impact:** A4 fully ships; A2 / A10 jump from Tier 2 → Tier 1.

### Block 3 (auth and tenancy spine)

11. **CC-06. JWT + session stdlib** — Phase 4 of interop plan.
12. **CC-05. OAuth/OIDC primitives** — Phase 4.
13. **CC-08. RBAC capability model** — Phase 4.
14. **CC-07. Multi-tenancy primitive** — P0 candidate, structural.
15. **CC-09. Audit log primitive** — couples to CC-08.

**Cumulative archetype impact:** A5 fully ships. A6 partial. A11, A12 progress.

### Block 4 (real-time spine)

16. **CC-00. WebSocket** — biggest single unlock.
17. **A13-04. Reactive table subscription** — surface the existing `subscription.rs`.
18. **CC-02. SSE / streaming responses** — partial overlap with CC-00 transport.
19. **A17-02. `Conversation` value type** — small, leveraging.

**Cumulative archetype impact:** A13, A14, A17 jump tier. A19, A20, A21 advance.

### Block 5 (commerce + storage spine)

20. **CC-01. File upload + blob storage** — long-blocked, broad unlock.
21. **CC-10. Payments stdlib (Stripe-first)** — pairs with CC-04.
22. **CC-14. Full-text search** — couples to CC-19 patterns of canonical primitive.
23. **CC-17. Cron / scheduled jobs runtime** — finishes the half-built `@scheduled`.
24. **CC-18. Durable jobs / queues** — pairs with CC-17.

**Cumulative archetype impact:** A6, A11, A12, A20 ship. A7 ships. A10 fully ships.

### Block 6 (RAG / AI-native spine)

25. **CC-16. Vector search** — A8 ships.
26. **A8-07 / A17-04. Prompt-template primitive** — small, broadly useful.
27. **A17-09. Safety filter chain** — coupling to provider-side classifier.

**Cumulative archetype impact:** A8 fully ships. A17 fully ships.

### Block 7 (rich content + analytics spine)

28. **CC-11. Time-series query ergonomics** — couples to charting.
29. **A3-01 / A15-01. Charting primitives** — large component-lib effort.
30. **CC-12. Rich text type** — A16-01 prereq.
31. **A16-02. Block-based document model** — couples to CC-12.

**Cumulative archetype impact:** A15 ships. A16 substantially advances.

### Block 8 (polish / quality-of-output)

32. **CC-23. Design-system tokens** — quality lift across every archetype.
33. **CC-24. i18n message catalog as types** — missing-translation structurally unrepresentable; unblocks 5+ multi-locale archetypes.
34. **A1-04. Loading / empty state slots** — language-level addition.
35. **A2-05. SEO metadata as types** — language-level addition.
36. **M11–M17. Dashboard / DevEx polish.**

**Cumulative archetype impact:** Quality of output across all archetypes.

### Block 9 (long-tail, deferred)

36. **CC-20. CRDT / collaborative state** — A18.
37. **CC-22. Service worker / PWA emit** — A21.
38. **A19. Social network primitives (timeline, follow-graph, federation)** — narrowest payoff per unit work; defer.

**Cumulative archetype impact:** A18 ships. A21 ships. A19 partial.

---

# §5 — What this map *does not* recommend

For honesty, list the items considered and rejected — these protect future contributors from re-litigating settled decisions.

- **GraphQL schema emit.** Coverage gain is small; OpenAPI 3.1 + the typed RPC client cover the same ground with one fewer canonical shape. C4 violation to ship both.
- **gRPC / protobuf wire format.** Same C4 reasoning. The wire format SSOT pins JSON-discriminated; adding a parallel format is a parallel emit path with maintenance debt.
- **Multiplayer-game real-time state primitives.** Narrow audience; the latency / consistency tradeoffs deserve a different language than Vox.
- **Native video transcoding / DRM.** Better to integrate existing services (Mux, Cloudflare Stream) via CC-04 webhooks + CC-01 upload than build it. Maintenance debt would be enormous.
- **A general Node-FFI bridge.** Already non-goal in [the interop plan](external-frontend-interop-plan-2026.md). Phase 5 component interop covers the intersection that matters.
- **Per-app design-system pluggability beyond CC-23 tokens.** Multiple competing component libs add decision-points without semantic content.
- **Bare keywords for new behaviors.** Per [AGENTS.md §Grammar Unification](../../../AGENTS.md), every blocker in this document is solved either with an existing keyword + decorator, a stdlib type, or runtime/codegen work. No new bare keywords are proposed.

---

# §6 — Cross-references

Anchored once at the bottom for the index walker:

- Priority stack: [LANGUAGE_DESIGN_PRIORITIES.md](../../../LANGUAGE_DESIGN_PRIORITIES.md)
- Five-phase interop plan: [external-frontend-interop-plan-2026.md](external-frontend-interop-plan-2026.md)
- Wire-format SSOT: [wire-format-v1-ssot.md](wire-format-v1-ssot.md)
- Model orchestration audit: [model-orchestration-ssot-audit-2026.md](model-orchestration-ssot-audit-2026.md)
- Orchestrator companion audit: [orchestrator-companion-audit-findings-2026.md](orchestrator-companion-audit-findings-2026.md)
- Nextgen orchestrator research: [nextgen-orchestrator-research-2026.md](nextgen-orchestrator-research-2026.md)
- Durability runtime audit: [durability-runtime-audit-2026.md](durability-runtime-audit-2026.md)
- v1 release criteria: [v1-release-criteria.md](v1-release-criteria.md)
- v0.5 core SSOT: [v0.5-core-ssot.md](v0.5-core-ssot.md)
- GUI native roadmap status: [gui-native-roadmap-status-2026.md](gui-native-roadmap-status-2026.md)
- Phase 3 HTTP ergonomics spec: [phase3-http-ergonomics-spec-2026.md](phase3-http-ergonomics-spec-2026.md)
- Phase 5 React interop spec: [phase5-react-interop-spec-2026.md](../archive/phase5-react-interop-spec-2026.md)

# Open questions

Items the author of this map is **not** confident about; flag-and-ask at next planning meeting:

1. **Should CC-07 (multi-tenancy) require explicit `Tenant` param, or implicit context?** Recommendation above is explicit, per P3 locality. Counter-argument: every `db.*` call gains a parameter. Decide before any A5 work.
2. **Does CC-17 leader election scale to Populi mesh, or is it single-node-only at v1?** Single-node simplifies; mesh-aware version pushes to v1.5. Decide before scoping CC-17.
3. **Is CC-10 (payments) Stripe-first acceptable, or must we ship two providers at once?** Recommendation: Stripe-first, defer second provider to validation phase. Counter-argument: locking in a Stripe-shaped API forecloses cleanly supporting Adyen / Braintree / Square.
4. **Is CC-12 (rich text) shipping a `BlockTree` editor in stdlib, or is that out of scope?** The type is required either way; the editor component is a separate question.
5. **Should A21 (PWA) live on top of CC-22 alone, or do we need a `vox build --target=mobile-app` (Capacitor / Tauri Mobile / native)?** Native target is a parallel emit path; out of scope for this map but call it out for product-side decision.

---

*Updates land here. When an item ships, mark it ✓ in place; do not delete. When an item is reclassified, leave a deprecation note. The map is the backlog spine — its job is to outlast any single contributor's tenure.*
