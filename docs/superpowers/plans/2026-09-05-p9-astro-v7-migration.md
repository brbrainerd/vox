# Astro 6 → 7 Migration (P9) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use `superpowers:subagent-driven-development`
> or `superpowers:executing-plans`. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Move `docs-astro` from Astro 6 to Astro 7 so the two dependabot
suppressions it forces can be deleted, and the docs toolchain stops being
pinned behind its ecosystem.

**Architecture:** One package changes: `docs-astro`. Nothing else in the repo
depends on Astro. The work is a coordinated version bump of Astro plus every
integration that declares an `astro` peer, followed by verifying the custom
plumbing (a remark plugin, a Shiki grammar, a generated sidebar, a route
middleware) still runs.

**Tech Stack:** Astro 7, Starlight, Vite, TypeScript, pnpm, Pagefind.

**Spec:** none — self-contained. Origin is the red-build investigation on
PR #515 and the triage on #487.

---

## Global Constraints

- **`docs-astro` is the only Astro consumer in the repo.** Do not touch
  `crates/vox-gui/ui` or `apps/experimental/visualizer`.
- **pnpm-managed.** `npm i --no-save` SILENTLY DOES NOTHING here — it reports
  success, changes nothing, and the build output is identical. Use `pnpm add`.
  ALWAYS re-read `node_modules/<pkg>/package.json` after installing and confirm
  the version actually changed before trusting any result.
- **`pnpm build` runs `prebuild` → `scripts/setup-content.mjs` first**, which
  symlinks `src/content/docs` → `../docs/src` and exposes repo-root `examples/`.
  A build in a fresh clone WILL fail until that has run. Never "fix" a failure
  by deleting or replacing those symlinks with real directories — a tracked copy
  of `src/examples` is a bug this repo has already had once.
- **Content lives OUTSIDE this package**, in repo-root `docs/src/`. The Astro
  package is a renderer over content it does not own.
- Do not publish releases. Do not create tags. Do not touch `main` directly.

---

## Background — why this is worth doing

Two dependabot `ignore` entries exist purely because Astro is pinned at 6:

| Suppressed | Declares | We pin |
|---|---|---|
| `@astrojs/starlight` 0.41+ | `peerDependencies: { astro: "^7.0.2" }` | astro ^6 |
| `starlight-llms-txt` 0.11.0 | pulls `@astrojs/mdx` 7, peer `astro: "^7.0.0"` | astro ^6 |

`starlight-llms-txt@0.11.0` actually shipped to `main` and **broke the docs
build for every page**. pnpm warns on an unsatisfied peer rather than failing,
so the wrong `@astrojs/mdx` installed and the build died at bundle time:

```
[ERROR] [vite] "chunkToString" is not exported by
  astro/dist/runtime/server/index.js, imported by @astrojs/mdx/dist/server.js
```

`chunkToString` is an Astro 7 runtime export, absent from 6.1.9 and 6.4.8 alike.
PR #515 pinned `starlight-llms-txt` back to `0.10.0` (mdx `^5.0.4`) and restored
the build — 931 pages, exit 0. **This migration is what lets that pin be
deleted.** Astro 7.3.1 is stable with 30 stable 7.x releases; this is not early
adoption.

---

## Compatible version matrix (measured 2026-09-05)

Bump these TOGETHER. A partial bump reproduces exactly the mixed-major failure
above, just with different packages.

| Package | Now | Target | Declares |
|---|---|---|---|
| `astro` | ^6.0.1 | **^7.3.1** | — |
| `@astrojs/starlight` | ^0.38.3 | **^0.42.0** | `astro ^7.2.10`, `@astrojs/markdown-remark ^7.2.0` |
| `starlight-llms-txt` | 0.10.0 (pinned) | **^0.11.0** | via mdx 7 → `astro ^7.0.0` |
| `@astrojs/rss` | ^4.0.19 | ^4.0.19 | no astro peer — verify empirically anyway |
| `@astrojs/check` | ^0.9.10 | ^0.9.10 | no astro peer |
| `@astrojs/sitemap` | (transitive) | 3.7.4 | no astro peer |

Note `@astrojs/starlight@0.41.0` wants `astro ^7.0.2` but `0.42.0` wants
`^7.2.10` — pick the astro version to satisfy the starlight you install, not
the other way round.

---

## File Structure

