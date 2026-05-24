# 02 — Concepts page prompt ("How Vox works in 60 seconds")
_Last reviewed: 2026-05-23. Output: single-file React JSX with Tailwind + shadcn/ui + lucide-react._

For `/concepts/`. The page that explains the language to people who do not yet know what a `@table` is. Run this prompt verbatim in Claude.ai.

---

## The full prompt (paste this verbatim into Claude.ai)

```xml
<role>
You are an explainer who writes for technically curious non-engineers and
for engineers who want the 60-second pitch before reading the syntax reference.
You output a single React component file.
</role>

<context>
Vox is an open-source, Apache 2.0 programming language compiled from one
source file into a database schema, a type-safe HTTP server, and a reactive
browser UI. Designed after the era of large language models.

This page is /concepts/ at voxlang.org. It links from the landing page's
pillar cards via anchor links (#one-source-of-truth, #designed-for-llms,
#react-interop, #durable, #local-training, #mesh). Those anchors MUST exist
on this page.

Primary audience: smart non-programmers — product managers, designers,
founders, researchers in adjacent fields, AI safety folks who don't write
code daily. They've heard of "compilers" and "schemas" but will not tolerate
jargon without translation.

Secondary audience: engineers who want a structured concept ladder before
the syntax reference.

Constraint: every code block must be paired with an immediately-following
plain-English explanation panel. The page is read in two passes: skim the
prose ladder; then drill into snippets if curious. Both passes must work.
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
React: useState (rarely needed)
shadcn/ui: Card, CardHeader, CardTitle, CardContent, Badge
lucide-react: ArrowRight, Check, ChevronDown, Code, Database, Layers,
              Network, Server, Sparkles, Terminal, Zap, Quote

Tailwind semantic classes: bg-background, bg-card, bg-secondary, bg-muted,
text-foreground, text-muted-foreground, text-primary, border-border
Custom hex (for code blocks): #1A1F2E (bg), #E2E8F0 (code text),
#94A3B8 (muted code text), #F2EFE7 (deeper Vellum for meaning panels)
</available>

<style-tokens>
[Paste from 04-visual-style-guide.md "Brand color tokens" :root block]
</style-tokens>

<sandbox-constraints>
- No fetch, no localStorage, no remote images. Use /api/placeholder/W/H.
- Single file only — no relative imports.
- Use Tailwind standard scale, no arbitrary values.
</sandbox-constraints>

<task>
Build a single React component named VoxConcepts. Eight sections, in order.

This is a reading-heavy page. Background is bg-background throughout (no
alternating dark/light sections). Code blocks use the Deepwell hex bg
(#1A1F2E). Meaning panels use #F2EFE7 (deeper Vellum). Prose max-width is
max-w-prose (~65ch) centered.

Use the CodeMeaningPanel component from <reference-components> for every
code-and-explanation pair.

═════════════════════════════════════════════════════════════════════════

Section 1 — Intro

  <h1> (exact text, 5 words): "What Vox actually is."

  Lead paragraph (font-display? No — keep body Inter, size text-lg, 38–55 words):

    "Vox is a programming language. Programming languages are the
    instructions a human — or a language model — writes to tell a computer
    what to do. Most programming languages were designed before language
    models existed. Vox was designed after — and that changes almost
    everything about how you read it, write it, and run it."

  Second paragraph (text-base, 50–75 words) — use this EXACTLY:

    "Most modern web apps need a database, a server, and a browser
    interface — three pieces of software, three programming languages,
    three sets of types describing the same things. Drift between them
    is the source of about half of all production bugs. Vox collapses
    those three into one declaration, in one file, in one language. The
    compiler does the rest."

  Third paragraph (text-base, 35–50 words) — use this EXACTLY:

    "The next five sections are the five ideas that distinguish Vox
    from the languages that came before it. Each one ends with a
    small code snippet and a translation. Read the prose; the code
    is there if you want it."

═════════════════════════════════════════════════════════════════════════

Section 2 — #one-source-of-truth

  <h2>: "One declaration. The whole stack."

  Three paragraphs (50–70, 40–60, 30–45 words). Write them in the
  <voice-sample> register. They must cover, in order:
    P1: define "source of truth" in plain language.
    P2: contrast with the normal stack where a `Task` type is restated
        as SQL column, API schema, server type, and client type — four
        declarations of the same shape; Vox: one.
    P3: practical consequence — "you can't have a column the API doesn't
        know about, because there's only one declaration."

  CodeMeaningPanel:
    filename: "tasks.vox"
    code:
      @table type Task {
          title: str
          done:  bool
          owner: str
      }
    meaningTitle: "What this declaration creates"
    meaningBody: rendered as JSX with one short paragraph and a 4-item
      <ul> listing what gets generated:
      - A database table called Task with three columns.
      - A typed API that knows about those three columns.
      - A typed client in the browser that calls that API.
      - SQL migrations describing how to update existing databases.

  Image (after the panel): /api/placeholder/1200/600 with alt:
    "A prism splitting one source of light into three exit rays:
    a database cylinder, a stack of server endpoints, and a
    browser interface frame."
  Caption (10–14 words).

═════════════════════════════════════════════════════════════════════════

Section 3 — #designed-for-llms

  <h2>: "A language a model can actually write."

  Four paragraphs (50–70, 40–60, 40–60, 35–50 words). Cover, in order:
    P1: why "designed for LLMs" is non-trivial. Language models predict
        next tokens; most programming languages have edge cases that look
        reasonable but compile to garbage. A language designed for models
        tries to make failing examples impossible to write, not just easy
        to flag.
    P2: PHONETIC OPERATORS. In most languages, "not equal" is `!=`. In
        Vox it's `isnt`. The model is more likely to predict the word
        than the symbol because words appear in training data far more
        often than punctuation patterns. Vox uses not, and, or, is, isnt.
    P3: ERRORS-IN-TYPES. Most languages have exceptions: code can throw
        an error and the caller might or might not catch it. Vox doesn't
        have exceptions. A function that can fail returns a Result[T] —
        either Ok(value) or Error(message). The compiler refuses to build
        code that doesn't handle both arms.
    P4: GRAMMAR-CONSTRAINED DECODING. When vox populi generates code,
        it constrains the model's output to only tokens that produce
        valid Vox. Invalid programs are literally unsamplable.

  CodeMeaningPanel:
    filename: "safe_math.vox"
    code:
      fn safe_divide(a: int, b: int) to Result[int] {
          if b is 0 {
              return Error("cannot divide by zero")
          }
          return Ok(a / b)
      }

      fn main() {
          let result = safe_divide(10, 2)
          match result {
              Ok(value) => print("got " + str(value))
              Error(msg) => print("failed: " + msg)
          }
      }
    meaningTitle: "Read this top-to-bottom"
    meaningBody: 5-item <ul>, one bullet per concept, in order:
      - safe_divide takes two whole numbers and returns either a number
        or an error.
      - "if b is 0" — the word `is` is Vox's equality. Same idea as `==`
        in other languages, but it reads aloud.
      - The function never throws. It returns Ok(...) or Error(...). The
        caller has to handle both.
      - The `match` block is the only way to unpack a Result. You can't
        accidentally use the value without acknowledging the error case.
      - Bare `!=` parses with a helpful error pointing at `isnt`. The
        compiler nudges toward the phonetic form.

═════════════════════════════════════════════════════════════════════════

Section 4 — #react-interop (NEW vs. previous kit)

  <h2>: "From one file to a working app."

  Three paragraphs (50–70, 40–60, 30–45 words). Cover:
    P1: most apps need a frontend framework, a server framework, and a
        gluing layer. Three separate things, three sets of conventions.
    P2: Vox's `component`, `@query`, `@mutation`, and `@server` produce
        React/TSX, typed endpoints, and a vox-client.ts RPC bridge from
        the same module graph.
    P3: external React or mobile apps can import the emitted components
        or call the endpoints via the bridge — Vox is not a walled garden.

  CodeMeaningPanel:
    filename: "tasks_ui.vox"
    code:
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
    meaningTitle: "What `vox build` produces"
    meaningBody: paragraph + 4-item <ul>:
      - A React/TSX component file.
      - A typed `vox-client.ts` with `list_tasks()`.
      - An Axum HTTP server with the typed endpoint.
      - Routing config that mounts TaskList at `/`.

  Badge near the H2: <Badge variant="secondary">Preview</Badge>
  Inline note (small text, text-muted-foreground): "React interop is
  Preview pre-1.0 — the codegen is shipping; ergonomics are still
  tightening."

═════════════════════════════════════════════════════════════════════════

Section 5 — #durable

  <h2>: "Workflows that survive crashes."

  Three paragraphs (50–70, 40–60, 30–45 words). Cover:
    P1: the problem — long-running processes (charging a card, training
        a model) might take minutes; if the server crashes halfway, you
        lose progress. Durable execution patterns solve this by
        checkpointing state at every step so it can resume on another
        machine.
    P2: in most stacks, durable execution is a LIBRARY — you wrap
        functions in a special API. Vox makes it a LANGUAGE concern.
        The `workflow` and `activity` keywords are real; the runtime
        journals each activity result; a crash mid-run resumes from the
        journal with completed activities replayed, not re-executed.
    P3: practical implication today — wrap your side-effect steps in
        `activity`, sequence them inside a `workflow`, and the runtime
        gives you journal-backed replay for free. Supported subset is
        linear activities + deterministic `if` + `workflow_wait` timer
        replay; the determinism lint enforces the rest.

  CodeMeaningPanel:
    filename: "checkout.vox"
    code:
      activity charge_card(amount: int) to Result[str] {
          if amount > 1000 {
              return Error("amount too large")
          }
          return Ok("tx_" + str(amount))
      }

      workflow checkout(amount: int) to Result[str] {
          let tx = charge_card(amount)?
          return Ok("Completed " + tx)
      }
    meaningTitle: "What ships today"
    meaningBody: paragraph + 3-item <ul>:
      - `workflow` and `activity` are real keywords; the runtime
        ships in `vox-workflow-runtime`.
      - Each activity result is persisted under
        `(run_id, workflow, activity_id)` — replay reads from the
        journal and skips completed activities.
      - Supported subset: linear activities + deterministic `if` +
        `workflow_wait`. Unrestricted control-flow replay is an
        explicit non-goal.

  Badge near the H2: <Badge variant="default">Stable</Badge>
  Inline note: "Stable for the supported subset (linear activities,
  deterministic branches, `workflow_wait` timer). Replay is
  journal-backed; crashes resume completed activities without
  re-execution. See [ADR-041]." Link to
  /architecture/041-durable-functions-completion-2026/.

  Image (after the panel): /api/placeholder/1200/600 with alt:
    "A continuous mobius-like ribbon with four faceted checkpoint
    markers evenly distributed around the loop."
  Caption (10–14 words).

═════════════════════════════════════════════════════════════════════════

Section 6 — #local-training

  <h2>: "Train a model on your laptop. Serve it from the same binary."

  Three paragraphs (55–75, 40–60, 30–45 words). Cover:
    P1: why a programming language has a training pipeline. Vox is new,
        so language models haven't seen much during their general
        training — out-of-the-box, models generate worse Vox than Python.
        The fix is to fine-tune on Vox examples. Most projects build
        this externally; Vox treats it as part of the toolchain.
    P2: QLoRA on consumer hardware. Full fine-tuning needs a data
        center. QLoRA fine-tunes a low-rank adapter — tiny in comparison,
        runs on a single consumer GPU. `vox populi train` runs this
        end-to-end via Candle (Rust ML) and Burn (Rust ML). No Python.
    P3: OpenAI-compatible serving. Once trained, the same binary serves
        the model over HTTP that speaks the OpenAI protocol. Anything
        that talks to OpenAI talks to your local model.

  CodeMeaningPanel:
    filename: "two-commands.sh"
    code: |
      vox populi train --base qwen-2.5-coder-7b --epochs 3 --output ./adapter
      vox populi serve --adapter ./adapter --port 8001
    meaningTitle: "Two commands"
    meaningBody: paragraph + 2-item <ul>:
      - First command fine-tunes Qwen 2.5 Coder on your project's Vox
        files. Takes hours on a consumer GPU.
      - Second serves at http://localhost:8001/v1/chat/completions — the
        shape any OpenAI client expects.

  Badge near the H2: <Badge variant="secondary">Preview</Badge>
  Inline note: "Hardware coverage is expanding; CUDA + Metal are the
  primary paths today."

═════════════════════════════════════════════════════════════════════════

Section 7 — #mesh

  <h2>: "One mesh of agents and machines."

  Three paragraphs (50–70, 40–60, 30–45 words). Cover:
    P1: agentic systems and ML training both need specific hardware —
        a GPU here, a TPU there, a workstation with 64GB of RAM
        elsewhere. Today this is solved by Kubernetes (heavy) or
        hand-rolled coordination (fragile). Vox's mesh is opt-in and
        lighter.
    P2: NODE CAPABILITIES. A node advertises what hardware it has when
        it joins the mesh. The orchestrator routes work accordingly. The
        wire format for advertisements and dispatch is part of the
        language — type errors at compile time, not runtime crashes.
    P3: practical implication — type-safe agent-to-agent messages.
        When agent A sends a message to agent B, the message shape is
        checked at compile time. Wire-format mismatches are not possible.

  CodeMeaningPanel:
    filename: "join-mesh.sh"
    code: |
      VOX_MESH_ENABLED=1 VOX_MESH_NODE_ID=workstation-a vox populi serve
    meaningTitle: "Joining the mesh"
    meaningBody: paragraph + 3-item <ul>:
      - Set two environment variables and start the orchestrator on
        each machine.
      - The node tells the mesh what hardware it has on startup.
      - The orchestrator routes work to matching nodes.

  Badge near the H2: <Badge variant="outline">Experimental</Badge>
  Inline note: "The mesh is opt-in and pre-1.0. The capability declaration
  syntax is still under design."

═════════════════════════════════════════════════════════════════════════

Section 8 — The five-minute take-away

  <h2>: "If you remember one thing."

  Single prose paragraph, text-lg, 65–90 words, no code. Use this EXACTLY:

    "Programming languages were designed for humans. Then we asked
    language models to use them. Most of the friction in human-AI
    co-authorship — the hallucinated APIs, the wrong types, the silent
    error swallowing, the schema drift — is a consequence of that
    ordering. Vox is what happens when you reverse it: design the
    language after the model, then ask humans to use it. Humans turn
    out to like it too. The compiler does the rest."

═════════════════════════════════════════════════════════════════════════

Section 9 — Where next

  Three side-by-side blocks (no card borders, just spacing). Each has:
    - small icon (lucide)
    - heading (display font)
    - one link

  Block 1: <Play /> "See it run" → "Try it in the browser" href="/#playground"
  Block 2: <Terminal /> "Build something" → "Follow the 30-minute tutorial" href="/tutorials/tut-getting-started/"
  Block 3: <Layers /> "Dig deeper" → "Read why-Vox-for-AI" href="/explanation/why-vox-for-ai/"
</task>

<reference-components>
(Paste the CodeMeaningPanel JSX from docs/design-system/06-component-specs.md
here. Inline it in the artifact.)
</reference-components>

<output-spec>
Output: one JSX artifact, default-exporting VoxConcepts.
- All anchor IDs from <context> are present and reachable from the
  landing page's pillar cards.
- Each major section is a <section id="anchor-id">.
- Each <h2> is preceded by a sufficient top margin (mt-16) to create
  visual section breaks without using horizontal rules.
- Code blocks use the filename labeling pattern from CodeMeaningPanel.
- The page works keyboard-only and honors prefers-reduced-motion.
</output-spec>

<self-check>
Verify each item, prefix response with PASS/FAIL per item. Fix FAILs first.

1. All six anchors present: #one-source-of-truth, #designed-for-llms,
   #react-interop, #durable, #local-training, #mesh.
2. Exactly one <h1> on the page (Section 1's "What Vox actually is.").
3. Every code block has an immediately-following CodeMeaningPanel.
4. No section uses `@endpoint(kind: ...)` syntax (deprecated). Only
   `@query`, `@mutation`, or `@server`.
5. The "Durable" section uses `workflow`/`activity`/`side_effect`
   keywords; nowhere uses `@durable` as a function decorator.
6. The "MCP" examples (if added) use `@mcp.resource("vox://path",
   "description")` — URI string, not HTTP pattern.
7. No `@require_capability` anywhere — that syntax does not exist.
8. Pillars 3 and 5 carry "Preview" Badge near the H2; Pillar 4 (Durable) carries "Stable".
9. Pillar 7 (mesh) carries "Experimental" Badge.
10. Lead paragraph stays within 38–55 words.
11. Take-away paragraph is the exact text in Section 8 (verbatim).
12. Every <img> has descriptive alt text (not "image" or "diagram").
13. Imports: only "react", "@/components/ui/*", "lucide-react".
14. No fetch, no localStorage, no remote URLs.
15. There is a "Skip to content" link at page top.
</self-check>

Return the PASS/FAIL list, then the JSX artifact.
```

---

## What changed vs. the previous version

- Output target switched from Astro/inline-CSS to single-file React JSX with Tailwind + shadcn.
- Pillar order updated: added explicit `#react-interop` section (the landing page links to it; previous concepts page omitted it).
- Vox snippets corrected: `@query` instead of `@endpoint(kind: query)`; `workflow`/`activity`/`side_effect` instead of `@durable`; `@mcp.resource("vox://...", "desc")` instead of HTTP-pattern strings; `@require_capability` removed entirely (it doesn't exist).
- Preview / Experimental badges added to the surfaces that are not yet stable, so the page doesn't oversell.
- Voice sample replaces voice adjectives.
- Self-check is closed-list against the available imports and the verified Vox syntax list — no open-ended "review the page."
