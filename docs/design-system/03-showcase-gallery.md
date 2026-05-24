# 03 — Showcase gallery prompt
_Last reviewed: 2026-05-23. Output: single-file React JSX with Tailwind + shadcn/ui + lucide-react._

For `/showcase/`. Eight focused demos. Each is a vertical slice of one capability — a snippet that compiles, runs, and demonstrates a single brand pillar.

---

## The full prompt (paste this verbatim into Claude.ai)

```xml
<role>
You are a senior product designer building a code-and-output showcase page
for an open-source programming language. You output a single React component
file.
</role>

<context>
Vox is an open-source, Apache 2.0 programming language compiled from one
source file into a database schema, a type-safe HTTP server, and a reactive
browser UI. Designed after the era of large language models.

This page is /showcase/ at voxlang.org. Visitors arrive here from the landing
page CTAs and from inbound links in blog posts, conference talks, and the
README. The story arc across the 8 demos is the design — they MUST appear
in this exact order:

  1. Single-file fullstack (elevator pitch — 14 lines)
  2. Errors-in-types (safety pitch)
  3. Phonetic operators (LLM pitch, in passing)
  4. Durable workflow (workflow + activity keywords; journal-backed replay per ADR-041)
  5. MCP tool exposure (AI integration pitch)
  6. Reactive UI (front-end pitch) — Preview
  7. Local training one-liner (MENS pitch) — Preview
  8. Mesh dispatch (distributed pitch) — Experimental

After demo 8, the page ends. No "view more examples" link, no upsell. The
full corpus lives at examples/golden/ on GitHub (linked once in the footer).
</context>

<voice-sample>
Match the cadence and register of this paragraph (NOT the topic):

"Most modern web apps need a database, a server, and a browser interface —
three pieces of software, three programming languages, three sets of types
describing the same things. Drift between them is the source of about half
of all production bugs. Vox collapses those three into one declaration in
one file. The compiler does the rest."
</voice-sample>

<available>
React: useState (for a single tiny copy-button toast if added)
shadcn/ui: Card, Badge
lucide-react: ArrowRight, Check, Copy, ExternalLink, Github, Zap

Tailwind semantic classes (use these, not raw colors):
bg-background, bg-card, bg-secondary, bg-primary, bg-accent, bg-muted
text-foreground, text-muted-foreground, text-primary, text-primary-foreground
border-border, ring-ring
Custom hex (code blocks only):
  Deepwell #1A1F2E (bg)
  Code text #E2E8F0
  Code muted #94A3B8
</available>

<style-tokens>
[Paste from 04-visual-style-guide.md "Brand color tokens" :root block]
</style-tokens>

<sandbox-constraints>
- No fetch, no localStorage, no remote images.
- Single file only — no relative imports.
- Tailwind standard scale only.
</sandbox-constraints>

<task>
Build a single React component named VoxShowcase. Page structure:

╔═════════════════════════════════════════════════════════════════════════╗
║ Page top                                                                ║
║   <h1> (exact text): "Eight ways Vox earns the file count of one."     ║
║   Subhead (one paragraph, 22–35 words). Use the <voice-sample>          ║
║   register. Reference: each demo is one .vox file; the output column    ║
║   shows what gets generated, executed, or exposed.                      ║
║   No image. Generous bottom margin (mb-16).                             ║
╚═════════════════════════════════════════════════════════════════════════╝

Then 8 demo blocks. Each demo uses the DemoBlock component from
<reference-components>. Each block has:
  - id="demo-{slug}" for deep-linking
  - aria-labelledby pointing at the H2
  - top-right Badge with pillar tag
  - 1-paragraph framing in 35–55 words (in the voice-sample register)
  - CodeOutputSplit with the code below and a list of 3–4 outputs

Use these EXACT specs.

═════════════════════════════════════════════════════════════════════════
Demo 1 — slug: "fullstack"
  Title: "A full-stack app in 14 lines."
  Pillar: "Single source of truth"
  Badge variant: default (Primary)
  Framing (35–55 words): one paragraph explaining: one file → @table becomes
    a SQLite table; @query becomes a typed HTTP route; component becomes a
    React component; routes wires the URL. Working app from a single compile.

  filename: "tasks.vox"
  code:
    @table type Task {
        title: str
        done:  bool
    }

    @query
    fn list_tasks() to list[Task] {
        return db.Task.all()
    }

    component TaskList(tasks: list[Task]) {
        view: column() {
            tasks.map(fn(t) { row() { text() { t.title } } })
        }
    }

    routes { "/" to TaskList }

  outputLabel: "Generated artifacts"
  outputs:
    - { title: "migrations/001_task.sql", description: "Schema with primary key, indexed columns." }
    - { title: "server/api.rs", description: "Axum router with GET /api/list_tasks returning typed JSON." }
    - { title: "client/TaskList.tsx", description: "React component with typed props." }
    - { title: "client/vox-client.ts", description: "RPC bridge with list_tasks() method." }

═════════════════════════════════════════════════════════════════════════
Demo 2 — slug: "errors"
  Title: "Errors are values."
  Pillar: "Designed for LLMs first"
  Badge variant: default

  Framing (35–55 words): one paragraph. Vox has no exceptions. A function
  that can fail returns Result[T]. The compiler refuses to build code that
  doesn't `match` both arms. Result: the model can't write "happy-path-only"
  code that crashes in production.

  filename: "transfer.vox"
  code:
    @mutation
    fn transfer(from: Id[Account], to: Id[Account], cents: int) to Result[Unit] {
        if cents <= 0 {
            return Error("amount must be positive")
        }
        let sender = db.Account.find(from)?
        if sender.balance < cents {
            return Error("insufficient funds")
        }
        db.update(sender, { balance: sender.balance - cents })
        let receiver = db.Account.find(to)?
        db.update(receiver, { balance: receiver.balance + cents })
        return Ok(Unit)
    }

  outputLabel: "Compiler guarantees"
  outputs:
    - { title: "Every caller must `match`", description: "Both Ok and Error arms required at the call site." }
    - { title: "`?` short-circuits cleanly", description: "No unwrapped value reaches the next line on Error." }
    - { title: "Atomicity", description: "Two db.update calls wrapped in an implicit transaction." }
    - { title: "Opaque IDs", description: "Id[Account] is type-checked — wrong account ID is a type error." }

═════════════════════════════════════════════════════════════════════════
Demo 3 — slug: "phonetic"
  Title: "Words a model already knows."
  Pillar: "Designed for LLMs first"
  Badge variant: default

  Framing (35–55 words): Vox uses `not`, `and`, `or`, `is`, `isnt` instead
  of `!`, `&&`, `||`, `==`, `!=`. Words appear in language-model training
  data far more often than punctuation patterns. The grammar is faster to
  predict correctly and reads aloud unambiguously.

  filename: "classify.vox"
  code:
    fn classify(score: int, override: bool) to str {
        if override is true {
            return "admin"
        }
        if score >= 90 and score isnt 100 {
            return "expert"
        }
        if score < 50 or not is_calibrated() {
            return "novice"
        }
        return "regular"
    }

  outputLabel: "Why this matters"
  outputs:
    - { title: "`is` reads aloud as 'is'", description: "Predictable token boundary." }
    - { title: "`isnt` is one token", description: "In most model vocabularies; `!=` is usually two." }
    - { title: "`!=` parses with a hint", description: "The lexer emits a diagnostic pointing at `isnt`." }

═════════════════════════════════════════════════════════════════════════
Demo 4 — slug: "durable" — STABLE
  Title: "A workflow that survives node death."
  Pillar: "Agents, MCP, and durable execution"
  Badge variant: default (Stable)

  Framing (35–55 words): `workflow` and `activity` are keywords. The
  runtime journals each activity result; a crash mid-flight resumes on
  restart with completed activities replayed from the journal. The
  supported subset is linear activity execution + deterministic `if` +
  `workflow_wait` timer replay.

  filename: "ship_order.vox"
  code:
    activity charge_card(order: Id[Order]) to Result[str] {
        return Ok("charged:" + str(order))
    }

    activity reserve_inventory(order: Id[Order]) to Result[str] {
        return Ok("reserved:" + str(order))
    }

    workflow ship_order(order: Id[Order]) to Result[str] {
        let tx = charge_card(order)?
        let inventory = reserve_inventory(order)?
        return Ok("shipped:" + tx + ":" + inventory)
    }

  outputLabel: "Runtime guarantees"
  outputs:
    - { title: "Per-run journal", description: "Each activity result persisted under (run_id, workflow, activity_id)." }
    - { title: "Crash resume", description: "Completed activities replayed from the journal on restart; only the remainder re-executes." }
    - { title: "Exactly-once on success", description: "charge_card runs exactly once even after a node restart." }
    - { title: "ADR-019 contract", description: "journal_version: 1 — replay shape locked." }

═════════════════════════════════════════════════════════════════════════
Demo 5 — slug: "mcp"
  Title: "Type-safe tools for any AI agent."
  Pillar: "Designed for LLMs first"
  Badge variant: default

  Framing (35–55 words): `@mcp.tool` exposes a function as a Model Context
  Protocol tool. Any MCP-compatible agent can call it with type-checked
  arguments. Tool description, parameter schema, and return type are all
  derived from the function signature.

  filename: "tools.vox"
  code:
    @mcp.tool "Search the project's docs by query"
    fn search_docs(query: str, limit: int) to list[DocResult] {
        return docs.semantic_search(query, limit)
    }

    @mcp.resource("vox://docs/recent", "Recent doc updates this week")
    fn recent_doc_updates() to list[DocUpdate] {
        return docs.recent(7)
    }

  outputLabel: "What the agent sees"
  outputs:
    - { title: "Tool name: search_docs", description: "Description from the docstring." }
    - { title: "Args: { query, limit }", description: "JSON Schema generated from the function signature." }
    - { title: "Resource: vox://docs/recent", description: "Discoverable via MCP resource listing." }

  IMPORTANT: do NOT write `@mcp.resource "GET /tasks/{owner}"` — the
  URI is a string, not an HTTP pattern. The shape above is verified.

═════════════════════════════════════════════════════════════════════════
Demo 6 — slug: "reactive" — PREVIEW
  Title: "A reactive component, no React imports."
  Pillar: "React interop"
  Badge variant: secondary (Preview)

  Framing (35–55 words): `component` declares a UI fragment. `state`
  declares reactive state. `on click` declares an event handler. The
  compiler emits a React/TSX component that uses hooks correctly. You
  write Vox; the user sees React.

  filename: "counter.vox"
  code:
    component Counter() {
        state count: int = 0

        view: column() {
            text() { "Clicks: " + str(count) }
            button() {
                label: "+1"
                on click: { count = count + 1 }
            }
        }
    }

    routes { "/counter" to Counter }

  outputLabel: "Generated React (Preview)"
  outputs:
    - { title: "Counter.tsx", description: "Functional component with useState." }
    - { title: "Setter wrapped in useCallback", description: "No unnecessary re-renders." }
    - { title: "Initial state via vox-client.ts", description: "Server-fetched on first paint." }
    - { title: "Progressive hydration", description: "Pre-rendered HTML; hydrates with the bundle." }

═════════════════════════════════════════════════════════════════════════
Demo 7 — slug: "training" — PREVIEW
  Title: "Fine-tune a model on your code."
  Pillar: "Local training"
  Badge variant: secondary (Preview)

  Framing (35–55 words): `vox populi train` reads the .vox files in your
  project, builds a QLoRA fine-tuning dataset, and runs Candle against a
  chosen open-source base model. On a consumer GPU it takes hours, not
  days. The output is a serveable adapter, not a 70GB checkpoint.

  filename: "two-commands.sh"
  code:
    vox populi train --base qwen-2.5-coder-7b --epochs 3 --output ./adapter
    vox populi serve --adapter ./adapter --port 8001

  outputLabel: "What you get (Preview)"
  outputs:
    - { title: "./adapter/model.safetensors", description: "QLoRA adapter, ~50–200 MB." }
    - { title: "./adapter/tokenizer.json", description: "HF tokenizer for the chosen base." }
    - { title: "./adapter/manifest.json", description: "Training run hash, eval scores, base model SHA." }
    - { title: "OpenAI-compatible HTTP", description: "Served at /v1/chat/completions on the chosen port." }

═════════════════════════════════════════════════════════════════════════
Demo 8 — slug: "mesh" — EXPERIMENTAL
  Title: "Heterogeneous machines, one orchestrator."
  Pillar: "One mesh"
  Badge variant: outline (Experimental)

  Framing (35–55 words): nodes join the mesh by setting two environment
  variables. The orchestrator routes workloads based on node capabilities.
  Cross-machine agent messages are type-checked at compile time; wire
  mismatches are not possible.

  IMPORTANT: do NOT use @require_capability — it does not exist yet.
  Show the env-var join pattern only.

  filename: "two-machines.sh"
  code:
    # On the workstation with a GPU
    VOX_MESH_ENABLED=1 VOX_MESH_NODE_ID=workstation-a vox populi serve

    # On the laptop
    VOX_MESH_ENABLED=1 VOX_MESH_NODE_ID=laptop-b vox populi serve

    # From any node, dispatch:
    vox populi run train_adapter.vox --route-to-capability gpu

  outputLabel: "What the mesh does (Experimental)"
  outputs:
    - { title: "Capability registration", description: "Each node advertises its hardware on join." }
    - { title: "Routing by best fit", description: "GPU work goes to a CUDA-capable node." }
    - { title: "Resilience", description: "Workflows survive the dispatched node going offline." }

═════════════════════════════════════════════════════════════════════════

Footer (single line, after demo 8):
  text-sm text-muted-foreground, centered:
  "The full corpus of runnable examples lives at examples/golden/. 30+ files. All tested in CI."
  Make "examples/golden/" a link to:
  https://github.com/vox-foundation/vox/tree/main/examples/golden

After this footer line, page ends. No fat footer.
</task>

<reference-components>
(Paste the DemoBlock and CodeOutputSplit JSX from
docs/design-system/06-component-specs.md here. Inline in the artifact.)
</reference-components>

<output-spec>
- Single JSX artifact, default-exporting VoxShowcase.
- Exactly 8 DemoBlock components, in the order specified.
- Each DemoBlock has its unique id="demo-{slug}" for deep-linking.
- Page background bg-background throughout (no alternating sections).
- Code blocks use #1A1F2E hex bg (Deepwell) per <available>.
- Output columns use bg-secondary (Stone).
</output-spec>

<self-check>
Verify each item, prefix response with PASS/FAIL per item. Fix FAILs first.

1. Exactly 8 demos, in the exact slug/order: fullstack, errors,
   phonetic, durable, mcp, reactive, training, mesh.
2. No demo uses `@endpoint(kind: ...)`. Only `@query`, `@mutation`, `@server`.
3. Demo 4 (durable) uses `workflow` + `activity` keywords and references
   ADR-019/ADR-041 for the supported subset.
4. Every `match` arm uses `=>` (fat arrow), not `->`.
5. Demo 5 (mcp) uses `@mcp.resource("vox://...", "description")` — URI
   string, two arguments. NOT a quoted HTTP pattern.
6. Demo 8 (mesh) does NOT contain `@require_capability` anywhere.
7. Each demo has a Badge with the variant specified (default / secondary /
   outline).
8. Demos 6, 7 carry "Preview" labeling in the outputLabel.
9. Demo 8 carries "Experimental" labeling in the outputLabel.
10. Page has exactly one <h1>.
11. Each DemoBlock has aria-labelledby pointing at its H2's id.
12. The trailing footer line is the only content after demo 8.
13. Imports: only "react", "@/components/ui/*", "lucide-react".
14. No fetch, no localStorage.
15. Skip-to-content link present at page top.
</self-check>

Return the PASS/FAIL list, then the JSX artifact.
```

---

## What changed vs. the previous version

- Output is single-file React JSX, not Astro/inline-CSS.
- Demo 1 now uses `@query` (was `@endpoint(kind: query)`).
- Demo 2 uses `@mutation` (was `@endpoint(kind: mutation)`).
- Demo 4 (durable) uses `workflow` + `activity` keywords with real journal-backed runtime per ADR-041 (was an `@durable fn` stub claim).
- Demo 5 (MCP) uses correct `@mcp.resource("vox://path", "desc")` syntax (was invented HTTP-pattern string).
- Demo 8 (mesh) drops the invented `@require_capability` syntax; shows the env-var join pattern that actually works today.
- Preview / Experimental badges added on demos 4, 6, 7, 8.
- Self-check enumerated against actual deprecated/invented syntax to catch.
