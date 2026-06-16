# vox-gui Action Manifest and Generic Forms Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Promote top 20 CLI surfaces to `generic_form` tier with a reusable `GenericCommandForm` component.

**Architecture:** Runtime manifest from `get_action_manifest` Tauri command (schema: `contracts/gui/action-manifest.v1.yaml`). React form renders `arguments[]` with safety class + confirmation policy. Catalog surface groups by `source_group` with badges.

**Tech Stack:** React 19, vitest, `vox ci gui-surface-registry`.

---

## Task 1: `GenericCommandForm` component

**Files:**
- Create: `crates/vox-gui/ui/src/components/ui/GenericCommandForm.tsx`
- Create: `crates/vox-gui/ui/src/components/ui/GenericCommandForm.test.tsx`

- [ ] **Step 1: Define manifest entry type** (mirror contract)

```typescript
export interface ManifestArgument {
  name: string;
  help: string;
  required: boolean;
  takes_value: boolean;
}

export interface ManifestAction {
  id: string;
  title: string;
  command: string;
  safety_class: 'read_only' | 'mutating' | 'destructive' | 'unknown';
  confirmation_policy: 'none' | 'recommended' | 'required';
  arguments: ManifestArgument[];
}
```

- [ ] **Step 2: Failing test — renders required string arg**

```typescript
it('renders text input for required string argument', () => {
  const action: ManifestAction = {
    id: 'ci.check',
    title: 'vox ci check',
    command: 'vox ci check',
    safety_class: 'read_only',
    confirmation_policy: 'none',
    arguments: [{ name: 'paths', help: 'Paths to check', required: false, takes_value: true }],
  };
  render(<GenericCommandForm action={action} onSubmit={vi.fn()} />);
  expect(screen.getByLabelText(/paths/i)).toBeDefined();
});
```

- [ ] **Step 3: Implement minimal form**

- String/bool args only in v1
- Submit calls `onSubmit(argv: string[])`
- Destructive + `confirmation_policy: required` → Radix `Dialog` confirm before submit

- [ ] **Step 4: Safety badge**

```tsx
<span className={cn('text-[9px] uppercase', safetyClassTone[action.safety_class])}>
  {action.safety_class}
</span>
```

- [ ] **Step 5: Commit**

---

## Task 2: `OutputViewer` for execute results

**Files:**
- Create: `crates/vox-gui/ui/src/components/ui/OutputViewer.tsx`
- Create: `crates/vox-gui/ui/src/components/ui/OutputViewer.test.tsx`

- [ ] **Step 1: Failing test**

```typescript
it('shows stdout and stderr in separate regions', () => {
  render(<OutputViewer result={{ exit_code: 1, stdout: 'ok', stderr: 'warn' }} />);
  expect(screen.getByText('ok')).toBeDefined();
  expect(screen.getByText('warn')).toBeDefined();
});
```

- [ ] **Step 2: Implement** — monospace scroll regions, `aria-label="Command output"`

- [ ] **Step 3: Wire Catalog** — on form submit:

```typescript
const res = await voxTransport.executeCommand(argv.join(' '));
setOutput(res);
```

- [ ] **Step 4: Commit**

---

## Task 3: Promote 20 surfaces in registry

**Files:**
- Modify: `contracts/gui/surface-registry.v1.yaml`

- [ ] **Step 1: Pick 20** (minimum set):

| Surface id | CLI group |
|------------|-----------|
| `ci-check` | `vox ci check` |
| `ci-pre-push` | `vox ci pre-push` |
| `ci-ssot-drift` | `vox ci ssot-drift` |
| `doctor` | `vox doctor` |
| `run-check` | `vox check` |
| `secrets-doctor` | `vox secrets doctor` |
| `audit-code` | `vox audit code` |
| … | (12 more from catalog `source_group: core`) |

- [ ] **Step 2: Set** `representation_tier: generic_form` for each

- [ ] **Step 3: Regenerate**

```bash
cargo run -q -p vox-cli -- ci gui-surface-registry --write
```

- [ ] **Step 4: Verify**

```bash
cargo run -q -p vox-cli -- ci gui-surface-registry
cargo run -q -p vox-cli -- ci gui-catalog-parity
```

- [ ] **Step 5: Commit**

---

## Task 4: Catalog UX — stop labeling CLI as Skills

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Catalog/Catalog.tsx`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Catalog/Catalog.test.tsx`

- [ ] **Step 1: Failing test**

```typescript
it('groups entries under Commands not Skills', () => {
  render(<Catalog pushToast={vi.fn()} skills={mockCatalog} />);
  expect(screen.queryByText(/^Skills$/i)).toBeNull();
  expect(screen.getByText(/Commands/i)).toBeDefined();
});
```

- [ ] **Step 2: Rename section headers; show `GenericCommandForm` when entry has manifest match**

- [ ] **Step 3: Commit**

---

## Exit criteria

- [ ] `GenericCommandForm` + `OutputViewer` tested
- [ ] ≥20 `generic_form` tiers in registry; gates green
- [ ] Catalog runs `vox ci check` via form with validation
- [ ] No "Skills" label on CLI catalog section
