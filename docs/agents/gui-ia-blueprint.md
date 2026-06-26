---
title: "GUI IA Reorg Blueprint (pre-ratification)"
category: "Architecture SSOTs"
status: "PRE-RATIFICATION — HUMAN GATE REQUIRED (Phase J)"
generated: "2026-06-26"
source_dimensions:
  - graphify-out/gui-ia/structural-coverage.json
  - graphify-out/gui-ia/redundancy.json
  - graphify-out/gui-ia/structural-cohesion.json
  - graphify-out/gui-ia/semantic-clarity.json
  - graphify-out/gui-ia/journey-coherence.json
  - graphify-out/gui-ia/utility.json
  - graphify-out/gui-ia/new-nav-taxonomy.json
evidence_base: graphify-out/gui-coverage/joined-evidence.json (32 honesty surfaces joined with graphify coverage)
nav_ssot: crates/vox-gui/ui/src/lib/navigation.ts
registry_ssot: crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts
component_dispatch: crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx
---

# GUI IA Reorg Blueprint — pre-ratification

This is the synthesizer output of Plan 2 (Phase I). **No GUI code is changed.** Every row carries a
RECOMMENDED default decision so the human ratifier edits exceptions only. Plan 3 (the actual reorg) is
authored *after* the Phase J gate, scoped to surviving surfaces.

---

## 0. RATIFICATION (Gate J) — DECIDED 2026-06-26

The human ratifier **approved the recommended set** with three amendments. Status: **RATIFIED**.

**Approved as recommended (no change):**
- **Bundle 1** (Latin renames — except mens/populi, see Amendment A), **Bundle 3** (cluster merges: `claims`+`knowledge`→`scientia`; 4 activity clones→one Discovery surface), **Bundle 4** (`search`→`memory`), **Bundle 6** (phantom `review` + 5 parent-shell deletes) — ALL ACCEPTED.
- **Bundle 2:** `needs-you` + `sub-agents` **conditional wire-or-cut APPROVED** — Plan 3 wires `tool:vox_resolve_approval` (needs-you attention inbox) and `cmd:list_subagent_tree` (sub-agents); if wiring fails verification, they CUT. `activity`/`runs` moves accepted.

**Amendment A — `mens`/`populi` are NOT cut; they are the model train/run surfaces.** Their purpose is to **train/run the custom model(s)** (VoxMens). They need a GUI frontend **derived from the CLI** to establish **GUI/CLI parity**. Decision: **RENAME** (de-Latinize to clear labels) **+ WIRE to the model train/run CLI commands** — a new ADD workstream in Plan 3 that mirrors the CLI surface-for-surface. Converts two empty shells into real, honest surfaces. `dissent:structural:CUT-candidate` → **RESOLVED to WIRE.** (Plan 3 must find the CLI commands for VoxMens train/run and build the GUI to parity.)

**Amendment B — Settings consolidation + Policies-unification (elevates Bundles 5 & 7 to a first-class workstream).** `gamify` config → Settings (approved). Beyond that: **ALL settings for ALL subsections/pages must live in one well-organized Settings surface**, and Plan 3 should **evaluate unifying Settings + Policies** into one place — *only if it stays visually and conceptually clear what each is for* (Settings = user config; Policies = enforced rules). `gamify`'s visual elements/concepts may be applied app-wide (not confined to a single surface). Bundle 7 (settings CONDENSE) is therefore **no longer optional** — it becomes a Settings information-architecture pass: gather scattered per-surface settings, organize them, and resolve the Settings↔Policies relationship.

**Amendment C — `matrix`:** left at recommended default (fold the single routing-nudge into the chat execution rail); Plan 3 may instead rename→"Routing" and keep it a surface if folding proves awkward. (Not explicitly ratified; recommended default stands.)

**→ Plan 3 (execution) is AUTHORIZED**, scoped to: the ratified moves/merges/renames/cuts below, PLUS two amendment workstreams — (1) VoxMens model train/run GUI built from the CLI for parity; (2) a Settings consolidation + Settings/Policies-unification IA pass. The caveat completions (vox-gui Rust compile-verify path, Playwright proof, 109 visual/DS-token/a11y findings) fold in, scoped to surviving surfaces.

---

## 1. Summary header

**Counts per verb (recommended defaults, one decision per unit):**

