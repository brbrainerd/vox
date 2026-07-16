# Axis GUI Audit Remediation — Design Spec

Date: 2026-07-16
Status: approved (forks resolved by user 2026-07-16)
Source: 5-agent audit (chat, tasks/workbench, model switching, test infra, frontend sweep) + graphify reachability analysis on the refreshed vox-gui graph (4,046 nodes / 8,437 edges).

## Goal

Make the Axis GUI a fully functional harness today: chat and the Tasks (to-do) surface correct and honest, model/backend selection proactively gated on available API keys and live local servers, and a Playwright pipeline that dynamically covers every registered tab/page — including error states, notifications, and key interactions — and actually gates PRs.

## Non-goals

- No new provider adapters, secret plumbing, or model registries (substrate exists).
- No redesign of the workbench tab system (audited healthy).
- No full unification of the orchestrator task graph and the hopper store (fork F1 below records the chosen scope).

## Audit findings (severity-ranked)

### Critical

| # | Finding | Evidence |
|---|---------|----------|
| C1 | Screenshot + visual-review CI never gates PRs. `gui-playwright-smoke` runs only post-merge on main or with the `full-ci` label, and is excluded from required `ci-summary`. A surface that crashes its error boundary merges green. | `.github/workflows/ci.yml:1625,1449` |
| C2 | Chat double-submission: every actionable composer message triggers `SUBMIT_TASK` twice — explicit submit plus the secretary classifier inside `chat_append_message`. Produces a spurious "Secretary proposed a task" toast (dedupe path, null task_id → `"unknown"`) or a wrong "near-duplicate — submit anyway?" dialog when the race is lost. | `crates/vox-gui/src/commands/chat.rs:170-224`, `App.tsx:719-737` |
| C3 | No top-level ErrorBoundary. `main.tsx` renders `<App/>` bare; a throw in App/Sidebar/StatusBar/DockShell white-screens the window. The full-screen recovery component `components/ErrorBoundary.tsx` exists and is imported nowhere. | `ui/src/main.tsx:40-48` |

### P1

| # | Finding | Evidence |
|---|---------|----------|
| B1 | Chat and Tasks write to two disjoint backends (orchestrator task graph vs SQLite hopper). TasksView subtitle "Chat submissions land here" is false; `chat.rs:168` comment "submit to hopper" is false. | `TasksView.tsx:253`, `chat.rs:178-197`, `task_submit.rs:106` |
| B2 | `HopperTaskDto` omits `session_id/agent_id/depends_on/write_files/remote_node`, silently killing 5 TasksView features: session filter chips, write-overlap warnings, depends-on badges, mesh badges, focused-file click-through. The hopper DOES persist `session_id`. | `orchestrator.rs:722-739`, `tasksHelpers.ts:89-94`, `sqlite_store.rs:75,93` |
| B3 | Model selection is reactive, not key-gated: primary selector (`decide()` via `resolve_mcp_chat_model_sync_inner`) ignores API-key presence; keyless picks fail at dispatch then recover via fallback. The credential filter `key_is_present_for()` exists as `#[allow(dead_code)]`. | `resolve.rs:197-392`, `select.rs:766-788`, `registry.rs:272-275` |
| B4 | Local-server health probe (`probe_populi_capabilities`) exists but is only called by the Mens actor — Ollama/local models are assumed reachable by selection. | `inference_env.rs:124`, `mens.rs:161` |
| B5 | Two fully-built, tested routing engines are wired to nothing: actor-runtime 7-way `resolve_chat_provider_route()` and `ModelPool::resolve()` (the rules ∪ picks − excludes design). Dead weight masquerading as the feature. | `model_resolution.rs:80-277`, `model_pool.rs:57-91` |
| B6 | CodeRabbit toasts render blank: pushed as `{kind, message}` but `Toasts.tsx` reads `{tone, title, body}`. | `CodeRabbitView.tsx:75-115`, `Toasts.tsx:5-12` |
| B7 | The `ds/` stylesheet is dead: `ds-section-head` (Cinzel + underline divider — a project rule) is defined in `ds/components.css`, imported only by `ds/styles.css`, which nothing under `ui/` imports. MissionControl/VoxGraph/NeedsYou headings render unstyled, violating the divider rule. | `ds/components.css:38`, `ui/src/index.css` |
| B8 | `MissionControl` is routed under viewKey `'mission-control'` which does not exist in `SURFACE_REGISTRY` — no nav entry, no screenshot, likely unreachable. Registry-escape class: surfaces wired only into `surfaceComponents.tsx` silently skip all screenshot coverage (also `Matrix` — not routed at all, likely dead — plus `DocReader`, `Loquela` special renders). | `surfaceComponents.tsx:178-179` |
| B9 | GUI has no backend-availability surface: only `openrouter_key_status` exists; `available_inference_providers()` is never exposed. User cannot see which backends are live. | `llm_settings.rs:57-61`, `key_guard.rs:11-55` |