- Modify: `docs-astro/package.json` — the version matrix above
- Modify: `docs-astro/pnpm-lock.yaml` — regenerated
- Verify (likely unchanged): `docs-astro/astro.config.mjs`
- Verify: `docs-astro/src/content.config.ts` — `docsLoader`/`docsSchema` API
- Verify: `docs-astro/src/routeData.ts` — `routeMiddleware` contract
- Verify: `docs-astro/src/plugins/remark-vox-include.mjs`, `vox-grammar.mjs`
- Verify: `docs-astro/src/utils/sidebar.mjs`
- Possibly modify: `.github/dependabot.yml` — DELETE two ignore entries
- Do not modify: `docs-astro/scripts/setup-content.mjs`

---

## Task 1: Establish a GREEN baseline and capture it

Without this the whole migration is unmeasurable. Do not skip it — the #487
triage nearly reported "astro 6.4.8 breaks the build" because it had no
baseline and the build was already red.

- [ ] **Step 1: Confirm green on current main**

```bash
cd docs-astro
pnpm install --frozen-lockfile
pnpm build
```
Expected, and assert on these lines specifically — not the exit code:
```
[build] NNN page(s) built in ...
[build] Complete!
[starlight:pagefind] Found NNN HTML files.
```
At the time of writing: **931 pages, 945 HTML files**. If it is red, STOP —
something regressed since #515 and that must be fixed first.

- [ ] **Step 2: Snapshot the page inventory**

```bash
( cd dist && find . -name '*.html' | sort ) > /tmp/astro6-pages.txt
wc -l < /tmp/astro6-pages.txt
```

- [ ] **Step 3: Snapshot the generated LLM artifacts** (starlight-llms-txt's
      whole purpose; a silent regression here is invisible in a page count)

```bash
ls -la dist/llms.txt dist/llms-full.txt 2>&1 | tee /tmp/astro6-llms.txt
wc -c dist/llms.txt dist/llms-full.txt
```

- [ ] **Step 4: Snapshot one fully-rendered page** to compare markup later

```bash
cp dist/index.html /tmp/astro6-index.html
grep -c '<' /tmp/astro6-index.html
```

- [ ] Commit nothing; these are `/tmp` artifacts.

---

## Task 2: Bump the whole matrix in one step

- [ ] **Step 1: Install**

```bash
cd docs-astro
pnpm add astro@^7.3.1 @astrojs/starlight@^0.42.0 starlight-llms-txt@^0.11.0
```

- [ ] **Step 2: Prove the install was not a silent no-op**

```bash
for p in astro @astrojs/starlight starlight-llms-txt @astrojs/mdx; do
  printf '%-24s %s\n' "$p" "$(node -p "require('./node_modules/$p/package.json').version" 2>&1)"
done
```
`astro` must report 7.x. If any still reads its old version, the install did not
take — do not proceed on a false baseline.

- [ ] **Step 3: Prove the mixed-major condition is GONE**

```bash
pnpm why @astrojs/mdx
```
Expected: exactly ONE version, on the 7.x/8.x line, matching what starlight and
starlight-llms-txt both want. **Two versions here is the exact failure that
broke main** — if you see two, stop and reconcile before building.

- [ ] **Step 4: Commit the manifest change alone**, so a bisect can separate the
      version bump from any code fixes.

---

## Task 3: Build, and treat the first failure as information

- [ ] **Step 1:** `pnpm build`

- [ ] **Step 2:** If it fails, classify before fixing. The likely categories, in
      descending order of probability:
  - **Starlight config/API drift** (`routeMiddleware`, `sidebar`, `expressiveCode`,
    `social`, `editLink`) — 0.38 → 0.42 is four minors of a 0.x package, where
    minors carry breaking changes.
  - **`docsLoader` / `docsSchema`** shape change in `src/content.config.ts`.
  - **Content-layer API** changes affecting `astro:content` (`defineCollection`,
    `getCollection`, `z`).
  - **Remark plugin contract** — `remark-vox-include.mjs` runs against
    `@astrojs/markdown-remark`, which starlight 0.42 pins at `^7.2.0`.

- [ ] **Step 3:** Fix the smallest thing that makes the category pass, then
      rebuild. Commit per category, not per file.

- [ ] **Step 4:** Assert the same artifact lines as Task 1 Step 1.

---

## Task 4: Diff against the baseline — the real acceptance test

A green build is weak evidence. A docs site can build perfectly and silently
lose 200 pages, or render every code block empty.

- [ ] **Step 1: Page inventory diff**