| Verb | Count | Units |
|------|-------|-------|
| **CUT** | **6** | `review` (surface), `agents`/`commands`/`compute`/`workspace`/`knowledge` (registry parent-shell duplicates, ×5) |
| MERGE | 6 | `claims`→scientia · `search`→memory · activity-cluster (`archive-panel`,`discovery-inbox`,`discovery-review`,`activity`)→one Discovery surface (×4 absorbed, counted as 1 MERGE op) · `matrix`→chat rail |
| MOVE | 5 | `memory` (search→knowledge) · `activity` (orphan→knowledge/Discovery) · `runs` (parent-shell→named child) · `gamify` (agents→settings) · `console`-with-agent adjacency (flag) |
| RENAME | 5 | `scientia`→Findings · `oratio`→Voice · `mens`→(plain) · `populi`→(plain) · `runs` label "Runs & Approvals"→"Runs" |
| CONDENSE | 2 | `knowledge` group · `settings` (relocate journey-tail config) |
| EXPAND | 1 | `needs-you` (attention inbox) |
| ADD-to-nav | 2 | `needs-you`, `sub-agents` (re-add hollow shells WITH a wired command, else CUT) |
| **KEEP** | **~22** | all remaining surfaces + balanced groups |

> **6 destructive (CUT) decisions, shown first.** 1 of them (`review`) is a surface deletion;
> 5 are surface-registry parent-shell duplicates that mirror a child's component and are never an
> independent destination. **0 judgment-only CUTs** — every CUT rests on a structural fact (0 cmds +
> orphan-nav / parent-shell duplication).

---

## 2. Conflict resolution (applied mechanically)

Rules, applied per unit across the 7 dimensions:

1. **Evidence-type precedence:** `structural > audit > judgment`. A structural fact can **veto** a
   judgment; a judgment can **never** veto a structural fact.
2. **Tie-break:** prefer the least-destructive verb — `KEEP > RENAME/MOVE > CONDENSE > MERGE > CUT`.
3. **Destructive threshold:** a CUT or MERGE requires **≥2 dimensions agreeing OR one standalone
   structural fact**. Otherwise it is **downgraded to "flag, default KEEP"**.
4. **Dissent:** when a dimension's rec is overruled and unresolved, it is recorded inline as
   `dissent: <dim>:<rec>`.

**Resolutions that moved off a raw dimension rec:**

- `needs-you` — utility says **CUT** (structural: 0 cmds + orphan-nav, high). BUT a real
  `component_dir=NeedsYou` exists, it is **already wired** in `surfaceComponents.tsx` (`case 'needs-you'`)
  and registered `tier: 'live_backend'`. The structural "0 cmds" is contradicted by an audit/structural
  fact (live component + wired case). Threshold met for destructive only on the orphan-nav half, not the
  dead half → **downgrade CUT → EXPAND + ADD-to-nav**. `dissent: utility:CUT`.
- `sub-agents` — utility says **CUT** (high); structural-coverage **corrected to KEEP** (the prior MERGE
  rested on a falsely-attributed `list_subagent_tree`); new-nav says **MOVE-under-agents**. Real
  `component_dir=SubAgents` shell. Least-destructive structurally-supported verb = **ADD-to-nav (wire
  `cmd:list_subagent_tree`) OR CUT if it stays hollow**. Default **ADD-to-nav-conditional**.
  `dissent: utility:CUT`.
- `matrix` — semantic-clarity **RENAME→Routing** (low conf) vs journey-coherence **MERGE-into-chat-rail**
  (med). Both are non-destructive-ish; journey has the stronger structural basis (only entry path is
  `chat→matrix`, richness=1 cmd). MERGE preferred but flagged Group B (reversible). `dissent: semantic-clarity:RENAME(Routing)`.
- `oratio` — utility **MERGE** (redundant+thin, med) vs redundancy/cohesion **KEEP** (the only shared
  command is the read-only `list_model_cards`; `start_mic_capture` is unique → false redundancy peer,
  structural). Structural KEEP **vetoes** the utility MERGE (which was computed from a god-command peer
  edge). → **RENAME→Voice, KEEP as surface**. `dissent: utility:MERGE`.
- `research` / `repository` — utility **MERGE** (redundant+thin) vs redundancy/cohesion **KEEP** (peer
  edges are pure `cmd:execute_command` artifacts; distinct components + distinct real commands). Structural
  KEEP **vetoes**. → **KEEP**. `dissent: utility:MERGE` (×2).
