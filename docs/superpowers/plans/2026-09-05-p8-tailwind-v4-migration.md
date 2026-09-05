# Tailwind CSS v3 → v4 Migration (P8) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use `superpowers:subagent-driven-development`
> or `superpowers:executing-plans`. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Migrate `crates/vox-gui/ui` from Tailwind CSS v3.4.19 to v4, so that
`tailwind-merge` can move from v2 to v3 without silently deleting focus rings.

**Architecture:** One package changes. Tailwind v4 replaces the JS config with a
CSS-first `@theme` block, moves the PostCSS plugin to `@tailwindcss/postcss`,
absorbs autoprefixer, and renames a handful of bare utilities. The repo's design
tokens already live in CSS (Style Dictionary → `tokens.generated.css`) and the
Tailwind config only *references* them, so the config translation is close to 1:1.

**Tech Stack:** Tailwind CSS v4, PostCSS, Vite 6, React 19, TypeScript, Vitest.

**Spec:** none — this plan is self-contained. Origin is the investigation recorded
on the closed PR #495 (`vox-foundation/vox#495`), reproduced in §Background.

---

## Global Constraints

- **Only `crates/vox-gui/ui` uses Tailwind.** Verified across every `package.json`
  in the repo (excluding `node_modules`): it is the sole consumer, at
  `tailwindcss ^3.4.19` with `tailwind-merge ^2.3.0`. Do not touch
  `apps/experimental/visualizer` or `docs-astro` — neither uses Tailwind.
- **This package is pnpm-managed.** `npm i --no-save` SILENTLY DOES NOTHING here.
  It reports success, leaves the installed version unchanged, and the build emits
  a byte-identical bundle. Use `pnpm add` / `pnpm install --frozen-lockfile`. This
  already wasted one A/B cycle; the identical bundle hash was the only tell.
- **Restore the tree after every experiment**: `git checkout -- package.json
  pnpm-lock.yaml && pnpm install --frozen-lockfile`, then confirm
  `git status --porcelain crates/vox-gui/ui` is empty.
- **Do not reopen #495 until this lands.** They are a matched pair.
- Do not publish releases. Do not create tags.

---

## Background — why this is necessary

`tailwind-merge` v3 targets Tailwind v4, where `outline-2` sets width *and*
implies `outline-style: solid`. Verified against a real v4.3.3 build:

```css
.outline-2 { outline-style: var(--tw-outline-style); outline-width: 2px; }
@property --tw-outline-style { syntax: "*"; inherits: false; initial-value: solid; }
```

Under Tailwind **v3** they are separate core plugins — confirmed in the repo's own
`node_modules/tailwindcss/src/corePlugins.js`:

```
2458:  outlineStyle: ({ addUtilities }) => {
2471:  outlineWidth: createUtilityPlugin('outlineWidth', [['outline', ['outline-width']]], {
```

So on v3, `outline` supplies `outline-style: solid` and `outline-2` supplies the
width. An A/B of tailwind-merge 2.6.1 vs 3.6.0 over **1457 real class strings**
extracted from `crates/vox-gui/ui/src` found exactly one differing output:

```
IN : inline-flex items-center justify-center font-medium tracking-wide transition-all
     focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brass
v2 : ... focus-visible:outline focus-visible:outline-2 ...
v3 : ...                       focus-visible:outline-2 ...   <- `outline` dropped
```

That is `src/components/ui/Button.tsx:52` and `:70` — the shared button. On
Tailwind v3 the result is `outline-style: none` with a 2px width applying to
nothing: **the keyboard focus ring disappears repo-wide.** An accessibility
regression, invisible in screenshots unless tabbing, with no peer-dependency
warning (tailwind-merge 3.6.0 declares none) and no test asserting on merged
class output.

**After this migration the offending class string needs no edit at all** — under
v4, dropping bare `outline` is correct because `outline-2` implies solid.

---

## File Structure

