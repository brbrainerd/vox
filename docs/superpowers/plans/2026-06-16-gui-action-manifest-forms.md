# vox-gui Action Manifest and Generic Forms Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Close the 69-surface `representation_tier: none` gap by generating runtime manifests from the CLI catalog and rendering `GenericCommandForm` for the top 20 operator commands.

**Architecture:** Contract: `contracts/gui/action-manifest.v1.yaml`. Generator: clap catalog + `contracts/operations/catalog.v1.yaml` → runtime JSON via `get_action_manifest` Tauri command. React: `GenericCommandForm` with typed args, safety class, confirmation, output viewer.

**Tech Stack:** Rust codegen, React Hook Form or controlled inputs, `vox ci gui-surface-registry`.

> **References:** `docs/src/architecture/cli-gui-surface-coverage-map-2026.md`, `contracts/gui/surface-registry.v1.yaml`.

---

## Task 1: Generator parity gate

**Files:**
- Read: `crates/vox-gui/src/commands/action_manifest.rs`
- Modify: `crates/vox-cli/src/commands/ci/` (gui gates)

- [ ] **Step 1:** Ensure `vox ci gui-catalog-parity` validates manifest schema against contract
- [ ] **Step 2:** Fail when clap path missing from manifest for `generic_form` tier surfaces
- [ ] **Step 3:** Document regen: `vox ci config-gui-codegen --write`

---

## Task 2: `GenericCommandForm` component

**Files:**
- Create: `ui/src/components/ui/GenericCommandForm.tsx`
- Create: `ui/src/components/ui/GenericCommandForm.test.tsx`

- [ ] **Step 1:** Render args from manifest entry (string, bool, enum, path)
- [ ] **Step 2:** Safety badges: `read_only` | `mutating` | `destructive`
- [ ] **Step 3:** Required confirmation for `destructive` + `confirmation_policy: required`
- [ ] **Step 4:** Submit via `voxTransport.executeCommand`; show stdout/stderr in `<OutputViewer>`

---

## Task 3: Promote top 20 surfaces

**Files:**
- Modify: `contracts/gui/surface-registry.v1.yaml`

Priority groups (by operator frequency):
1. `ci` — check, pre-push, ssot-drift
2. `doctor`
3. `run` / `check`
4. `secrets` doctor/parity
5. `audit` code/arch

- [ ] **Step 1:** Set `representation_tier: generic_form` for 20 entries
- [ ] **Step 2:** Run `vox ci gui-surface-registry --write`
- [ ] **Step 3:** Wire Catalog surface to render forms instead of raw command strings

---

## Task 4: Catalog surface UX

**Files:**
- Modify: `ui/src/components/surfaces/Catalog/Catalog.tsx`

- [ ] **Step 1:** Stop labeling CLI entries as "Skills"
- [ ] **Step 2:** Group by `source_group` with safety badges
- [ ] **Step 3:** Vitest: Catalog renders form for `ci check` mock manifest entry

---

## Exit criteria

- `vox ci gui-surface-registry` passes with ≥20 `generic_form` tiers
- Catalog runs `ci`, `doctor`, `check` with generated forms + validation
- Manifest schema version bumped only via contract workflow