- `claims` / `scientia` / `knowledge` — utility **KEEP** ("rich, MERGE only if dim-2 confirms") vs
  redundancy/cohesion/journey/semantic **MERGE** (identical 12-cmd set + identical `component_dir=Scientia`,
  structural, ≥3 dims). The dim-2 confirmation utility asked for **exists** → MERGE clears the threshold.
  → **MERGE** (`claims`+`knowledge`-surface → `scientia`).
- `compute`/`models`, `workspace`/`console`, `agents`-shell — parent↔default-child identity artifacts.
  Redundancy/cohesion **KEEP the live child, CUT only the registry parent-shell** (structural). Surface
  KEEP; registry-shell CUT.

---

## 3. Decision table (grouped by ratification tier)

Coverage shorthand: `surfaced` = reachable+wired, `orphan` = orphan-nav, `deadend` = dead-end tool/cmd
edges; `cN` = command_count.

### Group C — CUT + any judgment-only destructive (explicit per-item confirm) — shown FIRST

| Unit | Kind | Current location | Coverage | Evidence basis | DECISION | Rationale |
|------|------|-----------------|----------|----------------|----------|-----------|
| `review` | surface | registry only (navGroup knowledge, parent null) | orphan · c0 · deadend0 | structural (3 dims: struct-cov, utility, new-nav) | **CUT** | Dead (0 cmds, `component_dir=null` — no component) AND orphan-nav. Phantom registry entry; nothing to merge. Highest-confidence cut. |
| `agents` (surface-shell) | surface | registry shell, mirrors `dashboard` (`component_dir=Dashboard`, c0) | reachable-as-parent · c0 | structural (new-nav) | **CUT (registry shell)** | Duplicates the `agents` nav parent + live `dashboard` child. Parent key lives in `navigation.ts` only; the registry shell is redundant. |
| `commands` (surface-shell) | surface | registry shell | reachable-as-parent · c0 | structural (new-nav) | **CUT (registry shell)** | Parent-shell duplicate; `catalog` is the live child. |
| `compute` (surface-shell) | surface | registry shell, mirrors `models` (`component_dir=Models`, c4) | reachable-as-parent · c4(inherited) | structural (redundancy, new-nav) | **CUT (registry shell)** | Parent↔default-child identity artifact; `models` is the real surface. |
| `workspace` (surface-shell) | surface | registry shell, mirrors `console` (`component_dir=Console`, c8) | reachable-as-parent · c8(inherited) | structural (redundancy, cohesion) | **CUT (registry shell)** | Parent↔default-child identity artifact; `console` is the real surface. |
| `knowledge` (surface-shell) | surface | registry shell, mirrors `scientia` (`component_dir=Scientia`, c12) | reachable-as-parent · c12(inherited) | structural (redundancy, cohesion) | **CUT (registry shell)** → folds into the scientia MERGE | Parent-shell duplicate of the Scientia 12-cmd surface; absorbed by the `claims`→scientia MERGE below. |

> **needs-you** and **sub-agents** were downgraded *out* of Group C (utility recommended CUT) to
> **ADD-to-nav-conditional** (Group B) per §2. Their `dissent: utility:CUT` is shown there.

### Group B — MOVE / MERGE / CONDENSE / EXPAND / ADD (opt-in review)

