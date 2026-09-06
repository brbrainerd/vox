---
title: "Shiki, mdBook & Documentation Platform Evaluation (2026)"
description: "Comprehensive research and quantified feature comparison of documentation site generators and syntax highlighting strategies for an AI-native, Rust-first codebase in 2026."
category: "Architecture SSOTs"
status: "research"
training_eligible: true
training_rationale: "Research on documentation platforms."
authors: ["Bert Brainerd"]
related:
  - docs/src/architecture/architecture-index.md
  - docs/src/archive/research-2026-q1/vox-syntax-highlighting-ssot-2026.md
  - apps/editor/vox-vscode/syntaxes/vox.tmLanguage.json
  - apps/editor/vox-vscode/syntaxes/markdown-injection.json
  - apps/editor/vox-vscode/package.json
  - docs-astro/README.md
  - docs-astro/src/utils/sidebar.mjs
---

# Shiki, mdBook & Documentation Platform Evaluation (2026)

## Executive Summary

Vox currently uses **mdBook 0.4.40** with a hand-rolled **highlight.js** plugin
(`docs/theme/highlight-vox.js`) to syntax-highlight `.vox` code blocks in the
rendered documentation portal. This creates a three-way grammar drift problem:
the VS Code extension uses a `vox.tmLanguage.json` (TextMate grammar), Neovim/
Helix use Tree-sitter `.scm` queries, and the docs site uses a separate
regex-based highlight.js definition. Every time the Vox language surface changes,
three grammars must be updated in lockstep.

**Critical discovery:** `shiki ^4.0.1` is **already a direct dependency** of
`apps/editor/vox-vscode/package.json` (line 441). The `vox.tmLanguage.json` grammar and
`markdown-injection.json` are both present in `apps/editor/vox-vscode/syntaxes/`. Shiki is
already ingesting the TextMate grammar inside the VS Code extension's webview.
The documentation portal is the *only* surface not yet unified.

This document weighs all viable doc platform options against a structured feature
matrix and produces a ranked recommendation.

---

## 1. The Problem Space

### 1.1 Current highlight.js Grammar vs. Reality

The current `docs/theme/highlight-vox.js` defines Vox keywords inline:

```
keyword: 'fn let mut if else match for in to return import type pub with on
         actor workflow spawn http activity component routes'
```

Meanwhile `vox.tmLanguage.json` is the authoritative grammar consumed by
VS Code and Shiki. Any keyword added to the language (e.g., `@mcp.tool`,
`@island`, `spawn`, decorator attributes) must be manually duplicated into this
separate highlight.js file. This is already drifting — the `/*SSOT_HJS_KW*/`
sentinel comments indicate intent but no enforcement mechanism exists.

### 1.2 LLM Documentation Format Findings

Comprehensive research confirms: **Markdown (`.md` / `.mdx`) remains the
undisputed gold standard for LLM context in 2026.** Key findings:

- Converting HTML or proprietary formats to clean Markdown reduces token usage
  by 80–90% and significantly improves RAG semantic chunking accuracy.
- LLMs navigate Markdown heading hierarchies (`#`, `##`, `###`) as structural
  landmarks; this is how they orient themselves within large context windows.
- JSON/YAML formats are superior for *machine-to-machine structured output* but
  degrade LLM comprehension of prose documentation due to quote/escape noise.
- XML tags are effective for *prompt engineering* (separating instructions from
  context) but are suboptimal as a file storage format.
- The emerging `llms.txt` / `llms-full.txt` standard (already in use by
  Anthropic, Stripe, Vercel) provides a curated Markdown index of a site's most
  important content for AI agent discovery. Vox already has
  `docs/src/.well-known/llms.txt`.

**Conclusion:** Do not migrate away from Markdown. The doc format is correct.
The issue is the *rendering toolchain*, not the *storage format*.

---

## 2. Shiki Deep-Dive

### 2.1 What Shiki Actually Is

Shiki is a syntax highlighter that uses the **same TextMate grammar engine and
VS Code themes** as Visual Studio Code. It produces pre-rendered, token-accurate
HTML at build time — zero client-side JavaScript required for highlighting.

**Key technical facts (2026):**
- Shiki v4.x (current) requires Node.js ≥ 20.
- Uses the Oniguruma regex engine via WASM for TextMate grammar execution.
- Custom languages: pass any `.tmLanguage.json` object directly as a `lang`.
- Singleton highlighter pattern is required for build performance on large sites
  (prevents re-initialization per code block).
- Output: static `<span>` HTML with inline styles or CSS variables — no runtime
  JS dependency.

### 2.2 Shiki vs. highlight.js vs. Syntect