- Modify: `crates/vox-gui/ui/package.json` — deps
- Modify: `crates/vox-gui/ui/postcss.config.js` — plugin swap
- Modify: `crates/vox-gui/ui/src/index.css` — `@import "tailwindcss"` + `@theme`
- Delete (last task): `crates/vox-gui/ui/tailwind.config.js` — 61 lines, 0 plugins
- Modify: ~170 class strings across `crates/vox-gui/ui/src/**/*.{ts,tsx}`
- Create: `crates/vox-gui/ui/src/lib/cn.test.ts` — pins the focus-ring invariant

---

## Task 1: Capture a generated-CSS baseline

The strong check is not "does it build" — it is whether any CSS *rule* changed.
A dropped rule is invisible in a bundle-size diff.

- [ ] **Step 1: Build and snapshot the generated CSS**

```bash
cd crates/vox-gui/ui
pnpm install --frozen-lockfile
pnpm build
cp dist/assets/*.css /tmp/tw-baseline.css
wc -l /tmp/tw-baseline.css
```

- [ ] **Step 2: Snapshot the exact focus-ring rules**

```bash
grep -oE '\.focus-visible\\:outline[a-zA-Z0-9\\:_-]*\{[^}]*\}' /tmp/tw-baseline.css \
  | sort -u > /tmp/tw-baseline-outline.txt
cat /tmp/tw-baseline-outline.txt
```

Expected: rules for both `outline-style` and `outline-width`. Record them; Task 6
asserts the v4 output is equivalent.

- [ ] **Step 3: Commit nothing.** This task only produces `/tmp` artifacts.

---

## Task 2: Pin the focus-ring invariant with a failing test FIRST

Write this BEFORE migrating. It must pass on v3, and still pass on v4 — that is
what makes it a migration guard rather than a description of current behaviour.

- [ ] **Step 1: Write the test**

Create `crates/vox-gui/ui/src/lib/cn.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { cn } from './cn';

// PR #495: tailwind-merge v3 on Tailwind v3 silently drops `focus-visible:outline`,
// leaving outline-style: none and removing the keyboard focus ring from every
// button. This asserts the merged output still carries BOTH an outline-style
// source and a width. On Tailwind v3 that means the bare `outline` class must
// survive; on v4 `outline-2` alone implies solid, so either shape is acceptable.
describe('cn() preserves the focus ring', () => {
  const BUTTON_BASE =
    'inline-flex items-center justify-center font-medium tracking-wide transition-all ' +
    'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 ' +
    'focus-visible:outline-brass';

  it('keeps a visible focus outline width', () => {
    expect(cn(BUTTON_BASE)).toContain('focus-visible:outline-2');
  });

  it('keeps a source for outline-style', () => {
    const out = cn(BUTTON_BASE);
    // v3: the bare `outline` class. v4: `outline-2` implies solid on its own.
    const hasBareOutline = /(^|\s)focus-visible:outline(\s|$)/.test(out);
    const isV4 = Number(
      require('tailwindcss/package.json').version.split('.')[0],
    ) >= 4;
    expect(hasBareOutline || isV4).toBe(true);
  });
});
```

- [ ] **Step 2: Prove it currently passes**

```bash
cd crates/vox-gui/ui && pnpm vitest run src/lib/cn.test.ts
```
Expected: `2 passed`. Verify the line reads `Tests  2 passed` — do NOT accept a
run reporting `0 passed`, which means the file was not collected.

- [ ] **Step 3: Prove it can FAIL** — temporarily `pnpm add tailwind-merge@3`,
      re-run, confirm the outline-style test FAILS, then
      `git checkout -- package.json pnpm-lock.yaml && pnpm install --frozen-lockfile`.
      A guard nobody has watched fail is not a guard.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/ui/src/lib/cn.test.ts