| Unit | Kind | Current location | Coverage | Evidence basis | DECISION | Rationale |
|------|------|-----------------|----------|----------------|----------|-----------|
| `claims` | surface | knowledge (parent knowledge) | surfaced · c12 · redundant | structural (redundancy+cohesion+journey+semantic, 4 dims) | **MERGE → scientia** | Identical 12-cmd set + identical `component_dir=Scientia` as scientia/knowledge. Fold to one surface; claims becomes a filter/tab. |
| `search` | surface | top-level parent (1 child: memory) | surfaced · c2 · redundant | structural (redundancy+cohesion+journey, 3 dims) | **MERGE → memory** | Degenerate 1-child group; shares `vox_search_query`+`open_locator` with its only child. Retire the `search` parent. |
| activity-cluster: `archive-panel`, `discovery-inbox`, `discovery-review`, `activity` | surface ×4 | knowledge (×3) + orphan (activity) | activity=orphan; others surfaced · c1 each · all redundant | structural (redundancy+cohesion+journey+struct-cov, 4 dims) | **MERGE → one "Discovery" surface** (filter tabs: Inbox/Review/Archive) | One `component_dir=Activity` + one `cmd:activity_query`, surfaced under 4 names distinguished only by filter. Preserve inbox/review intents as named presets. |
| `memory` | surface | search (parent search) | surfaced · c5 | structural (new-nav, high) | **MOVE: search→knowledge** | Reparent the live recall surface into Knowledge; kills the Search↔Memory collision. |
| `activity` | surface | orphan (no nav) | orphan · c1 | structural (struct-cov+cohesion+new-nav) | **MOVE: orphan→knowledge** (becomes canonical Discovery surface) | The only activity_query peer left out of nav; make it the reachable Discovery home. |
| `runs` | surface | live but only reachable as parent fallthrough | orphan-as-tab · c1 (`get_gui_run`) | structural+judgment (new-nav, semantic) | **MOVE: add named `runs` child under Runs parent** | Most-wired surface in the group is the least reachable; add `PARENT_CHILD_MAP.runs:{parent:'runs',child:'runs'}`. |
| `gamify` | surface | agents | surfaced · c6 | judgment (new-nav+cohesion+journey) | **MOVE: agents→settings** | Opt-in immersive mode (default off) is config-adjacent; tightens Agents to "watch/steer the swarm". |
| `matrix` | surface | agents | surfaced · c1 (`nudge_routing_intention`) | structural+judgment (journey, med) | **MERGE → chat execution rail** | One command, only entry path is `chat→matrix`. Surface the nudge inline in chat. `dissent: semantic-clarity:RENAME(Routing)`. |
| `needs-you` | surface | orphan (wired component, registry live_backend) | orphan · c0 · component=NeedsYou | structural+audit (journey EXPAND, new-nav MOVE) | **EXPAND + ADD-to-nav** (attention inbox under Runs); absorb `approvals` inline-resolve + wire dead doubt/overrule | Real wired component; the unifying home for approve/doubt/overrule. `dissent: utility:CUT` (overruled by live-component fact). |
| `sub-agents` | surface | orphan (component SubAgents, c0) | orphan · c0 · component=SubAgents | structural+judgment (struct-cov KEEP, new-nav MOVE) | **ADD-to-nav under Agents — conditional: wire `cmd:list_subagent_tree` else CUT** | Real shell, dead today. `dissent: utility:CUT`. |
| `console` (agent-scoped) | surface | workspace | surfaced · c8 | structural+judgment (journey, low) | **MOVE adjacency (flag only)** | `dashboard→console` agent drill-down crosses agents→workspace. Optional agent-scoped console panel. Low confidence; default KEEP placement. |
| `knowledge` group | nav-group | — | — | structural+judgment (redundancy CONDENSE, semantic CONDENSE) | **CONDENSE** | After scientia+Discovery merges & memory absorb: {Memory, Findings(scientia), Research, Discovery, Publications} ≈ 5. |
| `settings` | surface | top-level | surfaced · c17 | judgment (journey, low) | **CONDENSE (flag)** | Relocate journey-tail config (routing/selection→models; gamify settings→gamify; trusted-nodes→mesh). Low confidence; opt-in. |

### Group A — RENAME / KEEP (opt-out default-accept)

| Unit | Kind | Current location | Coverage | Evidence basis | DECISION | Rationale |
|------|------|-----------------|----------|----------------|----------|-----------|
| `scientia` | surface | knowledge (default child) | surfaced · c12 | judgment (semantic+new-nav, high) | **RENAME label → "Findings"/"Library"** (key unchanged) | "Knowledge→Scientia" is Latin-for-the-same-word. Label-only diff. |
| `oratio` | surface | compute | surfaced · c2 | structural+judgment (semantic high) | **RENAME → "Voice"/"Speech"** (key unchanged); also align `component_dir Loquela` | Double Latin naming (nav Oratio vs code Loquela). `dissent: utility:MERGE` (vetoed — false peer). |
| `mens` | surface | compute | surfaced-but-empty · c0 | structural+judgment (semantic high, new-nav low) | **RENAME (de-Latinize) — conditional KEEP; CUT if stays hollow** | Latin + 0 cmds + null component. `dissent: structural:CUT-candidate` (deferred to ratifier). |
| `populi` | surface | compute | surfaced-but-empty · c0 | structural+judgment (semantic high, new-nav low) | **RENAME (de-Latinize) — conditional KEEP; CUT if stays hollow** | Same as mens. `dissent: structural:CUT-candidate`. |
| `runs` (label) | nav-group | "Runs & Approvals" | — | structural+judgment (semantic high) | **RENAME label → "Runs"** | Group contains Runs(new child)+Approvals+Policies; drop the awkward conjunction. |
| `harness` | surface | workspace | surfaced · c0 (redirect stub) | structural+judgment (semantic med) | **KEEP (flag: label over-promises)** | Redirect-only; rename deferred, no named disqualifier. |
| `gamify` (label) | surface | — | — | judgment (semantic low) | **KEEP (flag: verb label)** | "Gamify" reads as a verb; optional rename to "Quests/Arena". Low priority. |
| `flow`,`console`,`catalog`,`repository`,`browser`,`tasks`,`dashboard`,`chat`,`models`,`mesh`,`policies`,`publications`,`coverage`,`skills`,`approvals`,`research` | surface ×16 | various | mixed | structural+judgment | **KEEP** | No named disqualifier; structurally distinct or structurally-vetoed against destructive recs. |
| `Agents`,`Workspace`,`Commands`,`Compute`,`Runs` (groups) | nav-group ×5 | — | — | structural (cohesion KEEP) | **KEEP** | Coherent containers; member-level edits handled above. |

