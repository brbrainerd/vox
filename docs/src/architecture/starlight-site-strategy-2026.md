---
title: "Vox Docs Portal: Astro Starlight Strategy 2026"
description: "Research findings, gap analysis, and execution roadmap for maximizing the Astro Starlight documentation portal against user journeys, AI-first indexing, and MENS pipeline integration."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Strategic documentation on documentation portal architecture, AI discoverability, and user experience design for the Vox programming language."
---

# Vox Docs Portal: Astro Starlight Strategy 2026

This document records the comprehensive research findings and action plan produced after the mdBook → Starlight migration. It covers: remaining legacy vestiges, landing page strategy, user journey design, AI discoverability gaps, MENS pipeline integration, and the highest-value next steps.

---

## 1. mdBook Retirement Status

### Confirmed Fully Retired
- `docs/book.toml` — **DELETED**
- `docs/theme/` (custom.css, head.hbs, highlight-vox.js) — **DELETED**
- `peaceiris/actions-mdbook`, `mdbook-metadata`, `mdbook-sitemap-generator` — **REMOVED from all workflows**
- `python docs/scripts/lychee_icons.py`, `python docs/scripts/seo_postprocess.py` — **REMOVED** (the scripts themselves never existed in the repo; they were dead CI references)
- `docs-quality.yml` — mdBook steps **REMOVED**; Starlight is now the primary blocking build
- `docs-deploy.yml` — Completely rewritten; uploads `docs-astro/dist/` to GitHub Pages

### Remaining Legitimate References (Not Retired — Intentional)
- `tmp/plans/plan-starlight-migration.md` — Historical plan document. Safe to archive to `docs/src/archive/`.
- `docs/src/architecture/shiki-mdbook-doc-platform-research-2026.md` — **Research document**. The title references mdBook for historical context. The document is correct as-is.
- `docs/src/architecture/architecture-index.md` — Links to the shiki-mdbook research doc. Correct.

### Verdict
**mdBook is 100% retired from active infrastructure.** Only historical research and plan documents reference it, which is correct behavior.

---

## 2. Landing Page Gap Analysis — RESOLVED

> **Status audit, 2026-08-22.** This section analysed `docs/src/index.md`, which
> no longer exists. It was replaced by [`docs/src/index.mdx`](../index.mdx). The
> original analysis is summarised below with the outcome of each point, rather
> than left in the present tense describing a deleted file.

### What was wrong (historical)
The mdBook-era `index.md` used raw `{{#include ../../../README.md:anchor}}`
directives, inline HTML relying on mdBook CSS variables
(`var(--table-border-color)`), and hardcoded `.md` links.

### What actually shipped
`index.mdx` is hand-written MDX. It uses Starlight's own CSS custom properties
(`--sl-color-accent`, `--sl-color-gray-2`), Starlight slug links
(`/tutorials/tut-getting-started/`), and imports the local `VoxPlayground.astro`
component. It does **not** use `template: splash`, `<CardGrid>`, or `<LinkCard>` —
the "required redesign" below was proposed, not adopted.

### The README ↔ landing page SSOT question
Two options were weighed: extract README anchors into `docs/src/_partials/` via
`vox-doc-pipeline` and import them from MDX, or accept controlled duplication.

**The second was taken.** `index.mdx` restates the README's narrative in its own
words; no partial-extraction step exists, and the page pulls nothing from
`README.md`. That is a real, accepted duplication: the landing page and the
README can drift, and nothing detects it. `README.md` still carries the
`<!-- ANCHOR: why_vox -->` markers from the old mechanism, but `tier_table` and
`community_license` were removed and no consumer references any of them.

Note the premise that forced this choice — "Starlight does not support mdBook
`{{#include}}` syntax" — is no longer true. `remark-vox-include.mjs` adds that
support, and 21 `vox` fences depend on it today (see Gap 7). Revisiting the
partials approach is therefore viable if the duplication becomes a problem; it
is not blocked by the platform.

---

## 3. User Journey Analysis

### Journey 1: First-Time Visitor ("What is Vox?")
**Lands on `voxlang.org/`**
Current gap: The hero message "The AI-Native Programming Language" is correct, but the CTAs point to broken `.md` links. The Diátaxis quadrant grid uses mdBook CSS variables.

