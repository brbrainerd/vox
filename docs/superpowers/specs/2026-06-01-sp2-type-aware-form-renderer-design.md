# SP-2: Type-Aware Generated Form Renderer — Design

**Date:** 2026-06-01
**Status:** Approved (scope confirmed)
**Umbrella:** [`2026-06-01-cli-gui-hybrid-spine-design.md`](2026-06-01-cli-gui-hybrid-spine-design.md) (Unit 2)
**Depends on:** SP-1 (typed arguments) — landed.

## Scope decision (refinement of the umbrella SP-2)

Ground-truth exploration of the React frontend changed SP-2's shape:

- The Catalog view (`CommandCatalogForm.tsx`) **already renders a generic form for every
  command** (checkbox for flags, text for values) and runs it through
  `voxTransport.callTool` → `execute_command`. So "Tier-3 surfaces appear as generated panels"
  is already partly true.
- The form **ignores SP-1's new typed data** (`value_kind`, `possible_values`, `default_values`):
  enum arguments get a free-text box instead of a dropdown, counts get text, defaults are not
  prefilled. This is the real, high-value gap.
- A formal **decorator registry would be a hollow seam now**: the App's `renderView()` switch is
  already a surface→component registry, and `GamifyView` is already a hand-built decorator for the
  `gamify` surface. Building a separate registry with no consumer would violate the no-stubs rule.
  It has a real consumer in **SP-4 (Scientia)** and is deferred there.

**SP-2 is therefore the type-aware generated renderer.** The decorator-registry seam moves to SP-4.

## Goal

Make the generated command form render type-accurate inputs from SP-1's argument metadata, so
every command across every surface (including all `scientia *` / `ludus *` commands) gets a correct
control and prefilled defaults.

## Design

Single file: `crates/vox-gui/ui/src/components/CommandCatalogForm.tsx`.

### Control selection (pure helper)

Add an exported pure function so the mapping is explicit and reviewable:

```ts
export type ArgControl = 'flag' | 'select' | 'count' | 'text';

export function argControl(arg: CommandCatalogArgument): ArgControl {
  if (arg.value_kind === 'flag') return 'flag';
  if (arg.value_kind === 'count') return 'count';
  if (arg.possible_values && arg.possible_values.length > 0) return 'select';
  return 'text';
}
```

### Rendering

Replace the `isFlag ? checkbox : text` branch with a switch on `argControl(arg)`:

- `flag` → checkbox (existing behavior).
- `count` → `<input type="number" min="0">`.
- `select` → `<select>` with one `<option>` per `possible_values` entry, plus a leading empty option
  ("—") when the argument is not required.
- `text` → text input (existing behavior).

The argument is typed `CommandCatalogArgument` (was `any`).

### Default prefill

On command select (`handleCommandSelect`), seed `argValues` from each argument's `default_values[0]`
for `value`/`count`/`select` controls (flags stay unchecked unless a default of `"true"` is present).
This makes the form reflect the CLI's real defaults instead of starting blank.

### Execution path — unchanged

`handleExecute` already maps `argValues` (string → `--key value`, boolean → `--key`) and calls
`voxTransport.callTool`. A `<select>` produces a string value and a number input a numeric string, so
both flow through the existing mapping with no change to the run path.

## Non-goals / known limits

- **Count → repeated short flag** (e.g. `-vvv`) is not synthesized; the numeric value is passed
  through as-is. Count args are rare in the catalog; perfecting this is deferred (YAGNI).
- No decorator registry (moved to SP-4).
- No new JS unit-test runner: this repo has no `tsc`/vitest setup for the UI; its verification bar is
  `vite build` (the `lint` script). SP-2 verifies via `pnpm build` plus the extracted pure `argControl`
  helper being trivially inspectable. Adding a UI test runner is out of scope.

## Verification

- `pnpm --dir crates/vox-gui/ui build` succeeds (baseline already green).
- Manual review of the `argControl` mapping and the four render branches.