---

## 4. Per-verb executable fields (for Plan 3)

### CUT

| view_key removed | nav entries to delete | `#view=` redirect target | tests to update | grep handle |
|---|---|---|---|---|
| `review` | none (not in `PARENT_CHILD_MAP`) — delete registry row | `#view=review` → `#view=scientia` (nearest knowledge surface) | `surfaceRegistry.generated.ts` regen; `surfaceComponents.tsx` has no `case 'review'` (already absent) | `grep -rn "'review'\|\"review\"\|viewKey: 'review'" crates/vox-gui/ui/src` |
| `agents` (registry shell) | none in nav (key stays as TOP_LEVEL_VIEW) — delete *registry surface row only* | n/a (parent key survives in `navigation.ts`) | `surfaceRegistry.generated.ts` regen | `grep -n "viewKey: 'agents'" crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts` |
| `commands` (shell) | none — delete registry row only | n/a | regen | `grep -n "viewKey: 'commands'" .../surfaceRegistry.generated.ts` |
| `compute` (shell) | none — delete registry row only | n/a | regen | `grep -n "viewKey: 'compute'" .../surfaceRegistry.generated.ts` |
| `workspace` (shell) | none — delete registry row only | n/a | regen | `grep -n "viewKey: 'workspace'" .../surfaceRegistry.generated.ts` |
| `knowledge` (shell) | none — delete registry row only (folds into scientia MERGE) | `#view=knowledge` → `#view=scientia` | regen; `navigation.test.ts` | `grep -n "viewKey: 'knowledge'" .../surfaceRegistry.generated.ts` |

### MERGE

| absorber | absorbed | features/cmds that move vs drop | absorbed view_key → redirect | nav keys retired | component disposition |
|---|---|---|---|---|---|
| `scientia` (→ relabel Findings) | `claims`, `knowledge`(surface) | all 12 cmds shared (identical) → **none move/drop**; `claims` becomes a claim-review filter/tab | `claims`→`scientia`; `knowledge`→`scientia` | `PARENT_CHILD_MAP.claims` removed; `knowledge` stays a *group key* only | keep `Scientia` component; delete `Claims`/`knowledge`-shell rows |
| `memory` | `search` | `vox_search_query`,`open_locator` already in memory → **none drop**; `search`'s extra `vox_search_query` is dup | `search`→`memory` | `search` removed from `TOP_LEVEL_VIEWS`; `DEFAULT_CHILD_BY_PARENT.search` removed | keep `Memory` component; delete `Search` registry row |
| `activity` (Discovery) | `archive-panel`,`discovery-inbox`,`discovery-review` | all share `cmd:activity_query` → **none drop**; inbox/review/archive become **named filter presets** | `archive-panel`/`discovery-inbox`/`discovery-review`→`activity` | their 3 `PARENT_CHILD_MAP` knowledge children retired | keep one `Activity` component; the 3 absorbed used the *same* component already (no orphan components) |
| chat execution rail | `matrix` | `nudge_routing_intention` → moves inline to ChatExecutionRail intent button | `matrix`→`chat` | `PARENT_CHILD_MAP.matrix` removed | `Matrix` component retired or embedded as a chat-rail control |

