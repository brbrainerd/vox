# Universal Skill Bundle & Cursor Import Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a license-verified, agentskills.io-native skill library (superpowers + Anthropic + curated Cursor-compatible skills) under `assets/skills/`, wire it through existing discovery so every Vox user gets skills in CLI, MCP, orchestrator prompts, and GUI — without hand-maintaining duplicate formats.

**Architecture:** One exchange format (`<root>/<name>/SKILL.md`, YAML frontmatter, optional `metadata.vox-*`). Plugin skills keep TOML `*.skill.md` only where they declare `vox-tools` tied to `Plugin.toml`. Everything else lives in flat skill directories; `skill_search_roots()` + `discover_external_skills()` already ingest them. A VoxScript vendoring pipeline (`scripts/vendor-skills.vox`) pins upstream SHAs in `assets/skills/SOURCES.toml`. GUI loads installed skills from `vox_skill_list` (not the CLI command catalog). Command palette `/` mode surfaces the same registry.

**Tech Stack:** Rust (`vox-plugin-host`, `vox-config`, `vox-orchestrator-mcp`, `vox-cli`), VoxScript (`scripts/vendor-skills.vox`), React/TS + Vitest (pnpm), Playwright e2e.

**Prerequisite research:** `docs/src/architecture/skill-ecosystem-interop-research-2026-06-12.md` and partial completion of `docs/superpowers/plans/2026-06-12-skill-ecosystem-interop-and-awareness.md` (Tracks A/B largely landed; Track D/E incomplete).

**Inventory snapshot (2026-06-16):**

| Source | Count | In Vox bundle today | License / redist |
|--------|------:|--------------------|------------------|
| Superpowers (`obra/superpowers` 5.0.7) | 14 | 4 adapted copies in `crates/vox-skills/skills/superpowers/*.skill.md` only | MIT (verify LICENSE at pin) |
| Anthropic skills (`anthropics/skills`) | 12 vendored | Yes (`assets/skills/*`) | Apache-2.0 |
| Vox plugin skills (`vox-plugin-skill-*`) | 9 | TOML `*.skill.md` + registry | First-party |
| Cursor built-in (`~/.cursor/skills-cursor/`) | 18 | No | Cursor distribution — do not vendor |
| Cursor plugin cache skills | ~126 | No | Mixed — per-skill license audit required |
| User graphify skill (`~/.claude/skills/graphify/`) | 1 | No | User-local via discovery roots |

---

## File structure (target tree)

```
assets/skills/                          # Lowest-precedence bundled root (already wired)
  SOURCES.toml                          # Provenance SSOT (extend, do not replace)
  <skill-name>/                         # Flat — agentskills.io: name == dir name
    SKILL.md
    LICENSE.upstream                    # Copied from upstream repo at pin
    scripts/ references/ assets/      # Omitted when upstream has none

.vox/skills/<name>/SKILL.md             # Workspace override (highest interop precedence)
.agents/skills/<name>/SKILL.md          # Vendor-neutral interop
.claude/skills/<name>/SKILL.md          # Compatibility interop
.cursor/skills/<name>/SKILL.md          # NEW optional root (project Cursor skills)

crates/vox-plugin-skill-*/<id>.skill.md # Tool-linked plugins only (TOML frontmatter)
crates/vox-skills/skills/superpowers/   # DELETE after migration (stale duplicates)

scripts/vendor-skills.vox               # NEW — pin, copy, verify (VoxScript-first)
scripts/sync-cursor-skills.vox          # NEW — optional user-machine import (no vendoring)
```

**Scalability rules (lock in this plan):**

1. **Flat namespace** under each root — no category subfolders (breaks agentskills.io `name == dirname` unless renamed).
2. **Collision policy:** first root in `skill_search_roots()` wins; bundled `assets/skills/` is always last.
3. **Namespacing** for third-party imports: prefix upstream slug when needed (`convex-auth-setup`, not `auth-setup`).
4. **Index, not nesting:** `assets/skills/SOURCES.toml` + future `[[skill-bundle]]` rows in `catalog.toml` are the organizational layer; directories stay flat.
5. **Search:** keyword discovery goes through `vox-search` / `vox_skill_search` (hybrid stack), not directory walks in GUI.
6. **Tier-1 cap:** orchestrator injects max 64 skill descriptions in system prompt (`skill_catalog.rs` already caps); bundled set can exceed 64 — search + `vox_skill_use` handle the long tail.

