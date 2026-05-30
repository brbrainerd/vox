# 04 — Visual style guide
_Last reviewed: 2026-05-30. The token map every page prompt cites as `04-visual-style-guide.md §palette`._

The shared visual vocabulary: the shadcn token map, the Tailwind class palette, the typography stack, and voice samples. Every page prompt (`01`/`02`/`03`/`07`) references this file by section. Edit tokens here; regenerations pick them up.

The palette is also inlined in `01-landing-page.md`'s `<style-tokens>` block so that prompt is self-contained when pasted into Claude.ai. **This file is the SSOT.** If the two ever disagree, this file wins and `01`'s block should be re-synced from here.

---

## §palette

HSL custom properties, light theme (the site ships a single warm-paper theme; no dark mode for marketing pages). These are the shadcn `:root` variables.

```css
:root {
  --background: 40 33% 97%;   /* warm paper */
  --foreground: 222 31% 8%;   /* near-black ink */
  --card: 40 33% 97%;
  --card-foreground: 222 31% 8%;
  --primary: 16 100% 60%;     /* orange accent */
  --primary-foreground: 0 0% 100%;
  --secondary: 36 22% 87%;    /* warm gray */
  --secondary-foreground: 222 31% 8%;
  --muted: 36 22% 87%;
  --muted-foreground: 217 13% 47%;
  --accent: 167 42% 66%;      /* teal accent */
  --accent-foreground: 222 31% 8%;
  --border: 36 22% 87%;
  --ring: 16 100% 60%;
  --radius: 0.75rem;
}
```

**Semantic Tailwind classes (use these, never raw colors):**

- Backgrounds: `bg-background`, `bg-card`, `bg-primary`, `bg-secondary`, `bg-muted`, `bg-accent`, `bg-foreground` (Ink sections).
- Text: `text-foreground`, `text-muted-foreground`, `text-primary`, `text-primary-foreground`, `text-accent-foreground`.
- Borders/rings: `border-border`, `ring-ring`.

**Raw hex — permitted only inside code blocks (Deepwell):**

| Token | Hex | Use |
|-------|-----|-----|
| Deepwell | `#1A1F2E` | Code-block / playground background. |
| Code text | `#E2E8F0` | Code foreground. |
| Code muted | `#94A3B8` | Comments, filenames, line gutters. |

Everywhere outside a code block, use the semantic class. Positive-framing rule from the README: say "use the orange-accent palette" (`bg-primary`), never "don't use blue."

---

## §typography

| Role | Family | Tailwind class | Notes |
|------|--------|----------------|-------|
| Display / headings | Fraunces (serif) | `font-display` | `<h1>`/`<h2>` marketing headlines. |
| Body | Inter (sans) | `font-sans` | Prose, captions, UI labels. |
| Code | JetBrains Mono | `font-mono` | Code blocks, inline code, the playground. |

- Headline scale: `text-4xl md:text-6xl` for `<h1>`, `text-3xl md:text-4xl` for `<h2>`, `text-xl` for `<h3>`.
- Body prose: `text-base md:text-lg leading-relaxed`.
- Eyebrows / labels: `text-sm font-medium uppercase tracking-wide text-primary`.
- Sentence case for sub-headers; Title Case for major headings (per `07` § style notes).

---

## §spacing

Stick to the standard Tailwind scale. Arbitrary values like `h-[613px]` silently fail in the artifact sandbox.

- Allowed `p-`/`m-`/`gap-` steps: `1, 2, 3, 4, 6, 8, 12, 16, 24, 32`.
- Section vertical rhythm: `py-16 md:py-32`.
- Card padding: `p-6`. Inter-card grid gap: `gap-6`.
- Border radius: `rounded-lg` (maps to `--radius: 0.75rem`).

---

## §shadcn-components

The closed set of shadcn primitives available in the artifact sandbox. Import from `@/components/ui/<name>`.

`Button`, `Card`, `CardHeader`, `CardTitle`, `CardDescription`, `CardContent`, `Badge`, `Separator`.

**Badge variants (status mapping — shared with `06` § PillarCard):**

- `Stable` → `variant="default"` (primary fill).
- `Preview` → `variant="secondary"` (warm gray).
- `Experimental` → `variant="outline"`.

**Button variants:** `default` (primary CTA), `outline` (secondary CTA), `ghost` (in-context links).

---

## §icons

`lucide-react`, imported by name. The marketing pages draw from this closed set:

`ArrowRight`, `ArrowUpRight`, `Check`, `ChevronRight`, `Code`, `Database`, `ExternalLink`, `Github`, `Layers`, `Network`, `Play`, `Server`, `Sparkles`, `Terminal`, `Zap`.

Decorative icons take `aria-hidden="true"`. Icons that carry meaning (e.g. an external-link affordance) get an `aria-label`.

---

## §voice

The register marketing prose must match. Full samples live in `07-content-blocks.md` § "Voice sample"; the short form for embedding in `<voice-sample>` prompt blocks:

> Most modern web apps need a database, a server, and a browser interface — three pieces of software, three programming languages, three sets of types describing the same things. Drift between them is the source of about half of all production bugs. Vox collapses those three into one declaration in one file. The compiler does the rest.

Voice rules (the full list is in `07` § style notes): em-dashes for parentheticals, Oxford comma always, contractions in body prose, no exclamation points, one idea per sentence, concrete claims over hedges. The forbidden-word table also lives in `07`.

---

## §accessibility

Baked into every component contract (`06` § shared rules):

- Exactly one `<h1>` per page; everything else `<h2>`/`<h3>` in document order.
- Every `<img>` has a descriptive `alt` — never "hero image" or "icon".
- Color is never the only signal; status also carries a text label (the Badge text), not just hue.
- The orange primary on warm paper and the ink-on-paper body text both clear WCAG AA contrast at body sizes. Do not place `text-muted-foreground` on `bg-muted` for body copy — reserve that pairing for large or decorative text.
- Pages are keyboard-navigable and honor `prefers-reduced-motion`.
</content>
