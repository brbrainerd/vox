# 01 — Landing page prompt
_Last reviewed: 2026-05-23. Output: single-file React JSX with Tailwind + shadcn/ui + lucide-react._

Run this prompt verbatim in [Claude.ai](https://claude.ai). The output is a React Artifact you copy into the Astro site as the new `/` route.

---

## The full prompt (paste this verbatim into Claude.ai)

```xml
<role>
You are a senior product designer building a marketing landing page for an
open-source programming language. You output a single React component file.
</role>

<context>
The product is Vox — an open-source, Apache 2.0 programming language compiled
from one source file into a database schema, a type-safe HTTP server, and a
reactive browser UI. It was designed after the era of large language models,
with grammar-constrained decoding, errors-as-values, and durable execution as
first-class language primitives.

The site is voxlang.org, hosted statically on Cloudflare Pages, built by Astro.
This page is the marketing landing — the URL is `/`. Other routes (concepts,
showcase, reference, tutorials) exist separately and are linked from here.

Two audiences read this page:
1. Senior engineers evaluating whether to spend a Saturday trying it.
2. Curious non-engineers who heard "AI-native language" on a podcast.
Both leave with the same gestalt: this is grown-up software with a generous heart.
</context>

<voice-sample>
Match the cadence and register of this paragraph (NOT the topic):

"Most modern web apps need a database, a server, and a browser interface —
three pieces of software, three programming languages, three sets of types
describing the same things. Drift between them is the source of about half
of all production bugs. Vox collapses those three into one declaration in
one file. The compiler does the rest."

Em-dashes for parentheticals. Contractions allowed. One idea per sentence.
Concrete claims, not hedged. No exclamation points anywhere.
</voice-sample>

<available>
<!-- Use only these imports. Do not invent new ones. -->
React: useState, useEffect, useRef (rarely needed; mostly static page)
shadcn/ui (import from "@/components/ui/<name>"):
  Button, Card, CardHeader, CardTitle, CardContent, Badge, Separator
lucide-react (import by name):
  ArrowRight, ArrowUpRight, Check, ChevronRight, Code, Database, ExternalLink,
  Github, Layers, Network, Play, Server, Sparkles, Terminal, Zap

<!-- Tailwind semantic classes (use these, not raw colors) -->
bg-background, bg-card, bg-primary, bg-secondary, bg-muted, bg-accent
text-foreground, text-muted-foreground, text-primary, text-primary-foreground
border-border, ring-ring
font-display (Fraunces serif headings), font-sans (Inter body), font-mono (JetBrains Mono)

<!-- Custom hex tokens for code blocks (use directly when needed) -->
Deepwell #1A1F2E (code block background)
Code text #E2E8F0
Code muted #94A3B8

<!-- Spacing scale -->
p-/m-/gap-: 1, 2, 3, 4, 6, 8, 12, 16, 24, 32
</available>

<style-tokens>
:root {
  --background: 40 33% 97%;
  --foreground: 222 31% 8%;
  --card: 40 33% 97%;
  --card-foreground: 222 31% 8%;
  --primary: 16 100% 60%;
  --primary-foreground: 0 0% 100%;
  --secondary: 36 22% 87%;
  --secondary-foreground: 222 31% 8%;
  --muted: 36 22% 87%;
  --muted-foreground: 217 13% 47%;
  --accent: 167 42% 66%;
  --accent-foreground: 222 31% 8%;
  --border: 36 22% 87%;
  --ring: 16 100% 60%;
  --radius: 0.75rem;
}
</style-tokens>

<sandbox-constraints>
The output runs in a React artifact sandbox. Hard limits:
- No fetch, no network calls, no remote images. Use /api/placeholder/W/H for images.
- No localStorage / sessionStorage. State is useState only.
- No relative imports — single file only.
- No Next.js, no Vite, no Astro inside the artifact. Plain React + Tailwind.
- Tailwind arbitrary values (h-[613px]) silently fail — stick to the standard scale.
</sandbox-constraints>

<task>
Build a single React component named VoxLanding that renders a marketing
landing page with exactly these 7 sections in order. Reuse the patterns
in <reference-components> for each.

1. Hero
   - Eyebrow (exactly 5 words): "An AI-native programming language."
   - Headline (exactly 9 words): "One file. Schema, server, and UI — all type-safe."
   - Subhead (range 28–34 words): One paragraph naming the concrete benefit and
     the concrete audience. Use the <voice-sample> register.
   - Two CTAs side-by-side:
     - Primary: "Read the 5-minute tour" → href="/concepts/"
     - Secondary: "Get started" → href="/tutorials/tut-getting-started/"
   - Trust strip (exact text): "Apache 2.0 · 101 Rust crates · Open Collective backed"
   - Right column: hero image placeholder (/api/placeholder/800/600) with
     descriptive alt "Compilation as a prism: one source unfolding into
     database, server, and browser UI layers." Caption (10–14 words) underneath.
   - Hero bg: bg-background. Section padding: py-16 md:py-32.

2. The pitch in one snippet (Deepwell background)
   - Above the split: H2 (exact text): "One declaration. The compiler does the rest."
   - CodeOutputSplit with this exact code (filename "tasks.vox"):
       @table type Task {
           title: str
           done:  bool
           owner: str
       }

       @query
       fn open_tasks(by: str) to list[Task] {
           return db.Task.where({ owner: by, done: false })
       }

       component TaskList(tasks: list[Task]) {
           view: column() {
               tasks.map(fn(t) { row() { text() { t.title } } })
           }
       }
   - Output column ("Generated artifacts"):
     - "Database" — SQLite migration, indexed on (owner, done)
     - "Server" — Axum router with typed JSON, OpenAPI-described
     - "Client" — React/TSX component + vox-client.ts RPC bridge
     - "Tooling" — LSP diagnostics live; CI catches drift before merge
   - Below the split, single right-aligned link in small font:
     "See more snippets →" → href="/showcase/"
   - Section bg: bg-[#1A1F2E]. Headings and prose: text-[#FAF8F3].
     Code block keeps its own Deepwell bg. Output column: bg-secondary (warm gray).

3. Five pillars (bg-background)
   - 3-column grid on lg, 2-col on sm, 1-col mobile. 5 cards total — the 5th
     centers itself on row 2 (use the spec in <reference-components>).
   - Pillar content (use this exactly, including the Status badges):

   #1 Single source of truth | Stable
       Body: "One declaration becomes the schema, the API surface, and the
       typed client. Schema drift is a compile error, not a 2 AM page."
       Learn more → href="/concepts/#one-source-of-truth"

   #2 Designed for LLMs first | Stable
       Body: "Grammar-constrained decoding means a language model literally
       cannot sample invalid syntax. Errors-in-types means there is no
       exception escape hatch."
       Learn more → href="/concepts/#designed-for-llms"

   #3 React interop | Preview
       Body: "`component` declarations compile to React/TSX. `@query`,
       `@mutation`, and `@server` compile to typed endpoints plus a
       generated vox-client.ts bridge."
       Learn more → href="/concepts/#react-interop"

   #4 Agents, MCP, and durable execution | Stable
       Body: "`@mcp.tool` and `@mcp.resource` expose typed functions to
       any Model Context Protocol client. The vox-orchestrator routes
       work by file affinity and ten policy modules. `workflow` and
       `activity` get journal-backed replay (ADR-041) for linear
       activities + deterministic branches + `workflow_wait` timers."
       Learn more → href="/concepts/#durable"

   #5 Local training | Preview
       Body: "`vox populi` runs QLoRA fine-tunes on consumer GPUs through
       Candle and Burn — no Python in the path. Serves over an
       OpenAI-compatible HTTP endpoint."
       Learn more → href="/concepts/#local-training"

   Render the "Stable"/"Preview" labels as small shadcn <Badge> in the top-right
   corner of each card. Stable: variant="default". Preview: variant="secondary".

4. Static playground (Ink background, bg-foreground)
   - H2 (exact text): "Try it without installing."
   - Subhead (one sentence, 18–24 words): describes that the playground compiles
     and runs Vox in a sandboxed environment; state resets per session.
   - StaticPlayground component (see <reference-components>) with this code:
       fn main() {
           let g = "world"
           print("Hello, " + g + "!")
       }
     simulatedOutput: "Hello, world!"
   - Two small links below, side-by-side, text-muted-foreground:
     "Open the full playground →" href="/playground/"
     "Or install locally →" href="/tutorials/tut-getting-started/"

5. Who Vox is for (bg-background)
   - H2 (exact text): "Three readers. Three first steps."
   - 3-up grid. Each persona has:
     - 80×80px placeholder image (/api/placeholder/80/80) with descriptive alt
     - Display-font name
     - One-line value claim (exact text below)
     - One concrete next step (text + arrow)

   Persona 1: "The backend engineer"
     Value: "You will write less integration code and your migrations will
       stop fighting your API types."
     Next: "Read: how @table collapses three layers →" href="/concepts/#one-source-of-truth"

   Persona 2: "The agentic builder"
     Value: "MCP tools are typed and discoverable from the same file; the
       orchestrator routes agent work by capability. Durable `workflow`
       and `activity` get journal-backed replay (ADR-041) — crashes
       resume completed activities without re-execution."
     Next: "Read: durable execution and MCP tools →" href="/how-to/how-to-ai-agents/"

   Persona 3: "The researcher"
     Value: "Run QLoRA on your laptop, serve over an OpenAI-compatible HTTP
       endpoint, ship the model with the language. No Python."
     Next: "Read: the MENS pipeline →" href="/reference/mens-training/"

6. Where Vox stands today (bg-background, after Personas)
   - H2 (exact text): "Where Vox stands today."
   - Subhead (one sentence, 18–24 words): describes the maturity model briefly.
   - Stability table — render the following exactly. (On the live site this
     section is replaced by a sync marker; for the artifact, include the
     table inline.)

   | Surface                         | Tier         |
   |---------------------------------|--------------|
   | Compiler core                   | Stable       |
   | @table + data layer             | Stable       |
   | MCP tool exposure               | Stable       |
   | @query/@mutation/@server        | Preview      |
   | Durable runtime                 | Preview      |
   | React interop                   | Preview      |
   | Local training (MENS)           | Preview      |
   | Distributed mesh                | Experimental |

   Each tier is a shadcn Badge: Stable→default (Primary), Preview→secondary,
   Experimental→outline.

7. Footer (use the Footer component from <reference-components>)

After section 7, no trailing content. Page ends.
</task>

<reference-components>
(Paste the JSX for Hero, CodeOutputSplit, PillarCard, StaticPlayground, and
Footer from docs/design-system/06-component-specs.md here. The artifact must
include the JSX for these components in the same file as VoxLanding.)
</reference-components>

<output-spec>
Output: one JSX artifact, default-exporting a component named VoxLanding.
- All listed components and VoxLanding live in one file.
- No `import` from any path other than "react", "@/components/ui/*", and "lucide-react".
- All images use /api/placeholder/W/H paths.
- All copy is exact when specified; ranges are followed for prose.
- The page works keyboard-only and honors prefers-reduced-motion.
</output-spec>

<self-check>
Before emitting the artifact, verify each item and prefix your response with
PASS/FAIL per item. Fix any FAIL before responding.

1. The file contains exactly one `export default function VoxLanding`.
2. Every import path is one of: "react", "@/components/ui/<name>", "lucide-react".
   List every import line and verify against <available>.
3. Every lucide icon used appears in the <available> list.
4. Every shadcn component used appears in the <available> list.
5. No `fetch`, no `localStorage`, no `sessionStorage`, no `window.location.href` assignment.
6. Every <img> has a descriptive alt (not "hero image", not "icon").
7. The Hero headline is the exact 9 words specified.
8. The Hero eyebrow is the exact 5 words specified.
9. The Pitch H2 is exactly: "One declaration. The compiler does the rest."
10. The "5 pillars" section contains exactly 5 cards in the order listed.
11. Each pillar card shows a Stable or Preview Badge as specified.
12. Section 6 (stability table) shows the 8 surfaces in the order listed.
13. The Footer renders the three columns specified, with no newsletter signup.
14. There is exactly one <h1> on the page; subsequent sections use <h2>.
15. There is a `Skip to content` link at page top (sr-only, focus:not-sr-only).
</self-check>

Return the PASS/FAIL list, then the JSX artifact.
```

---

## Why this prompt looks like this (reading guide for prompt authors)

- **`<role>`, `<context>`, `<voice-sample>`, `<available>`, `<style-tokens>`, `<sandbox-constraints>`, `<task>`, `<reference-components>`, `<output-spec>`, `<self-check>`** — XML-tagged structure. Anthropic's Claude 4 docs note this structure produces measurably higher-fidelity output than equivalent prose. Most load-bearing constraints go last; Claude weights tail tokens higher.
- **Voice sample, not adjectives.** Research finding: "quietly confident" produces meta-commentary; a 40-word sample produces voice match.
- **Available imports listed before the task.** Verification-before-generation — the highest-ROI anti-hallucination move for React output. Without this, Claude invents `@/lib/utils` paths and unspecified icons.
- **Self-check enumerates a closed list of items.** Open-ended "review accessibility" gets rubber-stamped. Item-by-item closed-list verification works.
- **Hard word counts on hero copy, ranges on body prose.** Hard counts produce sharper compression. Ranges allow natural rhythm.
- **Positive reframing.** The "do not" list is short and only covers sandbox limits (which are factual, not aesthetic). Aesthetic constraints are expressed as available tokens + reference components.
- **One concrete pillar per Stable / Preview status.** Honest framing prevents marketing claims that would break on inspection — `<reference-components>` pulls verified syntax from `07-content-blocks.md` § "Code-snippet templates."

## Integration after the artifact returns

1. Paste the JSX into a temporary `.jsx` file in your editor.
2. Run `node docs-astro/scripts/integrate-landing.mjs path/to/file.jsx` (the integration script — write this once when you first run a prompt; see `08-integration-workflow.md` TODO).
3. The script moves the component into `docs-astro/src/pages/index.astro`, wraps it for Astro's React integration, and rewrites `/api/placeholder/...` paths to real `/assets/...` paths if matching images exist.
4. Preview with `pnpm dev`. Adjust the **prompt** to fix structural issues. Re-run.