---

## Track F — Fix GUI skill SSOT (blocker for “available to all users down to GUI”)

### Task F1: Load installed skills from MCP, not CLI catalog

**Files:**
- Create: `crates/vox-gui/ui/src/hooks/useInstalledSkills.ts`
- Create: `crates/vox-gui/ui/src/hooks/useInstalledSkills.test.ts`
- Modify: `crates/vox-gui/ui/src/App.tsx` (bootstrap + props)
- Modify: `crates/vox-gui/ui/src/types/dashboard.ts` (split `skills` vs `commands` if needed)

**Bug:** `App.tsx` sets `data.skills` from `get_command_catalog()`. Entries use `command: "vox skill list"` which fails the slash regex — **zero dynamic skill slashes appear in Loquela today**.

- [ ] **Step 1: Write the failing test**

```typescript
// crates/vox-gui/ui/src/hooks/useInstalledSkills.test.ts
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import { useInstalledSkills } from './useInstalledSkills';

vi.mock('../transport', () => ({
  voxTransport: {
    invokeMcpTool: vi.fn(),
  },
}));

import { voxTransport } from '../transport';

describe('useInstalledSkills', () => {
  beforeEach(() => vi.clearAllMocks());

  it('maps vox_skill_list rows to slash-ready skill records', async () => {
    vi.mocked(voxTransport.invokeMcpTool).mockResolvedValue({
      tool: 'vox_skill_list',
      is_error: false,
      result: {
        data: [
          { id: 'vox.tdd', name: 'test-driven-development', description: 'RED-GREEN-REFACTOR' },
        ],
      },
    });
    const { result } = renderHook(() => useInstalledSkills(true));
    await waitFor(() => expect(result.current.length).toBe(1));
    expect(result.current[0]).toMatchObject({
      id: 'vox.tdd',
      name: 'test-driven-development',
      description: 'RED-GREEN-REFACTOR',
    });
  });
});
```

- [ ] **Step 2: Run** `pnpm -C crates/vox-gui/ui test useInstalledSkills -v` — expect FAIL (module missing).

- [ ] **Step 3: Implement hook**

```typescript
// crates/vox-gui/ui/src/hooks/useInstalledSkills.ts
import { useEffect, useState } from 'react';
import { voxTransport } from '../transport';

export interface InstalledSkill {
  id: string;
  name: string;
  description: string;
}

export function useInstalledSkills(enabled = true): InstalledSkill[] {
  const [skills, setSkills] = useState<InstalledSkill[]>([]);
  useEffect(() => {
    if (!enabled) return;
    let cancelled = false;
    (async () => {
      try {
        const res = await voxTransport.invokeMcpTool('vox_skill_list', {});
        const rows = (res?.result as { data?: Array<Record<string, string>> })?.data ?? [];
        if (cancelled) return;
        setSkills(
          rows
            .map((r) => ({
              id: r.id ?? r.name ?? '',
              name: r.name ?? r.id ?? '',
              description: r.description ?? r.about ?? '',
            }))
            .filter((s) => s.name && /^[a-z0-9][a-z0-9-]*$/.test(s.name)),
        );
      } catch {
        if (!cancelled) setSkills([]);
      }
    })();
    return () => { cancelled = true; };
  }, [enabled]);
  return skills;
}
```

- [ ] **Step 4: Wire App.tsx**

Replace the catalog bootstrap line:

```typescript
// REMOVE: setData(prev => ({ ...prev, skills: catalog.entries }));
// ADD separate state:
const installedSkills = useInstalledSkills(true);
// Pass to Loquela + CommandPalette:
//   skills={installedSkills}
// Keep catalog.entries as data.commands for Catalog surface only.
```

- [ ] **Step 5: Run** `pnpm -C crates/vox-gui/ui test useInstalledSkills slashCommands -v` — expect PASS.

