---
category: "Architecture SSOTs"
title: "Settings Consolidation + Settings/Policies Unification (GUI-IA Amendment B)"
date: 2026-06-26
status: design
---

# Settings Consolidation + Settings/Policies Unification

Amendment B of the ratified GUI-IA blueprint. Mandate: ALL settings for ALL
subsections/pages live in ONE well-organized Settings surface; gamify config
moves to Settings; and EVALUATE unifying Settings + Policies into one place —
**only if** it stays visually and conceptually clear what each is for
(Settings = user configuration; Policies = enforced rules).

This is a design doc. It changes no code. Two decisions need the human's call
(see §6).

---

## 1. Scattered-settings inventory (consolidation scope)

Settings outside the Settings surface today. The persistence command/key is the
SSOT and **does not move** — only the *control's location* moves into Settings.

### Backend-persisted config (Tauri command) — TRUE settings, should consolidate

| Surface / file | Setting | Persistence | Notes |
|---|---|---|---|
| `Models/ModelsView.tsx` | Active model selection | `invoke('set_active_model', { modelId })` (L72) | Belongs in **Models & Routing** |
| `Repository/RepositoryView.tsx` | VCS isolation strategy (default + per-agent) | `invoke('set_vcs_isolation_strategy', …)` (L57) | The *default* belongs in Settings; per-agent override is contextual (see §5 "lives where the user expects it") |
| `Memory/MemoryView.tsx` | Auto-recall toggle | gui-pref `gui.memory.autoRecall` (L197/205) | Belongs in a **Memory & Context** domain |

### localStorage UI/layout prefs — mixed (some consolidate, some stay)

| Surface / file | Setting | Persistence key | Disposition |
|---|---|---|---|
| `Dashboard/Dashboard.tsx` | Widget grid layout | `SHELL_PREFERENCE_KEYS.dashboardLayout` (L16/82) | **Stays in place** — direct-manipulation layout (drag widgets), not a config form |
| `Chat/ChatExecutionRail.tsx` | Execution rail collapsed | `gui.chat.execution_rail_collapsed.v1` | **Stays** — ephemeral per-pane collapse, not a preference |
| `Chat/ChatSessionRail.tsx` | Sessions rail collapsed | `gui.chat.sessions_collapsed.v1` | **Stays** — ditto |
| `Console/DiscoveryRail.tsx` | Discovery rail collapsed | `gui.console.discovery_rail_collapsed.v1` | **Stays** — ditto |
| `Policies/PoliciesView.tsx` | Rail collapsed / group collapse | `vox_policy_rail_collapsed`, `vox_policy_groups` | **Stays** — view state |

### Already in the Settings surface (the ~17-command baseline, for completeness)

`set_orchestrator_config` (concurrency, budget cap, doubt threshold, isolation,
auto-doubt, auto-budget, scaling fields), `set_llm_config`, routing weights
(`setRoutingPriority` → `VOX_AUTO_ROUTING_PRIORITY`), `PriorityChainEditor`,
`get/set_user_config` (RuntimeConfig catalog), `trust_mesh_node` /
`untrust_mesh_node`, `rotate_signing_key` + `sign` gate, secrets
(`set/remove_secret`, `import_env`, `migrate_auth_store`), telemetry
(`gui.telemetry`), keybinds (`gui.keybinds`), theme (`gui.theme`), HUD tiles,
`set_gamify_settings`.

### Consolidation verdict

**A "setting" = a deliberate, persisted configuration choice the user expects to
find in one place.** A "view-state" = ephemeral collapse/layout the user
manipulates in-context and never hunts for. Only the first class migrates.

**Migrates into Settings:** active model (Models), VCS isolation default
(Repository), memory auto-recall (Memory). Plus gamify config is already in
Settings — the mandate only requires it *stay* there and be grouped correctly.

**Stays in place (with rationale, not omission):** dashboard layout, all rail
collapses, policy view-state. Forcing these into Settings would *violate* the
"lives where the user expects it" principle — nobody opens Settings to collapse a
sidebar.

---

## 2. Current Settings surface (`Settings/SettingsView.tsx`)

A two-pane master-detail: a left nav (`SECTIONS`) + a right content `Glass`,
switched by `section === …`. It already has **search-within-settings**
(`searchSettings` over `settingsIndex.ts`, including registry-derived
`GENERATED_SETTINGS_INDEX` from `vox ci config-gui-codegen`) and omni-search deep
links (`vox_settings_seed`).

13 flat sections today: `orchestrator`, `scaling`, `llm`, `routing`, `runtime`,
`mesh`, `signing`, `secrets`, `telemetry`, `keybinds`, `theme`, `display`,
`gamify`. Keybinds is the newest (click-to-capture chord, `gui.keybinds`,
`ACTION_REGISTRY` + reset).