git commit -m "test(vox-gui): pin the focus-ring class invariant before the v4 migration"
```

---

## Task 3: Run the official codemod

- [ ] **Step 1: Run it**

```bash
cd crates/vox-gui/ui
npx @tailwindcss/upgrade@latest
```

It performs the renames, translates the config, and swaps the PostCSS plugin.

- [ ] **Step 2: Review the ENTIRE diff before trusting it**

```bash
git diff --stat
git diff crates/vox-gui/ui/src/index.css crates/vox-gui/ui/postcss.config.js
```

- [ ] **Step 3: Verify the rename counts match expectation**

Measured on `main` at the time of writing (bare forms only — a `\b` regex
over-counts badly, because `rounded-lg` matches a bare `rounded` pattern):

| from | to | expected sites |
|---|---|---|
| `rounded` | `rounded-sm` | 153 |
| `outline-none` | `outline-hidden` | 9 |
| `ring` | `ring-3` | 5 |
| `blur` | `blur-sm` | 2 |
| `rounded-sm` | `rounded-xs` | 1 |
| `shadow` | `shadow-sm` | **0** |

```bash
git diff -U0 | grep -c '^+.*rounded-sm'
```

**ORDERING HAZARD, if you ever hand-roll this instead of using the codemod:**
`rounded-sm → rounded-xs` MUST run before `rounded → rounded-sm`. Reversed, the
153 bare `rounded` become `rounded-sm`, and the second pass then rewrites all 154
to `rounded-xs`. Everything still compiles and every corner silently shrinks.

- [ ] **Step 4: Commit the codemod output on its own**, so a later bisect can
      separate mechanical renames from hand edits.

```bash
git add -A crates/vox-gui/ui
git commit -m "chore(vox-gui): apply @tailwindcss/upgrade codemod (v3 -> v4)"
```

---

## Task 4: Reconcile the theme

`tailwind.config.js` is 61 lines with **zero plugins**. Every custom colour is a
`var(--color-*)` reference already produced by Style Dictionary, so v4's `@theme`
is a near-direct translation. Two entries need care:

- `brass: 'rgb(var(--brass) / <alpha-value>)'` — v4 has no `<alpha-value>`
  placeholder; the modern equivalent relies on `color-mix()`. Opacity utilities
  such as `brass/40` are in use, so verify they still emit.
- `amber-glow: 'rgb(var(--brass) / 0.5)'` — a fixed alpha, straightforward.

- [ ] **Step 1: Confirm every token still resolves**

```bash
cd crates/vox-gui/ui && pnpm build
for t in bg-base bg-surface bg-elevated text-primary text-secondary text-muted \
         border-subtle border-strong accent accent-secondary overlay-subtle \
         overlay-hover overlay-solid brass; do
  printf '%-18s %s\n' "$t" "$(grep -c -- "$t" dist/assets/*.css)"