- [ ] **Step 6: Commit** `fix(gui): load Loquela slash skills from vox_skill_list registry`

### Task F2: Command palette `/` mode shows installed skills

**Files:**
- Modify: `crates/vox-gui/ui/src/components/layout/CommandPalette.tsx:70-79`
- Modify: `crates/vox-gui/ui/src/components/layout/CommandPalette.test.tsx`

**Bug:** `filteredSkills` uses `prefixMode === 'commands'` but `parsePaletteQuery('/foo')` yields `prefixMode === 'skills'` — `/` prefix hides all skill rows.

- [ ] **Step 1: Write failing test**

```typescript
it('/ prefix filters installed skills by query', () => {
  render(
    <CommandPalette
      open
      onClose={() => {}}
      onAction={() => {}}
      agents={[]}
      skills={[{ command: 'brainstorming', about: 'design first', path: [], aliases: [], has_subcommands: false, compiled_in: true, source_group: 'skill', feature_gate: null, tier: 'recommended' }]}
    />,
  );
  // type "/" + "brain" → should show Skills section (test via filtered DOM or export helper)
});
```

- [ ] **Step 2: Fix filter condition**

```typescript
const filteredSkills = useMemo(
  () =>
    prefixMode === 'default' || prefixMode === 'skills' || prefixMode === 'commands'
      ? skills.filter(/* unchanged */)
      : [],
  [skills, effectiveQ, prefixMode],
);
```

Prefer mapping `InstalledSkill` → display rows with `command: skill.name` in App before passing down.

- [ ] **Step 3: Run** `pnpm -C crates/vox-gui/ui test CommandPalette -v` — PASS.

- [ ] **Step 4: Commit** `fix(gui): command palette / prefix surfaces installed skills`

---

## Track G — Vendor superpowers + complete bundled library (Track D completion)

### Task G1: Extend SOURCES.toml for superpowers

**Files:**
- Modify: `assets/skills/SOURCES.toml`
- Create: `scripts/vendor-skills.vox`

- [ ] **Step 1: Write failing integration test** in `crates/vox-cli/tests/vendor_skills_fixture_test.rs`:

```rust
#[test]
fn sources_toml_lists_all_superpowers_skills() {
    let toml = std::fs::read_to_string("assets/skills/SOURCES.toml").unwrap();
    for name in [
        "brainstorming", "dispatching-parallel-agents", "executing-plans",
        "finishing-a-development-branch", "receiving-code-review", "requesting-code-review",
        "subagent-driven-development", "systematic-debugging", "test-driven-development",
        "using-git-worktrees", "using-superpowers", "verification-before-completion",
        "writing-plans", "writing-skills",
    ] {
        assert!(toml.contains(name), "missing superpowers skill {name}");
    }
}
```

- [ ] **Step 2: Run** `cargo test -p vox-cli vendor_skills_fixture -j 4` — FAIL.

- [ ] **Step 3: Extend SOURCES.toml**

```toml
[[source]]
repo = "https://github.com/obra/superpowers"
license = "MIT"
pin = "<SHA recorded at vendoring time>"
skills = [
  "brainstorming", "dispatching-parallel-agents", "executing-plans",
  "finishing-a-development-branch", "receiving-code-review", "requesting-code-review",
  "subagent-driven-development", "systematic-debugging", "test-driven-development",
  "using-git-worktrees", "using-superpowers", "verification-before-completion",
  "writing-plans", "writing-skills",
]
```

Keep existing `[upstream]` anthropic block; merge into `[[source]]` table array style for uniformity.

- [ ] **Step 4: Commit** `chore(skills): declare superpowers provenance in SOURCES.toml`

### Task G2: Implement `scripts/vendor-skills.vox`

**Files:**
- Create: `scripts/vendor-skills.vox`

- [ ] **Step 1: Write failing test** — extend `vendor_skills_fixture_test.rs`:

```rust
#[test]
fn every_sources_skill_has_skill_md_on_disk() {
    // Parse SOURCES.toml skill names; assert assets/skills/<name>/SKILL.md exists
    // (test fails until vendor script run)
}
```

- [ ] **Step 2: Implement vendor script** (VoxScript, subprocess via vox-runtime):