### P2 (selected)

- Chat: token-loss race when `task_started` precedes `submitResolved` and the frame lacks `session_id` (`chatCorrelation.ts:143-158`); `chat_rename_session`/`chat_archive_session` implemented + registered but no UI calls them; `model_id` persistence dead end-to-end; redundant double hydrate per session switch; silent `.catch(() => {})` on assistant-message persist (`App.tsx:849-856`).
- Tasks: `completed` mapping unreachable (`hopper_list` returns only inbox+assigned); no "mark done" affordance (cancel is the only removal); priority enum coupling relies on implicit 0/1/2 discriminants with no shared constant or guard test.
- Frontend: listener leaks in `SubAgentsView.tsx:32-42` and `NeedsYouSurface.tsx:48-60` (missing disposed-flag pattern); unhandled `listen()` rejections in `TasksView.tsx:83-86`, `SettingsView.tsx:760`, `CodeRabbitView.tsx:80-84`; `useLocalStorage` errors go to `console.log` only; CodeRabbitView is a light-theme island using nonexistent CSS vars with light literal fallbacks; hardcoded colors bypassing tokens in `AttentionStrip`, `Popover`, `chartWidgetShared`, `LudusSandbox`, `ActivitySurface`; `Omnibar.tsx:283` TODO(VG-1) graph corpus not passed.
- Tests/CI: `screenshots-variants.spec.ts` (empty/error states) default-skipped, never run in CI; no e2e asserts toasts/`role=alert`/failed IPC; `visual-review.spec.ts` asserts nothing about rendering; visual-review cache keys only on screenshot sha256 (model/prompt changes reuse stale verdicts) and holds ~10 dead viewKeys with `schema_version: 0`; `playwright.screens.config.ts` is an orphan config referenced by nothing; staged `contracts/reports/gui-visual-review/0000-00-00.json` is a placeholder from a local run without `--date`/`--now` and must be unstaged, with a guard added so it cannot recur.
- Dead code: only `Scientia/DiscoveryReviewView.tsx` is actually dead (sole references: its own test + the ipcBoundaries allowlist) — removed in Phase 3 Task 3. The graph's other 0-dependent flags were WRONG on verification: `DiscoveryReview.tsx` is live via `DiscoverySurface.tsx:66`, `ScientiaDashboard.tsx` via `ScientiaSurface.tsx:40` (decoratorRegistry), `PriorityChainEditor.tsx` via `SettingsView.tsx:1407`. ~30 components still bypass the transport hub with raw `invoke()` (tracked by the shrinking `ipcBoundaries` allowlist); new code must not grow that allowlist.

### What is healthy (verified, no action)

Workbench tabs (open/close/pin/persist/migrate, tested); chat streaming pipeline and persistence with dedupe; hopper CRUD + restart persistence (test-covered); TasksView shared-poll migration (the known follow-up landed in `f225b8b33b`); dynamic screen enumeration from generated `SURFACE_REGISTRY` with PR-enforced drift guard; reactive credential-aware fallback chain in the infer loop; ModelsView wired to live registry state; honesty-scan guard blocking no-op handlers; polling hooks (cleanup + embedded-gating) clean.

## Design

### Phase 1 — Bug fixes (correctness, small verified diffs)

