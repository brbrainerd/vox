# 06 — Component specs
_Last reviewed: 2026-05-30. Output target: single-file React JSX with Tailwind + shadcn/ui + lucide-react._

Reusable JSX building blocks. The page prompts (`01`–`03`) instruct Claude to paste these component definitions into the same artifact file as the page component, so every page renders from the same primitives.

These specs are **shapes and contracts**, not finished code. Each entry lists the component's props, the shadcn/lucide primitives it composes, and the layout rules. When a page prompt says "paste the JSX for Hero, CodeOutputSplit, PillarCard, StaticPlayground, and Footer from `06-component-specs.md`," it means: generate JSX that satisfies these contracts in the same file.

The reason these live as specs rather than committed `.jsx`: the artifacts are regenerated, not edited (see `README.md` § "Why prompt-first"). A spec replays cleanly under a new palette; a frozen `.jsx` file rots.

All components use only the symbols enumerated in each page prompt's `<available>` block — shadcn from `@/components/ui/*`, icons from `lucide-react`, semantic Tailwind tokens from `04-visual-style-guide.md` § palette. No raw hex except the Deepwell code-block tokens.

---

## `<Hero>`

The landing-page first viewport. Two-column on `lg`, stacked on mobile.

**Props**

| Prop | Type | Notes |
|------|------|-------|
| `eyebrow` | string | Small caps label above headline. Word count is set by the page prompt. |
| `headline` | string | The `<h1>`. Exactly one per page. |
| `subhead` | string | One paragraph, prose register from the voice sample. |
| `primaryCta` | `{ label, href }` | shadcn `<Button>` `variant="default"`. |
| `secondaryCta` | `{ label, href }` | shadcn `<Button>` `variant="outline"`. |
| `trust` | string | The `·`-separated trust strip. Render as `text-muted-foreground`. |
| `image` | `{ src, alt, caption }` | `/api/placeholder/W/H` src; descriptive alt; 10–14 word caption. |

**Layout**

- Section bg `bg-background`, padding `py-16 md:py-32`.
- Left column: eyebrow (`text-primary`, `text-sm`, `font-medium`, `uppercase tracking-wide`), `<h1>` in `font-display`, subhead in `font-sans text-muted-foreground`, CTA row (`flex gap-4`), trust strip below.
- Right column: image in a `<Card>` with caption underneath in `text-sm text-muted-foreground`.
- Icon allowance: `Sparkles` beside the eyebrow, `ArrowRight` inside the primary CTA.

---

## `<CodeOutputSplit>`

Side-by-side panel: a Vox source block on the left, the generated artifacts on the right. The visual argument for "one declaration, the compiler does the rest."

**Props**

| Prop | Type | Notes |
|------|------|-------|
| `filename` | string | e.g. `"tasks.vox"`. Rendered as a tab/label on the code panel. |
| `code` | string | The Vox source. Use only verified syntax (`07` § code-snippet templates). |
| `outputs` | `{ label, detail }[]` | One row per generated artifact (Database / Server / Client / Tooling). |
| `footerLink` | `{ label, href }` | Optional. Right-aligned small link beneath the split. |

**Layout**

- Two columns on `md+`, stacked on mobile, joined by an `ArrowRight` chevron (or `ChevronRight` on mobile, rotated).
- Code panel: Deepwell background `bg-[#1A1F2E]`, `font-mono`, code text `#E2E8F0`, comments/muted `#94A3B8`. Filename tab in `#94A3B8`.
- Output panel: `bg-secondary` (warm gray). Each output row is `{ icon, label (font-medium), detail (text-sm text-muted-foreground) }`. Icons: `Database`, `Server`, `Code`, `Terminal`.
- Do not syntax-highlight beyond the three Deepwell tokens — the sandbox has no highlighter.

---

## `<PillarCard>`

One brand pillar. Used in a grid of five (see `01` § section 3 for the 5th-card centering rule).

**Props**

