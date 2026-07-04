# GUI Honesty Triage Decision Table
Generated: 2026-06-25. Awaiting Gate G2 human approval.

## Summary
- Total behavioral findings: 62
- KEEP: 53 | WIRE: 6 | HIDE: 3 | TOAST-FIX: 4
- Visual findings (informational): 68

## Decision Policy Applied
- `verdict: "works"` → **KEEP**
- `verdict: "dead"` with `cheap_to_wire: true` → **WIRE**
- `verdict: "dead"` with `cheap_to_wire: false` → **HIDE**
- `verdict: "noop-toast"` with `cheap_to_wire: true` → **WIRE** + **TOAST-FIX**
- `verdict: "noop-toast"` with `cheap_to_wire: false` → **HIDE** + **TOAST-FIX**
- `verdict: "placeholder"` → **HIDE** (unless cheap real value → **WIRE**)

---

## Behavioral Decisions

| Surface | File:Line | Label | Verdict | Cheap? | Backend Command | DECISION |
|---------|-----------|-------|---------|--------|-----------------|----------|
| Activity | ActivitySurface.tsx:328 | Agent filter onChange | works | — | activity_query | **KEEP** |
| Activity | ActivitySurface.tsx:346 | Event Type filter onChange | works | — | activity_query | **KEEP** |
| Activity | ActivitySurface.tsx:361 | Refresh button onClick | works | — | activity_query | **KEEP** |
| Activity | ActivitySurface.tsx:209 | Fold/Expand toggle button onClick | works | — | none (local state) | **KEEP** |
| Catalog | CommandCatalogForm.tsx:5 | skills prop (catalog data source) | works | — | tauri:get_command_catalog | **KEEP** |
| Catalog | CommandCatalogForm.tsx:88 | Command list item onClick → handleCommandSelect | works | — | none (local state) | **KEEP** |
| Catalog | CommandCatalogForm.tsx:197 | Execute button onClick → handleExecute | works | — | tauri:execute_command / tauri:invoke_mcp_tool | **KEEP** |
| Catalog | CommandCatalogForm.tsx:135 | Flag checkbox onChange → setArgValues | works | — | none (local state) | **KEEP** |
| Catalog | CommandCatalogForm.tsx:147 | Select dropdown onChange → setArgValues | works | — | none (local state) | **KEEP** |
| Catalog | CommandCatalogForm.tsx:162 | Count number input onChange → setArgValues | works | — | none (local state) | **KEEP** |
| Catalog | CommandCatalogForm.tsx:170 | Text input onChange → setArgValues | works | — | none (local state) | **KEEP** |
| Catalog | CommandCatalogForm.tsx:191 | Additional Raw Arguments onChange → setArgsInput | works | — | none (local state) | **KEEP** |
| Chat | ChatSurface.tsx:66 | loadSessions (useEffect on mount) | works | — | chat_list_sessions | **KEEP** |
| Chat | ChatSurface.tsx:128 | createSession (onCreateSession handler) | works | — | chat_create_session | **KEEP** |
| Chat | ChatSurface.tsx:191 | SecretaryToast onViewTask | works | — | none (navigation) | **KEEP** |
| Chat | ChatAgentEventRow.tsx:97 | handleApprovePlan (PhaseChip planning button) | works | — | approve_orchestrator_task_plan | **KEEP** |
| Chat | ChatAgentEventRow.tsx:100 | handleSkipVerify (PhaseChip verifying button) | works | — | skip_orchestrator_verify | **KEEP** |
| Chat | ChatAgentEventRow.tsx:103 | handleForceVerify (PhaseChip acting button) | works | — | force_orchestrator_verify | **KEEP** |
| Chat | ChatAgentEventRow.tsx:79 | View in Flow button onClick (token_group row) | works | — | none (navigation) | **KEEP** |
| Chat | ChatAgentEventRow.tsx:150 | View in Flow button onClick (agent event row) | works | — | none (navigation) | **KEEP** |
| Chat | ChatAgentEventRow.tsx:60 | token_group expand/collapse button | works | — | none (local state) | **KEEP** |
| Chat | ChatAgentEventRow.tsx:130 | agent event row expand/collapse button | works | — | none (local state) | **KEEP** |
| Chat | ChatExecutionRail.tsx:165 | intent button onClick | works | — | none (navigation) | **KEEP** |
| Chat | ChatExecutionRail.tsx:183 | Agents KPI onClick | works | — | none (navigation) | **KEEP** |
| Chat | ChatExecutionRail.tsx:221 | ContextWindowMeter usedTokens prop | dead | no | none | **HIDE** |
| Chat | ChatSessionRail.tsx:81 | session tab Button onClick | works | — | none (navigation) | **KEEP** |
| Chat | ChatSessionRail.tsx:101 | New chat session Button onClick | works | — | chat_create_session | **KEEP** |
| Chat | SecretaryToast.tsx:54 | View task button onClick | works | — | none (navigation) | **KEEP** |
| Chat | SecretaryToast.tsx:63 | Dismiss button onClick | works | — | none (local state) | **KEEP** |
| Chat | ModelBadge.tsx:33 | model badge toggle button onClick | works | — | none (local state) | **KEEP** |
| Dashboard | Dashboard/Dashboard.tsx:303 | Open Chat CTA button | works | — | none (navigation) | **KEEP** |
| Dashboard | Dashboard/Dashboard.tsx:349 | Customize dashboard toggle | works | — | none (local state) | **KEEP** |
| Dashboard | Dashboard/Dashboard.tsx:330 | Add widget button | works | — | none (local state) | **KEEP** |
| Dashboard | Dashboard/Dashboard.tsx:336 | Reset to default button | works | — | none (local state) | **KEEP** |
| Dashboard | Dashboard/Dashboard.tsx:162 | Stream filter buttons (all/validated/in-progress/doubted/speculative) | works | — | none (local state) | **KEEP** |
| Dashboard | Dashboard/Dashboard.tsx:368 | Immersive View link (Workspace Simulation Mini-Map) | works | — | none (navigation) | **KEEP** |
| Dashboard | Dashboard/Dashboard.tsx:369 | Collapse/Expand button (Workspace Simulation Mini-Map) | works | — | none (local state) | **KEEP** |
| Dashboard | Dashboard/AgentRow.tsx:49 | Pause/Resume agent button | works | — | pause_orchestrator_agent / resume_orchestrator_agent | **KEEP** |
| Dashboard | Dashboard/AgentRow.tsx:57 | Open in Console button | works | — | none (navigation) | **KEEP** |
| Dashboard | Dashboard/AgentRow.tsx:67 | Approve button (inline approval) | works | — | vox_resolve_approval (MCP) | **KEEP** |
| Dashboard | Dashboard/AgentRow.tsx:69 | Reject button (inline approval) | works | — | vox_resolve_approval (MCP) | **KEEP** |
| Dashboard | Dashboard/StreamCard.tsx:39 | Doubt button on StreamCard | dead | no | none | **HIDE** |
| Dashboard | Dashboard/StreamCard.tsx:44 | Overrule button on StreamCard | dead | no | none | **HIDE** |
| Dashboard | Dashboard/LudusBanner.tsx:27 | Acknowledge alert (X) button | works | — | ack_ludus_notification | **KEEP** |
| Flow | AgentFlow.tsx:231 | Node onClick (graph node / agent shard selection) | works | — | none (local state) | **KEEP** |
| Flow | AgentFlow.tsx:232 | Node onKeyDown (keyboard nav: Enter/Space) | works | — | none (local state) | **KEEP** |
| Harness | HarnessRedirect.tsx:26 | Focus composer | works | — | none (prop callback) | **KEEP** |
| Harness | HarnessRedirect.tsx:16 | recordGamifyGuiEvent('harness_redirect_viewed') | works | — | voxTransport.recordGuiEvent | **KEEP** |
| Memory | MemoryView.tsx:202 | Auto-recall toggle | works | — | voxTransport.setGuiPreference | **KEEP** |
| Memory | MemoryView.tsx:328 | Reindex button | works | — | mnemosyne_reindex | **KEEP** |
| Memory | MemoryView.tsx:360 | Recall button / Enter key | works | — | vox_search_query | **KEEP** |
| Memory | MemoryView.tsx:419 | Recent recall row click | works | — | vox_search_query | **KEEP** |
| Memory | MemoryView.tsx:445 | Pin all to context button | works | — | none (local + prop) | **KEEP** |
| Memory | MemoryView.tsx:120 | HitCard per-item pin button | works | — | none (local + prop) | **KEEP** |
| Memory | MemoryView.tsx:110 | HitCard open button | works | — | voxTransport.openLocator | **KEEP** |
| Memory | MemoryView.tsx:375 | Corpus scope chip toggle | works | — | none (local state) | **KEEP** |
| Mesh | MeshView.tsx:190 | Refresh button | works | — | invoke_mcp_tool(vox_mesh_nodes + vox_mesh_queue_stats) | **KEEP** |
| Mesh | MeshView.tsx:321 | Target node select (onChange) | works | — | none (local state) | **KEEP** |
| Mesh | MeshView.tsx:338 | Task kind input (onChange) | works | — | none (local state) | **KEEP** |
| Mesh | MeshView.tsx:350 | Source textarea (onChange) | works | — | none (local state) | **KEEP** |
| Mesh | MeshView.tsx:362 | Dispatch button | works | — | invoke_mcp_tool(vox_mesh_dispatch) | **KEEP** |
| Runs | RunsView.tsx:56 | get_model_scoreboard (auto-poll) | works | — | get_model_scoreboard | **KEEP** |
| Runs | RunsView.tsx:58 | list_gui_runs (auto-poll) | works | — | list_gui_runs | **KEEP** |
| Runs | RunsView.tsx:70 | get_routing_summary_live (auto-poll) | works | — | get_routing_summary_live | **KEEP** |
| Runs | RunsView.tsx:91 | get_gui_run (row click → detail panel) | works | — | get_gui_run | **KEEP** |
| Runs | RunsView.tsx:151 | Workflow name button → setSelectedRunId | works | — | get_gui_run | **KEEP** |
| Runs | RunsView.tsx:62 | recordGamifyGuiEvent('workflow_completed') | works | — | none (gamify side-effect) | **KEEP** |
| Runs | RunsView.tsx:73 | pushToast on load failure | works | — | none (error reporting) | **KEEP** |
| Settings | SettingsView.tsx:1258 | Orchestrator — Max concurrent agents slider | works | — | set_orchestrator_config | **KEEP** |
| Settings | SettingsView.tsx:1261 | Orchestrator — Global budget cap slider | works | — | set_orchestrator_config | **KEEP** |
| Settings | SettingsView.tsx:1286 | Orchestrator — Auto-doubt toggle | works | — | set_orchestrator_config | **KEEP** |
| Settings | SettingsView.tsx:1271 | Orchestrator — Default isolation tier buttons | works | — | set_orchestrator_config | **KEEP** |
| Settings | SettingsView.tsx:1413 | Telemetry — Off/Local/Cloud selector buttons | works | — | voxTransport.setGuiPreference | **KEEP** |
| Settings | SettingsView.tsx:1475 | Theme — Arcane/Void/Glacier selector | works | — | voxTransport.setGuiPreference | **KEEP** |
| Settings | SettingsView.tsx:1338 | Routing — Emphasis preset buttons | works | — | voxTransport.setRoutingPriority | **KEEP** |
| Settings | SettingsView.tsx:1448 | Gamification — enabled checkbox | works | — | set_gamify_settings | **KEEP** |
| Settings | SettingsView.tsx:1453 | Gamification — Mode select | works | — | set_gamify_settings | **KEEP** |
| Settings | SettingsView.tsx:1430 | Keybinds section — all entries | dead | no | none | **HIDE** |
| Settings | SettingsView.tsx:259 | Mesh — trust/untrust button per peer | works | — | trust_mesh_node / untrust_mesh_node | **KEEP** |
| Settings | SettingsView.tsx:303 | Signing keys — create/rotate identity button | works | — | rotate_signing_key | **KEEP** |
| Settings | SettingsView.tsx:358 | Signing — Require signature toggle | works | — | set_orchestrator_config | **KEEP** |
| Settings | SettingsView.tsx:525 | Keys & Secrets — save button (per secret) | works | — | set_secret | **KEEP** |
| Settings | SettingsView.tsx:589 | Keys & Secrets — remove button (per secret) | works | — | remove_secret | **KEEP** |
| Settings | SettingsView.tsx:629 | Keys & Secrets — Migrate auth.json button | works | — | migrate_auth_store | **KEEP** |
| Settings | SettingsView.tsx:640 | Keys & Secrets — Preview .env button | works | — | import_env | **KEEP** |
| Settings | SettingsView.tsx:648 | Keys & Secrets — Import N button (conditional) | works | — | import_env | **KEEP** |
| Settings | SettingsView.tsx:775 | Runtime config — save button per field | works | — | set_user_config | **KEEP** |
| Settings | SettingsView.tsx:873 | Runtime config — reset button per field | works | — | reset_user_config | **KEEP** |
| Settings | SettingsView.tsx:809 | Runtime config — enum option buttons (save-on-click) | works | — | set_user_config | **KEEP** |
| Settings | PriorityChainEditor.tsx:133 | Priority chain — remove step button | works | — | voxTransport.setSelectionPolicy | **KEEP** |
| Settings | PriorityChainEditor.tsx:135 | Priority chain — move up/down buttons | works | — | voxTransport.setSelectionPolicy | **KEEP** |
| Settings | PriorityChainEditor.tsx:430 | Priority chain — 'add to chain' button | works | — | voxTransport.setSelectionPolicy | **KEEP** |
| Settings | HudTilesEditor.tsx:40 | HUD tiles — enable/disable checkbox | works | — | none (local + prop) | **KEEP** |
| Settings | HudTilesEditor.tsx:51 | HUD tiles — move up/down buttons | works | — | none (local + prop) | **KEEP** |
| Settings | HudTilesEditor.tsx:74 | HUD tiles — Reset to defaults button | works | — | none (local + prop) | **KEEP** |
| Settings | SettingsView.tsx:1491 | HudTiles section | works | — | none | **KEEP** |
| Settings | SettingsView.tsx:303 | Signing key rotation — uses window.prompt for password | works | — | rotate_signing_key | **KEEP** |
| SkillsPlugins | SkillsPluginsView.tsx:193 | Refresh button | works | — | invoke_mcp_tool(vox_skill_list\|vox_plugin_catalog\|vox_skill_discover) | **KEEP** |
| SkillsPlugins | SkillsPluginsView.tsx:218 | Uninstall skill button | works | — | invoke_mcp_tool(vox_skill_uninstall) | **KEEP** |
| SkillsPlugins | SkillsPluginsView.tsx:220 | Remove plugin button | works | — | invoke_mcp_tool(vox_plugin_remove) | **KEEP** |
| SkillsPlugins | SkillsPluginsView.tsx:222 | Skill Info button (installed tab) | noop-toast | yes | invoke_mcp_tool(vox_skill_info) | **WIRE** + **TOAST-FIX** |
| SkillsPlugins | SkillsPluginsView.tsx:223 | Plugin Info button (installed tab) | noop-toast | yes | invoke_mcp_tool(vox_plugin_info) | **WIRE** + **TOAST-FIX** |
| SkillsPlugins | SkillsPluginsView.tsx:233 | Install plugin button (marketplace tab) | works | — | invoke_mcp_tool(vox_plugin_install) | **KEEP** |
| SkillsPlugins | SkillsPluginsView.tsx:236 | Plugin Info button (marketplace tab) | noop-toast | yes | invoke_mcp_tool(vox_plugin_info) | **WIRE** + **TOAST-FIX** |
| SkillsPlugins | SkillsPluginsView.tsx:329 | Search button / Enter key in search input | works | — | invoke_mcp_tool(vox_skill_search) | **KEEP** |
| SkillsPlugins | SkillsPluginsView.tsx:346 | Installed skills rows (search results) — actions=[] | dead | yes | invoke_mcp_tool(vox_skill_install) | **WIRE** |
| SkillsPlugins | SkillsPluginsView.tsx:398 | View button (discovered tab) | noop-toast | yes | invoke_mcp_tool(vox_skill_use) | **WIRE** + **TOAST-FIX** |
| Tasks | TasksView.tsx:54 | Load tasks (hopper_list) | works | — | hopper_list | **KEEP** |
| Tasks | TasksView.tsx:124 | Add task (TaskComposer onSubmit) | works | — | hopper_submit | **KEEP** |
| Tasks | TasksView.tsx:129 | Cancel task (X button per row) | works | — | hopper_cancel | **KEEP** |
| Tasks | TasksView.tsx:151 | Reprioritize task (priority select onChange) | works | — | hopper_reprioritize | **KEEP** |
| Tasks | TasksView.tsx:262 | Refresh button onClick | works | — | hopper_list | **KEEP** |
| Tasks | TasksView.tsx:281 | Session filter pills onClick | works | — | none (local state) | **KEEP** |
| Tasks | TasksView.tsx:300 | Show blocked tasks checkbox onChange | works | — | none (local state) | **KEEP** |
| Tasks | TasksView.tsx:179 | Description span onClick (focus file in Ludus) | works | — | none (local state) | **KEEP** |
| Tasks | TaskComposer.tsx:12 | Form onSubmit / Add button onClick | works | — | hopper_submit | **KEEP** |

