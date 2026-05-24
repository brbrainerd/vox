# 07 — Content blocks
_Last reviewed: 2026-05-23. Vox syntax verified against compiler tokens at this date._

Evergreen copy + verified Vox snippets. When a page prompt says "use this exactly," this is the file it points at. Edit here; regenerations pick up changes.

The defining property of every block here: **it is true in v0.5, v0.7, and v1.0**, AND uses only the [verified Vox syntax](README.md#verified-vox-syntax-use-only-these-in-code-samples) listed in the kit README.

---

## Tagline (≤80 characters)

> The AI-native programming language.

Variants (30-character budget):

> AI-native programming.
> Designed after the model.

---

## One-paragraph elevator pitch (35–55 words)

> Vox compiles a single file into a database schema, a type-safe HTTP server, and a reactive browser UI. It was designed *after* the era of large language models — with grammar-constrained decoding, errors-as-values, and durable execution as first-class primitives. Not libraries. Keywords.

---

## Two-paragraph deeper pitch (80–130 words)

> Most modern web apps need a database, a server, and a browser interface. Three pieces of software, three programming languages, three sets of types describing the same things. Drift between them is the source of about half of all production bugs.
>
> Vox collapses the three into one declaration in one file. The compiler does the rest. Errors are values, not exceptions, so the model can't write "happy-path-only" code that crashes in production. Long-running workflows survive node death by design — the runtime is hardening pre-1.0 but the grammar is locked. Local model fine-tuning ships in the toolchain because Vox is new, and the model needs to learn it from somewhere.

---

## The signature one-liner (verbatim, used multiple places)

> One declaration. The compiler does the rest.

Use at the end of major sections. Treat as a refrain.

---

## The five-pillar table (use exactly)

| # | Pillar | Status | One-sentence claim |
|---|--------|--------|--------------------|
| 1 | Single source of truth | **Stable** | One declaration becomes the schema, the API surface, and the typed client. |
| 2 | Designed for LLMs first | **Stable** | Phonetic operators, errors-in-types, grammar-constrained decoding. |
| 3 | React interop | **Preview** | `component` declarations compile to React/TSX; `@query`/`@mutation`/`@server` to typed endpoints. |
| 4 | Agents, MCP, and durable execution | **Stable** | `@mcp.tool`/`@mcp.resource` ship; `workflow`/`activity` get journal-backed replay (ADR-019/ADR-021/ADR-041); the orchestrator routes work. |
| 5 | Local training (MENS) | **Preview** | `vox populi` runs QLoRA fine-tunes on consumer GPUs via Candle and Burn — no Python in the path. |

**Honest framing rule:** When marketing copy describes Pillars 3 or 5 in present-tense ("Vox does X"), append a maturity note ("(Preview)") OR reframe to the grammar-level claim ("Vox includes the X keyword"). Pillar 4 ships in full: MCP, the orchestrator, and durable execution (linear activities + deterministic branches + `workflow_wait` timer replay) all land per [ADR-041](https://github.com/vox-foundation/vox/blob/main/docs/src/adr/041-durable-functions-completion-2026.md). Unrestricted control-flow replay remains an explicit non-goal.

---

## Audience claim (target reader)

> Vox is for engineers who write code alongside language models — and for the people building the systems those models live inside. If you are evaluating whether to spend a Saturday trying it, start with the [showcase](/showcase/). If you are curious without being an engineer, start with the [concepts page](/concepts/).

---

## Negative-space claims (what Vox is NOT)

Use sparingly. One per page max.

- Vox is not a replacement for Python, Rust, or TypeScript. It compiles to all three.
- Vox is not a no-code tool. The code is the source of truth; the schema and the UI are derived.
- Vox is not a framework. Frameworks live inside a host language; Vox is the host language.
- Vox is not a JavaScript fork. The TypeScript output is a target, not a basis.
- Vox is not VC-funded. It is community-backed via [Open Collective](https://opencollective.com/vox-foundation).
- Vox is not closed-source. The whole project is Apache 2.0.

---

## CTAs (button text — pick from this list)

Primary CTAs (page-leading):
- "Read the 5-minute tour" → `/concepts/`
- "Get started" → `/tutorials/tut-getting-started/`
- "See the showcase" → `/showcase/`

Secondary CTAs (in-context):
- "Learn more →" (pillar cards, section blocks)
- "Read the reference →" (technical-detail follow-ups)
- "See it in action →" (showcase deep-links)
- "Install locally →" (tutorials)

Never use:
- "Try Vox" (implies a sales product)
- "Sign up" (no account system)
- "Watch the demo" (no video; the playground IS the demo)
- "Book a call", "Get a quote" (this is Apache 2.0, not enterprise software)
- "Get started for free" (implies a paid tier)

---

## Trust strip (under hero CTAs)

Three items, separated by `·`. Default trio:

> Apache 2.0 · 101 Rust crates · Open Collective backed

Substitutions allowed (pick three; keep order stable across regenerations):

- License: "Apache 2.0"
- Repository scale: "101 Rust crates" (recount: `git ls-files crates/*/Cargo.toml | wc -l`)
- Community: "Open Collective backed"
- Compiler maturity: "Stable compiler core"
- Test surface: "Tested on Linux, macOS, Windows"
- Examples: "30+ runnable examples"

---

## Three personas (use exact wording)

For the landing-page "Who Vox is for" section.

### Persona 1 — The backend engineer
**Value claim:** You will write less integration code and your migrations will stop fighting your API types.
**Next step:** Read: how `@table` collapses three layers → `/concepts/#one-source-of-truth`

### Persona 2 — The agentic builder
**Value claim:** Long-running workflows survive node death by design; MCP tools are typed and discoverable from the same file.
**Next step:** Read: durable execution and MCP tools → `/how-to/how-to-ai-agents/`

### Persona 3 — The researcher
**Value claim:** Run QLoRA on your laptop, serve over an OpenAI-compatible HTTP endpoint, ship the model with the language. No Python.
**Next step:** Read: the MENS pipeline → `/reference/mens-training/`

---

## Footer block (license + community)

Synced from README anchor `community_license`:

> **License.** Apache 2.0 — commercial use permitted, patent rights granted, modifications allowed with attribution.
> **Community.** Backed by [Open Collective](https://opencollective.com/vox-foundation) — every dollar raised and spent is public.

Always prefer "Apache 2.0" over "open source" in marketing copy. Be specific.

---

## Code-snippet templates (verified against compiler tokens — paste-ready)

Each snippet uses only syntax in the [verified list](README.md#verified-vox-syntax-use-only-these-in-code-samples). Reference these by letter; don't invent new snippets for marketing.

### Snippet A — Single declaration becomes everything

```vox
@table type Task {
    title: str
    done:  bool
    owner: str
}
```

What this generates (use in `outputs` for `<CodeOutputSplit>`):
- `migrations/001_task.sql` — Schema with primary key, indexed on `(owner, done)`.
- `server/api.rs` — Axum router with typed JSON.
- `client/TaskList.tsx` — React component with typed props.
- `client/vox-client.ts` — RPC bridge with typed methods.

### Snippet B — Errors as values

```vox
@mutation
fn add_task(title: str, owner: str) to Result[Id[Task]] {
    if title is "" {
        return Error("title required")
    }
    return Ok(db.insert(Task, { title: title, done: false, owner: owner }))
}
```

**Notes:**
- `@mutation` (replaces deprecated `@endpoint(kind: mutation)`).
- `is` is the phonetic equality operator. `==` works but the lexer prefers `is`.
- `Result[Id[Task]]` is the return type; `Ok(...)` and `Error(...)` are the constructors.

### Snippet C — Phonetic operators in branching

```vox
fn classify(score: int, override: bool) to str {
    if override is true { return "admin" }
    if score >= 90 and score isnt 100 { return "expert" }
    if score < 50 or not is_calibrated() { return "novice" }
    return "regular"
}
```

**Notes:**
- `is`, `isnt`, `and`, `or`, `not` — all verified tokens.
- Bare `!=` parses but the compiler emits a hint pointing to `isnt`.

### Snippet D — Durable workflow

```vox
activity charge_card(amount: int) to Result[str] {
    if amount > 1000 { return Error("amount too large") }
    return Ok("tx_" + str(amount))
}

workflow checkout(amount: int) to Result[str] {
    let tx = charge_card(amount)?
    return Ok("Completed " + tx)
}
```

**Notes:**
- `match` arms use `=>` (fat arrow). The thin arrow `->` is **not** Vox syntax for match arms.
- `workflow` and `activity` are real keywords. The runtime journals every `activity` result; a crash mid-run resumes with completed activities replayed (ADR-019).
- Supported subset: linear activity execution + deterministic `if` + `workflow_wait` timer replay (ADR-021). Unrestricted control-flow replay is explicit non-goal.
- Completion ADR: [ADR-041](../../../docs/src/adr/041-durable-functions-completion-2026.md).

### Snippet E — MCP tool

```vox
@mcp.tool "Search the project docs by query"
fn search_docs(query: str, limit: int) to list[DocResult] {
    return docs.semantic_search(query, limit)
}

@mcp.resource("vox://docs/recent", "Recent doc updates this week")
fn recent_doc_updates() to list[DocUpdate] {
    return docs.recent(7)
}
```

**Notes:**
- `@mcp.tool` takes a description string.
- `@mcp.resource` takes `(uri_string, description)` — the URI is a string like `vox://...`, **not** an HTTP pattern like `GET /tasks/{owner}`. (Common mistake; verified in `crates/vox-compiler/src/lexer/token.rs`.)

### Snippet F — Reactive UI

```vox
component TaskList(tasks: list[Task]) {
    view: column() {
        tasks.map(fn(t) { row() { text() { t.title } } })
    }
}

routes { "/" to TaskList }
```

**Notes:**
- `component` and `view` are real keywords (verified).
- Label as Preview when used in marketing — codegen ergonomics still tightening.

### Snippet G — One-line MENS workflow

```bash
vox populi train --base qwen-2.5-coder-7b --epochs 3 --output ./adapter
vox populi serve --adapter ./adapter --port 8001
```

**Notes:**
- Real CLI invocation; flag names verified against `vox-cli`.
- Output is a QLoRA adapter (~50–200 MB), not a full checkpoint.
- Served via OpenAI-compatible HTTP at the chosen port.

**Snippet selection guide:** Match the snippet's first line to the pillar being shown.
- Pillar 1 (source of truth) → Snippet A
- Pillar 2 (LLM-first) → Snippet B or C
- Pillar 3 (React interop) → Snippet F (label Preview)
- Pillar 4 (agents, MCP, durable) → Snippet E for MCP, Snippet D for durable workflows. Both ship today (ADR-041).
- Pillar 5 (MENS) → Snippet G (label Preview)

Don't combine snippets — more concepts per block, slower to read.

---

## Voice sample (paste into prompts as `<voice-sample>`)

```
Most modern web apps need a database, a server, and a browser interface — three pieces of software,
three programming languages, three sets of types describing the same things. Drift between them is
the source of about half of all production bugs. Vox collapses those three into one declaration in
one file. The compiler does the rest.
```

When using in a prompt: **instruct Claude to match the cadence and register, not the topic.**

What this sample demonstrates (note these to Claude alongside the sample):
- Em-dash for the parenthetical, not parens, not commas.
- Concrete claim ("half of all production bugs"), not hedged.
- Closes with the project's signature sentence.
- Contractions allowed.
- One idea per sentence.

---

## Words/phrases that never appear in Vox marketing

| Forbidden | Why | Replacement |
|-----------|-----|-------------|
| Revolutionary | Empty superlative | Drop |
| Game-changing | Cliché | Drop |
| Lightning fast / Blazingly fast | Unmeasured cliché | "Compiles in N ms" or drop |
| Cutting edge | Aging | "Designed for the LLM era" |
| Next-generation | Aging | Drop |
| Robust | Vague | Specify what doesn't break |
| Seamless | Vague | Specify the integration |
| Empower / Empowered | Corporate-speak | "lets" or "you can" |
| Leverage | Corporate-speak | "use" |
| Synergy / synergistic | Corporate-speak | Drop the sentence |
| Solution | Generic | Name the thing |
| Best-in-class | Unmeasured | Drop |
| Designed with X in mind | Soft commitment | "X is part of the language" |
| Built from the ground up | Implicit comparison | Drop |
| End-to-end | Vague | Name the start and end |
| Try Vox today | Implies trial | "Get started" |
| Join the community | Vague | Name the channel ("GitHub Discussions") |

---

## Style notes for prose generation

- **Em-dashes for parentheticals** (not parens). "Most languages — Python, Rust, TypeScript — predate LLMs."
- **Oxford comma always.** "Schema, API, and client."
- **Contractions in body prose.** "It's", "don't", "we're."
- **No exclamation points.** Ever.
- **First word of sentences capitalized.** Sentence case for sub-headers; Title Case for major headings.
- **One idea per sentence.** Compound sentences belong in essays.

---

## Versioned content (this section drifts; re-verify before reuse)

These claims have a shelf life. Re-validate when regenerating any page that uses them.

| Claim | How to verify | Last verified |
|-------|---------------|---------------|
| Workspace crate count: 101 | `git ls-files crates/*/Cargo.toml \| wc -l` | 2026-05-23 |
| Golden example count: 30+ | `ls examples/golden/*.vox \| wc -l` | 2026-05-23 |
| Stability tier table | Lives in `README.md` under `<!-- ANCHOR: tier_table -->`. Synced to `docs/src/index.mdx` via `sync-readme-sections.mjs`. | 2026-05-23 |
| `@query`/`@mutation`/`@server` are canonical | `crates/vox-compiler/src/lexer/token.rs:145-155` | 2026-05-23 |
| `@endpoint(kind: ...)` is deprecated | Same source; comment marks deprecation | 2026-05-23 |
| Phonetic operators present | Same source, lines 101–117 | 2026-05-23 |
| `workflow` / `activity` durable runtime | Grammar in lexer; runtime in `vox-workflow-runtime` ships supported subset per ADR-041 | 2026-05-23 |

When any drifts, update this file and re-run any prompts citing it.