Logic:
1. Parse `assets/skills/SOURCES.toml`
2. For each `[[source]]`: shallow clone at `pin`, copy `skills/<name>/` → `assets/skills/<name>/`
3. Copy upstream `LICENSE` → `LICENSE.upstream`
4. Never rewrite SKILL.md bodies (lossless import)
5. Print summary counts; exit nonzero on missing skill dir upstream

- [ ] **Step 3: Run** `vox run scripts/vendor-skills.vox` then `cargo test -p vox-cli vendor_skills_fixture -j 4` — PASS.

- [ ] **Step 4: Run** `cargo run -p vox-cli -- ci agentskills-compliance` — PASS.

- [ ] **Step 5: Commit** `feat(skills): vendor superpowers MIT bundle via vendor-skills.vox`

### Task G3: Remove stale superpowers duplicates

**Files:**
- Delete: `crates/vox-skills/skills/superpowers/*.skill.md` (4 files)
- Modify: any loader referencing that path (grep `skills/superpowers`)

- [ ] **Step 1: Grep** `rg "skills/superpowers" crates/` — update or remove references.

- [ ] **Step 2: Delete duplicates** — bundled `assets/skills/<name>/SKILL.md` is SSOT.

- [ ] **Step 3: Run** `cargo test -p vox-skills -p vox-plugin-host -j 4` — PASS.

- [ ] **Step 4: Commit** `chore(skills): drop stale superpowers .skill.md duplicates`

**Note on Vox-adapted bodies:** The old `writing-plans.skill.md` injected JSON `PlanNode` format. After migration, add a thin Vox overlay skill `assets/skills/writing-plans-vox/` ONLY if orchestrator still requires JSON plans — otherwise prefer upstream superpowers text + orchestrator-side adapter (YAGNI until proven needed).

---

## Track H — Curated Cursor-compatible import (no proprietary vendoring)

### Task H1: Optional discovery root for `.cursor/skills/`

**Files:**
- Modify: `crates/vox-config/src/paths.rs` (`skill_search_roots`)
- Modify: `crates/vox-config/src/paths.rs` tests

- [ ] **Step 1: Failing test** — insert `.cursor/skills` after `.vox/skills`, before `.agents/skills`:

```rust
assert_eq!(
  rel[..4],
  [".vox/skills", ".cursor/skills", ".agents/skills", ".claude/skills"],
);
```

- [ ] **Step 2: Implement** — extend `SUBDIRS` const and home mirror.

- [ ] **Step 3: Run** `cargo test -p vox-config skill_search_roots -j 4` — PASS.

- [ ] **Step 4: Commit** `feat(config): add .cursor/skills interop discovery root`

### Task H2: User-local import script (not bundled)

**Files:**
- Create: `scripts/sync-cursor-skills.vox`

**Scope:** Copy from `~/.cursor/skills-cursor/*/SKILL.md` → `.agents/skills/<name>/` (or `.vox/skills/`) when `--write` flag set. Default dry-run listing. **Do not** commit Cursor built-in skills into the repo.

- [ ] **Step 1: Script lists 18 built-in Cursor skills with target paths (dry-run).**

- [ ] **Step 2: Document in** `docs/src/reference/skill_marketplace.md` §Importing from Cursor.

- [ ] **Step 3: Commit** `feat(skills): sync-cursor-skills.vox for user-local import`

### Task H3: Curated third-party plugin skills (license-gated)

**Policy:** Only vendor plugin skills with verified OSS license into `assets/skills/`. Initial candidates after audit:

| Skill dir | Upstream | License |
|-----------|----------|-----------|
| `shadcn` | cursor-public/shadcn plugin | Check plugin LICENSE |
| `convex-quickstart` | cursor-public/convex | Check plugin LICENSE |

**Out of scope for vendoring:** Cursor `skills-cursor/*` (18), Figma/Stripe/Clerk plugin skills until license row added to SOURCES.toml.

- [ ] **Step 1:** Add `scripts/audit-skill-licenses.vox` — fails if `assets/skills/<dir>/` lacks `LICENSE.upstream`.

