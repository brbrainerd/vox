---
category: "Architecture SSOTs"
title: "Plan 3C — Settings Consolidation + Settings/Policies Co-location (TDD)"
date: 2026-06-26
status: plan
---

# Plan 3C — Settings Consolidation + Settings/Policies Co-location

Implementation plan for GUI-IA Amendment B (spec:
`docs/superpowers/specs/2026-06-26-settings-consolidation-policies-unification-design.md`).
Ratified decisions are baked in below; this plan does not re-open them.

## Workflow Execution

This section makes Plan 3C dispatchable by a write-through workflow: every phase
is classified `[PARALLEL-SAFE]` or `[SEQUENTIAL]`, grouped into fan-out batches a
workflow can launch concurrently, and each phase ends in a concrete sub-agent
commit (the commit commands already embedded in each phase). Sub-agents commit
with **add + commit only** — never `git push`, never `git rebase`, never
`git reset`. Use the worktree-scoped form in every command:
`git -C /c/Users/Owner/vox-graphify-gui add <paths>` then
`git -C /c/Users/Owner/vox-graphify-gui commit -m "…"`.

### Cross-plan dependency header

| Dependency | Requirement |
| --- | --- |
| **Plan 3A (gamify nav move)** | **MUST precede Phase 5** of this plan. 3A owns the gamify row in `contracts/gui/surface-registry.v1.yaml` + `surfaceRegistry.generated.ts`; Phase 5 edits the `policies` row in the same YAML and regenerates. Landing 3A first avoids a generated-TS merge collision. **Phases 0–4 do NOT touch the YAML and have NO dependency on 3A** — they may run before, during, or after 3A. |
| **Plan 3F (P6, CLI-governance)** | **MUST precede Phase 5** as well — 3F and 3C are **mutually SEQUENTIAL on the generated registry, NOT parallel.** Both regenerate the single re-sorted `surfaceRegistry.generated.ts` (the generator re-sorts by `(cli_group, view_key)` on every `--write`, so it is **not** append-only and concurrent regens collide). 3F adds the CI/Database (+ secrets/auth/cli-only) rows; this plan's Phase 5 then reparents the `policies` row and regenerates **on top of** 3F's rows. INDEX DAG order: **3A → 3F → 3C**. |
| **Plan 3B (VoxMens identity/keys)** | No hard ordering with 3C, but 3B routes its key handling into the **Secrets** domain that Phase 0 declares. If 3B and 3C both land, 3C's Secrets domain (single key store) is the home; no merge conflict (different files). |
| **Spec** | `docs/superpowers/specs/2026-06-26-settings-consolidation-policies-unification-design.md` (ratified; not re-opened). |

If 3A has not landed when the workflow reaches Batch C, **hold Phase 5** until 3A
merges, then proceed (see the Phase 4-internal ordering note already in this
plan, repeated at Phase 5.5).

### Per-phase classification

| Phase | Concern | Class | Depends on | Touches (conflict surface) |
| --- | --- | --- | --- | --- |
| **Phase 0** | 8-domain SSOT (`settingsDomains.ts` + test) | **[SEQUENTIAL]** (foundation) | — | new files only |
| **Phase 1** | Re-group nav into 8 domains | **[SEQUENTIAL]** | Phase 0 | `SettingsView.tsx`, `SettingsView.test.tsx`, `settingsIndex.ts` |
| **Phase 2** | Stray #1: Memory auto-recall → Settings | **[PARALLEL-SAFE]** within Batch B | Phase 1 | `SettingsView.tsx`, `SettingsView.domains.test.tsx`, `settingsIndex.ts`, `MemoryView.*` |
| **Phase 3** | Stray #2: VCS isolation default → Settings | **[PARALLEL-SAFE]** within Batch B | Phase 1 | `SettingsView.tsx`, `SettingsView.domains.test.tsx`, `settingsIndex.ts`, `Repository/*` |
| **Phase 4** | Stray #3: Active model → Settings | **[PARALLEL-SAFE]** within Batch B | Phase 1 | `SettingsView.tsx`, `SettingsView.domains.test.tsx`, `settingsIndex.ts` |
| **Phase 5** | Policies co-location (nav group) | **[SEQUENTIAL]** | Phase 1 + **Plan 3A** | `navigation.*`, `Sidebar.*`, `surface-registry.v1.yaml`, generated TS |
| **Phase 6** | Final regression + drift gate | **[SEQUENTIAL]** (terminal) | Phases 0–5 | no source edits (verify only) |

### Fan-out batches

The workflow dispatches batches in order; within a batch, listed phases run
concurrently. A batch is complete only when **all** its phases are green and
committed.

- **Batch A — foundation (sequential, 1 agent):** `Phase 0 → Phase 1`.
  These are strictly ordered (Phase 1 imports the SSOT Phase 0 creates and
  deletes the flat `SECTIONS` array). Run them as a single sequential agent, or
  two agents where the Phase 1 agent starts only after Phase 0's commit lands.
  **Gate:** Phase 1 nav-grouping test green before Batch B fans out.

- **Batch B — stray-setting migrations (fan-out, up to 3 agents):**
  `Phase 2 ∥ Phase 3 ∥ Phase 4`. These are logically independent (distinct
  section blocks, distinct source surfaces: Memory / Repository / Models). They
  are **[PARALLEL-SAFE]** in intent but **share three files**
  (`SettingsView.tsx`, `SettingsView.domains.test.tsx`, `settingsIndex.ts`).
  Two safe dispatch modes for a write-through workflow:
  - **Mode B1 (recommended — isolated worktrees):** give each phase its own
    git worktree off the post-Batch-A commit, run all three concurrently, then
    integrate sequentially (the workflow merges the three commits back; the
    shared-file hunks are additive — a new `section ===` block, a new
    `SETTINGS_INDEX` entry, a new test — and merge cleanly because each appends
    in a distinct region). Each phase still ends in its own commit.
  - **Mode B2 (serialized commits, shared tree):** if worktree isolation is
    unavailable, run the three phases' *implementation* in parallel but
    **serialize their commits** (Phase 2 commit → Phase 3 commit → Phase 4
    commit) so each `git -C … add` stages only that phase's hunks. The phases do
    not depend on each other's code, only on Batch A.
  - **Gate:** all three commits present and the full Settings regression green
    before Batch C.

