# Scientia/GUI — Four Outstanding Net-New Features: Audit & Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Each feature (F1–F4) is independent and may be executed in its own branch/PR.

**Context:** PR #136 (`cc_bdesktop2/nifty-noyce-07ae23`) closed the GUI↔CLI/Scientia *coverage* gap: a self-surfacing CI gate (Track A), the Scientia/gamify/search surfaces it guarded (Tracks B/C + search), the five backend "doesn't-exist" pieces, and works-as-expected polish (typecheck gate, empty states, real per-phase cost split for the **extraction** phase, live dashboard). This plan scopes the **four remaining net-new features** that PR deliberately left as honest TODOs. Each is audit-grounded against the live tree (anchors verified by hand 2026-06-03, not LoC/graph inference).

> **Status (2026-06-03): ALL FOUR IMPLEMENTED** on `cc_bdesktop2/nifty-noyce-07ae23` (PR #136).
> F1 `feat(scientia): surface LlmResponse.cost_usd + wire critic-phase cost emit`;
> F2 `feat(gui): live Scientia-queue push bridge + fix gateway cost 3-tuple` (also fixed a latent PR-#136 compile break in `vox-orchestrator-mcp`);
> F3 `feat(gui): gamify leaderboard / companions / quests surfaces`;
> F4 `feat(gui): publication-lifecycle drill-down + venue routing`.
> Verification: vox-actor-runtime + vox-cli + vox-gui unit tests green; `tsc --noEmit` + 21 vitest green; `vox ci gui-surface-registry` green. Prereg (F4) intentionally deferred — no CLI/DB backing.

**The four features, ordered by leverage-per-effort:**

| # | Feature | Effort | Honest blocker |
|---|---|---|---|
| **F1** | Cost emit-side instrumentation for `critic`/`novelty`/`scholarly` (+ verifier $) | M | 3 of 4 phases don't flow through a *priced* LLM facade — cost must be sourced where the model is actually called, or operator-supplied |
| **F2** | Live GUI↔daemon push bridge for the Scientia queue | S–M | `assemble_scientia_queue` is `pub(crate)`; needs a daemon-side subscribe stream to mirror |
| **F3** | Gamify leaderboards / companions / quests surfaces | S | None — pure mirror of an existing command + existing `vox-gamify::db` functions |
| **F4** | Scientia publication-lifecycle surfaces (detail drill-down + venue routing) | M | prereg has no CLI/DB wiring — defer that one sub-surface |

---

## F1 — Cost emit-side instrumentation (critic / novelty / scholarly + verifier)

### The honest framing (read first)
The per-phase cost split shipped in PR #136 is **real but only wired for `extraction`**. The mechanism:
`insert_scientia_cost_telemetry(phase, provider, cost_usd)` (`crates/vox-db/src/facade/writer_raw.rs:177`) writes a phase-tagged row into `agent_telemetry_flat`; `scientia_cost_by_phase` (`crates/vox-db/src/facade/scientia_cost.rs:130`) groups by `pipeline_phase`. A phase shows non-zero cost **only once its call sites emit phase-tagged rows**.

Two facts make the other three phases non-trivial, not just "copy extraction":

1. **The priced facade already computes `cost_usd`** — `crates/vox-actor-runtime/src/llm/chat.rs:150-156` derives it from `usage.total_cost` (provider-reported) or `config.cost_per_1k` (token estimate). But that value is recorded to the **model scoreboard sink**, *not* `agent_telemetry_flat`. So even phases that call the facade don't auto-populate the per-phase table — they must additionally call `insert_scientia_cost_telemetry` with the cost the facade computed and the phase tag.
2. **Not every phase calls an LLM.** `crates/vox-scientia/src/critic_gate/gate.rs` is pure approval-*policy* logic over `ApproverRole` records — it has **no model call**, so it has **no LLM cost** to attribute (its cost, if any, is the cost of the critics that produced the approvals upstream). Attributing $0 to the gate itself is *correct*, not a gap.

**Therefore "complete cost" means, per phase:** find the real model-invocation site, capture the facade's `cost_usd`, and emit a phase-tagged row. Where a phase has no model call, document that it is legitimately $0 and attribute the spend to the phase that *did* call the model.

### Audited call sites (where the models actually run)
- **extraction** — `claim_extractor/pipeline.rs` (`crates/vox-scientia/src/claim_extractor/`), surfaced via `publication_extract_claims` (`crates/vox-cli/src/commands/scientia_phase_handlers.rs:219`). **Already instrumented** in PR #136 (the reference implementation; see `apply_phase_costs` at :559).
- **critic / novelty** — the MiniCheck-style verifier path. `crates/vox-scientia/src/critic_gate/` + `class_routing/routing.rs`. The "novelty" signal is a verifier score; its cost is the verifier's model cost.
- **scholarly** — `crates/vox-scientia/src/critic_gate/venue.rs` + `crates/vox-cli/src/commands/db/publication/scholarly.rs` (venue/scholarly routing).

### Latent gap to fix for future facade phases (do this once)
`LlmResponse` (`crates/vox-actor-runtime/src/llm/types.rs:253`) **drops `cost_usd`** — the field is computed in `chat.rs` and consumed for the scoreboard, but is not surfaced on the returned response object, so a caller (a scientia phase) cannot read "what did that call cost." Add `cost_usd: Option<f64>` to `LlmResponse` and populate it in `chat.rs`. This is the keystone that makes every *future* facade-routed phase one-line-instrumentable: `db.insert_scientia_cost_telemetry(phase, resp.provider, resp.cost_usd.unwrap_or(0.0))`.

### Tasks
- [ ] **F1-T1** — Surface cost on the facade. Add `cost_usd: Option<f64>` to `LlmResponse` (`types.rs:253`); populate it at the existing computation site (`chat.rs:150-156`). Unit test: a response with `usage.total_cost = Some(x)` yields `cost_usd == Some(x)`; with only `cost_per_1k` yields the token estimate. *No behavior change to the scoreboard sink.*
- [ ] **F1-T2** — Instrument the **critic/novelty** verifier site. At the model-call boundary in `crates/vox-scientia/src/critic_gate/` (the MiniCheck verifier), after the facade call, emit `insert_scientia_cost_telemetry("critic", provider, resp.cost_usd.unwrap_or(0.0))` (and `"novelty"` if the novelty score is a *separate* model call; if it's derived from the same call, attribute once to `critic` and document novelty as derived). Follow the `apply_phase_costs` reference pattern.
- [ ] **F1-T3** — Instrument the **scholarly** site (`critic_gate/venue.rs` / `db/publication/scholarly.rs`) the same way, phase tag `"scholarly"`.
- [ ] **F1-T4** — Document the **gate** as legitimately $0 (pure policy, no model call) in `scientia_cost.rs`'s module doc honesty note — extend the existing note so the four-line breakdown is self-explaining in `vox scientia cost` output.
- [ ] **F1-T5** — Integration test in `crates/vox-db/tests/scientia_cost_phase_tests.rs`: drive a fake critic + scholarly emit, assert `scientia_cost_by_phase` returns the right per-phase totals and that an uninstrumented phase stays absent (→ 0.0), not erroring.
- [ ] **F1-T6** — Verify: `vox scientia cost` shows non-zero `critic`/`scholarly` lines after a real run; pre-v70 DB still degrades to 0.0 (the PRAGMA guard already covers this).

**Out of scope (be honest):** retroactively pricing historical rows (no stored token counts per phase); a budget-forecast vs actuals reconciliation UI (separate feature).

---

## F2 — Live GUI↔daemon push bridge for the Scientia queue

### The honest framing
PR #136 shipped the **server half**: a `scientia.queue.changed` WS topic + a DB-driven poller (task #13) and live `/api/v2/scientia/{queue,cost}` REST. The GUI dashboard currently *polls* (10s `setInterval`). "Live push" means giving the **Tauri GUI** a daemon-subscribed stream so updates are pushed, exactly mirroring the existing orchestrator pattern — not inventing a new transport.

### The exact template to mirror
- `crates/vox-gui/src/commands/orchestrator.rs:21` — `spawn_orchestrator_status_stream` subscribes via `OrchDaemonClient::new(addr).subscribe(tx)` (:37) and re-emits each snapshot as the Tauri event `vox://orch-status` (:43). Its sibling `subscribe_events` (:79) → `vox://agent-events` (:83) is the second instance of the same pattern.
- Both are spawned in `crates/vox-gui/src/main.rs:51-57` inside `.setup()`.
- Front-end consumes via `transport.ts` `listenOrchStatus` with a `setInterval` poll fallback.

### The blocker (small, local)
`assemble_scientia_queue` is `pub(crate)` (`crates/vox-orchestrator-mcp/src/http_gateway/dashboard_api.rs:472`). The daemon needs to *push* its output. Two implementation options:

- **Option A (recommended — fewest moving parts):** add a `subscribe_scientia(tx)` method to `OrchDaemonClient` that mirrors `subscribe`/`subscribe_events`, backed daemon-side by the **already-built** `scientia.queue.changed` poller (reuse it as the change signal; on each tick call the now-`pub` `assemble_scientia_queue`). No new transport.
- **Option B:** have the GUI open the gateway WS (`/v1/ws`) and subscribe to the `scientia.queue.changed` topic directly. Rejected: duplicates the daemon-client transport the other two streams already use, and the GUI doesn't otherwise speak gateway-WS.

### Tasks
- [ ] **F2-T1** — Make `assemble_scientia_queue` `pub` (dashboard_api.rs:472); confirm no visibility ripple (it currently has one in-crate caller).
- [ ] **F2-T2** — Add `OrchDaemonClient::subscribe_scientia(tx)` mirroring `subscribe_events`, driven by the existing `scientia.queue.changed` poller as the change tick; each tick serializes `assemble_scientia_queue()`.
- [ ] **F2-T3** — Add `spawn_scientia_queue_stream` in `crates/vox-gui/src/commands/orchestrator.rs` (mirror of `spawn_orchestrator_status_stream`), emit Tauri event `vox://scientia-queue`. Resilient-by-design: on daemon-absent, exit without crashing (same as the template).
- [ ] **F2-T4** — Spawn it in `main.rs` `.setup()` next to the other two.
- [ ] **F2-T5** — Front-end: `transport.ts` `listenScientiaQueue`; `ScientiaDashboard.tsx` subscribes and keeps the existing 10s `setInterval` as the fallback (don't remove it — the orchestrator streams keep theirs).
- [ ] **F2-T6** — Verify: with the daemon up, mutate the queue (`vox scientia ...`) and observe push-update < 1s; with the daemon down, the GUI still renders and falls back to polling.

**Out of scope:** a generic GUI gateway-WS client (Option B); per-publication granular events (queue-level snapshot is enough).

---

## F3 — Gamify leaderboards / companions / quests surfaces

### The honest framing
This is the **cheapest** of the four: a pure mirror. The data functions exist in `vox-gamify::db`; the GUI-command template exists; only the Tauri commands + a panel are missing. PR #136 surfaced the gamify *profile HUD + notifications*; this completes the trio (leaderboard / companions / quests).

### Audited anchors (all confirmed present)
- `crates/vox-gamify/src/db/leaderboards.rs:19` — `pub async fn leaderboard(db, limit) -> Vec<PlayerRankEntry>`
- `crates/vox-gamify/src/db/companion.rs:10` — `pub async fn list_companions(db, user_id) -> Vec<Companion>`
- `crates/vox-gamify/src/db/quest_battle.rs:13` — `pub async fn list_quests(db, user_id) -> Vec<Quest>`
- `crates/vox-gamify/src/sprite_svg.rs:184` — `generate_svg_from_mood(mood, character_id) -> SvgSprite` whose `.svg_body` (`:71`) is renderable inline (already used in `event_router.rs:280`). Companions render as real SVG, not ASCII.
- **Template:** `crates/vox-gui/src/commands/gamify.rs:106` — `list_ludus_notifications` (the existing mirror to copy).

### Tasks
- [ ] **F3-T1** — Three Tauri commands in `crates/vox-gui/src/commands/gamify.rs` mirroring `list_ludus_notifications`: `gamify_leaderboard(limit)`, `gamify_companions()`, `gamify_quests()`. Each opens the Codex, calls the `vox-gamify::db` fn, returns typed serde structs.
- [ ] **F3-T2** — Register the commands in the Tauri `invoke_handler` and add typed wrappers in `transport.ts`.
- [ ] **F3-T3** — `GamifyView.tsx` (or extend the existing gamify surface) with three sections: leaderboard table (rank/name/score), companions grid (render `svg_body` inline via `dangerouslySetInnerHTML` on sanitized SVG, or an `<img>` data-URL), quests list (name/state/progress).
- [ ] **F3-T4** — Register the surface in `contracts/gui/surface-registry.v1.yaml` with a real `representation_tier` and a `view_key` so the **Track A gate passes** (this is the self-test: an unwired surface fails CI). Re-run `vox ci gui-surface-registry --write` to regenerate `surfaceRegistry.generated.ts`.
- [ ] **F3-T5** — Empty states for each section (no companions / no quests / empty leaderboard), matching the Tier-1 empty-state pattern.
- [ ] **F3-T6** — Verify: panel renders with seeded gamify data; Track A gate green; vitest for the transport wrappers.

**Out of scope:** quest *battle* interactions (write actions) — read-only surfacing first.

---

## F4 — Scientia publication-lifecycle surfaces

### The honest framing
The Scientia *pipeline* board shipped in PR #136 (Track B) shows the queue at a glance. What's missing is **drill-down**: a single publication's lifecycle (manuscript → critic-gate → venue) and the **venue routing** decision surface. Both reuse the existing `execute_command` bridge over CLI subcommands that already exist — no new backend. **Prereg is the exception: it has no CLI/DB wiring**, so it is explicitly deferred (building a surface over a non-existent command would be a stub — forbidden).

### Audited anchors (CLI commands that already exist → drive the surfaces)
- `crates/vox-cli/src/commands/db/publication/` — `route.rs`, `decision.rs`, `prepare.rs`, `preflight.rs`, `scholarly.rs`, `discovery.rs`, `media.rs`, `remote_jobs.rs` (full lifecycle subcommands).
- `crates/vox-cli/src/commands/scientia_phase_handlers.rs` — `manuscript_draft` (:329 dispatch), `critic_gate_check` (:332), `critic_approve` (:339), `publication_extract_claims` (:347), `publication_claims` (:353).
- `crates/vox-scientia/src/critic_gate/venue.rs` + `crates/vox-cli/src/commands/db/publication/route.rs` — venue routing logic.
- Queue assembly to drill from: `assemble_scientia_queue` (dashboard_api.rs:472) — the per-publication rows already carry the IDs.

### Two surfaces (scope)
1. **Publication Detail drill-down** — click a queue row → a panel showing that publication's state (manuscript/critic-gate/venue), claims (`publication_claims`), and the critic-gate approval status (`critic_gate_check`). Read-first; the two safe actions (`critic_approve`, `publication_extract_claims`) wired as buttons over `execute_command`.
2. **Venue Routing panel** — surface `route.rs`/`decision.rs` output: candidate venues, the routing decision, scholarly-venue constraints (`venue.rs` forbids LLM-critic-only approvals — show that diagnostic).

### Tasks
- [ ] **F4-T1** — Publication Detail Tauri command(s): reuse `execute_command` to call `vox scientia publication-claims <id>` and the critic-gate check; return structured (not stringly) results where the CLI already emits JSON, else parse the typed handler output.
- [ ] **F4-T2** — `PublicationDetailView.tsx`: lifecycle stepper (manuscript→critic-gate→venue), claims list, approval status. Drill-in from the existing publication board row.
- [ ] **F4-T3** — Action buttons: `critic_approve`, `publication_extract_claims` over `execute_command`, with confirm + result toast. (Read-safe subset only.)
- [ ] **F4-T4** — `VenueRoutingView.tsx`: candidate venues + decision + the scholarly-constraint diagnostic from `venue.rs` (`"add an audited LLM critic approval, or a second human approver"`).
- [ ] **F4-T5** — Register both surfaces in `contracts/gui/surface-registry.v1.yaml` (+ `view_key`); regenerate; **Track A gate green**.
- [ ] **F4-T6** — Empty/loading/error states; verify drill-down from the board with seeded publication data.

**Explicitly deferred (no backing exists — would be a stub):**
- **Prereg surface** — no CLI subcommand and no DB table for pre-registration. Building it requires first a `vox scientia prereg` command + a `prereg` schema/migration (BASELINE_VERSION bump per the migration policy). That is its own spec→plan→implement cycle; do **not** scaffold a UI over it here.

---

## Cross-cutting notes (apply to all four)

- **Surface-registry gate is the self-test.** F3 and F4 add GUI surfaces; each *must* land a `view_key` row in `contracts/gui/surface-registry.v1.yaml` or `vox ci gui-surface-registry` fails the build. Regenerate `surfaceRegistry.generated.ts` via `--write`; never hand-edit the generated file.
- **No stubs.** Where a backing command/table doesn't exist (prereg; possibly a separate novelty model call), scope **down** to the real artifact and document the deferral — don't ship a hollow panel.
- **Migration policy.** Any new column/table (only F-deferred prereg needs one) = bump `BASELINE_VERSION` in `crates/vox-db/src/schema/manifest.rs` **only**, refresh the digest in `contracts/db/baseline-version-policy.yaml`. No date-stamped SQL files.
- **dep-sprawl.** `vox-actor-runtime`, `vox-db`, `vox-cli`, `vox-gui` are frozen-core — add no new crate deps; all four features reuse existing deps.
- **Doc frontmatter.** Any `.md` authored under `docs/src/` needs canonical `category:` (`"Architecture SSOTs"`), else the doc-pipeline pre-push blocks. (This plan lives under `docs/superpowers/`, which is exempt.)
- **Sequencing recommendation:** F3 (cheapest, pure mirror) → F2 (small, unblocks live UX) → F4 (medium, reuses CLI) → F1 (medium, touches the LLM facade — do the `LlmResponse.cost_usd` keystone first so future phases are one-liners). Each ships its own PR.

---

## Verification gates (every feature)
- `cargo test -p <touched crate>` green; new tests assert the *real* behavior (cost emit, stream emit, command output), not just compilation.
- `cargo run -p vox-arch-check` clean (no layer/fan-in/orphan violations).
- `vox ci gui-surface-registry` green (F3/F4).
- GUI: `npm run typecheck` (the `tsc --noEmit` gate from PR #136) + vitest green.
- Manual: the "Verify" task in each feature must be run against seeded/real data and its output pasted into the PR — evidence before assertions.