**Design critique of the current surface:** 13 flat peers is already near the
"findability cliff." Adding the §1 migrations + future registry keys pushes
toward a 20+ section monster with weak information scent (e.g. is "OpenRouter
concurrency" under `llm`, `routing`, or `runtime`? Today it's split across all
three). The flat list has no grouping logic — `signing` and `secrets` and `mesh`
are all "trust/security" but sit between unrelated items.

---

## 3. Current Policies surface (`Policies/`) + backend

**What Policies IS:** a read view + (newly wired) enable/disable + edit over the
**policy-registry SSOT** (`contracts/policy/policy-registry.v1.yaml`). It is
*governance*, not preference.

- UI: master-detail. Left rail = multi-**branch** selector + worst-status master
  badge + "needs attention" list + collapsible group tree (sorted worst-first).
  Right pane = policy detail (description, domain, severity,
  blocking/non-blocking, protected, runsOn, origin; source kind/ref/detail;
  per-branch last-run status).
- Controls: **Edit** (`policy_edit` → `.vox/policy-overrides.json`) and
  **Enable/Disable** (`policy_set_enabled` → same overrides file). Both disabled
  when `protected: true`.
- Backend commands (`commands/policy.rs`): `policy_list`, `policy_show`,
  `policy_status(branches)`, `policy_set_enabled`, `policy_edit`,
  `list_branches`.
- A policy: `id`, `domain` (ci-gate / audit-check / code-audit-rule / arch-rule /
  gui-design-rule / …), `severity`, `blocking`, `runs_on`, `source{kind,ref}`,
  `default_enabled`, `protected`, `origin`. Status per branch is `pass | fail |
  warn | not_run` from `.vox/policy-status/<branch>.json` (never faked green).

**The bright line:** Settings = *reversible user preferences* (effect: changes
how the app behaves for you, instantly, undoably). Policies = *enforced
governance rules/gates* (effect: blocks commits/merges/dispatch, has CI run
status, branch-scoped, some are `protected` and you literally cannot edit them).
A policy's *enable/disable* is the only place the two genuinely touch — and even
that is an override against an SSOT, not a free-form preference.

---

## 4. Consolidated Settings IA (design)

Replace the flat 13-section list with **8 top-level domains**, each a group that
expands to its existing sections (progressive disclosure). The search box already
present becomes the primary findability tool — it short-circuits the hierarchy
entirely (type "openrouter", jump straight to the field), so the grouping only
has to be *good enough to browse*, not perfect.

### Top-level domains (left nav, grouped)

1. **Models & Routing** — Active model (migrated from Models), LLM & providers
   (concurrency/retry), Model routing emphasis + priority chain. *Resolves the
   "where does OpenRouter concurrency live" scatter by co-locating all three.*
2. **Agents & Orchestration** — Orchestrator (concurrency, budget cap, doubt
   threshold, isolation, auto-doubt, auto-budget), Scaling, Runtime config
   catalog.
3. **Mesh & Trust** — Mesh peers (trust/untrust), Signing keys. *Both are
   "who/what compute do I trust."*
4. **Memory & Context** — Memory auto-recall (migrated from Memory), checkpoint
   cadence, future context-window prefs.
5. **Appearance & Layout** — Theme, HUD tiles (display), Keybinds. *(Repository
   VCS-isolation default lands here too, or under Agents — see §6 open Q2.)*
6. **Gamification** — Enabled + mode. *Config only; see §5 dual-nature note.*
7. **Secrets** — Keys & secrets (API keys, import .env, vault migrate). *Kept its
   own top-level domain by intent: high-stakes, write-only, distinct mental model
   from a "preference."*
8. **Telemetry & Privacy** — Telemetry mode (off/local/cloud).

### Critique-lens checks

- **Avoiding a 30-section monster:** 8 browsable groups instead of 13 flat peers;
  sub-sections sit one level down. Combined with the existing search, the
  user never scans more than 8 things at the top.
- **Information scent:** each domain name predicts its contents (a user looking
  for "trust a node" reads "Mesh & Trust" and stops). The current flat list fails
  this — `signing` next to `secrets` next to `telemetry` has no through-line.
- **Progressive disclosure:** Secrets and Gamification stay shallow; Models &
  Routing and Agents & Orchestration carry the depth. Advanced routing axes are
  already behind a "▸ Advanced (all 6 axes)" toggle — keep that pattern.
- **Lives where the user expects it:** the §1 migrations move *form-style*
  config into Settings; *direct-manipulation* state (dashboard drag, rail
  collapse, policy branch picker) stays in its surface. This is the line that
  keeps the consolidation from feeling like a junk drawer.
- **Implementation note:** this is a nav re-grouping over the *existing*
  `section ===` blocks — the section components don't change, only `SECTIONS`
  becomes a 2-level structure and `settingsIndex.ts` entries get a `domain`
  field. Search behavior is unchanged.

---

## 5. Settings + Policies unification — recommendation

Three options, scored on the "stays clear what each is for" test.

