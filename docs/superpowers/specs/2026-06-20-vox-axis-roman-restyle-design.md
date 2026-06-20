# Vox Axis — "Roman" restyle (codename *Limes*) — design

**Status:** approved direction, pending spec review
**Date:** 2026-06-20
**Scope:** GUI frontend visual restyle (`crates/vox-gui`), token layer + 3 hero surfaces + app/favicon mark + Claude Design handoff shape. **Preview-first: nothing is applied/merged without explicit review.**

## 1. Goal & principles

Reinvigorate the Vox Axis GUI from a flat, programmatic black-on-text system into an artisanal, distinctly **Roman/Latin** interface — while making **visual clarity the top priority, above theming**.

Hard principles (in priority order):

1. **Clarity beats theme.** No restyle decision may drop text/UI contrast below **WCAG AA** (4.5:1 body, 3:1 large/UI). When theme and legibility conflict, legibility wins.
2. **Roman character comes from type, palette, proportion, and engraved detail** — never from texture, glow, drop-shadow, or ornament that competes with content.
3. **Dual scope, co-equal:** `basalt` (dark, default) and `travertine` (light), both first-class this pass.
4. **Reuse the existing spine.** Extend the style-dictionary → CSS-vars → Tailwind pipeline; do not introduce a parallel theming system.

## 2. Visual language (settled)

- **Base temperature:** cool **basalt** (dark) / warm **travertine** (light).
- **Accents (restrained):** **imperial gold** primary (the existing `--brass`, retained), **verdigris** (oxidized copper) reserved as the secondary/positive/success accent; **terracotta** for warn, **deep red** for fail. Outline/ghost controls by default; solid gold reserved for the single primary action per surface.
- **Ornament:** engraved L-shaped **corner ticks** (gold + verdigris) and a **hairline rule that fades to nothing** (gold → transparent). Used sparingly on panel headers/cards.
- **Typography (serif-for-display):**
  - Display/headings/section + nav labels: **Cinzel** (Trajan-style caps), used sparingly.
  - UI/data/body: **Inter** (workhorse sans) for all dense surfaces.
  - Quiet captions/labels: **EB Garamond italic** (e.g. the latin tag lines such as *orbis operis*).
  - Code/console/numerics: existing mono stack.
  - All fonts **bundled locally** (self-hosted `@font-face`, no runtime network fetch — required for Tauri + the app CSP `font-src 'self'`).

## 3. Token architecture

Current state (verified):
- `tokens/primitive.json` + `tokens/semantic.json` → `src/styles/tokens.generated.css` (emitted under `:root` only) + `tokens.generated.ts`; `semantic.contrast.json` → `tokens.contrast.generated.css` under `[data-theme="high-contrast"]`. Built by `style-dictionary.config.mjs` via `pnpm tokens:build`.
- Light/dark is currently faked in `index.css` by overriding `--brass` + backgrounds per `[data-theme]` (`arcane`/`void`/`glacier`). Generated tokens themselves are single-scope.

Target state:
- **Primitives** (`primitive.json`): add Roman ramps — `basalt` neutrals (cool), `travertine` neutrals (warm), `gold` ramp (imperial), `verdigris` ramp, `terracotta`, `oxblood`; add `font.family.{display,sans,serif,mono}` tokens; keep space/radius/size/weight/motion.
- **Semantic dark = default** (`semantic.json`): map `color.bg/text/border/accent/status` to basalt + gold/verdigris → emitted under `:root` (basalt is the default scope).
- **Semantic light** (`semantic.travertine.json`, new): same semantic slots mapped to travertine + daylight-darkened accents (gold `#8a6a26`, verdigris `#1f5a50` on tinted fills) → emitted under `[data-theme="travertine"]` via a new build target in `style-dictionary.config.mjs`.
- Retire `void`/`glacier`/`arcane` accent themes; `data-theme` axis becomes `basalt` (default) ↔ `travertine`, with `high-contrast` retained.
- Every accent/background pairing in both scopes must be contrast-checked (AA). Contrast assertions live in a unit test fed by `tokens.generated.ts`.