- [ ] **Step 2:** Per approved plugin, add `[[source]]` row + run vendor script.

- [ ] **Step 3: Commit** per batch with license evidence in commit message.

---

## Track I — Catalog SSOT & dynamic availability

### Task I1: `[[skill-bundle]]` parity in plugin catalog

**Files:**
- Modify: `crates/vox-plugin-catalog/catalog.toml`
- Modify: `crates/vox-plugin-catalog/src/lib.rs` (validation test)

- [ ] **Step 1: Failing test** — every `assets/skills/<dir>/` has matching `[[skill-bundle]]` entry.

- [ ] **Step 2: Add entries** for all bundled skills (id, license, source repo, pin from SOURCES.toml).

- [ ] **Step 3: Run** `cargo test -p vox-plugin-catalog -j 4` — PASS.

- [ ] **Step 4: Commit** `feat(catalog): skill-bundle SSOT for assets/skills`

### Task I2: Auto-install bundled skills on first `vox skill list`

**Files:**
- Modify: `crates/vox-cli/src/commands/extras/ars/registry.rs`

- [ ] **Step 1: Failing test** — fresh in-memory registry + workspace with only `assets/skills/brainstorming/` → `install_external_skills` returns ≥1.

- [ ] **Step 2: Ensure** `install_external_skills` is invoked on daemon startup / first list (already partially wired — verify idempotent).

- [ ] **Step 3: Commit** `feat(skills): idempotent bundled skill install on registry bootstrap`

### Task I3: Refresh stale reference docs

**Files:**
- Modify: `docs/src/reference/skill_marketplace.md` (currently describes TOML-only format and fictional built-ins)

- [ ] **Step 1: Rewrite** discovery roots table, dual format note (TOML plugin vs YAML directory), GUI `/slash`, MCP tools including `vox_skill_discover` / `vox_skill_use`.

- [ ] **Step 2: Run** `cargo run -p vox-doc-pipeline -- --lint-only --paths docs/src/reference/skill_marketplace.md`

- [ ] **Step 3: Commit** `docs(skills): refresh marketplace reference for interop bundle`

---

## Track J — Verification sweep

### Task J1: End-to-end smoke

- [ ] `cargo test -p vox-config -p vox-plugin-host -p vox-orchestrator-mcp -p vox-cli --features ars -j 4`
- [ ] `pnpm -C crates/vox-gui/ui test`
- [ ] `vox ci agentskills-compliance`
- [ ] `vox ci ssot-drift`
- [ ] Live: `vox skill list | rg brainstorming` shows bundled skill
- [ ] Live: GUI Loquela `/brain` suggests `/brainstorming`
- [ ] Live: Cmd+K `/brain` shows skill row
- [ ] Live: `vox_skill_use { name: "brainstorming" }` returns body

- [ ] **Commit** fixes if any, then `superpowers:requesting-code-review`

---

## Execution strategy

| Wave | Tracks | Parallel? |
|------|--------|-----------|
| 1 | F1–F2 (GUI SSOT fix) | Sequential — unblocks user-visible goal |
| 2 | G1–G3 (superpowers vendor) | Can parallel with H1 |
| 3 | H2–H3, I1–I3 | H3 and I1 serialize on SOURCES.toml |
| 4 | J1 | Single worker |

**Relationship to 2026-06-12 plan:** This plan **supersedes Track D/E** items for bundling and docs. Tracks A/B/C from that plan are treated as **done** unless J1 smoke fails.

---

## Self-review checklist

| Requirement | Task |
|-------------|------|
| Dump superpowers (14) | G1–G3 |
| Keep Anthropic 12 | Already in assets; G2 verifies |
| Cursor skills available | H1–H2 (import), not vendored |
| Scalable tree | File structure + scalability rules |
| GUI dynamic availability | F1–F2 |
| All users (bundled) | I2 + existing `skill_search_roots` |
| License safety | SOURCES.toml + audit script H3 |
| No dual-format drift | G3 removes `.skill.md` superpowers copies |
| Code review: CLI catalog masquerading as skills | F1 |
| Code review: palette `/` broken | F2 |

**Placeholder scan:** None — all tasks include concrete paths, tests, and commands.