done
```
Any token reporting `0` did not survive translation.

- [ ] **Step 2: Confirm the alpha utilities emit**

```bash
grep -oE '\.(bg|text|border|outline)-brass\\/[0-9]+\{[^}]*\}' dist/assets/*.css | head
```
Expected: non-empty. Empty means `<alpha-value>` did not translate and every
`brass/NN` in the UI is now a dead class.

- [ ] **Step 3: Keyframes and animations** — the config defines four
      (`vox-ping`, `vox-shimmer`, `vox-toast-in`, `shimmer`). Confirm each:

```bash
for k in vox-ping vox-shimmer vox-toast-in shimmer; do
  printf '%-14s %s\n' "$k" "$(grep -c "@keyframes $k" dist/assets/*.css)"
done
```
All must be `1`.

- [ ] **Step 4: Commit** with the token/keyframe counts pasted in the message.

---

## Task 5: Delete the old config

- [ ] **Step 1:** `git rm crates/vox-gui/ui/tailwind.config.js`
- [ ] **Step 2:** `pnpm build` — must still succeed. If it fails, v4 was reading
      the JS config via `@config` and Task 4 is incomplete. Do not paper over
      this by restoring the file; finish the `@theme` translation.
- [ ] **Step 3: Commit.**

---

## Task 6: Diff the generated CSS against the baseline

The real acceptance test.

- [ ] **Step 1: Diff rule-by-rule, not byte-by-byte** (v4 reorders and reformats,
      so a raw diff is pure noise):

```bash
cd crates/vox-gui/ui
norm() { grep -oE '\.[a-zA-Z0-9\\:/._-]+\{[^}]*\}' "$1" | sed 's/[[:space:]]//g' | sort -u; }
norm /tmp/tw-baseline.css > /tmp/a.txt
norm dist/assets/*.css   > /tmp/b.txt
echo "only in v3 baseline:"; comm -23 /tmp/a.txt /tmp/b.txt | head -40
echo "only in v4:";          comm -13 /tmp/a.txt /tmp/b.txt | head -40
```

Selectors present only in the baseline are the ones to justify individually.
Renamed utilities legitimately appear on both sides.

- [ ] **Step 2: Focus rings specifically** — re-run Task 1 Step 2's grep against
      the v4 output and confirm an `outline-width` rule plus an `outline-style`
      source (v4 emits the latter via `--tw-outline-style`).

- [ ] **Step 3: Full test + typecheck**

```bash
pnpm vitest run     # assert on the "Tests  N passed" line; N must be > 0
pnpm build          # tsc -b && vite build
```

- [ ] **Step 4: Commit** with the comm output summarised in the message.

---

## Task 7: Bump tailwind-merge to v3 and re-run the 1457-string A/B

- [ ] **Step 1:** `pnpm add tailwind-merge@3`
- [ ] **Step 2: Re-extract and re-run the A/B** that found the original bug:

```bash
cd crates/vox-gui/ui
grep -rhoE "['\"][a-z0-9][a-zA-Z0-9:/\[\]().,%_-]*( +[a-zA-Z0-9:/\[\]().,%_-]+)+['\"]" \
  src --include="*.ts" --include="*.tsx" \
  | sed "s/^['\"]//;s/['\"]\$//" \
  | grep -E "(flex|text-|bg-|rounded|border|p[xytblr]?-|m[xytblr]?-|size-|gap-|absolute|relative|grid|w-|h-|opacity|font-|tracking|hover:|focus:)" \
  | sort -u > /tmp/classes-v4.txt
wc -l /tmp/classes-v4.txt
```

Compare v2 vs v3 output over that corpus, as on #495. Post-migration the
`focus-visible:outline` difference is EXPECTED and CORRECT; every other
difference needs justifying individually.

- [ ] **Step 3:** `pnpm vitest run src/lib/cn.test.ts` — Task 2's guard must still pass.
- [ ] **Step 4: Commit.**

---

## Task 8: Open the PR and close the loop

- [ ] **Step 1:** Push and open a PR. Include the Task 6 `comm` output and the
      Task 7 A/B result in the body.
- [ ] **Step 2:** Comment on `vox-foundation/vox#495` linking this PR, and ask a
      maintainer to reopen it. Do NOT reopen it yourself before this merges.
- [ ] **Step 3:** If a dependabot `ignore` entry for `tailwind-merge` was added to
      `.github/dependabot.yml` in the interim, REMOVE it in this PR — it exists
      only to suppress a bump that is now correct.

---

## Notes for whoever picks this up

- The `cn()` wrapper is four lines (`src/lib/cn.ts`) and the only `twMerge` call
  site in the repo, with 26 `cn()` callers. The surface is genuinely tiny — that
  is what made the A/B cheap, not what made the bump safe. A one-import
  dependency still removed a focus ring from every button.
- Bundle size is a weak signal here. The #495 regression changed the *content* of
  one class string and would not have moved the byte count meaningfully.
- If the codemod produces something surprising, prefer finishing the `@theme`
  translation by hand over reverting to `@config`. Leaving the JS config in place
  works but keeps two sources of truth for the theme, which is the same
  one-name-several-authorities problem tracked elsewhere in these plans.