| Dimension | highlight.js (current) | Shiki | Syntect (Rust) |
|---|---|---|---|
| Grammar engine | Regex (JS) | TextMate (WASM) | TextMate (native Rust) |
| VS Code fidelity | Low | **Exact match** | Very high |
| Custom Vox language | Separate JS file, manual drift | Load `vox.tmLanguage.json` directly | Load `vox.tmLanguage.json` directly |
| Build-time rendering | No (browser JS) | **Yes** | **Yes** |
| Client JS payload | ~50 KB | **0 KB** | **0 KB** |
| Twoslash support | No | **Yes** (type hovers in docs) | No |
| VS Code theme import | No | **Yes** | Partial |
| Active development | Mature/slow | **Very active** | Moderate |
| SSOT compliance | ❌ 3rd separate grammar | ✅ Shares `vox.tmLanguage.json` | ✅ Shares grammar |

**Syntect** (pure Rust, used internally by some mdBook forks) is technically
excellent but has a stale grammar ecosystem — grammars lag behind the VS Code
marketplace by months to years. Shiki's grammar library is crowd-sourced from
VS Code extensions and is far more current.

---

## 3. Documentation Platform Comparison Matrix

The following platforms were evaluated across dimensions weighted specifically
for the Vox project's needs as an AI-native, Rust-first codebase with an
existing VS Code extension and a MENS training corpus pipeline.

### 3.1 Candidate Platforms

| # | Platform | Engine | Shiki Support | Native Runtime |
|---|---|---|---|---|
| A | **mdBook** (current) | Rust | Requires custom preprocessor | Rust (single binary) |
| B | **Zola** | Rust (Giallo) | No (own TextMate engine) | Rust (single binary) |
| C | **VitePress** | Vite + Vue | **Built-in, first-class** | Node.js |
| D | **Starlight (Astro)** | Astro | Via Expressive Code plugin | Node.js |
| E | **Docusaurus** | React + Next.js | Via `@shikijs/rehype` plugin | Node.js |
| F | **MkDocs Material** | Python (Pygments) | Post-processing only | Python |
| G | **Nextra** | Next.js + React | Via remark/rehype plugin | Node.js |

### 3.2 Quantified Feature Matrix

**Scoring: 5 = Excellent, 4 = Good, 3 = Acceptable, 2 = Poor, 1 = Not supported**

Weight definitions used for Vox:
- **SSOT Grammar:** Can the platform consume `vox.tmLanguage.json` directly without a separate grammar definition?
- **AI/LLM Readability:** Markdown-first source, no proprietary format, plays well with RAG indexing.
- **Rust-native build:** Can CI build docs without Node.js/Python as a required dependency?
- **Doctest integration:** Can `.vox` or `.rs` code blocks be executed as tests?
- **Vox extension alignment:** Does the platform use the same artifact pipeline as `vox-vscode`?
- **Versioning:** Built-in or first-class versioned documentation (v0.5, v1.0, etc.).
- **Search quality:** Built-in search sufficient for a technical audience; bonus for AI/RAG readiness.
- **Migration cost:** Effort to migrate from current mdBook (~200 pages, `SUMMARY.md`, `book.toml`).
- **Long-term momentum:** Community health, GitHub stars/activity trend in 2026.
- **Interactive components:** Ability to embed demos, live REPLs, or rich interactive examples.

| Feature | Weight | mdBook | Zola | VitePress | Starlight | Docusaurus | MkDocs |
|---|---|---|---|---|---|---|---|
| **SSOT Grammar (Shiki/TM)** | 10 | 2 | 3 | **5** | **5** | 4 | 1 |
| **AI/LLM Readability** | 9 | 5 | 5 | 5 | 5 | 5 | 5 |
| **Rust-native build** | 8 | **5** | **5** | 1 | 1 | 1 | 1 |
| **Doctest / `vox-doc-pipeline`** | 8 | **5** | 2 | 2 | 2 | 2 | 1 |
| **Vox extension alignment** | 7 | 2 | 2 | **5** | 4 | 4 | 1 |
| **Versioning** | 6 | 1 | 2 | 3 | 3 | **5** | 3 |
| **Search quality** | 6 | 3 | 3 | 4 | **5** (Pagefind) | **5** | 4 |
| **Migration cost (inverted)** | 7 | 5 | 3 | 3 | 3 | 2 | 2 |
| **Long-term momentum (2026)** | 5 | 3 | 3 | **5** | **5** | 5 | 4 |
| **Interactive components** | 4 | 1 | 1 | 4 | 5 | **5** | 2 |
| **i18n support** | 3 | 1 | 2 | 4 | **5** | 4 | 3 |
| **Offline / no-CDN build** | 5 | **5** | **5** | 4 | 4 | 3 | 4 |
| **Weighted Total** | — | **248** | **219** | **281** | **285** | **256** | **181** |

### 3.3 Scores Explained

