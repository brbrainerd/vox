# P3 — DiscoveryReview GUI surface — Design

**Date:** 2026-06-06
**Status:** Draft (awaiting user review)
**Phase:** SCIENTIA P3 (consumes the P2 human-gated review backend)
**Predecessors:** P1 (#144, per-user nanopub identity) · P2 (#176, review backend) · P2-hardening (#196, publication-bound `ApprovalToken`)
**Mockup:** [`mockups/2026-06-06-p3-discovery-review.html`](mockups/2026-06-06-p3-discovery-review.html)

## Goal

Give the user an in-app surface to review extracted claims for a publication and,
**only with explicit human approval**, drive each claim through the existing
offline nanopublication path. The GUI never bypasses any P2 security invariant and
never publishes to a network — "nanopublish" here means the existing
build + sign + offline-validate + persist-local flow (`published_state="local"`,
`validated_offline=true`).

## Non-goals (explicitly deferred)

- **Network / test-server publishing** — does not exist anywhere in the codebase
  and stays off (test-enforced by `no_production_network_publish_symbol_on_nanopub_path`).
  When real publishing lands post offline-conformance, the GUI's downstream gains
  a publish step; this PR adds no such symbol.
- **REST `/api/v2/scientia/review` + WS `scientia.review.changed`** (P2b) — the GUI
  reads the canonical DB directly via Tauri commands, matching the existing
  `list_research_sessions` / `list_publication_manifests` pattern. No HTTP layer.
- **Per-claim decision-history / audit-trail viewer** — P3b follow-up.
- **Surfacing-accuracy work** (SPECTER2, reachable `Contradicted`) — P4.

## Architecture & data flow

DB-direct Tauri commands in `crates/vox-gui/src/commands/scientia.rs`, each opening
the canonical DB (`connect_canonical_db`) and returning a `serde::Serialize` DTO.
Three new commands:

| Command | Kind | Backend it calls |
|---|---|---|
| `list_publication_review_queue(publication_id) -> Vec<ClaimAwaitingReviewDto>` | read | `vox_db::research_pipeline::list_claims_awaiting_review` (verdict present, no terminal approved/rejected decision) |
| `record_publication_claim_review(publication_id, claim_id, decision, reason) -> ReviewDecisionDto` | write | binds `bound_digest = publication_manifests.content_sha3_256`, then `record_review_decision` |
| `nanopublish_approved_claim(publication_id, claim_id) -> NanopubResultDto` | build | re-derives the `ApprovalToken` via `approval_for`, then the existing `nanopub_build` |

**SSOT for the digest-binding rule.** The CLI already implements
`record_claim_review` (binds `bound_digest`) and `approval_for` (mints a token only
from a persisted `"approved"` decision) in
`crates/vox-cli/src/commands/scientia_nanopub.rs`. To avoid a second copy of the
security logic, the binding/mint/build helpers are extracted to a shared location
both the CLI and the GUI Tauri commands call. Candidate home:
`vox-cli-core::scientia` (already exists and is depended on by both) — confirmed
during implementation; if a cycle appears, the helpers move to `vox-scientia`.

**Live refresh.** Reuse the existing `vox://scientia-queue` event emitted by
`spawn_scientia_queue_stream`; the panel refetches the queue on each ping and keeps
a 10 s interval fallback (matching `ScientiaDashboard`).

## Security model (load-bearing)

1. The build command **never constructs** an `ApprovalToken`. It calls
   `approval_for(db, publication_id, claim_id)`, which loads the latest decision
   scoped to `(claim_id, publication_id)` and mints a token only when that decision
   is `"approved"`. A claim with no approval → `nanopublish_approved_claim` errors.
2. `nanopub_build` enforces, in order: `token.publication_id() == publication_id`,
   `token.claim_id() == claim_id`, `token.bound_digest() == manifest.content_sha3_256`.
   An edit after approval changes the manifest digest → the stale token is rejected,
   so the UI must re-review. The detail pane surfaces this as
   `digest current` vs `digest stale — re-review`.
3. Two explicit human gates in the UI: (a) the Approve decision, then (b) a confirm
   dialog on **Nanopublish (offline)** that restates the offline-only guarantee and
   shows the digest match.
4. No network-publish symbol is added to the GUI command file; the
   `no_production_network_publish_symbol_on_nanopub_path` invariant is extended to
   cover it.

## UI / layout

A new **Discovery Review** surface in the **Knowledge** sidebar group (alongside
Search, Memory, Research, Scientia, Claims, Publications). Faithful to the existing
shell (`Glass` panels, `brass #d4af37` accent, zinc palette, `font-display`
uppercase tracking, mono metadata). See the HTML mockup for the exact treatment.

Layout (two-pane master/detail):

- **Header:** title + publication selector + KPI strip (awaiting / approved /
  rejected / nanopub-local counts).
- **Left — Awaiting Review list:** one card per claim (id, verdict chip, claim
  text, novelty ★, trace count, digest-freshness). Only `awaiting` claims show;
  approved/rejected/built claims collapse into a "N reviewed (hidden)" divider.
  Active card carries the brass left-rail marker.
- **Right — Claim Detail:** claim blockquote, meta grid (verdict, novelty, evidence,
  digest+freshness), optional reason textarea, **Approve / Reject / Defer** buttons.
  After an approval for the selected claim, a brass post-approval zone appears with
  the approval provenance and the **Nanopublish (offline)** button.
- **Confirm dialog:** restates offline-only guarantee; shows publication, claim,
  approval digest match, ORCID; `Cancel` / `Build & sign locally`.

## Registry + surfacing gate

P3 is registry-gated. The surface is added to the SSOT surface registry
(`crates/vox-cli/src/commands/ci/gui_surface_registry.rs`) with
`view_key="discovery-review"`, `nav_group="knowledge"`, a `nav_label`/`nav_icon`,
and the generated `surfaceRegistry.generated.ts` regenerated via the official sync
command (never hand-edited). The `vox ci gui-surface-registry` / coverage gate then
accounts for it.

## Testing

- **Rust (vox-gui + shared helpers):**
  - queue read returns only awaiting claims (verdict present, no terminal decision);
  - decision write persists with `bound_digest == manifest.content_sha3_256`;
  - `nanopublish_approved_claim` succeeds for an approved claim and **errors with no
    approval** (negative path);
  - stale-digest rejection (edit after approval → build refused);
  - extend `no_production_network_publish_symbol_on_nanopub_path` to the GUI command file.
- **Frontend:** vitest for the panel's queue rendering + decision dispatch if the GUI
  test harness covers it (the panel is otherwise covered by the registry/coverage gate).
- **Gates:** `cargo test -p vox-db -p vox-scientia -p vox-cli -p vox-gui` green;
  `vox ci ssot-drift` green (run from source per binary-freshness note);
  `vox ci gui-surface-registry` green; `vox-arch-check` exit 0.

## Acceptance (P3 done when all true)

- Discovery Review appears in the Knowledge sidebar group and is accounted for by the
  surface-registry/coverage gate.
- The queue lists exactly the claims awaiting review for the selected publication.
- Approve/Reject/Defer record an append-only decision bound to the current manifest digest.
- A claim can be nanopublished from the GUI **only** after an explicit approval +
  confirm; the artifact lands `published_state="local", validated_offline=true`.
- No network-publish symbol exists on the GUI nanopub path (test-enforced).
- All gates above green.