1. **Secretary double-submit (C2):** in `chat_append_message`, skip `secretary::classify` when the message arrives flagged as already-submitted (composer passes an `already_submitted` marker), and suppress `emit_secretary_proposed` when the daemon reply is a duplicate (null task_id). Test: unit on chat.rs paths + e2e composer submit asserting exactly one task and zero secretary toasts.
2. **Top-level ErrorBoundary (C3):** wrap `<App/>` in the existing `components/ErrorBoundary.tsx` in `main.tsx`. Test: unit render throwing child.
3. **CodeRabbit toast shape (B6):** convert pushes to `{tone, title, body, cause}`. Add a type-level guard (the push site currently type-checks because of a loose store signature — tighten it).
4. **ds stylesheet (B7):** import `ds/styles.css` from `ui/src/index.css` (or migrate the 3 used classes into `ui/src/styles/` if the ds sheet conflicts with tokens — decide at implementation by diffing selectors). Verify via screenshot diff on NeedsYou/VoxGraph.
5. **Listener leaks + unhandled rejections:** apply the disposed-flag pattern (`AgentTab.tsx:18-36` reference) to `SubAgentsView`, `NeedsYouSurface`; add `.catch` guards to the unguarded `listen()` sites INCLUDING the two in `ActivitySurface.tsx:283-303` (review-found gap — without them the task's "no unhandled rejection" invariant stays false); route `useLocalStorage` errors through `console.warn`; delete the redundant `ChatSurface.tsx:130-132` hydrate effect (App.tsx:652 owns hydration — closes the P2 double-hydrate finding). `BrowserView.tsx`'s three effects are explicitly out of scope (they `.catch` already; leak-only, no rejection risk).
6. **Tasks copy honesty (B1 interim):** correct the TasksView subtitle and `chat.rs:168` comment to state where each store's items actually come from (full merge is Phase 2, per fork F1).
7. **Unstage `0000-00-00.json`** and make `gui-visual-review` refuse to write a report when `--date` is absent (default to system UTC date instead of the `0000-00-00` literal).
8. **Chat token-loss race:** buffer unroutable `token_streamed`/`task_started` frames for a short window and replay once `submitResolved` lands, instead of dropping.

### Phase 2 — Wiring (make the harness real)

1. **Key-gated selection (B3):** activate `key_is_present_for()` inside the candidate filter of `decide()` (or fold `available_inference_providers()` into `routing_allows` in `resolve.rs:233`). Keyless providers are excluded at selection time; reactive fallback stays as the safety net.
2. **Local-server health in selection (B4):** call `probe_populi_capabilities()` with a short-TTL cache before offering Ollama/PopuliMesh candidates (both served via `local_ollama_populi_base_url()`). VoxLocal is explicitly NOT gated on this probe — it is a different server (`VOX_LOCAL_ENDPOINT`, :7863) already guarded by its own `probe_vox_local_health` at the call path.
3. **Delete the two dead routing engines (B5)** (`resolve_chat_provider_route`, `ModelPool::resolve` + its config field), per fork F3. The single exercised path is `decide()` + fallback, now credential- and health-gated.
4. **Availability panel (B9):** one new Tauri command wrapping `available_inference_providers()` + the cached probe; render per-backend live status in the Models surface. The chat-surface model picker must thread the pick as `enqueue_hints.model_override` on `SUBMIT_TASK` (`TaskEnqueueHints.model_override` → `StreamRoute::UserModelOverride`, the channel the daemon actually consumes) — `set_active_model` alone only mutates GUI-process env + an unread DB preference and would be display-only dead wiring.
5. **Hopper DTO fields (B2):** extend `HopperTaskDto` with the fields the hopper actually persists — `session_id, agent_id, remote_node` (plan-verified; `depends_on`/`write_files`/`estimated_complexity` are NOT persisted by SqliteHopper and instead arrive via the merge-view's orchestrator-side rows); delete the corresponding hardcoded nulls in `mapHopperTasksToRows`.
6. **Tasks surface merge (B1, per fork F1):** frontend union in TasksView — `hopper_list` is extended only with terminal `done` items (bounded, most-recent-N) so the `completed` branch is reachable, and TasksView merges those rows with the existing `list_orchestrator_tasks` read, origin-tagged, so chat submissions appear in the Tasks surface. Scope limit (recorded): `hopper_list` itself stays hopper-only — other consumers (`useAttentionInbox` blocked-count, urbs harness) keep hopper-only semantics by design, so gated orchestrator tasks show as blocked rows in Tasks but are not counted in the attention strip. Add "mark done" for hopper to-dos; shared Rust↔TS priority constant with a dual guard test.
7. **Session management:** wire `chat_rename_session`/`chat_archive_session` into the session rail context menu; persist and render `model_id` on assistant messages (or delete the DTO fields if the badge is not wanted — recommend wiring, the field is already tested backend-side).

### Phase 3 — Test & CI buildout

1. **Post-merge hardening (C1, per fork F2 — user chose no PR gating):** the smoke job stays post-merge/`full-ci`-only. Plan investigation found the asserting sweep step ALREADY lacks `continue-on-error` (only the 3 advisory steps carry it) — so the change is a comment pin to keep it that way, plus enabling variant screenshots advisorily (`VOX_VARIANT_SCREENSHOTS=1`, `continue-on-error: true`) and uploading variant PNGs in the artifact.
2. **Close the registry escape (B8):** a guard test that walks `surfaceComponents.tsx` render cases + special tab types (DocReader) and fails if any routed surface lacks a registry viewKey or an explicit allowlist entry. `MissionControl`: REMOVE (plan-verified unreachable; duplicates registered surfaces). `Matrix`: KEEP — plan investigation found it is NOT dead; it renders inside the chat rail as the routing panel (`ChatSurface.tsx:329`); the audit's dead-code flag was wrong.
3. **Error/notification coverage:** un-skip `screenshots-variants.spec.ts` in the post-merge job (empty + error states for key surfaces); add toast assertions (`role=status`/`role=alert`) to the variant error runs; add an IPC-failure spec using `installErrorStateMock` asserting each key surface degrades with visible error UI, not a blank panel.
4. **Interaction specs:** initial set landing with Phase 3 — approvals approve/reject flow, task create→reprioritize→cancel, chat submit→stream→persist (mocked). A second set explicitly owned by Phase 3 Task 13 and sequenced AFTER Phase 2 lands — model picker apply, session rename/archive, task mark-done — including their tauriMock command cases (`set_active_model`, `chat_rename_session`, `chat_archive_session`, `hopper_mark_done`), since the wiring they exercise ships in Phase 2. Deduplicate the `bootstrapResponse` block shared by `tauriMock.ts`/`tauriMockVariants.ts`.
5. **Visual-review cache correctness:** include model id + prompt version in the cache key; prune dead viewKeys; set/check `schema_version: 1`. Delete the orphan `playwright.screens.config.ts`.

## Testing strategy

Every Phase 1 fix lands with the regression test named beside it, plus wiring-level guards where the fix's seam is otherwise unobservable (the C2 `already_submitted` contract is asserted both at the App.tsx payload level and in the Phase 3 chat e2e). Phase 2 backend changes get Rust unit tests (key-gating candidate filter, health-gate cache plumbing + resolver wiring via a test-only override, DTO field mapping) and TS tests for the merge-view mapping/origin tagging + the shared priority-constant guard pair; the e2e interaction specs exercising Phase 2 wiring land in Phase 3 Task 13, sequenced after Phase 2. Phase 3 is itself the test buildout; its own guard is the registry-escape test failing on a deliberately unregistered surface.

## Explicit deferrals (recorded so they don't silently fall through)

- **CodeRabbitView light-theme island** (PILL colors + inline light-literal styles referencing nonexistent `--bg-success`/`--surface-1`/`--bg-pro` vars, `CodeRabbitView.tsx:30-34,126`) and **hardcoded colors bypassing tokens** in `AttentionStrip.tsx:13`, `Popover.tsx:13`, `chartWidgetShared.tsx:6,46-51`, `LudusSandbox.tsx:300,306`, `ActivitySurface.tsx:220` — deferred to a dedicated styling pass after this remediation (Phase 1 fixes only the toast shape; restyling the island is bigger than a bug fix and lower risk to batch).
- **Omnibar `TODO(VG-1)`** (graph corpus not passed, `Omnibar.tsx:283`) — deferred; belongs to the graph-search initiative, not this remediation.
- **Raw-`invoke()` transport-hub migration** (~30 files) — stays governed by the shrink-only `ipcBoundaries` allowlist; not expanded here, and no new code may grow the allowlist.

## Rollout

Three PR series in phase order (bugs → wiring → tests/CI), each independently green and revertable. Phase 1 items are small enough to land as one PR of independent commits. The staged `0000-00-00.json` unstage happens immediately, outside the series.

## Decision forks (RESOLVED by user, 2026-07-16)

- **F1 — Task store scope: merge-view.** Unified origin-tagged read; both stores kept; "mark done" added.
- **F2 — CI gating: keep post-merge only.** No PR gate. Mitigation only: asserting sweep fails the post-merge job loudly (drop `continue-on-error` on that step).
- **F3 — Routing engines: delete both.** Single exercised path = `decide()` + credential/health gating + reactive fallback.