- **Batch C — nav co-location (sequential, 1 agent):** `Phase 5`.
  Blocked on **Plan 3A** (YAML/generated-TS). Runs after Batch B (Phase 5 reads
  no Batch-B code, but sharing one integration head keeps the registry
  regeneration deterministic). Ends in its own commit.

- **Batch D — gate (sequential, 1 agent):** `Phase 6`. Whole-suite vitest +
  typecheck + surface-registry drift no-write check. No source edits; produces
  no commit (verification only) — reports branch state to the user.

### Write-through commit rule (every phase)

Each phase already terminates in an `add` + `commit` step (Steps 0.3, 1.4, 2.5,
3.5, 4.4, 5.6). For workflow dispatch, rewrite those commands to the
worktree-scoped, push-free form, e.g.:

```
git -C /c/Users/Owner/vox-graphify-gui add <the exact paths listed in that step>
git -C /c/Users/Owner/vox-graphify-gui commit -m "<the message already given in that step>"
```

No sub-agent runs `git push`, `git merge`, `git rebase`, `git reset`, or
`git clean`. Integration of Batch-B worktrees and the final merge are the
workflow/human's responsibility, not a sub-agent's.

## Ratified scope (decisions, not options)

1. **Unification = option (b) co-located, distinct.** Settings and Policies
   become **sibling surfaces under ONE nav group** ("Configuration &
   Governance"), NOT merged tabs. The bright line stays: Settings = reversible
   user config; Policies = enforced governance (branch-scoped, CI status,
   `protected` rules). Settings is ordered **before** Policies (config before
   governance).
2. **Consolidation is MODEST.** Re-group the existing 13 flat `section ===`
   blocks in `SettingsView.tsx` into **8 named domains** (a nav re-grouping over
   the existing blocks, NOT a rewrite). Pull in the **3 stray settings**
   (active-model, memory auto-recall, VCS-isolation default). Move gamify
   **config** (enabled + mode) into the Gamification domain.
3. **Secrets domain is the single key-management home.** VoxMens identity/key
   handling (Plan 3B) routes here. No second key store.
4. **gamify VISUAL concepts (XP/HUD/badges) stay app-wide** — only the config
   moves.

## Dependency on Plan 3A (state explicitly)

Plan 3A reparents the **gamify nav surface** (`view_key: gamify`, the
`GamifyView` curated-decorator surface) out of `parentSurface: operate` into its
new IA home, and is the owner of `surface-registry.v1.yaml` churn for gamify's
**nav location**. Plan 3C is **distinct**: it touches only gamify's **config
control** (the `enabled` + `mode` toggles already inside `SettingsView`'s
`section === 'gamify'` block) — it does not move the `GamifyView` surface.

- **3A = gamify nav move** (where the Gamify *surface* lives in the sidebar).
- **3C = gamify config move** (grouping the existing enabled/mode toggles under
  the Settings "Gamification" domain).

**Ordering:** If 3A and 3C both edit `contracts/gui/surface-registry.v1.yaml`
(3A for the gamify surface row, 3C for the `policies` row in **Phase 5**),
**land 3A first** to avoid a generated-TS merge collision in
`surfaceRegistry.generated.ts`. Likewise **land Plan 3F first** (it adds the
CI/Database/secrets/auth rows and regenerates the same file) — 3F and this plan
are mutually sequential on the generated registry. **Only Phase 5 touches the
YAML/generated registry** in this plan: Phases 0–4 edit `SettingsView.tsx` /
`settingsIndex.ts` / surface components only (Phase 4 is the **active-model**
selector — additive, no registry edit). So execute Phases 0–4 first (no YAML
dependency on 3A/3F), then run Phase 5 only after both 3A and 3F's registry
writes have landed, then regenerate. Phase 5 below assumes 3A **and** 3F are
merged.

## Working directory & commands

All UI work happens in the vitest workspace:

```
cd /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui
```

Per-step verify loop (substitute the file under test):

```
cd /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui && pnpm vitest run src/components/surfaces/Settings/SettingsView.domains.test.tsx && pnpm typecheck
```

Full Settings + nav regression before each commit:

```
cd /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui && pnpm vitest run src/components/surfaces/Settings src/lib/navigation.test.ts src/components/surfaces/Memory src/components/surfaces/Models src/components/surfaces/Repository && pnpm typecheck
```

Phase 4 also runs the Rust generator from the repo root:

```
cd /c/Users/Owner/vox-graphify-gui && cargo run -p vox-cli -- ci gui-surface-registry --write
```

## Branch

Work on the current branch `claude/graphify-general-gui-ia`. Commit after every
green step (the user commits the final state; you commit intermediate steps so
the history is bisectable). Never `git push`.

---

## Phase 0 — Domain model SSOT (no UI behavior change yet) [SEQUENTIAL]

_Batch A (foundation). Depends on: nothing. Must precede Phase 1._

The 8 domains must be a single declarative structure so the nav, the search
index `domain` field, and the tests all read from one place.

### Step 0.1 — Write the failing test for the domain map

Create `src/components/surfaces/Settings/settingsDomains.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { SETTINGS_DOMAINS, sectionDomain, DOMAIN_ORDER } from './settingsDomains';

describe('SETTINGS_DOMAINS', () => {
  it('defines exactly the 8 ratified domains in order', () => {
    expect(DOMAIN_ORDER).toEqual([
      'models',
      'agents',
      'mesh',
      'memory',
      'appearance',
      'gamification',
      'secrets',
      'telemetry',
    ]);
  });

  it('maps every existing section id to a domain', () => {
    const sectionIds = [
      'orchestrator', 'scaling', 'llm', 'routing', 'runtime', 'mesh',
      'signing', 'secrets', 'telemetry', 'keybinds', 'theme', 'display',
      'gamify', 'active-model', 'memory-context', 'vcs-isolation',
    ];
    for (const id of sectionIds) {
      expect(sectionDomain(id), `section ${id} must have a domain`).toBeDefined();
    }
  });

  it('groups Models & Routing as active-model + llm + routing', () => {
    const models = SETTINGS_DOMAINS.find(d => d.id === 'models')!;
    expect(models.sections.map(s => s.id)).toEqual(['active-model', 'llm', 'routing']);
  });

  it('places VCS isolation default under Agents & Orchestration', () => {
    const agents = SETTINGS_DOMAINS.find(d => d.id === 'agents')!;
    expect(agents.sections.map(s => s.id)).toContain('vcs-isolation');
  });

  it('keeps Secrets as its own shallow domain (single key store)', () => {
    const secrets = SETTINGS_DOMAINS.find(d => d.id === 'secrets')!;
    expect(secrets.sections.map(s => s.id)).toEqual(['secrets']);
  });
});
```

Run it — it fails (module missing):

```
cd /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui && pnpm vitest run src/components/surfaces/Settings/settingsDomains.test.ts
```

### Step 0.2 — Implement `settingsDomains.ts`

Create `src/components/surfaces/Settings/settingsDomains.ts`:

```ts
/**
 * Single source of truth for the 8 Settings domains (GUI-IA Amendment B).
 * Each domain expands to the existing `section ===` blocks in SettingsView.
 * This is a grouping layer over the existing sections — no section component
 * changes here. New section ids ('active-model', 'memory-context',
 * 'vcs-isolation') are the 3 stray-setting migrations (Phases 1–3).
 */
export interface DomainSection {
  /** Matches `section` state in SettingsView (drives `section ===` blocks). */
  id: string;
  icon: string;
  label: string;
}

export interface SettingsDomain {
  id: string;
  label: string;
  icon: string;
  sections: DomainSection[];
}

export const SETTINGS_DOMAINS: SettingsDomain[] = [
  {
    id: 'models',
    label: 'Models & Routing',
    icon: 'cpu',
    sections: [
      { id: 'active-model', icon: 'cpu',    label: 'Active model' },
      { id: 'llm',          icon: 'bolt',   label: 'LLM & providers' },
      { id: 'routing',      icon: 'matrix', label: 'Model routing' },
    ],
  },
  {
    id: 'agents',
    label: 'Agents & Orchestration',
    icon: 'flow',
    sections: [
      { id: 'orchestrator',  icon: 'cpu',    label: 'Orchestrator' },
      { id: 'scaling',       icon: 'cpu',    label: 'Scaling' },
      { id: 'runtime',       icon: 'flow',   label: 'Runtime' },
      { id: 'vcs-isolation', icon: 'branch', label: 'VCS isolation default' },
    ],
  },
  {
    id: 'mesh',
    label: 'Mesh & Trust',
    icon: 'flow',
    sections: [
      { id: 'mesh',    icon: 'flow',   label: 'Mesh & peers' },
      { id: 'signing', icon: 'shield', label: 'Signing keys' },
    ],
  },
  {
    id: 'memory',
    label: 'Memory & Context',
    icon: 'memory',
    sections: [
      { id: 'memory-context', icon: 'memory', label: 'Memory auto-recall' },
    ],
  },
  {
    id: 'appearance',
    label: 'Appearance & Layout',
    icon: 'spark',
    sections: [
      { id: 'theme',    icon: 'spark',   label: 'Theme' },
      { id: 'display',  icon: 'monitor', label: 'Display' },
      { id: 'keybinds', icon: 'command', label: 'Keybinds' },
    ],
  },
  {
    id: 'gamification',
    label: 'Gamification',
    icon: 'bolt',
    sections: [
      { id: 'gamify', icon: 'bolt', label: 'Gamification' },
    ],
  },
  {
    id: 'secrets',
    label: 'Secrets',
    icon: 'shield',
    sections: [
      { id: 'secrets', icon: 'shield', label: 'Keys & Secrets' },
    ],
  },
  {
    id: 'telemetry',
    label: 'Telemetry & Privacy',
    icon: 'scale',
    sections: [
      { id: 'telemetry', icon: 'scale', label: 'Telemetry' },
    ],
  },
];

export const DOMAIN_ORDER = SETTINGS_DOMAINS.map(d => d.id);

const SECTION_TO_DOMAIN: Record<string, string> = Object.fromEntries(
  SETTINGS_DOMAINS.flatMap(d => d.sections.map(s => [s.id, d.id])),
);

export function sectionDomain(sectionId: string): string | undefined {
  return SECTION_TO_DOMAIN[sectionId];
}
```

Run Step 0.1 test — green. Run typecheck.

> Note on the 3 new section ids: `active-model`, `memory-context`, and
> `vcs-isolation` are declared here now but their `section ===` blocks are added
> in Phases 1–3. Until then, selecting them renders an empty content pane; the
> domain-grouping test (Phase 0.3) only asserts the *nav* renders, so this is
> safe between commits.

### Step 0.3 — Commit

```
git add src/components/surfaces/Settings/settingsDomains.ts src/components/surfaces/Settings/settingsDomains.test.ts
git commit -m "feat(gui-settings): add 8-domain SSOT for Settings consolidation (Plan 3C Phase 0)"
```

---

## Phase 1 — Re-group SettingsView nav into 8 domains [SEQUENTIAL]

_Batch A (foundation). Depends on: Phase 0. Gates Batch B (Phases 2/3/4)._

Replace the flat `SECTIONS` left-nav with a 2-level domain → section nav, driven
by `SETTINGS_DOMAINS`. The right-pane `section ===` blocks are **unchanged** in
this phase (the 3 new ones come in Phases 2–4).

### Step 1.1 — Write the failing nav-grouping test

Create `src/components/surfaces/Settings/SettingsView.domains.test.tsx`. Reuse
the mock scaffold from the existing `SettingsView.test.tsx` (copy the
`invokeMock`, the `@tauri-apps/api/core`, `@tauri-apps/api/event`, `transport`,
`gamifyGuiEvents`, and `PriorityChainEditor` mocks verbatim — they are required
for `SettingsView` to mount). Then:

```tsx
import { SettingsView } from './SettingsView';
import { SETTINGS_DOMAINS } from './settingsDomains';

describe('SettingsView domain grouping', () => {
  function renderView() {
    return render(<SettingsView pushToast={vi.fn()} />, { wrapper });
  }

  it('renders all 8 domain group headers in ratified order', () => {
    renderView();
    const headers = screen.getAllByTestId(/^settings-domain-header-/);
    expect(headers.map(h => h.getAttribute('data-domain'))).toEqual(
      SETTINGS_DOMAINS.map(d => d.id),
    );
  });

  it('lists the right sections under Models & Routing', () => {
    renderView();
    const group = screen.getByTestId('settings-domain-group-models');
    expect(group.querySelector('[data-section="active-model"]')).toBeTruthy();
    expect(group.querySelector('[data-section="llm"]')).toBeTruthy();
    expect(group.querySelector('[data-section="routing"]')).toBeTruthy();
  });

  it('lists VCS isolation default under Agents & Orchestration', () => {
    renderView();
    const group = screen.getByTestId('settings-domain-group-agents');
    expect(group.querySelector('[data-section="vcs-isolation"]')).toBeTruthy();
  });

  it('keeps the settings search input unchanged', () => {
    renderView();
    expect(screen.getByLabelText('Search settings')).toBeTruthy();
  });
});
```

Run it — fails (no `settings-domain-header-*` testids yet):

```
cd /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui && pnpm vitest run src/components/surfaces/Settings/SettingsView.domains.test.tsx
```

### Step 1.2 — Implement the 2-level nav in `SettingsView.tsx`

In `SettingsView.tsx`:

1. Add the import:

```ts
import { SETTINGS_DOMAINS } from './settingsDomains';
```

2. **Delete** the flat `const SECTIONS = [ … ];` array (lines 21–35). It is
   replaced by `SETTINGS_DOMAINS`.

3. Replace the non-filtered `<nav>` block (the `) : (` branch around lines
   1239–1259 that maps `SECTIONS`) with a domain-grouped nav. The filtered
   search branch (`searchSettings`) is **unchanged**. New markup:

```tsx
) : (
  <nav className="flex flex-col gap-2">
    {SETTINGS_DOMAINS.map(domain => (
      <div key={domain.id} data-testid={`settings-domain-group-${domain.id}`}>
        <div
          data-testid={`settings-domain-header-${domain.id}`}
          data-domain={domain.id}
          className="mx-2 mb-1 border-b border-border-subtle px-0 pb-1 font-display text-[9px] uppercase tracking-[0.28em] text-text-muted"
        >
          {domain.label}
        </div>
        <div className="flex flex-col gap-0.5">
          {domain.sections.map(s => {
            const IcoCmp = (Icon as any)[s.icon] ?? Icon.bolt;
            const on = section === s.id;
            return (
              <button
                key={s.id}
                type="button"
                data-section={s.id}
                onClick={() => setSection(s.id)}
                className={`flex items-center gap-2.5 rounded-lg px-3 py-2 text-left transition ${
                  on ? 'bg-overlay-subtle text-text-primary' : 'text-text-muted hover:bg-overlay-subtle hover:text-text-secondary'
                }`}
              >
                <IcoCmp className={`size-4 ${on ? 'text-brass' : 'text-text-muted'}`} />
                <span className="font-display text-[12px] tracking-[0.12em] uppercase">{s.label}</span>
              </button>
            );
          })}
        </div>
      </div>
    ))}
  </nav>
)
```

This uses the existing `.ds-section-head`-style underline (`border-b`) for the
domain header — matching the project rule that a divider *underlines* a label,
never caps it from above.

> The default initial `section` state stays `'orchestrator'` (a real block), so
> the right pane renders correctly on mount.

Run Step 1.1 test — green. Run the full Settings regression. Then fix any fallout
in the existing `SettingsView.test.tsx` (the old `renders section nav items for
all 12 settings sections` assertion counts flat buttons; update it to assert the
domain-grouped buttons instead, or relax to `getAllByTestId(/^settings-domain-group-/)`).

### Step 1.3 — Add `domain` field to `settingsIndex.ts` entries

Per spec §4: "`settingsIndex.ts` entries get a `domain` field." This keeps the
search results domain-aware without changing search behavior.

Failing test — append to `src/components/surfaces/Settings/settingsIndex.test.ts`
(create the file if absent):

```ts
import { describe, expect, it } from 'vitest';
import { SETTINGS_INDEX } from './settingsIndex';
import { DOMAIN_ORDER } from './settingsDomains';

describe('settingsIndex domain tagging', () => {
  it('every static index entry carries a valid domain', () => {
    for (const e of SETTINGS_INDEX) {
      if (!e.domain) continue; // generated entries may omit domain
      expect(DOMAIN_ORDER).toContain(e.domain);
    }
  });
});
```

Implement: add `domain?: string;` to the `SettingEntry` interface and tag each
of the 20 static entries with its domain (`orchestrator`→`agents`, `llm`→`models`,
`routing`→`models`, `mesh`/`signing`→`mesh`, `secrets`→`secrets`,
`telemetry`→`telemetry`, `keybinds`/`theme`/`display`→`appearance`,
`gamify`→`gamification`, `scaling*`/`runtime`→`agents`). Leave
`...GENERATED_SETTINGS_INDEX` untouched (domain is optional there).

Run both index + domains tests — green. Typecheck.

### Step 1.4 — Commit

```
git add src/components/surfaces/Settings/SettingsView.tsx src/components/surfaces/Settings/SettingsView.domains.test.tsx src/components/surfaces/Settings/SettingsView.test.tsx src/components/surfaces/Settings/settingsIndex.ts src/components/surfaces/Settings/settingsIndex.test.ts
git commit -m "feat(gui-settings): re-group 13 sections into 8 nav domains (Plan 3C Phase 1)"
```

---

## Phase 2 — Stray setting #1: Memory auto-recall → Settings (Memory & Context) [PARALLEL-SAFE]

_Batch B (fan-out). Depends on: Phase 1. Independent of Phases 3 & 4 (distinct
section block + Memory surface); shares `SettingsView.tsx` / `settingsIndex.ts` /
`SettingsView.domains.test.tsx` — isolate via worktree (Mode B1) or serialize the
commit (Mode B2)._

The pref key `gui.memory.autoRecall` is the SSOT and does **not** change. We add
a toggle in Settings, and replace the Memory surface's toggle button with a
deep-link stub. Memory keeps hydrating `recallOn` from the pref (its
in-surface behavior must still react), so only the *toggle control* moves.

### Step 2.1 — Failing test: auto-recall toggle renders in Settings

Append to `SettingsView.domains.test.tsx`:

```tsx
it('renders the memory auto-recall toggle and persists via gui.memory.autoRecall', async () => {
  renderView();
  fireEvent.click(screen.getByTestId('settings-domain-group-memory').querySelector('[data-section="memory-context"]')!);
  const toggle = await screen.findByTestId('settings-memory-autorecall-toggle');
  fireEvent.click(toggle);
  await waitFor(() =>
    expect(mockSetGuiPreference).toHaveBeenCalledWith('gui.memory.autoRecall', 'true'),
  );
});
```

(The `transport` mock must expose `getGuiPreference`/`setGuiPreference` — it
already does in the copied scaffold.)

Run — fails (no `memory-context` block).

### Step 2.2 — Implement the `memory-context` section block in SettingsView

Add a self-contained `MemoryContextSection` component near `LlmSettingsSection`,
reading/writing the pref through `voxTransport`:

```tsx
function MemoryContextSection({ pushToast }: { pushToast: (t: Toast) => void }) {
  const [recallOn, setRecallOn] = useState(false);
  useEffect(() => {
    voxTransport.getGuiPreference('gui.memory.autoRecall')
      .then(v => { if (v != null) setRecallOn(v === 'true'); })
      .catch(() => {});
  }, []);
  const toggle = () => {
    setRecallOn(prev => {
      const next = !prev;
      voxTransport.setGuiPreference('gui.memory.autoRecall', String(next))
        .catch(err => pushToast({ tone: 'warn', title: 'Could not persist auto-recall', body: String(err), cause: 'backend-error' }));
      return next;
    });
  };
  return (
    <>
      <h2 className="font-display text-[18px] font-semibold tracking-tight text-text-primary">Memory &amp; Context</h2>
      <p className="mt-0.5 text-[11px] text-text-muted">Recall behavior for the Memory surface and context retrieval</p>
      <div className="mt-4 space-y-3">
        <Row label="Auto-recall" hint="Recall memory hits as you type in the Memory surface (vs. press Enter)">
          <button
            type="button"
            data-testid="settings-memory-autorecall-toggle"
            onClick={toggle}
            aria-pressed={recallOn}
          >
            <Toggle on={recallOn} onClick={toggle} />
          </button>
        </Row>
      </div>
    </>
  );
}
```

> Use the existing `Toggle` for visuals but expose the `data-testid` button
> wrapper so the test has a stable handle. (Simpler: give `Toggle` an optional
> `testId` prop and pass it through — pick whichever keeps `all buttons have
> type=button` green; the wrapper approach nests two buttons, so prefer adding a
> `testId` prop to `Toggle` instead and drop the wrapper.)

Wire it into the content switch:

```tsx
{section === 'memory-context' && <MemoryContextSection pushToast={pushToast} />}
```

Run Step 2.1 — green.

### Step 2.3 — Failing test: Memory surface no longer renders its own toggle button; shows deep-link stub

In `src/components/surfaces/Memory/MemoryView.test.tsx` (create if absent, reuse
its existing mock scaffold for `invoke`/`voxTransport`):

```tsx
it('replaces the in-surface auto-recall toggle with a Settings deep-link', () => {
  render(<MemoryView pushToast={vi.fn()} />);
  // The old toggle button is gone…
  expect(screen.queryByRole('button', { name: /auto-recall/i })).toBeNull();
  // …replaced by a deep-link to Settings.
  expect(screen.getByTestId('memory-autorecall-settings-link')).toBeTruthy();
});

it('still hydrates recall behavior from gui.memory.autoRecall', async () => {
  // recallOn state must still be read so auto-recall keeps working.
  render(<MemoryView pushToast={vi.fn()} />);
  await waitFor(() =>
    expect(getGuiPreferenceMock).toHaveBeenCalledWith('gui.memory.autoRecall'),
  );
});
```

Run — fails (toggle still present).

### Step 2.4 — Move the control out of MemoryView

In `MemoryView.tsx`:

- **Keep** the `recallOn` state + the mount-hydration `useEffect` (lines ~196–200)
  and the auto-recall debounce `useEffect` (lines ~257–264) — behavior stays.
- **Remove** `toggleAutoRecall` (lines ~202–209) and the toggle `<button>`
  (lines ~313–325).
- Replace the button with a deep-link stub that reuses the existing
  `vox_settings_seed` mechanism (the same one omni-search uses). The stub writes
  the seed and dispatches the event so Settings opens on the `memory-context`
  section:

```tsx
<button
  type="button"
  data-testid="memory-autorecall-settings-link"
  onClick={() => {
    localStorage.setItem('vox_settings_seed', JSON.stringify({ section: 'memory-context' }));
    window.dispatchEvent(new Event('vox-settings-seed'));
    onNavigate?.('settings');
  }}
  className="inline-flex items-center gap-1.5 rounded-md border border-border-subtle bg-overlay-subtle px-2 py-1.5 font-mono text-[10px] text-text-muted hover:text-text-secondary transition"
>
  <Icon.settings aria-hidden="true" className="size-3" /> Auto-recall in Settings →
</button>
```

If `MemoryView` has no `onNavigate` prop, drop that call — the seed +
`vox-settings-seed` event is sufficient once the user opens Settings; verify
against the actual `MemoryViewProps`. (Read the props first; do not invent a prop.)

Run Step 2.3 + Memory regression — green. Typecheck.

### Step 2.5 — Add the search index entry + commit

Add to `SETTINGS_INDEX`:

```ts
{ id: 'memory-autorecall', section: 'memory-context', domain: 'memory', label: 'Auto-recall', hint: 'Recall memory hits as you type', keywords: ['memory', 'recall', 'context', 'retrieval'] },
```

```
git add src/components/surfaces/Settings/SettingsView.tsx src/components/surfaces/Settings/SettingsView.domains.test.tsx src/components/surfaces/Settings/settingsIndex.ts src/components/surfaces/Memory/MemoryView.tsx src/components/surfaces/Memory/MemoryView.test.tsx
git commit -m "feat(gui-settings): move memory auto-recall toggle into Settings, deep-link from Memory (Plan 3C Phase 2)"
```

---

## Phase 3 — Stray setting #2: VCS isolation default → Settings (Agents & Orchestration) [PARALLEL-SAFE]

_Batch B (fan-out). Depends on: Phase 1. Independent of Phases 2 & 4 (distinct
section block + Repository surface); shares `SettingsView.tsx` /
`settingsIndex.ts` / `SettingsView.domains.test.tsx` — isolate via worktree
(Mode B1) or serialize the commit (Mode B2)._

The command `set_vcs_isolation_strategy` ({ default, agentId: null, strategy:
null }) and the read `get_vcs_isolation` are the SSOT and do not change. The
**default** strategy selector moves into Settings; the **per-agent override**
stays in `RepositoryView` (contextual, per spec §5).

### Step 3.1 — Failing test: VCS-isolation default selector in Settings persists via set_vcs_isolation_strategy

In the SettingsView domains test, extend the `invokeMock` to handle:

```ts
if (cmd === 'get_vcs_isolation') return Promise.resolve({ default: 'worktree', perAgent: [], conflicts: [] });
if (cmd === 'set_vcs_isolation_strategy') return Promise.resolve({ default: (args as any)?.default ?? 'worktree', perAgent: [], conflicts: [] });
```

Test:

```tsx
it('sets the VCS isolation default via set_vcs_isolation_strategy with agentId null', async () => {
  renderView();
  fireEvent.click(screen.getByTestId('settings-domain-group-agents').querySelector('[data-section="vcs-isolation"]')!);
  const btn = await screen.findByTestId('settings-vcs-default-branch'); // a strategy choice button
  fireEvent.click(btn);
  await waitFor(() =>
    expect(invokeMock).toHaveBeenCalledWith('set_vcs_isolation_strategy',
      expect.objectContaining({ agentId: null, strategy: null })),
  );
});
```

(Match the real `IsolationStrategy` values by reading
`Repository/isolationHelpers.ts` for the exact enum — use real strategy ids, not
placeholders. Name the testid after the real strategy id.)

Run — fails.

### Step 3.2 — Implement the `vcs-isolation` section block

Read `src/components/surfaces/Repository/IsolationPanel.tsx` and
`isolationHelpers.ts` to reuse the existing default-strategy control. Add a
`VcsIsolationDefaultSection` to `SettingsView.tsx` that renders **only the
default selector** (not the per-agent table) by calling `get_vcs_isolation` on
mount and `set_vcs_isolation_strategy` with `{ default: s, agentId: null,
strategy: null }` on change. Reuse `IsolationPanel` if it can render
default-only (pass an empty per-agent list); otherwise extract the default-row
sub-control. Do not duplicate logic — import the shared `IsolationStrategy` type
and any strategy-label map from `isolationHelpers`.

Wire it:

```tsx
{section === 'vcs-isolation' && <VcsIsolationDefaultSection pushToast={pushToast} gamifyEnabled={gamifyEnabled} />}
```

Preserve the `recordGamifyGuiEvent('isolation_strategy_set', { strategy, scope:
'default' }, …)` call so the gamify event still fires from the new home.

Run Step 3.1 — green.

### Step 3.3 — Failing test: Repository keeps per-agent override, drops the default selector; shows deep-link

In `RepositoryView.test.tsx` (reuse its mock scaffold):

```tsx
it('moves the isolation DEFAULT selector to Settings (deep-link present), keeps per-agent override', async () => {
  render(<RepositoryView pushToast={vi.fn()} />);
  // The default selector is now a deep-link to Settings…
  expect(await screen.findByTestId('repo-isolation-default-settings-link')).toBeTruthy();
  // …but the per-agent override control remains in Repository.
  expect(screen.getByTestId('repo-isolation-per-agent')).toBeTruthy();
});
```

(If `IsolationPanel` doesn't yet expose a `repo-isolation-per-agent` testid, add
it to the per-agent sub-section as part of this step — read the component first.)

Run — fails.

### Step 3.4 — Remove the default selector from Repository, add deep-link

In `RepositoryView.tsx` / `IsolationPanel.tsx`:

- Remove (or hide) the **default-strategy** selector rendered via
  `onSetDefault` / `handleSetDefault`. Keep `handleSetDefault` only if the
  per-agent flow still needs it; otherwise delete it and the `onSetDefault` prop
  plumbing.
- Keep the per-agent override UI and conflict display intact.
- Add a deep-link stub (same `vox_settings_seed` pattern, `section:
  'vcs-isolation'`) with `data-testid="repo-isolation-default-settings-link"`.

Run Step 3.3 + Repository regression — green. Typecheck.

### Step 3.5 — Search index entry + commit

```ts
{ id: 'vcs-isolation-default', section: 'vcs-isolation', domain: 'agents', label: 'VCS isolation default', hint: 'Default sandbox strategy for new agents', keywords: ['vcs', 'isolation', 'worktree', 'repository', 'sandbox'] },
```

```
git add src/components/surfaces/Settings/SettingsView.tsx src/components/surfaces/Settings/SettingsView.domains.test.tsx src/components/surfaces/Settings/settingsIndex.ts src/components/surfaces/Repository/RepositoryView.tsx src/components/surfaces/Repository/IsolationPanel.tsx src/components/surfaces/Repository/RepositoryView.test.tsx
git commit -m "feat(gui-settings): move VCS isolation default into Settings under Agents, keep per-agent in Repository (Plan 3C Phase 3)"
```

---

## Phase 4 — Stray setting #3: Active model selection → Settings (Models & Routing) [PARALLEL-SAFE]

_Batch B (fan-out). Depends on: Phase 1. Independent of Phases 2 & 3 (additive —
Models surface unchanged); shares `SettingsView.tsx` / `settingsIndex.ts` /
`SettingsView.domains.test.tsx` — isolate via worktree (Mode B1) or serialize the
commit (Mode B2)._

`get_active_model` / `set_active_model` ({ modelId }) are the SSOT. The Models
surface keeps its **per-card "set default"** buttons (contextual, like the
per-agent VCS override). Settings adds a single **active-model selector** in the
Models & Routing domain. This is additive — Models' grid is unchanged.

### Step 4.1 — Failing test: active-model selector in Settings

Extend `invokeMock`:

```ts
if (cmd === 'get_active_model') return Promise.resolve('openrouter/auto');
if (cmd === 'list_model_cards') return Promise.resolve([{ id: 'openrouter/auto', provider: 'openrouter', tier: 'hosted' }, { id: 'local/qwen', provider: 'ollama', tier: 'local' }]);
if (cmd === 'set_active_model') return Promise.resolve(null);
```

Test:

```tsx
it('sets the active model via set_active_model from Settings', async () => {
  renderView();
  fireEvent.click(screen.getByTestId('settings-domain-group-models').querySelector('[data-section="active-model"]')!);
  const select = await screen.findByTestId('settings-active-model-select');
  fireEvent.change(select, { target: { value: 'local/qwen' } });
  await waitFor(() =>
    expect(invokeMock).toHaveBeenCalledWith('set_active_model', { modelId: 'local/qwen' }),
  );
});
```

Run — fails.

### Step 4.2 — Implement the `active-model` section block

Add `ActiveModelSection` to `SettingsView.tsx`: on mount, `Promise.all([
list_model_cards, get_active_model ])`; render a `<select>`
(`data-testid="settings-active-model-select"`) whose options are the model ids
plus an `auto-route` (null) choice; on change call `set_active_model`. Reuse the
`gamifyGuiEvents` event `model_activated`. Wire:

```tsx
{section === 'active-model' && <ActiveModelSection pushToast={pushToast} gamifyEnabled={gamifyEnabled} />}
```

Run Step 4.1 — green.

### Step 4.3 — Verify Models surface is untouched

Active-model is *additive* — `ModelsView` keeps its per-card `setDefault`. Run
the existing `Models/ModelsView.test.tsx` to confirm no regression. No removal
from Models is required (unlike Phases 2–3, where a duplicate would have been
confusing). Add a search index entry:

```ts
{ id: 'active-model', section: 'active-model', domain: 'models', label: 'Active model', hint: 'Pin a default model or auto-route', keywords: ['model', 'active', 'default', 'route'] },
```

### Step 4.4 — Commit

```
git add src/components/surfaces/Settings/SettingsView.tsx src/components/surfaces/Settings/SettingsView.domains.test.tsx src/components/surfaces/Settings/settingsIndex.ts
git commit -m "feat(gui-settings): add active-model selector to Settings Models & Routing (Plan 3C Phase 4)"
```

---

## Phase 5 — Policies co-location (nav group, sibling-distinct) [SEQUENTIAL]

_Batch C. Depends on: Phase 1 + **Plan 3A** (shared `surface-registry.v1.yaml` /
`surfaceRegistry.generated.ts`). Hold this phase until 3A has landed; if 3A is
not merged when the workflow reaches Batch C, pause and land 3A first._

Reparent the `policies` surface so it is a sibling of `settings` under the
"Configuration & Governance" group. The surface internals, the
`policy_*` backend commands, the branch selector, status dots, and `protected`
lock affordances are **unchanged** — only the nav location moves. Settings is
ordered first.

The nav SSOT is `contracts/gui/surface-registry.v1.yaml` → generated to
`src/generated/surfaceRegistry.generated.ts` via `vox ci gui-surface-registry
--write`; `src/lib/navigation.ts` carries the resolution map; the Sidebar's
"System" footer renders the group.

### Step 5.1 — Failing test: navigation resolves policies under the config group

Add to `src/lib/navigation.test.ts`:

```ts
it('co-locates policies with settings (config-before-governance group)', () => {
  const nav = resolveNavigation('policies');
  expect(nav.parent).toBe('settings');
  expect(nav.child).toBe('policies');
});

it('orders settings before policies in the config group', () => {
  // CONFIG_GROUP is the ordered sibling list for the nav group.
  expect(CONFIG_GROUP).toEqual(['settings', 'policies']);
});
```

Add the `CONFIG_GROUP` import. Run — fails.

### Step 5.2 — Update `navigation.ts`

- In `PARENT_CHILD_MAP`, change
  `policies: { parent: 'runs', child: 'policies' }` →
  `policies: { parent: 'settings', child: 'policies' }`.
- Add the ordered group constant and label:

```ts
/** Sibling surfaces in the "Configuration & Governance" nav group, in order. */
export const CONFIG_GROUP = ['settings', 'policies'] as const;
```

- Add to `NAV_LABELS`: `'config-governance': 'Configuration & Governance'` (used
  as the group header label by the Sidebar).
- Leave `approvals` under `runs` (only policies moves).

Run Step 5.1 — green.

### Step 5.3 — Failing test: Sidebar renders Policies under the config group

Add to `src/components/layout/Sidebar.test.tsx` (reuse its scaffold):

```tsx
it('renders Policies as a sibling under the Configuration & Governance group, after Settings', () => {
  renderSidebar({ view: 'policies' });
  const group = screen.getByTestId('sidebar-config-group');
  const items = within(group).getAllByRole('button').map(b => b.textContent);
  const settingsIdx = items.findIndex(t => /settings/i.test(t ?? ''));
  const policiesIdx = items.findIndex(t => /policies/i.test(t ?? ''));
  expect(settingsIdx).toBeGreaterThanOrEqual(0);
  expect(policiesIdx).toBeGreaterThan(settingsIdx); // config before governance
});

it('does NOT render Policies under Runs & Approvals anymore', () => {
  renderSidebar({ view: 'runs' });
  // policies child no longer surfaces under runs filter expansion
  expect(screen.queryByTestId('sidebar-runs-child-policies')).toBeNull();
});
```

Run — fails.

### Step 5.4 — Update the Sidebar "System"/config footer group

In `Sidebar.tsx`, the footer block (lines ~271–301) currently renders a "System"
group with Settings + Coverage. Convert it to the "Configuration & Governance"
group rendering, in order, **Settings → Policies → Coverage**:

- Rename the group header from `System` to
  `labelForNavKey('config-governance')` and wrap the group in
  `data-testid="sidebar-config-group"`.
- After the existing Settings `NavItem`, add a Policies `NavItem` (look it up via
  `SURFACE_REGISTRY.find(e => e.viewKey === 'policies')`), `onClick={() =>
  setView('policies')}`, `active={view === 'policies'}`, carrying the
  **policy status badge** (`policyBadge`) — note: the badge currently hangs off
  the Settings item; move it onto the Policies item where the governance status
  belongs (per spec §5 "never present a policy without its status"). Keep
  Settings' badge-less.
- Keep Coverage where it is (still under this group is fine; it is CI surface
  coverage).
- Because `policies` is no longer a child of `runs`, the
  `visibleChildTabs('runs')` expansion will naturally drop it (it reads
  `parentSurface` from `SURFACE_REGISTRY`, which Phase 5.5 updates). For the
  `sidebar-runs-child-policies` assertion, ensure no hard-coded policies child
  remains under runs.

Run Step 5.3 — green. Run the full Sidebar regression.

### Step 5.5 — Update the registry SSOT + regenerate

Edit `contracts/gui/surface-registry.v1.yaml`, the `policies` entry (lines
164–171): change `parent_surface: runs` → `parent_surface: settings`, and (if a
`nav_group` value distinguishes the config group) set `nav_group: system` to
match Settings' group (Settings is `nav_group: system`). Update the `notes` to
note the co-location.

Regenerate from the repo root:

```
cd /c/Users/Owner/vox-graphify-gui && cargo run -p vox-cli -- ci gui-surface-registry --write
```

This rewrites `src/generated/surfaceRegistry.generated.ts` and the report JSON.
Confirm the generated `policies` row now reads `parentSurface: 'settings'`.

> If 3A has **not** landed and also edits this YAML, STOP and land 3A first (see
> the dependency note), then redo this step so the regeneration includes both
> changes.

### Step 5.6 — Full regression + commit

```
cd /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui && pnpm vitest run src/lib/navigation.test.ts src/components/layout/Sidebar.test.tsx src/components/surfaces/Settings && pnpm typecheck
```

```
cd /c/Users/Owner/vox-graphify-gui
git add contracts/gui/surface-registry.v1.yaml contracts/reports/gui-surface-registry.v1.json crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts crates/vox-gui/ui/src/lib/navigation.ts crates/vox-gui/ui/src/lib/navigation.test.ts crates/vox-gui/ui/src/components/layout/Sidebar.tsx crates/vox-gui/ui/src/components/layout/Sidebar.test.tsx
git commit -m "feat(gui-nav): co-locate Policies with Settings under Configuration & Governance (Plan 3C Phase 5)"
```

---

## Phase 6 — Final regression + drift gate [SEQUENTIAL]

_Batch D (terminal). Depends on: Phases 0–5 all green and committed. Verification
only — produces no commit._

### Step 6.1 — Full vitest + typecheck

```
cd /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui && pnpm vitest run && pnpm typecheck
```

Expect all green except any pre-existing failures unrelated to this work (the
project has known Axis-branding vitest fails per MEMORY — confirm the count is
unchanged, do not "fix" unrelated failures here).

### Step 6.2 — Surface-registry drift gate (no-write check)

```
cd /c/Users/Owner/vox-graphify-gui && cargo run -p vox-cli -- ci gui-surface-registry
```

Must print "registry and generated TS are up to date" (Phase 5.5 already wrote
them). If it reports drift, the generated TS was not committed — re-run
`--write`, re-commit.

### Step 6.3 — Stop. Do not push, do not merge.

Report the branch state to the user. The user commits the final state and
decides on merge.

---

## Self-Review

**Against the writing-plans discipline:**

- **Bite-sized, one concern per step:** Each phase is one stray setting or one
  nav change; each step is a single failing test → minimal implementation →
  green → commit.
- **Exact paths:** Every file is named with its full repo-relative path;
  commands are copy-pasteable with the right `cd`.
- **Real code, no placeholders:** Code blocks use the real command names
  (`set_active_model`, `set_vcs_isolation_strategy`, `gui.memory.autoRecall`,
  `policy_*`), the real `vox_settings_seed`/`vox-settings-seed` deep-link
  mechanism, the real generated-registry pipeline (`vox ci
  gui-surface-registry --write`), and the existing test-mock scaffold.
- **TDD throughout:** Every behavior change is preceded by a failing test and the
  exact `pnpm vitest run …` to prove red→green.
- **Frequent commits:** 7 commits (Phases 0–5 + per-stray), all bisectable.

**Against the ratified decisions:**

- ✅ Unification (b): nav group "Configuration & Governance", Settings + Policies
  **siblings**, not tabs; Settings ordered first; policy status badge moved onto
  the Policies item (governance status stays attached to governance); branch
  selector / `protected` locks untouched.
- ✅ Consolidation modest: Phase 1 is a nav re-grouping over existing `section
  ===` blocks (the flat `SECTIONS` array is replaced by `SETTINGS_DOMAINS`, the
  right-pane blocks are unchanged); 8 domains exactly as specified.
- ✅ 3 strays moved with command-as-SSOT preserved: active-model (additive to
  Settings, per-card stays), memory auto-recall (toggle moves, Memory keeps
  reacting to the pref, deep-link stub), VCS default (moves to **Agents &
  Orchestration**, per-agent override stays in Repository).
- ✅ gamify config (enabled + mode) grouped under Gamification domain; visual
  concepts untouched.
- ✅ Secrets remains the single shallow key store (one section, its own domain).
- ✅ Plan 3A dependency stated, with explicit gamify nav-move (3A) vs config-move
  (3C) separation and a YAML-collision ordering rule.

**Risks / things the executor must verify before coding (called out inline):**

1. `Toggle` nesting in Phase 2 — prefer a `testId` prop on `Toggle` over a
   wrapping button to keep the "all buttons type=button" invariant.
2. `IsolationPanel` may need a default-only render mode or a small extraction
   (Phase 3) — read it before deciding reuse vs. extract; use real
   `IsolationStrategy` ids in the testids.
3. `MemoryView`/`RepositoryView` props — read the actual `*ViewProps` before
   adding any `onNavigate`; the seed + event is the fallback if no nav prop
   exists.
4. The Sidebar policy badge currently hangs off Settings — moving it to Policies
   is a deliberate spec-§5 requirement, not an oversight.
5. The known pre-existing Axis-branding vitest fails must not be conflated with
   regressions in Phase 6.1.