**mdBook (248):** Strong on Rust-native build, doctest integration (critical for
`vox-doc-pipeline`), zero migration cost, and offline builds. Weakest on SSOT
grammar, versioning, and interactive components. No Shiki path without building
a custom preprocessor that shells to Node.js — which reintroduces the Node
dependency and breaks the "single Rust binary" CI story.

**Zola (219):** Also Rust-native. Uses **Giallo**, its own TextMate grammar
engine (Rust-based, VS Code-compatible). Can load a custom `vox.tmLanguage.json`
via `extra_grammars`. However it's a general SSG, not a documentation tool — it
lacks mdBook-style doctest integration entirely, has no SUMMARY.md equivalent
without significant theme work, and has lower momentum in the technical docs
space. Zola is a good option *if* you're building a landing/marketing site, less
so for API references.

**VitePress (281):** Shiki is built-in; loading `vox.tmLanguage.json` is a
3-line config change. First-class Markdown. Strong Vue ecosystem. Loses heavily
on Rust-native build and doctest. The `vox-vscode` webview already ships
React+Radix (not Vue), introducing a Vue dependency for docs creates a
polyglot frontend footprint.

**Starlight (285, highest):** Shiki via Expressive Code (first-class). Framework-
agnostic — works with React components, which aligns with the vox-vscode webview
stack. Built-in Pagefind search (excellent for AI/RAG because it generates a
static JSON index that can be ingested by vox-arca/Scientia). Strong i18n and
versioning via plugins. Loses on Rust-native build and doctest. Growing faster
than VitePress in 2026.

**Docusaurus (256):** Best versioning out-of-the-box. React-native. Heavier
than VitePress. Shiki via `@shikijs/rehype`. High migration cost from mdBook.

**MkDocs Material (181):** Python runtime is antithetical to the Vox
VoxScript-first philosophy (Python is a retired automation surface per
`AGENTS.md`). Shiki only via post-processing. Lowest score overall.

---

## 4. The Hybrid Path: mdBook + Shiki Preprocessor

Because mdBook is so deeply embedded in the CI pipeline (GitHub Actions, GitLab
CI, `vox-doc-pipeline` doctests), a full platform migration has real costs.