**Required fix:** Full splash page rewrite with working native links and proper Starlight CardGrid layout.

### Journey 2: Developer Evaluating ("Can Vox replace X?")
**Scans the stability tier table, looks for proof points**
Current gap: The tier table (`{{#include}}`) is broken; no visible GitHub stars or community trust signals.

**Required fix:** Inline the tier table directly, add community links (GitHub Discussions, Open Collective).

### Journey 3: Returning Developer ("I need the CLI reference")
**Uses search (Pagefind) or sidebar navigation**
Current state: **Working.** Pagefind is enabled and auto-generated 530 pages of search index. Sidebar is dynamically generated from `SUMMARY.md`.

Gap: Sidebar currently exposes ALL 500+ pages including archive content. Recommend adding `pagefind: false` and excluding archive content from the primary sidebar.

### Journey 4: AI Agent / LLM ("What is the Vox syntax?")
**Hits `/llms.txt` or `/_pagefind/`**
Current gap:
- `llms.txt` URLs point to `vox.foundation` (wrong domain — should be `voxlang.org`)
- `llms-full.txt` is a stub — does NOT contain the actual full documentation content
- `vox-docs.json` exists but may be stale
- No `starlight-llms-txt` plugin is installed to auto-generate and keep in sync

---

## 4. Gaps: Unexploited Astro/Starlight Capabilities

> **Status audit, 2026-08-22.** Seven of the eight gaps below were closed after
> this document was written; it had continued to present them as open, while
> carrying `status: current` and `training_eligible: true`. Each entry now
> records the evidence that settles it. Only Gap 3 is still open.

### Gap 1: No MDX Landing Page — RESOLVED
*Originally CRITICAL.* The landing page is now [`docs/src/index.mdx`](../index.mdx);
the mdBook-era `index.md` no longer exists.

### Gap 2: No Automatic `llms.txt` Generation — RESOLVED
*Originally HIGH.* `starlight-llms-txt` is installed and configured in
[`docs-astro/astro.config.mjs`](../../../docs-astro/astro.config.mjs) with
`llmsFullTxt: true`, so `/llms.txt` and `/llms-full.txt` are generated from the
live sidebar at build time.

### Gap 3: No Open Graph Image Generation (MEDIUM) — OPEN
Every page shares the same default OG image when shared on social media. This is
a missed opportunity for branded, page-specific social cards.

**Fix:** install `astro-og-canvas` and configure `routeMiddleware` to inject
per-page OG image meta tags. Nothing in `astro.config.mjs` or `package.json`
references OG generation today.

### Gap 4: Archive Content Pollutes Search and Sidebar — RESOLVED
*Originally MEDIUM.* [`content.config.ts`](../../../docs-astro/src/content.config.ts)
excludes `archive/**` from the `docs` collection, so the 296 archived documents
are never built into pages and therefore never reach Pagefind or the sidebar.
This is stronger than the proposed `pagefind: false` frontmatter sweep, and it
matches the AGENTS.md §Archival Protocol tombstone rule.

### Gap 5: Broken Internal Links Due to URL Shape Change — RESOLVED
*Originally HIGH.* [`docs-astro/public/_redirects`](../../../docs-astro/public/_redirects)
ships the `*.html` → `*/` mapping for Cloudflare Pages.

### Gap 6: `llms.txt` Domain Mismatch — RESOLVED
*Originally HIGH.* [`docs/src/.well-known/llms.txt`](../.well-known/llms.txt)
uses `voxlang.org`, matching `site: 'https://voxlang.org/'` in the Astro config.
No `vox.foundation` references remain.

### Gap 7: `{{#include}}` Directives in `index.md` — RESOLVED
*Originally CRITICAL.* Two independent things closed this: `docs/src/index.md`
was replaced by `index.mdx`, and
[`remark-vox-include.mjs`](../../../docs-astro/src/plugins/remark-vox-include.mjs)
now resolves the mdBook `#include` directive (`path:anchor` syntax) inside fenced code blocks at build time.

