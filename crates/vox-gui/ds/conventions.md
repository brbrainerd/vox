# Limes Design System — Conventions

The "Limes" system is the Roman/Latin visual language of **Vox Axis**. It favors
**clarity first**: engraved structure, quiet ornament, no decorative glow. This
folder is a self-contained, presentational slice of it for design handoff — the
live app expresses the same tokens through Tailwind utilities.

## Scopes

Two color scopes share one set of primitive ramps:

| Scope | Selector | Feel | Background | Text | Accent | Secondary |
|---|---|---|---|---|---|---|
| **Basalt** (default) | `:root` | Dark stone | `--color-bg-base` `#0c0e10` | `#fafafa` | gold.500 `#c9a24a` | verdigris.400 `#4a9e8f` |
| **Travertine** | `[data-theme="travertine"]` | Sunlit limestone | `#ece5d6` | ink `#2a2620` | gold.700 `#8a6a26` | verdigris.700 `#1f5a50` |

Switch a subtree by setting `data-theme="travertine"` on any ancestor. Both
scopes pass WCAG AA (4.5:1 text, 3:1 UI) — enforced by a contrast guard in the
app.

## The `--brass` channel

The accent is also published as a space-separated RGB triple, `--brass`, so it
can drive opacity: `rgb(var(--brass) / 0.1)`. It is restated per scope in
`styles.css` (the token build does not emit it).

## Typography

| Role | Family | Use |
|---|---|---|
| Display | **Cinzel** SemiBold | Headings, nav labels, button text, KPI values — all-caps with `letter-spacing: 0.13em` (the `.ds-display` treatment). |
| Body / UI | **Inter** (400/500) | Default sans for everything that isn't display. |
| Caption | **EB Garamond** Italic | Quiet, literary asides only. |
| Mono | system mono | Code, numerics where alignment matters. |

Cinzel is a Trajan-column capitalis — reserve it for structural labels, never
long-form text.

## Ornament (use sparingly)

- `.ds-rule` — a hairline that fades from gold to transparent; section dividers.
- `.ds-tick-tl` / `.ds-tick-tr` — engraved L-corner ticks (gold top-left,
  verdigris top-right). Apply to a positioned container (see `Card ticks`).
- No glows, no neon. The active-nav indicator is a solid gold rail, not a halo.

## Components

| Component | Notes |
|---|---|
| `AxisMark` | The groma app mark. `currentColor`, defaults to `--brass`. Pass `size`. |
| `Button` | `variant="primary"` for the gold struck-metal action; default is quiet. |
| `StatusPill` | `tone`: neutral / pass / warn / fail / accent → semantic status tokens. |
| `Card` | The surface primitive. `ticks` adds engraved corners. |
| `NavItem` | Sidebar row; `active` adds the gold leading rail. |
| `KpiTile` | HUD metric; signed `delta` renders verdigris ▲ / fail ▼. |

## Do / Don't

- **Do** drive every color from a token variable. Never hardcode hex in a
  component.
- **Do** keep accents scarce — gold marks the one primary action or active
  state; verdigris is the live/positive secondary.
- **Don't** reintroduce glows, multi-hue gradients, or zinc/neutral grays —
  the system uses basalt/travertine ramps, not Tailwind's default palette.
- **Don't** set Cinzel on body copy or anything over ~3 words that isn't a label.