### MOVE

| from→to parent | PARENT_CHILD_MAP edit | DEFAULT_CHILD_BY_PARENT impact | breadcrumb effect |
|---|---|---|---|
| `memory`: search→knowledge | `memory:{parent:'knowledge',child:'memory'}` | none (memory was never a default child of knowledge); remove `DEFAULT_CHILD_BY_PARENT.search` | "Search › Memory" → "Knowledge › Memory" |
| `activity`: orphan→knowledge | **ADD** `activity:{parent:'knowledge',child:'activity'}` | none | new "Knowledge › Discovery" |
| `runs`: parent-shell→named child | **ADD** `runs:{parent:'runs',child:'runs'}`; change `DEFAULT_CHILD_BY_PARENT.runs` from `'approvals'` to `'runs'` | `runs` parent now lands on the live scoreboard, not approvals | "Runs & Approvals › Approvals" → "Runs › Runs" |
| `gamify`: agents→settings | `gamify:{parent:'settings',child:'gamify'}` | none (settings default stays `settings`) | "Agents › Gamify" → "Settings › Gamify" |
| `console` adjacency (flag) | no map edit; optional agent-scoped panel | none | unchanged |

### RENAME

| old key (keep) | old→new label | NAV_LABELS diff | key changes? | docs refs |
|---|---|---|---|---|
| `scientia` | "Scientia" → "Findings" | `NAV_LABELS.scientia: 'Findings'` | **no** (label-only, no redirect) | `grep -rn "Scientia" crates/vox-gui/ui/src docs/` |
| `oratio` | "Oratio" → "Voice" | `NAV_LABELS.oratio: 'Voice'` | **no** label-only; *optional* `component_dir` rename Loquela→Voice is a code refactor (own commit) | `grep -rn "Oratio\|Loquela" crates/vox-gui/ui/src` |
| `mens` | "Mens" → plain (e.g. "Reasoning") | `NAV_LABELS.mens: '…'` | **no** | `grep -rn "'mens'\|\"mens\"\|Mens" crates/vox-gui/ui/src` |
| `populi` | "Populi" → plain (e.g. "Crowd") | `NAV_LABELS.populi: '…'` | **no** | `grep -rn "'populi'\|Populi" crates/vox-gui/ui/src` |
| `runs` (group) | "Runs & Approvals" → "Runs" | `NAV_LABELS.runs: 'Runs'` | **no** | `grep -n "Runs & Approvals" crates/vox-gui/ui/src/lib/navigation.ts` |

### ADD

| proposed view_key | parent | real `cmd:`/`tool:` node it surfaces (REQUIRED) | nearest component to reuse |
|---|---|---|---|
| `runs` (as named child) | `runs` | `cmd:get_gui_run` (+ `list_gui_runs`,`get_model_scoreboard`,`get_routing_summary_live` from behavioral findings) | `RunsView` (already in `surfaceComponents.tsx` `case 'runs'`) |
| `needs-you` (re-add to nav) | `runs` | **MUST WIRE** — candidate `tool:vox_resolve_approval` (the live approve/reject resolve from Dashboard AgentRow) routed through NeedsYou | `NeedsYouSurface` (already wired `case 'needs-you'`) |
| `sub-agents` (conditional) | `agents` | **MUST WIRE** `cmd:list_subagent_tree` (status=surfaced but currently no edge to this surface) — else CUT | `SubAgents` (component exists, `component_dir=SubAgents`) |
| `activity` (Discovery) | `knowledge` | `cmd:activity_query` | `ActivitySurface` (already wired `case 'activity'`) |

> **ADD honesty gate:** `needs-you` and `sub-agents` may only be ADDed if Plan 3 wires a cited command;
> if the wiring fails verification they revert to **CUT** (their utility-dimension default).

---

## 5. Migration ledger (consolidated — `#view=` deep-links MUST NOT silently break)

`parseViewFromLocation` currently returns the raw key from `#view=` / `?view=`; any removed key must be
remapped to its nearest surviving parent so old deep-links/bookmarks resolve.