**Important coverage caveat:** many components use hardcoded `text-zinc-*` and `bg-white/[0.0x]` utilities. Token swaps alone reskin dark acceptably but **break light**. The 3 hero surfaces must migrate hardcoded neutrals/whites to semantic tokens (`text-primary/secondary/muted`, `bg-surface/elevated`, `border-subtle/strong`, `white/α` → a themed `--overlay` token) so they render correctly in both scopes. Non-hero surfaces inherit the global token reskin and are explicitly out of pixel-scope this pass (tracked as follow-up).

## 4. Hero surfaces (full pixel restyle)

1. **App shell** — `Sidebar.tsx` + `TopHud.tsx` (+ shared `AppShell.tsx` chrome). Nav rail already uses `font-display … tracking uppercase` → becomes Cinzel for free; add corner-tick + fading-rule treatment to the top HUD; active-item gold bar retained, contrast-tuned.
2. **Loquela** — `surfaces/Loquela/Loquela.tsx` + `Transcript.tsx` + `InlineApprovals.tsx`: transcript rows, message framing, composer, approvals.
3. **Dashboard / Mesh** — `surfaces/Dashboard/Dashboard.tsx` + `AgentRow.tsx` + `StreamCard.tsx`: metric cards (Cinzel numerals), status dots (verdigris/gold/terracotta/red), engraved card headers.

## 5. App & favicon mark (the groma)

- **Concept:** the Roman surveyor's **groma** — the instrument that set every camp/city on its founding axes, the *cardo* (N–S) and *decumanus* (E–W). Semiotically exact for "Axis." A bold equal-armed cross in a ring, with cardinal terminal arrowheads.
- **Why:** the current compass mark is too thin to read at favicon/taskbar sizes. The groma reduces to a heavy cross that survives 16px (proved in brainstorm).
- **Deliverables:**
  - New **vector master** `crates/vox-gui/icons/axis-mark.svg` (the source of truth — none exists today).
  - A **simplified 16px variant** (drop ring/terminals, cross only) so the small size stays crisp.
  - Regenerated raster set: `32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.png` (512), `icon.ico`, `icon.icns`, plus `Square*Logo`/`StoreLogo` if targeting MSIX later.
  - Add a **`bundle.icon` array** to `tauri.conf.json` (currently absent) pointing at the set.
  - Add a **web favicon** to `ui/index.html` (currently none): `<link rel="icon" href="/axis-mark.svg">` + an `.ico` fallback, asset placed under `ui/public/`.
  - In-app brand mark: a React `AxisMark` component rendering the SVG, gold via `currentColor`/`--brass` so it themes with the scope.

## 6. Claude Design handoff shape

Structure outputs so they can later feed `/design-sync` → claude.ai/design **as a separate, opt-in step** (not run in this pass):

- Keep restyled hero components self-contained with token-driven styles reachable from a single `styles.css` `@import` closure (tokens + fonts + component CSS).
- Emit a `.design-sync/conventions.md` draft naming the real vocabulary: the gold/verdigris/terracotta tokens, the `font-display`/Cinzel + Inter + Garamond idiom, the corner-tick/fading-rule recipes, and the `data-theme` basalt/travertine switch — so a future design agent builds on-brand.
- No claude.ai upload happens here; this pass only produces the artifacts in a sync-ready shape.

## 7. Delivery & review (preview-first)

- All work on the current worktree branch; **no auto-apply, no merge** until reviewed.
- Build via `pnpm tokens:build && pnpm build`; verify with `pnpm test` (vitest) + `pnpm typecheck`.
- Provide **rendered previews** (screenshots of the running app, both scopes, all 3 hero surfaces + the icon set) for sign-off before any integration.
- Contrast test + existing component tests must stay green.

## 8. Out of scope (this pass)

- Pixel restyle of non-hero surfaces (inherit global tokens only).
- The actual `/design-sync` upload to claude.ai/design.
- Any logic/behavior change; this is presentation-only.
- New gamification visuals beyond token reskin.

## 9. Risks

- **Hardcoded color sprawl** → light theme breakage; mitigated by hero-surface token migration + contrast test.
- **Font licensing/size** → use SIL-OFL fonts (Cinzel, EB Garamond, Inter all OFL); subset to Latin to keep bundle small.
- **Icon raster pipeline on Windows** → use a JS/Node rasterizer (sharp) or the `@tauri-apps/cli icon` generator from the SVG master to avoid hand-exporting.