The plugin **throws** on an unresolved path or anchor rather than emitting the
raw directive as visible text, so a broken include fails the build instead of
shipping. That makes the 21 remaining include-backed `vox` fences a working
single-source-of-truth mechanism — the code is pulled from `examples/golden/*.vox`,
which the golden corpus compiles directly. They should **not** be inlined:
inlining would duplicate compiled source into prose and reintroduce exactly the
split-brain drift the include prevents.

### Gap 8: Content Collection Config Duplicated — RESOLVED
*Originally LOW.* Only [`docs-astro/src/content.config.ts`](../../../docs-astro/src/content.config.ts)
exists; there is no `src/content/config.ts`.

---

## 5. MENS Pipeline Integration

### How Documentation Feeds the Training Pipeline

The Vox documentation corpus is a **primary training lane** for the MENS model (`vox-lang` domain). The connection is:

1. `vox-doc-pipeline` → generates `docs/src/SUMMARY.md` (metadata index)
2. CI builds → `docs-astro/dist/` (rendered HTML)
3. `vox populi` corpus ingest → reads from `docs/src/**/*.md` directly (SSG-agnostic)

### Current MENS Integration Gaps

**No structured corpus export from Starlight build**: The MENS pipeline currently ingests raw `.md` files. It does NOT have a pipeline to ingest the **rendered, Shiki-highlighted** HTML output from Starlight, which would give the model awareness of how code blocks look to end users.

**`llms-full.txt` is a stub**: The ideal MENS corpus entrypoint is a complete, clean plaintext dump of all documentation. Currently `llms-full.txt` is only 28 lines. With `starlight-llms-txt`, this becomes a full automatically-generated corpus file.

**`training_eligible: false` is inconsistently applied**: The pipeline generation marks `SUMMARY.md` and `architecture-index.md` as `training_eligible: false`, but many individual architecture docs lack this field or have it set incorrectly.

### Recommended MENS → Docs Pipeline

```
docs/src/**/*.md (training_eligible: true)
    ↓ vox-doc-pipeline (corpus mode, strips frontmatter)
    ↓ output: docs/dist/corpus.jsonl
    ↓ vox populi corpus add --source docs/dist/corpus.jsonl --lane vox-lang
```

The `corpus.jsonl` format per record:
```json
{"id": "reference/ref-syntax", "title": "Vox Syntax Reference", "content": "...", "category": "reference", "training_eligible": true}
```

This is already partially built. The `vox-doc-pipeline` needs a `--mode corpus` flag to emit JSONL instead of SUMMARY.md.

---

## 6. Execution Roadmap

### P0 — Critical (Blocks Production)

| Item | Action | File |
|---|---|---|
| Landing page broken | Rewrite `docs/src/index.md` → `index.mdx` using `template: splash` and Starlight components | `docs/src/index.md` |
| `{{#include}}` broken | Replace with inline content or MDX imports | `docs/src/index.md` |
| `llms.txt` domain mismatch | Fix `vox.foundation` → `voxlang.org` in llms.txt and llms-full.txt | `docs/src/.well-known/` |
| Duplicate content config | Remove `docs-astro/src/content.config.ts` (redundant) | `docs-astro/src/content.config.ts` |

### P1 — High Value

| Item | Action | File |
|---|---|---|
| Auto-generate `llms.txt` | Install `starlight-llms-txt` plugin | `docs-astro/astro.config.mjs` |
| HTML → slug redirects | Create `_redirects` in public for `*.html` → `*/` | `docs-astro/public/_redirects` |
| Archive noise | Exclude archive from sidebar + mark `pagefind: false` | `vox-doc-pipeline` + `docs-astro` |
| OG images | Install `astro-og-canvas` + routeMiddleware | New files |

### P2 — Recommended

| Item | Action |
|---|---|
| MENS corpus export | Add `--mode corpus` to `vox-doc-pipeline` emitting JSONL |
| Search Algolia upgrade | Consider `@astrojs/starlight-docsearch` for typo-tolerant search |
| Interactive playground | Vox REPL via WebAssembly island on landing page |
| Page feedback widget | Simple thumbs-up/down `pagefind`-compatible form |