---

## Summary Table by Surface

| Surface | Total | KEEP | WIRE | HIDE | TOAST-FIX |
|---------|-------|------|------|------|-----------|
| Activity | 4 | 4 | 0 | 0 | 0 |
| Catalog | 8 | 8 | 0 | 0 | 0 |
| Chat | 17 | 16 | 0 | 1 | 0 |
| Dashboard | 13 | 10 | 0 | 2 | 0 |
| Flow | 2 | 2 | 0 | 0 | 0 |
| Harness | 2 | 2 | 0 | 0 | 0 |
| Memory | 8 | 8 | 0 | 0 | 0 |
| Mesh | 5 | 5 | 0 | 0 | 0 |
| Runs | 7 | 7 | 0 | 0 | 0 |
| Settings | 28 | 27 | 0 | 1 | 0 |
| SkillsPlugins | 10 | 6 | 4 | 0 | 4 |
| Tasks | 9 | 9 | 0 | 0 | 0 |
| **TOTAL** | **113** | **104** | **4** | **4** | **4** |

> Note: final behavioral count is 113 (some surfaces had more findings than the initial rough count of 62 behavioral entries). TOAST-FIX applies to all 4 WIRE decisions in SkillsPlugins — those are all noop-toast verdicts.

---

## Visual Findings (Informational — no code change required for triage approval)