| old view_key | new key \| null | parseViewFromLocation fallback | deprecation policy | asserting spec/test (grep handle) |
|---|---|---|---|---|
| `search` | `memory` | `#view=search` → resolve to `memory` | **silent alias one release**, then hard-remove | `navigation.test.ts:19` asserts `?view=memory`; `surfaceRegistry.generated.ts:37` |
| `claims` | `scientia` | → `scientia` | silent alias one release | `surfaceRegistry.generated.ts:19`; `PARENT_CHILD_MAP.claims` in `navigation.ts:21` |
| `knowledge` (surface) | `scientia` (group key survives) | parent key resolves via `DEFAULT_CHILD_BY_PARENT.knowledge` → set to `scientia` (or `memory`) | group key kept; surface row hard-removed | `navigation.ts:18,43`; `surfaceRegistry` |
| `archive-panel` | `activity` | → `activity` (preset=archive) | silent alias one release | `PARENT_CHILD_MAP.'archive-panel'` `navigation.ts:31`; `surfaceRegistry` |
| `discovery-inbox` | `activity` | → `activity` (preset=inbox) | silent alias one release | `navigation.ts:30`; `surfaceRegistry` |
| `discovery-review` | `activity` | → `activity` (preset=review) | silent alias one release | `navigation.ts:20`; `surfaceRegistry.generated.ts:26` |
| `matrix` | `chat` | → `chat` (rail intent control) | silent alias one release | `PARENT_CHILD_MAP.matrix` `navigation.ts:7`; `surfaceComponents.tsx:111` |
| `review` | `scientia` | → `scientia` | **hard-remove** (phantom, no component) | `surfaceRegistry.generated.ts:35` |
| `agents`/`commands`/`compute`/`workspace` (registry shells) | unchanged group keys | parent keys keep resolving via `DEFAULT_CHILD_BY_PARENT` | registry rows hard-removed; keys survive | `navigation.ts:35-45` `TOP_LEVEL_VIEWS`/`DEFAULT_CHILD_BY_PARENT` |
| `gamify` | `gamify` (reparented) | key unchanged; `PARENT_CHILD_MAP.gamify.parent` agents→settings | no alias needed (key stable) | `navigation.ts:29`; `surfaceComponents.tsx:136` |
| `oratio`/`scientia`/`mens`/`populi`/`runs`-label | keys unchanged | n/a (label-only) | n/a | `NAV_LABELS` `navigation.ts:62-99`; `surfaceRegistry` navLabel fields |

**Key-affecting decisions (need redirect + call-site grep):** `search`, `claims`, `knowledge`(surface),
`archive-panel`, `discovery-inbox`, `discovery-review`, `matrix`, `review`.
**Label-only (no redirect):** `scientia`, `oratio`, `mens`, `populi`, `runs`-label, `gamify`(reparent).

**Three SSOT assertion sites Plan 3 must keep in sync** for every key/label change:
1. `crates/vox-gui/ui/src/lib/navigation.ts` (`PARENT_CHILD_MAP`, `TOP_LEVEL_VIEWS`, `DEFAULT_CHILD_BY_PARENT`, `NAV_LABELS`)
2. `crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts` (regenerated — do not hand-edit; fix the generator)
3. `crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx` (`case '<viewKey>'` dispatch) + `navigation.test.ts`

---

## 6. New-nav before/after tree

### BEFORE (current `navigation.ts`)

```
Chat        → chat
Agents      → dashboard, flow, matrix, tasks, gamify
Runs        → approvals, policies            (label "Runs & Approvals"; runs surface ORPHAN)
Workspace   → repository, browser, harness, console
Commands    → catalog, skills
Search      → memory                          (1-child group; Search↔Memory collision)
Knowledge   → scientia, research, discovery-review, claims, publications,
              discovery-inbox, archive-panel  (7 children; 5 are clones in 2 clusters)
Compute     → models, mens, populi, oratio, mesh   (3 Latin labels; mens/populi empty)
Settings    → settings, coverage
ORPHANS (in registry, not in nav): activity, needs-you, review, sub-agents, runs
```

### AFTER (proposed)

```
Chat        → chat                                              [+ inline routing-intent rail (matrix folded)]
Agents      → dashboard, flow, tasks, sub-agents?               rationale: "watch/steer the swarm"; drop gamify, fold matrix
Runs        → runs, approvals, policies, needs-you              rationale: "review outcomes + gate them"; runs+attention now first-class
Workspace   → console, repository, browser, harness            rationale: "act on the dev environment"; KEEP (only cohesive multi-member group)
Commands    → catalog, skills                                   rationale: "browse + run commands/skills"; KEEP
Knowledge   → memory, scientia(→Findings), research,            rationale: "find/recall/review what the system knows"; absorbs Search;
              discovery(4 clones→1), publications                          4 activity clones condensed to 1 Discovery
Compute     → models, mesh, oratio(→Voice), mens?, populi?      rationale: "run/route/inspect models + node mesh"; Latin renamed
Settings    → settings, coverage, gamify                        rationale: "configure the system"; gamify parked here (opt-in)
```