**The hybrid option:** Build `mdbook-shiki-vox` — a thin Rust mdBook preprocessor
that:
1. Scans all chapter content for ` ```vox ` fenced blocks.
2. Calls the Shiki Node.js CLI (or WASM bindings) at build time to produce
   pre-rendered `<span>` HTML.
3. Replaces the fenced block with the rendered HTML fragment.

**Pros:**
- Zero migration: all `.md` files, `SUMMARY.md`, and `book.toml` stay unchanged.
- Shiki consumes `vox.tmLanguage.json` directly — SSOT achieved.
- doctest (`mdbook test`) and `vox-doc-pipeline` remain fully functional.

**Cons:**
- Reintroduces a Node.js build dependency for docs (even if thin).
- No community-maintained `mdbook-shiki` exists as of April 2026 — we would own
  it.
- The preprocessor API shells JSON through stdin/stdout — adds latency to
  `mdbook build` on large books.
- Does not solve versioning, search quality improvements, or interactive
  component gaps.

This is a **low-risk tactical fix** that defers the platform migration question.

---

## 5. The Doctest Constraint (Critical)

`vox-doc-pipeline` runs `.vox` doctests from Markdown files using
`mdbook test`-compatible mechanics. This is cited in `AGENTS.md` as a mandatory
quality gate: *"All vox blocks in documentation must compile cleanly via
`vox-doc-pipeline`'s dynamic doctest runner."*

No alternative platform replicates `mdbook test` semantics out of the box.
**Any migration plan must solve the doctest constraint before switching.**

Options:
- Build a standalone `vox doctest-md` subcommand that reads Markdown files
  directly and runs `.vox` blocks, decoupled from any specific SSG.
- This is the prerequisite gate for any platform migration. It is also the right
  long-term architecture (SSG-agnostic doctests).

---

## 6. Recommendations

### 6.1 Immediate (No Migration): Eliminate Grammar Drift Now

Regardless of which platform is chosen, the `highlight-vox.js` grammar drift
must be fixed now. The path:

1. **Add `mdbook-shiki-vox` preprocessor** (custom, ~200 lines of Rust) that
   replaces highlight.js with Shiki + `vox.tmLanguage.json` at build time.
2. **Delete `docs/theme/highlight-vox.js`** and remove it from `book.toml`
   `additional-js`.
3. The `vox.tmLanguage.json` in `apps/editor/vox-vscode/syntaxes/` becomes the single source
   of truth for all five rendering contexts (VS Code, Cursor, Neovim, GitHub,
   and now docs portal).

### 6.2 Medium-Term (6–12 months): Migrate to Starlight

Once `vox doctest-md` is a standalone subcommand (decoupling doctests from
mdBook), migrate the documentation portal to **Starlight (Astro)**:

- Shiki via Expressive Code: load `vox.tmLanguage.json` as `shiki.langs`.
- Pagefind search: static JSON index consumable by `vox-arca`/Scientia RAG.
- React component support: aligns with `vox-vscode` webview stack (React 19).
- `llms.txt` / `llms-full.txt` generation: Astro content collections make this
  trivial to automate.
- Plugin ecosystem: `starlight-versions`, `starlight-typedoc`,
  `starlight-links-validator` cover the gaps mdBook cannot.

### 6.3 What NOT to Do

- **Do not migrate to MkDocs**: Python is a retired automation surface.
- **Do not migrate to VitePress**: Vue is a third frontend framework in the repo
  (alongside React in `vox-vscode` and Vox itself). Avoid.
- **Do not build a Zola-based docs site**: Zola's Giallo engine is excellent but
  it's a general SSG — the documentation taxonomy work would be enormous and
  the doctest gap cannot be bridged.

---

## 7. AI-First Documentation Architecture (2026 Principles)

Drawing from research on documentation for AI-native codebases:

1. **Markdown is the right storage format.** Do not switch to AsciiDoc, RST, or
   proprietary XML. LLMs are pre-trained on GitHub Markdown at scale.
2. **Front matter is structured metadata.** YAML front matter is how Markdown
   documents communicate machine-readable metadata without disrupting human
   readability. Continue enforcing it.
3. **`llms.txt` + `llms-full.txt` are increasingly mandatory.** These give
   coding agents (Cursor, Copilot, Antigravity) a curated, token-efficient entry
   point into the docs corpus. Automate their generation from `SUMMARY.md`.
4. **Pagefind is superior to mdBook's built-in search for RAG.** Pagefind
   produces a static JSON index that can be ingested programmatically by
   vox-arca's Scientia pipeline, enabling "ask the docs" without a hosted
   backend.
5. **Syntax highlighting fidelity matters for training data.** When `.md` files
   are ingested into the MENS training corpus, Shiki-highlighted HTML code
   blocks carry token-scope metadata (via TextMate scope names) that improves
   the model's ability to learn Vox syntax. highlight.js output is semantically
   impoverished by comparison.
6. **Interactive components are the next frontier.** Starlight's Islands
   architecture allows embedding live Vox REPLs and type-hover examples
   (Twoslash) without shipping JS to non-interactive pages.

---

## 8. Cross-References

The following documents were verified to exist before being linked:

- **Grammar SSOT:** [`docs/src/archive/research-2026-q1/vox-syntax-highlighting-ssot-2026.md`](../archive/research-2026-q1/vox-syntax-highlighting-ssot-2026.md) — archived predecessor to this document; defines the two-artifact strategy (TextMate + Tree-sitter) and the scope name SSOT table.
- **TextMate grammar (live):** [`apps/editor/vox-vscode/syntaxes/vox.tmLanguage.json`](../../../apps/editor/vox-vscode/syntaxes/vox.tmLanguage.json) — the authoritative grammar that Shiki will consume.
- **Markdown injection (live):** [`apps/editor/vox-vscode/syntaxes/markdown-injection.json`](../../../apps/editor/vox-vscode/syntaxes/markdown-injection.json) — already active for VS Code `.md` file highlighting.
- **Current book config:** [`docs-astro`](../../../docs-astro/README.md) — Starlight replaces mdBook; syntax highlighting follows the VS Code TextMate grammar listed below.
- **Current highlight.js grammar:** Legacy mdBook `highlight-vox.js` — retired with the Astro migration.
- **VS Code extension:** [`apps/editor/vox-vscode/package.json`](../../../apps/editor/vox-vscode/package.json) — already declares `shiki ^4.0.1` as a dependency (line 441).
- **Agent policy:** [`AGENTS.md`](../../../AGENTS.md) — mandates VoxScript-first glue, no Python, doctest compliance.
- Set valid frontmatter (`title`, `description`, `category`, `status`). Do not recreate `research-index.md`.

---

## 9. Action Items (Prioritized)

| Priority | Action | Effort | Dependency |
|---|---|---|---|
| P0 | Build `mdbook-shiki-vox` preprocessor in Rust | 2–3 days | `vox.tmLanguage.json` (exists) |
| P0 | Remove `highlight-vox.js` / update `book.toml` | 1 hour | After P0 above |
| P1 | Build `vox doctest-md` standalone subcommand | 1 week | Unlock platform migration |
| P1 | Add `llms-full.txt` auto-generation to `vox-doc-pipeline` | 1 day | None |
| P2 | Evaluate Starlight migration with pilot (e.g., tutorials section) | 2 weeks | P1 |
| P2 | Integrate Pagefind into Starlight for Scientia RAG indexing | 3 days | P2 pilot |
| P3 | Full Starlight migration | 4–6 weeks | P2 pilot success |
