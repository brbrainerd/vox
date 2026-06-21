# Vox Axis — Brand Surfaces Design-Handoff Spec (2026-06-19)

Developer handoff for the Axis brand surfaces (Phase D of the rebrand plan). Written
against the **existing** GUI design system — it does not introduce a new visual
language, it threads the brand into the one already there.

## Current design language (audit)
- **Aesthetic:** dark glassmorphism. Base `#09090b` (`--color-bg-base`), panels via the
  `Glass` primitive (`bg-white/[0.025]`, `backdrop-blur-2xl`, border `white/[0.06]`,
  inset+drop shadow). Rounded (`rounded-xl/2xl/3xl`).
- **Accent:** a single theme-switched token `--brass` (arcane=gold `#d4af37`,
  void=violet `#8b5cf6`, glacier=cyan `#22d3ee`); also drives the global focus ring and
  the per-theme background. Tailwind exposes it as `brass` with `<alpha-value>`.
- **Type:** `font-display` (system-ui stack), `font-mono` (ui-monospace). Wordmarks use
  wide tracking (`tracking-[0.22em]`). (Note: `font-rajdhani` is referenced in a few
  components but is **not** defined in `tailwind.config.js` — a pre-existing fallback
  bug, out of brand scope; logged as a follow-up.)
- **Brand today:** a `size-6` gradient box (`from-brass via-amber-600 to-zinc-900`) with
  a literal `V`, plus a `VOX` wordmark and a mono build line — only rendered when the
  sidebar is not in `rail` (collapsed) mode.

## Design intent for the rebrand
Thread "Axis" in as a **first-class participant of the theme system**, not a static
logo: the in-app `AxisMark` is monochrome and inherits the accent via `currentColor`,
so it recolors with the theme (gold/violet/cyan). The static OS icon + favicon keep the
fixed brass tile (brand constant outside the themed app). This is the "improve while
sliding in" move — the mark unifies with the accent language instead of fighting it.

---

## Handoff Spec: `AxisMark` component

### Overview
Reusable brand glyph: a gimbal/gyroscope (two tilted rings + outer ring) pierced by a
spin-axis arrow. Monochrome, scalable, themeable. Used in the sidebar lockup (and
available for About/empty states later).

### Design tokens used (all EXISTING — no new tokens introduced)
| Token | Value | Usage |
|-------|-------|-------|
| `currentColor` | inherits `text-*` | ring + axis strokes (caller sets hue) |
| `bg-base` (`--color-bg-base`) | `#09090b` | hub fill via `fill-bg-base` |
| `brass` (`--brass`, theme-switched) | accent | the color callers pass as `text-brass` |

> No `--color-brand-*` tokens: the accent is already the themeable `--brass`. An SD
> brand token would lock to static `#d4af37` and break theming. A `tokens` test
> (Phase D2) asserts the mark carries no hardcoded hex.

### Props / API
| Prop | Type | Default | Notes |
|------|------|---------|-------|
| `className` | `string?` | — | sizing + color, e.g. `size-6 text-brass` |
| `title` | `string?` | `'Axis'` | `<title>` + `aria-label` |

### States
| State | Behavior |
|-------|----------|
| Default | strokes render in `currentColor`; hub punches through with the base-bg token |
| Color via theme | `text-brass` → mark follows arcane/void/glacier accent |
| Smallest (≤24px) | rings collapse to a clean disc + dominant axis arrow (verified at 40px); still reads as "axis" |

### Accessibility
- `role="img"` + `aria-label={title}` + an SVG `<title>`.
- Decorative usages (where an adjacent text label exists) may pass `aria-hidden` via `className` consumer — default is labeled.

### Edge cases
- No raster fallback needed (inline SVG).  - `viewBox="0 0 1024 1024"`, intrinsically square; never distort — size with equal width/height utilities (`size-*`).

---

## Handoff Spec: Sidebar brand lockup

### Overview
Replaces the gradient-`V` box. Two responsive forms keyed off `mode` (sidebar width).

### Layout & responsive behavior
| Sidebar mode | Lockup |
|--------------|--------|
| `default` / `wide` (not collapsed) | `[AxisMark size-6 text-brass]` + `AXIS` wordmark (`font-display text-[11px] tracking-[0.22em] text-zinc-200`); footer line `Vox Axis · build {appVersion} · tauri 2` (`font-mono text-[9px] text-zinc-500`) |
| `rail` (collapsed) | **AxisMark only**, centered (`size-6 text-brass`) — *improvement*: today the brand vanishes entirely when collapsed; keep the mark present |

### Design tokens used
| Token | Value | Usage |
|-------|-------|-------|
| `brass` | theme accent | mark color (`text-brass`) |
| `text-zinc-200` | `#e4e4e7` | `AXIS` wordmark |
| `text-zinc-500` | `#71717a` | footer build line |
| spacing | `gap-2`, `px-1`, `pb-3` | unchanged from current header |

### States & interactions
| Element | State | Behavior |
|---------|-------|----------|
| Lockup container | static | not a button (no hover) — matches current |
| Sidebar collapse toggle | existing | unchanged; only the lockup's rendered form switches on `mode` |

### Edge cases
- `appVersion` undefined → footer shows `Vox Axis · build unknown · tauri 2` (mirror the existing `?? 'unknown'`).
- Long `appVersion` → mono line truncates with the existing container; no wrap.

### Accessibility
- The `AxisMark` carries `aria-label="Axis"`; the visible `AXIS`/`Vox Axis` text is the accessible name for the brand region. No duplicate announcement (mark is labeled, text is text).

---

## Handoff Spec: Favicon + document title

### Overview
Static brand favicon (fixed brass, does not theme-switch) + `<title>Axis</title>`.

### Spec
| Item | Value |
|------|-------|
| File | `crates/vox-gui/ui/public/favicon.svg` (Vite serves at `/favicon.svg`) |
| Mark | gimbal on the fixed brass tile (the committed icon), web-trimmed |
| `index.html` `<link>` | `<link rel="icon" type="image/svg+xml" href="/favicon.svg" />` |
| Title | `<title>Axis</title>` |

### Edge cases
- SVG favicon unsupported (very old webview) → acceptable to also drop a `favicon.ico`; not required for the Tauri webview (Chromium/WebKit support SVG icons).

---

## What this intentionally does NOT change (scope guard)
- No new visual language, no spacing/typography overhaul, no `Glass` changes.
- `productName`/`identifier`, crate/binary names — unchanged (see rebrand spec §2).
- `font-rajdhani` fallback bug — **follow-up**, not part of this brand work.