```bash
( cd dist && find . -name '*.html' | sort ) > /tmp/astro7-pages.txt
diff /tmp/astro6-pages.txt /tmp/astro7-pages.txt && echo "IDENTICAL PAGE SET"
```
Any page present only under 6 must be explained individually. A route Astro 7
renames is acceptable; a page that vanished is not.

- [ ] **Step 2: LLM artifacts still generated and not truncated**

```bash
wc -c dist/llms.txt dist/llms-full.txt
```
Compare against `/tmp/astro6-llms.txt`. A large shrink means
`starlight-llms-txt` silently stopped collecting.

- [ ] **Step 3: The `{{#include}}` directives actually resolved**

This is the one most likely to fail silently. `remark-vox-include` pulls code
from repo-root `examples/` into fenced blocks; if it breaks, blocks render
EMPTY rather than erroring.

```bash
grep -rl 'class="expressive-code"' dist | head -1 | xargs grep -c '<code'
grep -rn '{{#include' dist --include='*.html' | head
```
The second command MUST return nothing. A literal `{{#include` in output means
the directive was passed through unprocessed.

- [ ] **Step 4: Vox syntax highlighting survived**

`voxGrammar` is a custom Shiki language registered through `expressiveCode`.

```bash
grep -rn 'language-vox\|data-language="vox"' dist --include='*.html' | head -3
```
Must be non-empty. Empty means the grammar failed to register and every Vox
sample is now unhighlighted plain text.

- [ ] **Step 5: Sidebar and search**

```bash
grep -c 'sidebar' /tmp/astro6-index.html; grep -c 'sidebar' dist/index.html
ls dist/pagefind/ | head -3
```

- [ ] **Step 6: Commit** with the diff results quoted in the message.

---

## Task 5: Delete the suppressions this migration exists to retire

If this step is skipped the migration has not paid for itself.

- [ ] **Step 1:** In `.github/dependabot.yml`, DELETE both entries and their
      comment blocks in the `/docs-astro` section:
      - `- dependency-name: "@astrojs/starlight"`
      - `- dependency-name: "starlight-llms-txt"`

- [ ] **Step 2:** Unpin `starlight-llms-txt` from the exact `0.10.0` that #515
      set — it should read `^0.11.0` after Task 2.

- [ ] **Step 3:** Confirm nothing else references the pins

```bash
grep -rn "starlight-llms-txt\|chunkToString" .github/ docs/ --include='*.yml' --include='*.md' | head
```
Update any doc that still describes the pin as current.

- [ ] **Step 4: Commit.**

---

## Task 6: CI and the PR

- [ ] **Step 1:** Both lanes that build this package (`docs-deploy.yml`,
      `docs-quality.yml`) pin `node-version: 24`. Confirm Astro 7's `engines`
      is satisfied by Node 24:

```bash
node -p "require('./node_modules/astro/package.json').engines"
```
If it requires Node ≥ 22 that is fine; if it requires something above 24, BOTH
workflows need bumping in this PR or the docs deploy breaks on merge.

- [ ] **Step 2:** `pnpm check` (`astro check`) — TypeScript across `.astro`.

- [ ] **Step 3:** Open the PR. Include the Task 4 page-inventory diff verbatim.

- [ ] **Step 4:** Comment on #487 (astro 6.1.9 → 6.4.8, currently DEFER) — this
      supersedes it. Close it once this merges.

---

## Notes for whoever picks this up

- **The recurring failure in this repo is a dependency whose new major targets a
  host framework version we do not pin.** Three instances so far: tailwind-merge
  3 → Tailwind 4, `@astrojs/starlight` 0.41 → astro 7, `@astrojs/mdx` 7 → astro
  7. Only the last actually shipped and broke `main`, because pnpm warns rather
  than fails on an unsatisfied peer. When this migration lands, re-run
  `pnpm why @astrojs/mdx` and confirm a single version — that command is the
  cheapest detector for the whole class.
- **Astro's own upgrade tool** (`npx @astrojs/upgrade`) handles the coordinated
  bump and is preferable to hand-editing `package.json`. Verify what it did.
- The custom plumbing is where the risk lives, not in Astro itself: a remark
  plugin, a Shiki grammar, a frontmatter-driven sidebar generator, and a route
  middleware. Each fails quietly rather than loudly — hence Task 4's specific
  greps instead of a general "does it build".
- The Tailwind v4 migration (P8, `2026-09-05-p8-tailwind-v4-migration.md`) is the
  sibling of this plan and retires the third suppression. They are independent
  and can run in parallel; they share no files.