### (a) One surface, two modes/tabs — "Configuration" with Settings + Policies tabs
- **Pros:** one nav entry; a tab strip is a strong, conventional visual divider;
  discoverable ("all the knobs are under Configuration").
- **Cons:** a top-level tab strip implies the two are *the same kind of thing*
  toggled by view. They are not — one is reversible prefs, the other is
  branch-scoped enforced gates with CI status. **High risk of blurring** "is this
  a suggestion or a hard rule." Policies' own branch-selector + status model
  fights the Settings master-detail layout inside one frame.

### (b) Co-located but distinct — sibling surfaces under one nav group
- **Pros:** ONE place to *find* both ("Configuration & Governance" nav group with
  **Settings** and **Policies** as siblings), satisfying the ratifier's "one
  well-organized place." Each keeps its own surface, layout, and language, so the
  conceptual line stays bright. Cheapest migration: it's a nav-grouping change,
  zero changes to either surface's internals or tests.
- **Cons:** two clicks to cross from a pref to a related gate; requires a clear
  group label so users don't think they're duplicates.

### (c) Keep fully separate
- **Pros:** zero risk of conceptual blur; no work.
- **Cons:** fails the mandate's "evaluate unifying" intent; the two stay
  unrelated in the IA even though users reasonably look for "the rules" near "the
  settings."

### RECOMMENDATION: **(b) Co-located but distinct.**

It is the only option that satisfies *both* halves of the mandate: it gives ONE
findable location (a single nav group) **and** keeps the line bright by NOT
merging two structurally different surfaces into one frame. (a) wins on "one
click" but loses the thing the ratifier explicitly protected — clarity that a
policy is a hard rule, not a toggle. (c) is safe but ignores the mandate.

### Clarity safeguards (apply to (b); mandatory if (a) is ever chosen)

- **Language:** Settings uses "preference / default / off-on." Policies uses
  "rule / gate / enforced / blocking / protected." Never call a policy a
  "setting" in copy.
- **Visual treatment:** Settings rows are toggles/sliders/inputs (reversible,
  instant). Policy rows carry a **status dot** (pass/fail/warn/not_run) and a
  **branch** dimension — never present a policy without its status. `protected`
  policies render their controls *disabled* with a lock affordance so "you cannot
  change this" is visible, not discovered on click.
- **Placement:** the nav group is ordered Settings → Policies (config before
  governance) with a divider/sub-label between them; the group label is
  "Configuration & Governance" (or "Settings & Policies"), explicitly two nouns
  so users don't expect one merged list.
- **Enable/disable framing:** a policy's enable/disable is the one shared gesture
  — frame it as "override this enforced rule for this repo," not "turn off a
  setting," and keep it visually inside the Policies surface where the status and
  `protected` context live.

---

## 6. Migration notes + open decisions

### Per-setting move cost (low, mechanical)
For each §1 migration: the persistence command **stays identical** (no backend
change); only the control's JSX moves into the matching Settings section, and one
`settingsIndex.ts` entry is added so search finds it.
- `set_active_model` control: Models surface → **Models & Routing**.
- `gui.memory.autoRecall` toggle: Memory surface → **Memory & Context**.
- `set_vcs_isolation_strategy` *default*: Repository surface → see Q2.

**Test/findability cost:** moving a control breaks any test asserting it renders
in its old surface, and changes where users look. Mitigate by (a) leaving a
one-line "Configure in Settings →" deep-link stub in the origin surface (reuses
the existing `vox_settings_seed` deep-link mechanism), and (b) updating the
origin surface's component tests. The §1 "stays in place" items have **zero**
migration cost by design.

### Gamify's dual nature (explicit)
- **Config → Settings:** `enabled` + `mode` (already there; just regroup under
  the Gamification domain). This is the only gamify *setting*.
- **Visual concepts/badges → stay app-wide:** XP, Ludus HUD, reward overlays,
  badges (`GamifyView`, `LudusHud`, `recordGamifyGuiEvent` call-sites across
  Settings/secrets/signing) are *presentation*, not configuration. They remain
  embedded in their surfaces and may apply app-wide per the mandate. Do NOT pull
  visual elements into Settings — only the enable/mode switch lives there.

### Decisions needing the human's call
1. **Unification a/b/c — RECOMMEND (b)** co-located-but-distinct nav group. This
   is the headline call: (a) is more unified but risks blurring prefs vs enforced
   rules; (b) keeps the bright line; (c) ignores the mandate. Confirm (b), or
   accept (a) *with* the §5 safeguards as hard requirements.
2. **VCS isolation default placement:** Settings under **Appearance & Layout** is
   a poor fit; it's really an **Agents & Orchestration** concern (alongside the
   isolation tier already there). Recommend Agents & Orchestration; the *per-agent
   override* stays in the Repository surface (contextual — "lives where the user
   expects it"). Confirm the default belongs in Settings at all, vs. staying a
   Repository-surface control.
