# Vox Design System

Prompt-based design kit for [Claude](https://claude.ai). Each prompt produces a **single-file React + Tailwind + shadcn artifact** that you copy into the Astro site.

## What this targets (the deliverable shape)

Claude's design output is one React functional component in an Artifact — JSX, no TypeScript, with this preloaded environment:

- **React 18** (functional components + hooks; `export default` at file end).
- **Tailwind CSS** (utility classes only; arbitrary values like `h-[613px]` are discouraged; stick to the standard scale).
- **shadcn/ui** components, importable from `@/components/ui/*` (Button, Card, Badge, etc.).
- **lucide-react** for icons, imported by name.
- **Recharts** for charts (rarely needed for marketing pages).
- **Mock data inlined** — no `fetch`, no `localStorage`, no remote URLs. The sandbox blocks them.

Each prompt's output spec is a single `.jsx` file. To ship it on voxlang.org:

1. Run the prompt in Claude.ai → receive a JSX artifact.
2. Copy the JSX (or use the [claude-artifacts-downloader extension](https://github.com/ashwanthkumar/claude-artifacts-downloader) to grab a zip if you ran several).
3. Convert to an Astro page by wrapping the component, or paste into a new `.astro` file's `<script>` slot. See `docs/design-system/integration-notes.md` (TODO) for the exact transformation.

## What this kit contains

| File | Purpose |
|------|---------|
| [01-landing-page.md](01-landing-page.md) | Prompt: marketing landing page (`/`). |
| [02-concepts-page.md](02-concepts-page.md) | Prompt: "How Vox works in 60 seconds" for non-programmers (`/concepts/`). |
| [03-showcase-gallery.md](03-showcase-gallery.md) | Prompt: 8-demo gallery for engineers (`/showcase/`). |
| [04-visual-style-guide.md](04-visual-style-guide.md) | shadcn token map, Tailwind class palette, voice samples. Cited by every page prompt. |
| [05-image-generation-prompts.md](05-image-generation-prompts.md) | Per-asset prompts for Imagen / FLUX / Midjourney. Run separately. |
| [06-component-specs.md](06-component-specs.md) | Reusable JSX building blocks (Hero, CodeOutputSplit, PillarCard, DemoBlock, Footer). |
| [07-content-blocks.md](07-content-blocks.md) | Evergreen copy + verified Vox snippets. The page prompts paste from here. |

## Why prompt-first

The site is static, hosted on Cloudflare Pages, built by Astro/Starlight. The marketing pages are visual deliverables. Designing them as **prompts that produce JSX** rather than hand-coded Astro pages means:

- **Visual iteration is fast.** Tweak one paragraph of the prompt, regenerate.
- **The prompt is the SSOT.** A future regeneration (new color palette, refreshed copy) replays cleanly.
- **Real copy and verified snippets live in the kit**, not in the generator's training prior — eliminating the "modern, clean" collapse to mean.

## Brand pillars — calibrated to what actually ships

The kit anchors visuals to five claims. Honesty about maturity prevents prompts from generating marketing that breaks under inspection.

| # | Pillar | Status (2026-05) |
|---|--------|------------------|
| 1 | Single source of truth | **Stable** — `@table` + codegen pipeline locked. |
| 2 | Designed for LLMs first | **Stable** — phonetic operators, grammar-constrained decoder, errors-in-types all real. |
| 3 | React interop via components and endpoints | **Preview** — `component` + `@query`/`@mutation`/`@server` shipping; ergonomics still tightening. |
| 4 | Agents, MCP, and durable execution | **Stable** — `@mcp.tool`/`@mcp.resource` ship; `workflow`/`activity` get journal-backed replay (ADR-019/ADR-021/ADR-041). |
| 5 | Local training (MENS) | **Preview** — `vox populi` runs QLoRA on Candle/Burn; hardware coverage expanding. |

When marketing copy implies a Preview feature is shipped, the prompt is broken. `07-content-blocks.md` carries the language that handles this correctly.

## Verified Vox syntax (use only these in code samples)

The kit's snippets are limited to syntax verified against the actual compiler as of 2026-05-23. Cited in `07-content-blocks.md` § "Code-snippet templates."

- `@table type T { ... }` — schema + codegen. **Stable.**
- `@query` / `@mutation` / `@server` — endpoint shapes. **Stable.** (Replaces deprecated `@endpoint(kind: ...)`.)
- `@mcp.tool "description"` — MCP tool exposure. **Stable.**
- `@mcp.resource("vox://path", "description")` — MCP resource via URI string (not HTTP pattern). **Stable.**
- `match`, `Result[T]`, `Ok(...)`, `Error(...)`, `?` postfix — error handling. **Stable.**
- `not`, `and`, `or`, `is`, `isnt` — phonetic operators. **Stable.** (Bare `!=` parses but the lexer emits a hint to use `isnt`.)
- `component Name(props) { view: column() { ... } }` — UI declaration. **Preview.**
- `routes { "/path" to Component }` — URL routing. **Preview.**
- `match expr { Ok(v) => ... Error(e) => ... }` — match arms use `=>` (fat arrow). **Stable.**
- `workflow`/`activity` for journal-backed durable execution — supported subset (linear activities, deterministic `if`, `workflow_wait`) ships per [ADR-041](../src/adr/041-durable-functions-completion-2026.md). **Stable.** Use the determinism lint to keep workflow bodies free of `std.time.now_ms` / `std.random` / `std.uuid` / `std.process.spawn`.

Do NOT use in marketing copy:
- `@require_capability(...)` — does not exist. Mesh capability declaration is design sketch.
- `@mcp.resource "GET /path/{var}"` — HTTP-pattern strings are not the syntax. Use URI strings.
- `->` (thin arrow) in match arms — use `=>` (fat arrow). The thin arrow is invalid syntax for match.

## Iteration loop

1. Edit a prompt file in this kit (the SSOT).
2. Run the prompt in Claude.ai → JSX artifact.
3. Integrate into the site.
4. If the output looks off, **edit the prompt, not the JSX.** The next run should produce a better baseline.

The compounding asset is the prompts. The JSX is replaceable.

## Versioning

No `vN.md` suffixes. When a prompt evolves materially, update the file in place; bump the `_Last reviewed:` date. Track history via git.

## Adding a new prompt

Add a file when a new top-level surface lands (`/community/`, `/playground/`). Don't add for one-off variants — paste-tweak the closest existing prompt.

## Quick reference for prompt authors

Five rules — drawn from the [Anthropic Frontend Aesthetics cookbook](https://platform.claude.com/cookbook/coding-prompting-for-frontend-aesthetics) and the [Claude 4 best-practices docs](https://docs.claude.com/en/docs/build-with-claude/prompt-engineering/claude-4-best-practices), validated against the existing kit's gaps:

1. **List the available symbols up front.** Every import path, every shadcn component, every Tailwind token Claude is allowed to use — in a `<available>` XML block at the top of the prompt. Prevents hallucinated `import` paths.
2. **Use real copy, not adjective stacks.** "Modern, clean" produces mean-collapse. A 40-word voice sample from `07-content-blocks.md` produces voice match.
3. **Reframe negatives as positives.** Not "don't use blue" but "use the orange-accent palette in `04-visual-style-guide.md` §palette."
4. **Self-check enumerates a closed list.** Not "review accessibility" but "verify each of the 8 items in §self-check; respond with PASS/FAIL per item before the JSX."
5. **Hard word counts for slots; ranges for prose; nothing for code.** Hero headlines: exact word count. Body paragraphs: range. JSX: constrain by component list, not LoC.