| Prop | Type | Notes |
|------|------|-------|
| `index` | number | 1–5. Shown as a muted ordinal. |
| `title` | string | Pillar name. |
| `status` | `"Stable" \| "Preview" \| "Experimental"` | Maps to a `<Badge>` (see below). |
| `body` | string | One short paragraph. Honest framing per `07` § honest-framing rule. |
| `icon` | lucide name | One of `Layers`, `Network`, `Code`, `Server`, `Zap`. |
| `learnMore` | `{ label, href }` | "Learn more →" link, `text-primary`. |

**Status → Badge mapping (use exactly)**

- `Stable` → `<Badge variant="default">` (primary).
- `Preview` → `<Badge variant="secondary">`.
- `Experimental` → `<Badge variant="outline">`.

**Layout**

- shadcn `<Card>`. Badge pinned top-right of `<CardHeader>`. Icon + title in the header; ordinal as a faint `text-muted-foreground` number. Body in `<CardContent>`; learn-more link at the card foot with a trailing `ArrowRight`.

---

## `<StaticPlayground>`

A non-interactive code/output pair that *looks* runnable but only renders a pre-baked result. The sandbox blocks real execution; this is an honest visual stand-in that links out to the real `/playground/`.

**Props**

| Prop | Type | Notes |
|------|------|-------|
| `code` | string | Vox source shown in the editor pane. |
| `simulatedOutput` | string | The fixed output shown in the result pane. |
| `runLabel` | string | Defaults to "Run". Decorative button; no handler that fakes computation. |

**Layout**

- Section bg `bg-foreground` (Ink); panels float above it.
- Editor pane: Deepwell `bg-[#1A1F2E]`, `font-mono`, code text `#E2E8F0`.
- A decorative `<Button>` with a `Play` icon. On click it may reveal the `simulatedOutput` pane — but it must never imply live compilation. If revealed immediately, no click handler is needed.
- Output pane: same Deepwell family, output prefixed with a muted `→`.
- Below: two `text-muted-foreground` links — "Open the full playground →" (`/playground/`) and "Or install locally →" (`/tutorials/tut-getting-started/`).

---

## `<Footer>`

Site footer. License + community, no newsletter signup (forbidden per `07` § CTAs).

**Props**

| Prop | Type | Notes |
|------|------|-------|
| `columns` | `{ heading, links: { label, href }[] }[]` | Navigation columns. |
| `license` | string | "Apache 2.0 — …" sentence from `07` § footer block. |
| `community` | `{ label, href }` | Open Collective link. |

**Layout**

- Three columns on `md+`, stacked on mobile. `bg-card`, top `border-border`.
- Left: wordmark + one-line tagline. Middle/right: nav columns.
- Bottom strip: license sentence (`text-sm text-muted-foreground`) and a `Github` icon link.
- No email input, no "subscribe", no "sign up" — see the forbidden-CTA list in `07-content-blocks.md`.

---

## `<DemoBlock>`

Used only by the showcase (`03`). One self-contained capability demo: a titled card with a snippet, a one-line claim, and the generated-output summary.

**Props**

| Prop | Type | Notes |
|------|------|-------|
| `pillar` | number | 1–5; ties the demo to a brand pillar for snippet selection (`07` § snippet selection guide). |
| `title` | string | Demo name. |
| `claim` | string | One sentence. Honest framing for Preview pillars. |
| `snippet` | string | Verified Vox (`07` § code-snippet templates). |
| `result` | string | What the snippet produces. |

**Layout**

- shadcn `<Card>`; Deepwell code area inside `<CardContent>`; claim as `<CardDescription>`. Optional Preview `<Badge>` when `pillar ∈ {3, 5}`.

---

## Shared rules for all components

1. **One file.** Every component above is defined in the same artifact file as the page component. No cross-file imports.
2. **Tokens, not raw color.** Use the semantic Tailwind classes from `04-visual-style-guide.md` § palette. The only raw hex permitted are the three Deepwell code-block tokens.
3. **Accessibility is in the contract.** Every `<img>` has a descriptive `alt`. Exactly one `<h1>` (the Hero headline); everything else is `<h2>`/`<h3>`. Decorative icons get `aria-hidden`. The page is keyboard-navigable and honors `prefers-reduced-motion`.
4. **No invented imports.** If a component seems to need a symbol not in the page prompt's `<available>` block, the prompt is wrong — fix the prompt, not the component.
</content>
</invoke>
