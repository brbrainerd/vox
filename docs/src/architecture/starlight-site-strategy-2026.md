---
title: "Vox Docs Portal: Astro Starlight Strategy 2026"
description: "Research findings, gap analysis, and execution roadmap for maximizing the Astro Starlight documentation portal against user journeys, AI-first indexing, and MENS pipeline integration."
category: "architecture"
status: "current"
training_eligible: true
training_rationale: "Strategic documentation on documentation portal architecture, AI discoverability, and user experience design for the Vox programming language."
last_updated: "2026-04-22"
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

## 2. Landing Page Gap Analysis

### Current State (`docs/src/index.md`)
The existing landing page was designed for mdBook's HTML pass-through rendering. It uses:
- Raw `{{#include ../../../README.md:anchor}}` directives — **BROKEN in Starlight** (Starlight does not support mdBook `{{#include}}` syntax).
- Inline HTML divs relying on mdBook's CSS variables (`var(--table-border-color)`) — **BROKEN** in Starlight context.
- Hardcoded `.md` links — need to be converted to Starlight-style slug paths.

### Critical Issue: README ↔ Docs Portal SSOT
The `docs/src/index.md` currently pulls sections from `README.md` via `{{#include}}` anchors:
- `{{#include ../../../README.md:why_vox}}`
- `{{#include ../../../README.md:tier_table}}`
- `{{#include ../../../README.md:community_license}}`

**This is broken in Starlight.** We need a new approach to the SSOT problem.

### Recommended SSOT Pattern: Starlight-Native
Rather than `{{#include}}`, use a **build-time content injection step** in `vox-doc-pipeline`:
1. The `README.md` anchors (`<!-- ANCHOR: why_vox -->` ... `<!-- ANCHOR_END: why_vox -->`) remain the source of truth.
2. `vox-doc-pipeline` extracts anchored sections at generation time and writes them into a dedicated `docs/src/_partials/` directory as standalone `.md` snippets.
3. The Starlight `index.mdx` imports those partials via native MDX `import`.

Alternatively: **Accept controlled duplication** — maintain `docs/src/index.md` as a maintained-in-parallel landing page that mirrors the README's core narrative, updated by the doc pipeline when content changes.

### Required Landing Page Redesign
The landing page must be rewritten as a **Starlight splash page** using `template: splash`:
- `template: splash` removes the sidebar on the index page (full-width marketing layout)
- `hero:` frontmatter configures a pre-styled hero with CTA buttons
- MDX `<CardGrid>` and `<LinkCard>` components provide the Diátaxis quadrant navigation
- Links must use Starlight slug format (`/tutorials/tut-getting-started/` not `.md`)

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

### Gap 1: No MDX Landing Page (CRITICAL)
The current `index.md` uses raw HTML and mdBook directives. It needs to become `index.mdx` using Starlight's built-in component library.

**Impact:** Users hitting `voxlang.org` see a broken layout. First impression is damaged.

### Gap 2: No Automatic `llms.txt` Generation (HIGH)
The `llms.txt` and `llms-full.txt` files are **manually maintained stubs** that are already out of date (wrong domain `vox.foundation` vs `voxlang.org`, missing content).

**Fix:** Install `starlight-llms-txt` plugin. It auto-generates `/llms.txt` and `/llms-full.txt` from the live sidebar at build time.

```bash
pnpm add starlight-llms-txt
```

This directly feeds: AI agent discoverability, the "Ask the Docs" RAG pipeline in `vox scientia`, and future MENS corpus ingestion.

### Gap 3: No Open Graph Image Generation (MEDIUM)
Every page in the Starlight site shares the same default OG image when shared on social media (GitHub, Twitter/X, LinkedIn). This is a missed opportunity for branded, page-specific social cards.

**Fix:** Install `astro-og-canvas` + configure `routeMiddleware` to inject per-page OG image meta tags.

### Gap 4: Archive Content Pollutes Search and Sidebar (MEDIUM)
The `docs/src/archive/` directory contains 50+ archived research documents that are included in Pagefind's search index and the generated sidebar. Users searching for current docs will surface stale archive material.

**Fix:**
- Add `pagefind: false` to all archive directory frontmatter via `vox-doc-pipeline`
- Add a `data-pagefind-ignore` wrapper in a custom Starlight component
- Exclude archive from SUMMARY.md sidebar or move it to a collapsed group

### Gap 5: Broken Internal Links Due to URL Shape Change (HIGH)
mdBook generated URLs like `/architecture/foo.html`. Starlight generates `/architecture/foo/`. Any bookmarked or externally linked URLs from the old site will 404.

**Fix:** Generate a `_redirects` file in `docs-astro/public/` mapping `*.html` → `*/` for GitHub Pages / Cloudflare Pages.

### Gap 6: `llms.txt` Domain Mismatch (HIGH)
All links in `llms.txt` point to `vox.foundation` not `voxlang.org`. This will cause 404s for AI agents trying to follow those references.

### Gap 7: `{{#include}}` Directives in `index.md` (CRITICAL)
The landing page `docs/src/index.md` contains mdBook-specific `{{#include ../../../README.md:anchor}}` directives that Starlight will render as literal text. This is the most visible regression.

### Gap 8: Content Collection Config Duplicated (LOW)
Both `docs-astro/src/content/config.ts` and `docs-astro/src/content.config.ts` appear to exist. Only one should be authoritative.

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