| Surface | File:Line | Dimension | Issue | Severity |
|---------|-----------|-----------|-------|----------|
| Activity | ActivitySurface.tsx:103 | ds-token | Hardcoded emerald-500 classes for success-state rows | med |
| Activity | ActivitySurface.tsx:106 | ds-token | Hardcoded rose-500 classes for failure rows | med |
| Activity | ActivitySurface.tsx:110 | ds-token | Hardcoded amber-500 classes for warning/alert rows | med |
| Activity | ActivitySurface.tsx:113 | ds-token | Hardcoded sky-500 classes for CostIncurred rows | med |
| Activity | ActivitySurface.tsx:315 | ds-token | text-emerald-400 hardcoded on bolt icon in heading | low |
| Activity | ActivitySurface.tsx:156 | a11y | Icon container div (kind icon) has no aria-label | med |
| Activity | ActivitySurface.tsx:364 | a11y | Refresh button uses Icon.alert (warning icon) as loading spinner — misleading icon; no aria-label for loading vs refresh state | med |
| Activity | ActivitySurface.tsx:374 | a11y | Loading spinner uses Icon.alert with animate-spin — semantically wrong; no aria-busy or role='status' | high |
| Activity | ActivitySurface.tsx:362 | hierarchy | Refresh styled identically to filter controls; no primary-action prominence | low |
| Catalog | CommandCatalogForm.tsx:90 | hardcoded color token | bg-cyan/10, border-cyan, text-cyan — raw palette alias not semantic token | low |
| Catalog | CommandCatalogForm.tsx:140 | hardcoded color token | text-red-400 for REQUIRED label — not semantic token | low |
| Catalog | CommandCatalogForm.tsx:208 | hardcoded color token | text-red-500 for error output — not semantic token | low |
| Catalog | CommandCatalogForm.tsx:211 | hardcoded color token | text-green-500 / text-red-500 for exit_code — not semantic token | low |
| Catalog | CommandCatalogForm.tsx:217 | hardcoded color token | text-cyan for stdout pre — raw palette alias | low |
| Catalog | CommandCatalogForm.tsx:223 | hardcoded color token | text-red-400 for stderr — not semantic token | low |
| Catalog | CommandCatalogForm.tsx:98 | hardcoded color token | bg-black/30 — should use bg-overlay-* token | low |
| Catalog | CommandCatalogForm.tsx:82 | a11y | Command list items use `<li onClick>` with no role="button" or tabIndex — not keyboard-navigable | high |
| Catalog | CommandCatalogForm.tsx:129 | a11y | Flag checkbox shares id={arg.name} with downstream inputs — ambiguous label association | med |
| Catalog | CommandCatalogForm.tsx:197 | a11y | Execute button has no disabled state or aria-busy while async in flight — double-submit risk | med |
| Catalog | CommandCatalogForm.tsx:115 | hierarchy | Two sibling h2 elements on same surface — breaks document outline | med |
| Catalog | Catalog/Catalog.tsx:16 | hierarchy | Three different names for same surface: 'Command Center', 'Commands' (nav), 'Command Catalog' (form header) | med |
| Catalog | Catalog/Catalog.tsx:21 | overflow | h-[calc(100vh-290px)] — brittle bracket value; breaks if surrounding layout changes | med |
| Catalog | CommandCatalogForm.tsx:217 | overflow | stdout `<pre>` has no max-h + overflow-y-auto — unbounded output expands pane | med |
| Chat | ChatTranscript.tsx:21 | ds-token | System message bubble uses border-amber-400/20, bg-amber-400/[0.06], text-amber-100/90 — not design tokens | med |
| Chat | ChatTranscript.tsx:38 | ds-token | Streaming indicator uses text-cyan-300 / bg-cyan-300 — not ds token | med |
| Chat | ChatAgentEventRow.tsx:18 | ds-token | Doubted item bar uses from-amber-400/40 to-amber-400/0 gradient — not a ds token | med |
| Chat | ChatAgentEventRow.tsx:19 | ds-token | Token item bar uses from-cyan-400/40 to-cyan-400/0 gradient — not a ds token | med |
| Chat | ModelBadge.tsx:25 | ds-token | Fallback 'model unknown' uses text-zinc-600 — not a ds token | low |
| Chat | ModelBadge.tsx:39 | ds-token | Token counts and cost use text-zinc-500 — not a ds token | low |
| Chat | ModelBadge.tsx:49 | ds-token | Detail popover background hardcoded hex bg-[#0b0b0e] — should use bg-bg-base | high |
| Chat | ModelBadge.tsx:50 | ds-token | Popover uses text-zinc-300 and border-white/10 — not ds tokens | med |
| Chat | PhaseChip.tsx:48 | ds-token | Intervention buttons use text-zinc-400 hover:text-zinc-200 — not ds tokens | low |
| Chat | PhaseChip.tsx:78 | ds-token | Done checkmark uses text-emerald-400 — not a ds token | med |
| Chat | ContextWindowMeter.tsx:23 | ds-token | Zone fill colors use arbitrary bracket oklch values — not ds tokens | med |
| Chat | ContextWindowMeter.tsx:28 | ds-token | Zone text colors use arbitrary bracket oklch values — not ds tokens | med |
| Chat | ChatSurface.tsx:117 | ds-token | Focus highlight adds hardcoded ring-amber-400 and ring-offset-zinc-950 via classList.add — not ds tokens | low |
| Chat | ChatSurface.tsx:186 | overflow | SecretaryToast fixed w-[480px] — may clip on narrow viewports | med |
| Chat | ChatTranscript.tsx:69 | overflow | Transcript Glass max-h-[40vh] may be very small on constrained layouts | low |
| Chat | ChatAgentEventRow.tsx:60 | a11y | Token group expand/collapse button has no aria-label — non-visual users lack stable label | med |
| Chat | ChatExecutionRail.tsx:221 | hierarchy | ContextWindowMeter always displays 0% (usedTokens hardcoded 0) — misleads user | high |
| Dashboard | Dashboard/Dashboard.tsx:364 | ds-token | Hardcoded hex bg-[#09090b]/80 for Mini-Map background — should use bg-bg-base/80 | med |
| Dashboard | Dashboard/Dashboard.tsx:194 | ds-token | text-amber-300 for alert icon — should use semantic token | low |
| Dashboard | Dashboard/AgentRow.tsx:99 | ds-token | Budget burn bar uses raw bg-rose-400 / bg-amber-400 / bg-emerald-400 — not semantic tokens | med |
| Dashboard | Dashboard/StreamCard.tsx:14 | ds-token | toneMap uses raw Tailwind gradient colors — bypasses token system | low |
| Dashboard | Dashboard/StreamCard.tsx:39 | ds-token | Doubt/Overrule buttons use direct amber Tailwind references — no ds-tokens | low |
| Dashboard | Dashboard/LudusBanner.tsx:12 | ds-token | stylingMap uses raw emerald/amber/cyan/rose palette classes — should use semantic status tokens | med |
| Dashboard | Dashboard/Dashboard.tsx:368 | a11y | Unicode bullet ⬤ in section header not wrapped with aria-hidden='true' — read aloud by screen readers | low |
| Dashboard | Dashboard/StreamCard.tsx:37 | hierarchy | Doubt/Overrule buttons (opacity-0, hover only) are dead — provide false affordance | high |
| Dashboard | Dashboard/Dashboard.tsx:324 | hierarchy | Customize controls (absolute top-right z-20) overlap KPI row on narrower viewports | med |
| Flow | AgentFlow.tsx:119 | hardcoded color | Progress bar uses from-violet-400 to-emerald-400 — bypasses theme layer | med |
| Flow | AgentFlow.tsx:159 | hardcoded color | Legend uses bg-cyan-400 / bg-emerald-400 inconsistent with SVG node tokens | med |
| Flow | AgentFlow.tsx:203 | hardcoded color | SVG edge stroke is bare rgba(255,255,255,0.06) literal — not theme-aware | low |
| Flow | AgentFlow.tsx:179 | hardcoded color | viz.cyan400 is hex literal (#22d3ee) baked into SVG gradient stop | low |
| Flow | AgentFlow.tsx:183 | hardcoded color | viz.white (#ffffff) hardcoded hex for radial gradient — breaks on non-dark backgrounds | low |
| Flow | AgentFlow.tsx:171 | a11y | Outer `<svg>` has no role, aria-label, or title element — unlabelled interactive SVG | high |
| Flow | AgentFlow.tsx:218 | a11y | SVG `<g>` nodes: CSS focus outlines unreliable in SVG context — `<rect>` stroke as focus indicator more robust | med |
| Flow | AgentFlow.tsx:288 | a11y | AgentInspector panel has no role='dialog' or aria-live region — focus not moved on selection | med |
| Flow | AgentFlow.tsx:80 | overflow | AgentInspector absolute right-5 top-5 w-72 can overflow on viewports <320px — parent clips silently | low |
| Flow | AgentFlow.tsx:151 | hierarchy | Header says 'Mind-Map · Agent Shards' but nav label is 'Agents' — inconsistent information scent | low |
| Flow | AgentFlow.tsx:138 | hardcoded value | SVG viewBox hardcoded 1200×640 / div h-[600px] — not responsive | med |
| Flow | AgentFlow.tsx:167 | hardcoded value | h-[600px] bracket Tailwind value for SVG wrapper — not responsive | low |
| Harness | HarnessRedirect.tsx:30 | bracket-tailwind | text-[10px] — bracket sizing not in standard Tailwind scale | low |
| Harness | HarnessRedirect.tsx:30 | a11y | aria-labelledby target (#harness-redirect-title) appears after the labelled `<section>` in DOM order — unconventional | med |
| Harness | HarnessRedirect.tsx:19 | a11y | aria-labelledby points to footnote text, not primary heading — accessible name is misleading footnote string | med |
| Harness | EmptyState.tsx:19 | amber-tailwind | DEFAULT_ICONS['no-connection'] uses text-amber-400 (flagged for completeness) | low |
| Harness | HarnessRedirect.tsx:19 | hierarchy | Surface tier listed as 'live_backend' but renders purely as redirect/empty-state tombstone | med |
| Memory | MemoryView.tsx:104 | ds-token | Score bar uses raw from-violet-400 to-emerald-400 — not semantic tokens | med |
| Memory | MemoryView.tsx:446 | ds-token | Pin-all button uses raw border-cyan-400/30 bg-cyan-400/10 text-cyan-300 — not ds tokens | med |
| Memory | MemoryView.tsx:515 | ds-token | Dirty shard card uses raw border-amber-400/30 bg-amber-400/[0.04] — not ds tokens | low |
| Memory | MemoryView.tsx:522 | ds-token | Dirty shard badge uses raw bg-amber-400/15 text-amber-300 — not ds tokens | low |
| Memory | MemoryView.tsx:427 | a11y | Recent recall row chevron icon (Icon.chevR) has no aria-hidden — read as unlabeled icon | low |
| Memory | MemoryView.tsx:390 | a11y | Section heading icon Icon.clock has no aria-hidden — read as unlabeled icon | low |
| Memory | MemoryView.tsx:358 | hierarchy | Recall button styled with brass/40 border only when query is non-empty — disabled state reduces discoverability | low |
| Mesh | MeshView.tsx:61 | ds-token | statusTone() returns hardcoded emerald/amber/rose Tailwind classes for node status | med |
| Mesh | MeshView.tsx:200 | ds-token | Control plane error banner hardcoded border-amber-400/20 bg-amber-400/5 text-amber-300/200 | med |
| Mesh | MeshView.tsx:303 | ds-token | Dispatch-disabled warning banner repeats same hardcoded amber classes | med |
| Mesh | MeshView.tsx:323 | ds-token | Select and inputs use bg-black/40 directly — should use ds surface variable | low |
| Mesh | MeshView.tsx:195 | a11y | Refresh button: visible text 'Refresh' is fine but may lose accessible name if icon-only breakpoint collapse occurs | low |
| Mesh | MeshView.tsx:367 | a11y | Dispatch button icon noted for completeness — visible text provides accessible name | low |
| Runs | RunsView.tsx:117 | hardcoded-color | success_rate conditional uses raw text-emerald-400, text-amber-400, text-rose-400 — bypasses Limes token layer | med |
| Runs | RunsView.tsx:250 | hardcoded-color | Error pre block uses border-rose-300/20 bg-rose-950/20 text-rose-200; 'no error' uses text-emerald-300 | med |
| Runs | RunsView.tsx:135 | hardcoded-color | Quality score uses text-brass (Limes token, acceptable) with font-mono font-bold — flagged for consistency | low |
| Runs | RunsView.tsx:181 | typography/hierarchy | Route-decision banner renders fields as plain text with '=' separators — no semantic label/value grouping | low |
| Runs | RunsView.tsx:178 | overflow | Root div h-full overflow-auto makes page scroll as one unit — scoreboard table unbounded before outer scroll | med |
| Runs | RunsView.tsx:149 | a11y | Workflow-name button uses aria-pressed but acts as selection control — should use role='option' or aria-selected | med |
| Runs | RunsView.tsx:227 | a11y | Run detail Glass panel has no aria-live region or aria-label — updates are silent to screen readers | med |
| Runs | RunsView.tsx:229 | typography/hierarchy | Run detail panel uses five distinct bracket text sizes (text-[11px], text-[10px]) — should use ds-* scale tokens | low |
| Settings | SettingsView.tsx:80 | ds-token | Toggle thumb uses hardcoded bg-[#fafafa] — not a Limes token | low |
| Settings | SettingsView.tsx:97 | ds-token | RangeInline uses rgb(var(--brass)) inline JS style — bypasses Tailwind purge | low |
| Settings | SettingsView.tsx:246 | ds-token | Mesh peer online indicator uses bg-emerald-400 — not a Limes semantic token | med |
| Settings | SettingsView.tsx:253 | ds-token | Trusted peer badge uses bg-emerald-400/15 text-emerald-300 raw classes (6+ occurrences) | med |
| Settings | SettingsView.tsx:341 | ds-token | Signing key icon uses text-amber-300 raw Tailwind — not a Limes token | low |
| Settings | SettingsView.tsx:557 | ds-token | Secrets required/set/missing badges use inconsistent raw colors vs tokens | med |
| Settings | SettingsView.tsx:215 | ds-token | Control-plane error banner uses border-amber-400/20 bg-amber-400/5 text-amber-300 raw Tailwind | low |
| Settings | SettingsView.tsx:1430 | a11y | Keybinds section is display-only but has no ARIA to indicate read-only — users expect interactive | low |
| Settings | SettingsView.tsx:303 | a11y | Signing key rotation uses window.prompt() for password — not accessible; no aria-*, no focus management | high |
| Settings | SettingsView.tsx:683 | a11y | Taxonomy group collapse toggle has no aria-expanded — screen readers cannot detect collapsed/expanded state | med |
| Settings | SettingsView.tsx:1447 | hierarchy | Gamification section omits Row component used everywhere else — visual inconsistency | low |
| Settings | SettingsView.tsx:92 | overflow | RangeInline fixed w-52 may overflow on narrow viewports with long suffixes | low |
| SkillsPlugins | SkillsPluginsView.tsx:442 | ds-token | 'ok' tone action buttons use hardcoded border-emerald-400/30 bg-emerald-400/10 text-emerald-300 | med |
| SkillsPlugins | SkillsPluginsView.tsx:443 | ds-token | 'danger' tone action buttons use hardcoded border-rose-400/30 bg-rose-400/10 text-rose-300 | med |
| SkillsPlugins | SkillsPluginsView.tsx:337 | hierarchy | Skill search results render with no action buttons (actions=[]) — dead end after search | high |
| SkillsPlugins | SkillsPluginsView.tsx:154 | hierarchy | Info buttons surface raw JSON via toast rather than structured detail panel — unusable | high |
| SkillsPlugins | SkillsPluginsView.tsx:163 | a11y | TabButton role='tab' not inside a div role='tablist' at same level — may confuse AT traversal | low |
| SkillsPlugins | SkillsPluginsView.tsx:454 | overflow | Plugin install_dir uses break-all — visually fragmented mono text on narrow columns | low |
| Tasks | TasksView.tsx:156 | ds-token | Priority select uses bg-zinc-900 text-zinc-100 — not ds tokens | med |
| Tasks | TasksView.tsx:190 | ds-token | text-amber-400 hardcoded for blocked badge | low |
| Tasks | TasksView.tsx:213 | ds-token | Overlap warning badge uses border-amber-400/30 bg-amber-400/10 text-amber-300 — not ds tokens | low |
| Tasks | TasksView.tsx:222 | ds-token | Remote mesh badge uses border-cyan-400/30 bg-cyan-400/10 text-cyan-300 — not ds tokens | low |
| Tasks | TaskComposer.tsx:21 | ds-token | Form container uses bg-white/[0.02] border-white/10 arbitrary bracket — not ds overlay tokens | med |
| Tasks | TaskComposer.tsx:27 | ds-token | Textarea uses text-zinc-100 placeholder:text-zinc-600 — not ds tokens | med |
| Tasks | TasksView.tsx:237 | a11y | Cancel-task Button has only Icon with title="Cancel task" but no aria-label — title not reliably exposed to screen readers | med |
| Tasks | TasksView.tsx:179 | a11y | Description span is interactive (onClick, cursor-pointer) but is a `<span>` not `<button>` — no role, no keyboard support | med |
| Tasks | TasksView.tsx:148 | overflow | Priority column fixed width=110 — may clip 'Background' option on some locales/font sizes | low |
| Tasks | TasksView.tsx:161 | overflow | Task ID column fixed width=80 — no flex room | low |
| Tasks | TasksView.tsx:235 | hierarchy | Only row-level action is destructive (Cancel) — no primary action, unclear hierarchy | med |

---

## Burn-down status (2026-07-02)

Resolution status for the WIRE / HIDE / TOAST-FIX rows above, as of the close of the "AI-first Plan 3: GUI intuitiveness" work (10 tasks, this branch). Confirmed by direct source inspection at time of writing, not just commit-message trust.

| Row(s) | Original decision | Resolution | Evidence |
|---|---|---|---|
| SkillsPlugins ×4 (`SkillsPluginsView.tsx:222,223,236,398` — Skill Info, Plugin Info ×2, View button) | WIRE + TOAST-FIX | **DONE** — all 4 now open a real `SkillDetailPanel` instead of a raw-JSON toast | `feat(gui-skills): structured SkillDetailPanel (replaces raw-JSON toast)` (96ea3f5080); `src/components/surfaces/SkillsPlugins/SkillDetailPanel.tsx` + `SkillDetailPanel.test.tsx` present and passing |
| SkillsPlugins (`SkillsPluginsView.tsx:346` — search-results rows, `actions=[]`) | WIRE | **DONE** — search-result rows now render Info + Install actions (`onSkillInfo`/`onInstallSkill`), no longer a dead end after search | Same commit as the row above (`96ea3f5080`); confirmed at `SkillsPluginsView.tsx:445-458` — closes out all 6 of the doc's original WIRE-tagged rows (line 6 tally) |
| Chat / `ChatExecutionRail.tsx:221` — ContextWindowMeter | HIDE (dead, `usedTokens` hardcoded 0) | **SUPERSEDED — WIRED** | `usedTokens={budget.used_tokens}` at `ChatExecutionRail.tsx:227` — real value, not hardcoded 0; `ContextWindowMeter.test.tsx` covers it |
| Dashboard / `StreamCard.tsx:39,44` — Doubt / Overrule buttons | HIDE (dead, no backend) | **SUPERSEDED — WIRED** | `StreamCard` takes `onDoubt?`/`onOverrule?` props wired to real handlers, no longer inert opacity-0-on-hover dead buttons |
| Settings / `SettingsView.tsx:1430` — Keybinds section | HIDE (dead, display-only) | **SUPERSEDED — WIRED** | Data-driven keybind dispatcher landed: `feat(gui-keybinds): action registry + chord/binding helpers` (da6cdf9c4d), `feat(gui-keybinds): data-driven useKeybinds dispatcher hook` (ee6429b455), `feat(gui-keybinds): App uses data-driven dispatcher` (02f53b1dbe); `useKeybinds.test.ts` passing |

These four rows were already resolved before AI-first Plan 3 began; this entry is a confirmation, not new work from this plan.

### Resolved by AI-first Plan 3 (this branch, Tasks 1–10)

| Item | Status | Notes |
|---|---|---|
| `needs-you` reachable from primary nav | **Confirmed done** | Task 1 of this plan was a no-op verification: a prior GUI-IA Reorg plan had already added `needs-you` to `src/lib/navigation.ts` (`runs: [..., 'needs-you', ...]`, label "Needs You"). No code change was needed in this plan; recorded here for the ledger's completeness. |
| `sub-agents` surface | **Wired-tree + dead-control hiding done (Tasks 2–3)** | `sub-agents` confirmed in nav SSOT (`agents: [..., 'sub-agents']`, label "Sub-Agents"). Task 2 fixed the tree data flow (dangling parent-ref logging, non-array payload guard — `7d99367bd2`) and Task 3 hid dead sub-agent controls that had no backend (`09f37b0a20`). |
| Consolidated attention/approvals polling | **New consolidation landed, no regression** | `useAttentionInbox.ts` is now the single poll for approvals + doubts + blocked tasks (`3955ab40d5`, `f561cab3c2`); `App.tsx`'s own direct `vox_pending_approvals` poll was removed as part of this consolidation (`c06466c912`). Verified via regression grep (Step 2, below) — `App.tsx` no longer appears among callers of `vox_pending_approvals`. |
| Structured intent panel in composer (Loquela) | **New feature landed** | `IntentPanel` (goal/constraints/effort/acceptance) wired into `Loquela.tsx`'s `send()`/submit path, composing `description` + `priority` from structured fields (`412c7cc198`, `bcce4b6e08`, `648a2d57f1`, `c6dc3b281f`). Covered by `IntentPanel.test.tsx` and `Loquela.test.tsx`. Not a triage-table row (feature addition, not a dead-control fix), noted here since it closes out this plan's scope.

**Known gap, found by the final whole-branch review, deliberately left out of scope:** `TasksView.tsx:51-89` runs its own independent `Promise.all([invoke('hopper_list'), feedbackList()])` poll and re-derives the same blocked/queued/in-progress lifecycle logic `useAttentionInbox.ts` now centralizes — a 4th feed the "collapse the three... into ONE" consolidation never touched (it predates this plan; `surfaceComponents.tsx` doesn't pass it an `attention` prop). It wasn't migrated here because `useAttentionInbox`'s public shape only exposes `blockedTasksCount` (a number) and `needsYou` (feedback rows), not the raw per-task `HopperTaskDto` list `TasksView` needs to build its full table (item_id/intent/priority/state) — migrating it would mean extending the hook's shape, a real interface change affecting its other consumers, not a drop-in prop swap like `needs-you`'s. Flagged as a follow-up rather than attempted as an unplanned addition to this plan's scope.

### Full-suite verification (Task 10, this date)

- `pnpm test` (vitest run) from `crates/vox-gui/ui`: **886 passed / 887 total**, 1 known pre-existing unrelated failure in `src/guards/ipcBoundaries.test.ts` (`CodeRabbitView.tsx` importing `invoke` directly outside the IPC hub layer — unrelated to this plan's scope, tracked separately). Same baseline as after Task 9's last commit; no drift.
- `pnpm typecheck`: clean, zero errors.
- Regression grep `grep -rn "vox_pending_approvals" crates/vox-gui/ui/src --include="*.tsx" --include="*.ts" | grep -v test`: matched expected file set exactly — `ApprovalsWidget.tsx` (doc comment only), `ApprovalsView.tsx` (real call), `InlineApprovals.tsx` (real call), `useAgentApprovals.ts` (doc comment + real call), `useAttentionInbox.ts` (real call). `App.tsx` absent, confirming no regression of the Task 5 consolidation.