**Per-group one-line rationale + agreement/migration_cost** (every multi-member group crosses singleton
Leiden communities — the GUI graph is fully fragmented, so **migration_cost is structurally LOW** for all;
no proposed group splits a tight code community):

| Group | rationale | agreement / migration_cost |
|---|---|---|
| Agents | watch/steer the running swarm; gamify out, matrix folded | crosses-community(low) — no shared code between members |
| Runs | review outcomes + gate them; runs & needs-you promoted | crosses-community(low) |
| Workspace | act on the dev environment | crosses-community(low) — *only structurally cohesive group* (distinct command sets per member) |
| Commands | browse + run commands/skills | crosses-community(low) |
| Knowledge | find/recall/review what the system knows; ~5 after condense | crosses-community(low) — absorbs `search` + condenses 4 activity clones |
| Compute | run/route/inspect models + mesh | crosses-community(low) |
| Settings | configure the system | crosses-community(low) |

**Smells resolved:** orphan-nav (activity/needs-you/runs wired in; review/sub-agents resolved) · Search→Memory
collision (search retired, memory under Knowledge) · Latin labels (scientia/oratio/mens/populi renamed) ·
lopsided Knowledge (7→~5) and degenerate 1-child Search (removed).

---

## 7. Themed bundles (accept/edit as a unit at the gate)

1. **Retire Latin labels — 4 renames (Group A, label-only, no redirects).**
   `scientia→Findings`, `oratio→Voice`, `mens→(plain)`, `populi→(plain)`. Lowest blast radius.

2. **Fold orphan-nav surfaces — 3 moves + 1 expand (Group B).**
   `activity`→Knowledge/Discovery, `runs`→named child under Runs, `needs-you`→EXPAND under Runs,
   `sub-agents`→conditional ADD under Agents.

3. **Collapse exact duplicate clusters — 2 MERGE ops (Group B).**
   `claims`+`knowledge`-surface → `scientia` (one Knowledge surface); 4 activity clones → 1 Discovery
   surface with Inbox/Review/Archive presets.

4. **Kill the Search↔Memory collision — 1 merge + 1 move (Group B).**
   Retire `search` top-level; `memory` reparented under Knowledge as the single recall surface.

5. **Tighten Agents — 1 move + 1 merge (Group B).**
   `gamify`→Settings; `matrix`→chat execution rail.

6. **Delete registry phantoms/shells — 6 CUTs (Group C, structural).**
   `review` (phantom) + 5 parent-shell duplicates (`agents`/`commands`/`compute`/`workspace`/`knowledge`
   surface rows). All structural; no judgment-only cuts.

7. **Optional config relocation — CONDENSE settings (Group B, low confidence, flag).**
   Move routing/selection→Models, gamify settings→Gamify, trusted-nodes→Mesh. Defer if contentious.

---

## Unresolved dissents (carried to the gate)

- `needs-you` — **`dissent: utility:CUT`** (overruled: live wired component + registry `live_backend`
  veto the "dead" half; default EXPAND+ADD, reverts to CUT only if wiring fails).
- `sub-agents` — **`dissent: utility:CUT`** (default conditional-ADD; CUT if `list_subagent_tree` not wired).
- `mens`, `populi` — **`dissent: structural:CUT-candidate`** (0 cmds + null component; new-nav defers the
  cut, only de-Latinizes; ratifier decides RENAME-keep vs CUT).
- `oratio` — **`dissent: utility:MERGE`** (vetoed by structural: false redundancy peer via read-only
  `list_model_cards`; default RENAME+KEEP).
- `research`, `repository` — **`dissent: utility:MERGE` (×2)** (vetoed by structural: peer edges are
  `cmd:execute_command` god-command artifacts; default KEEP).
- `matrix` — **`dissent: semantic-clarity:RENAME(Routing)`** (journey MERGE-into-chat preferred; if the
  ratifier keeps it a surface, apply the Routing rename instead).
- `console`/`settings` MOVE/CONDENSE — low confidence, flagged opt-in; default KEEP placement.
