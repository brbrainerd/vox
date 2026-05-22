---
title: "README: Vox Platform (Scientia Draft, April 2026)"
description: "Full-length README preserved for Vox Scientia publication. Contains extended rationale, academic citations, and pedagogical prose removed during the April 2026 brevity rewrite."
category: "explanation"
status: deprecated
archived_date: 2026-04-13
training_eligible: false
schema_type: "TechArticle"
---

> [!WARNING]
> **ARCHIVED DOCUMENT**: This file was archived on 2026-04-13. It is intentionally excluded from active AI context. It is preserved for potential Vox Scientia publication. Do not reference for contemporary development. See `README.md` at the repo root.

---

<div align="center">
  <img src="docs/src/assets/vox_hero_banner.jpeg" alt="Vox - The human voice acting as the great nerve of intelligence" width="100%" />

  <br><br>

  <p><strong>A unified language designed for human intent and machine execution—empowering developers and intelligent models to build complex systems and accelerate discovery together.</strong></p>
  <p><a href="https://voxlang.org"><strong>voxlang.org</strong></a></p>

</div>

<p align="center">
  <a href="https://voxlang.org"><img src="https://img.shields.io/badge/docs-voxlang.org-blue?style=flat-square" alt="Documentation"/></a>
  <a href="https://github.com/vox-foundation/vox/commits/main"><img src="https://img.shields.io/github/last-commit/vox-foundation/vox?style=flat-square&label=updated" alt="Last Updated"/></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-green?style=flat-square" alt="License"/></a>
  <a href="https://voxlang.org/feed.xml"><img src="https://img.shields.io/badge/RSS-updates-orange?style=flat-square" alt="RSS Feed"/></a>
</p>

---
<!-- Code examples in this file should mirror examples/golden/*.vox -->
<!-- Run: vox check examples/golden/*.vox to verify -->

<br>
<div align="center">
  <blockquote>
    <p><em>"Is it a fact — or have I dreamt it — that, by means of electricity, the world of matter has become a great nerve, vibrating thousands of miles in a breathless point of time? Rather, the round globe is a vast head, a brain, instinct with intelligence!"</em></p>
    <p>— Nathaniel Hawthorne, <em>The House of the Seven Gables</em> (1851)</p>
  </blockquote>
</div>

---

<!-- ANCHOR: why_vox -->
## Why Vox Exists

Today, developers direct language models to construct systems, but programming languages were designed before the advent of GPT. Unconstrained API surfaces and flexible paradigms—the highly dynamic typing of JavaScript yielding silent runtime failures, the hidden state mutations of C++ pointer arithmetic, or the unverified deep configuration boilerplate prominent in Python—give AI agents too much room to hallucinate, resulting in unintended consequences and unreliable systems.

Furthermore, internet-native code is notoriously slow to move and fragile to change. Decades of bridging the "object-relational impedance mismatch" (Copeland & Maier, 1984)—the fundamental friction between software logic and relational databases<sup>[7](#ref7)</sup>—has buried essential architectures beneath layers of ORMs, state management, and network glue code. This bloat rapidly compounds technical debt (Cunningham, 1992)<sup>[8](#ref8)</sup>. As codebases expand to manage stateless HTTP connections and fragmented persistence layers, they become extremely difficult for developers—and now AI agents—to safely traverse and refactor.

For Large Language Models, this fragmentation is catastrophic. Agents fail not simply because they hallucinate, but because their reasoning capacity is diluted by excessive contextual noise. While an LLM might technically boast a "one-million token context window," research shows models suffer from severe "context rot" (Liu et al., 2023)<sup>[9](#ref9)</sup> when trying to track complex state transitions spread across multiple REST endpoints and database files.

Vox was purposefully designed to address these constraints. By collapsing the database schema, server execution, and web interactivity into a single, unified intermediate representation, Vox radically reduces the cognitive load and token count required to synthesize full-stack engineering.

Vox is built as a language target for LLMs. By constraining engineering boundaries, it surfaces logical gaps and establishes a self-healing bounds loop that translates human intent into deterministic, executable code.

Vox is not designed to write hardware drivers, but it is fundamentally internet-native. Distributed networks are inherently more durable and often more powerful than isolated processes.

Our systems must be able to hear and be heard by the world before their internal logic can be truly useful. Vox exists to bridge the gap between legacy communication structures and the demands of probabilistic math. Instead of forcing developers and AI agents to manually wire together brittle HTTP endpoints, Vox abstracts online communication into strict, verifiable contracts. The compiler automatically translates high-level intent into stable APIs and interactive web interfaces capable of pausing and resuming execution across stateless connections. This empowers humans to jointly orchestrate distributed systems and power autonomous research with much less friction from legacy infrastructure and boilerplate translation.

*(Note: Mobile support is integrated for generated browser-apps and native on-device inference, but deploying the full Vox orchestration runtime directly on mobile devices is not currently supported.)*
<!-- ANCHOR_END: why_vox -->



### Platform Architecture & Stability
We stratify the platform based on a single metric: **model predictability**. For an AI to reliably write code, the underlying rules must be rigid. We lock down the core capabilities first—data, logic, and memory—because they anchor the LLM's understanding. Higher-level surfaces like visual rendering remain fluid as we discover the best ways for AI to construct them.

To make the system comprehensible for both human operators and AI agents, Vox divides its architecture into discrete shapes. This separation ensures that an AI generating a database schema does not accidentally modify how a button renders. Stability is enforced systemically through continuous integration and compiler test boundaries.

#### The Stability Tiers
* 🟢 **Tier 1 (Stable):** Production-ready. The rules are locked and mathematically verifiable, ensuring LLMs can generate predictable logic.
* 🟡 **Tier 2 (Preview):** Functionally complete, but the underlying execution lifecycle or AI-generation pipelines are still being optimized.
* 🚧 **Tier 3 (Experimental):** Under active architectural planning or gated behind CLI feature flags.

#### Domain Matrix
*The following matrix maps these stability tiers across the core functional boundaries of the Vox platform, detailing how each domain is managed and verified.*

| Domain & Purpose | What It Manages | Tier Status & Impact | Verification Pipeline |
|:-----------------|:----------------|:---------------------|:----------------------|
| **Core Syntax & Engine**<br>The foundation of the language. | The AST, type safety, compiler directives, and Language Server (LSP). | 🟢 **Stable**<br>Syntax rules are locked; generation is highly predictable. | Golden parsing suite, typed AST validations. |
| **Data & Connectivity**<br>How information is saved and shared. | `@table` auto-migrations, `@query`/`@server` endpoints, HTTP payloads. | 🟢 **Stable**<br>API contracts are  functionally complete. | In-memory DB roundtrips, strict schema testing. |
| **Agent Tooling System**<br>Giving AI access to external actions. | Orchestration logic, `@mcp.tool` exposure, and operational telemetry. | 🟢 **Stable**<br>Complete Model Context Protocol compliance is established. | MCP protocol assertions, telemetry gate checks. |
| **RAG & Knowledge Curation**<br>Memory retrieval for autonomous research. | `vox scientia` publication pipeline, Hallucination Guards (Socrates). If an AI can research the web, it can use metrics to verify if it is hallucinating. | 🟡 **Preview**<br>Retrieval heuristics and Socrates guard policies are actively evolving. | Citation alignment checks, novelty discovery scans. |
| **Durable Execution Lifecycles**<br>Multi-step tasks and logical continuity. | State survival across restarts via `workflow` and `actor` models. | 🟡 **Preview**<br>State preservation lifecycles may undergo optimization. | Durability integrity sweeps, zero-placeholder enforcement. |
| **Hardware & Tuning (MENS)**<br>Running AI and fine-tuning locally. | `vox populi` GPU mesh, local adapter training, and audio inference. | 🟡 **Preview**<br>Hardware-dependent support mappings are expanding. | Local hardware discovery tests, ML pipeline sweeps. |
| **Web UI & Rendering**<br>What the user actually sees. | `@island` browser wiring, React generation, UI routing. | 🟡 **Preview**<br>Client-side projections and web component translation may shift. | WebIR constraints, deterministic generation audits. |
| **Distributed Node Mesh**<br>Connecting multiple machines. | Cross-machine inference routing and agent task distribution. | 🚧 **Experimental**<br>Still under active design; not ready for deployment. | Pending standardizations. |

> *Current footprint as of **v0.4 — April 2026**.*

---

## How Vox Solves the Training Paradox

Legacy languages appear to hold a permanent AI advantage because models absorb massive quantities of their text scraped from the internet.

Vox bypasses this requirement. The repository includes local training primitives (`vox populi` and the MENS neural pipeline) that let developers natively fine-tune any foundation model to master Vox's structural boundaries. Because the platform ships with an inference mesh that scales across diverse hardware architectures, you aren't locked out of AI-assisted engineering just because a model hasn't seen enough of your syntax.

---

<!-- ANCHOR: how_vox -->
## How Vox Works

Code generation fails when an AI navigates fragmented files, hidden states, and chaotic lifecycles. Vox functions as a high-level abstraction that rigorously lowers into safe, deterministic infrastructure.

* **High-Level Intermediate Representation (HIR):** When an AI writes a `.vox` file, the parser lowers it into a strictly unified HIR. Database bindings and HTTP handshakes are resolved by the compiler before generation.
* **Deterministic Rendering (WebIR):** UI compiles directly to a Web Intermediate Representation. Agents don't juggle React hooks or state waterfalls—they emit pure data representations, and WebIR translates it to HTML.
* **Semantic Error Feedback:** Operations return strict `Result[T]` constraints. If an agent fails to handle an error state, the compiler catches it immediately and feeds syntax-level feedback to self-correct.
* **Native Protocol Projection:** AI capabilities aren't a bolted-on SDK. The AST inherently recognizes decorators like `@mcp.tool`. The compiler automatically projects these into Model Context Protocol manifests, meaning external agents can execute your logic without hand-written REST scaffolding.
<!-- ANCHOR_END: how_vox -->

---

## The Language

Here's a complete Vox program — a task tracker with a database table, a server endpoint, and a page:

```vox
// vox:skip
@table type Task {      // defines database schema
    title: str
    done:  bool
}

@server fn complete_task(id: Id[Task]) to Result[Unit] {
    db.Task.delete(id)
    ret Ok(Unit)        // signals success; the caller must handle failure too
}

@island TaskList {      // a live, interactive component in the browser
    tasks: list[Task]
}

component TaskPage() { // the static page that hosts it
    view: <div><TaskList tasks=[...] /></div>
}

routes { "/" to TaskPage }
```

One file. The compiler generates the SQL schema, the server endpoint, and the browser-side code that connects them. No separate ORM configuration, no hand-written API route, no TypeScript interface to keep in sync.

### Step 1 — Declare your data

In most projects, a data type lives in three places at once: a database schema, a server model, and a client type. They drift apart silently. Vox collapses all three into one declaration:

```vox
// vox:skip
@require(len(self.title) > 0)    // the compiler rejects empty titles on insert
@table type Task {
    title:    str
    done:     bool
    priority: int
    owner:    str
}

@index Task.by_owner on (owner)  // the database index, declared next to the type
```

`@table` generates the SQL table and handles schema migrations automatically. `@require` is baked into every write path — not just a runtime check, it can't be bypassed. `@index` creates a database index for fast lookups by owner.

### Step 2 — Write server functions

```vox
// vox:skip
@query
fn recent_tasks() to list[Task] {
    // read-only; becomes a GET /api/query/recent_tasks endpoint automatically
    ret db.Task.where({ done: false }).order_by("priority", "desc").limit(10)
}

@server fn get_task(id: Id[Task]) to Result[Task] {
    let row = db.Task.find(id)
    match row {
        Some(t) -> Ok(t)           // task found: return it
        None    -> Error("not found")  // task missing: return an error
    }
}

@mutation
fn add_task(title: str, owner: str) to Id[Task] {
    // writes are wrapped in a transaction automatically
    ret db.insert(Task, { title: title, done: false, priority: 0, owner: owner })
}
```

`@query` exposes a read-only endpoint — Vox enforces that it never changes data. `@mutation` wraps the write in a database transaction; if something goes wrong, the whole operation rolls back. The return type `Result[Task]` forces every caller to handle both the found and not-found cases. The compiler won't build code that ignores the error.

### Step 3 — Build the UI

Modern web apps split into two concerns: the **server**, which renders initial HTML and handles data, and the **browser**, which handles interactivity. Vox solves this with two distinct primitives:

```vox
// vox:skip
// An island is a piece of the page that's interactive in the browser.
// React lives inside the generated artifact — not in your .vox source.
@island TaskList {
    tasks: list[Task]              // same Task type from Step 1 — no duplication
    on_complete: fn(str) -> Unit   // a callback the browser can call
}

// A component is server-rendered — fast initial load, no JavaScript needed.
component TaskPage() {
    view: <div className="task-list">
        <TaskList tasks=[...] on_complete={complete_task} />
    </div>
}

routes { "/" to TaskPage }
```

`@island` marks the boundary where the browser takes over. The compiler generates the React component, the browser lifecycle wiring, and the typed client stub — none of that appears in your `.vox) source. `component` stays on the server: rendered to HTML, fast to load, written entirely in Vox syntax. React's mental model — hooks, lifecycle, client state — is confined to the generated layer.

> **v0.dev integration:** `vox island generate TaskDashboard "A minimal sidebar dashboard"` calls the v0.dev API (requires `V0_API_KEY`) and writes the generated component into `islands/src/TaskDashboard/`. The `@v0` build hook triggers this automatically during `vox build`.

### Step 4 — Durable logic and AI tools

```vox
// vox:skip
// An activity is a step that can be retried independently if it fails
activity charge_card(amount: int) to Result[str] {
    if amount > 1000 { ret Error("Amount too large") }
    ret Ok("tx_123")
}

// A workflow orchestrates activities and survives crashes — its state is durable
workflow checkout(amount: int) to str {
    let result = charge_card(amount)
    match result {
        Ok(tx)     -> "Success: " + tx
        Error(msg) -> "Failed: " + msg
    }
}

// One decorator makes this function callable by Claude, Cursor, or any AI agent
@mcp.tool "Search the knowledge base"
fn search_knowledge(query: str) to str {
    "Result for: " + query
}

// Tests live in the same file, run with `vox test`
@test
fn test_search() to Unit {
    assert(search_knowledge("hello") is str)
}
```

`workflow` tracks its own progress — if the server restarts halfway through `checkout`, it picks up where it left off. An `actor` is a named entity that receives typed messages and holds its own state across many calls. `@mcp.tool` connects your function to the [Model Context Protocol](https://modelcontextprotocol.io) in one line, making `search_knowledge` directly invocable from Claude, Cursor, or any compatible agent.

More examples: [`examples/golden/`](examples/golden/).

For a side-by-side comparison with C++, Rust, and Python solving the same problem, see [`docs/src/explanation/expl-rosetta-inventory.md`](docs/src/explanation/expl-rosetta-inventory.md).

---

## Quick Start

**macOS / Linux:**
```bash
curl -fsSL https://raw.githubusercontent.com/vox-foundation/vox/main/scripts/install.sh | bash
```

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/vox-foundation/vox/main/scripts/install.ps1 | iex
```

```bash
# Create your first project
vox init my-app
cd my-app
vox build src/main.vox -o dist
vox run src/main.vox
```

```text
vox init [name]          Scaffold a new project (templates: chatbot, dashboard, api)
vox build <file>         Compile → TypeScript + Rust output
vox check <file>         Fast type validation
vox run <file>           Development server (Axum + TanStack dev proxy)
vox dev <file>           Hot-reload dev mode
vox test <file>          Run @test functions
vox fmt <file>           Format source
vox bundle <file>        Full production build: codegen → pnpm build → single binary
vox doctor               Verify toolchain, environment, and secret health
```

Full command reference: [`docs/src/reference/cli.md`](docs/src/reference/cli.md).

## The CLI

Run `vox commands --recommended` for a curated first-time map of subcommands. For repository hygiene, `vox ci gui-smoke` runs deterministic Web Intermediate Representation (WebIR) routing tests and can opt into Vite (`VOX_WEB_VITE_SMOKE=1`) or Playwright (`VOX_GUI_PLAYWRIGHT=1`) lanes documented in the same CLI reference.

---

## Agent Orchestration & AI Capabilities

### Multi-agent coordination

The **orchestrator** (`vox-orchestrator`) assigns tasks to agents by file affinity and role. **`vox-dei`** handles human-in-the-loop review — pausing, reassigning, or confirming work before it proceeds. The control surface is available as MCP tools, usable from the VS Code sidebar or any MCP-compatible agent:

<!-- tool names sourced from crates/vox-orchestrator/src/mcp_tools/tools/dispatch.rs -->
```text
vox_pause_agent      Suspend a running agent and queue its tasks
vox_resume_agent     Resume a paused agent
vox_retire_agent     Retire an agent and release all locks
vox_reorder_task     Change dispatch priority of a queued task
vox_queue_status     Show orchestrator queue and agent states
```

### Agent-to-agent messaging

In most systems, passing results between agents means building your own protocol — a shared table, a queue, a webhook. In Vox, agent-to-agent messaging is built into the runtime. Agents exchange typed, encrypted messages; because both sides use the same declared Vox type, the compiler catches mismatches before anything runs.

The in-process message bus is active in every session. Cross-machine relay is available with the `populi-transport` feature.

### The Populi mesh

`vox populi` is a node registry for machines running Vox. Each node detects and advertises its hardware — CPU, CUDA, Metal, VRAM — on startup. The orchestrator routes training and inference jobs to the machines that can handle them.

```bash
VOX_MESH_ENABLED=1 VOX_MESH_NODE_ID=my-node vox populi serve
```

### Model selection & provider routing

| Provider | Support | Notes |
|---|---|---|
| Ollama (local) | First-class | No cost, no disclosure |
| Google Gemini | First-class | Privacy acknowledgment required |
| Groq | First-class | Authoritative rate-limit headers |
| OpenRouter | First-class | Local estimate |
| OpenAI / Anthropic | Gated | Pro / Enterprise |
| Together AI | Gated | ML-focused |

```bash
vox populi status --quotas   # view per-provider usage and remaining budget
```

### Local GPU & Native Training (MENS)

The MENS neural pipeline lets developers fine-tune foundation models to generate Vox code natively. `vox-tensor` and `vox-populi` run in Rust using [Burn](https://github.com/tracel-ai/burn) and [Candle](https://github.com/huggingface/candle) — no Python, no `pip install`, no virtual environments.

`vox populi probe` detects your local hardware topology (CUDA, Metal, WebGPU) and orchestrates multiple parallel AI pipelines:
1. **QLoRA Fine-Tuning:** Train specialized adapter weights from your team's internal `src/` repositories.
2. **Speech-to-Code (ASR):** Run real-time structured inference using local Whisper/Qwen models to map vocal commands to AST modifications.
3. **Local Mesh Serving:** Deploy models via an OpenAI-compatible `/v1/completions` endpoint for offline agentic orchestration.

```bash
# Automatically profile hardware and begin a QLoRA fine-tune
vox populi train --config qlora.toml

# Expose the fine-tuned adapter over the local mesh network
vox populi serve --model mens/runs/latest/model_final.bin --port 8080
```

---

## Documentation

Vox documentation is structured around the **Diátaxis** framework, explicitly separating tutorials, how-to guides, explanations, and pure reference material.

| Section | Description | Key Links |
|---------|-------------|-----------| 
| **Getting Started** | High-level overviews and introductory setup. | [What is Vox?](docs/src/index.md) <br> [Getting Started](docs/src/tutorials/tut-getting-started.md) |
| **Journeys & Tutorials** | Step-by-step guides for full-stack patterns. | [First Full-Stack App](docs/src/how-to/first-full-stack-app.md) <br> [AI Agents & MCP](docs/src/how-to/how-to-ai-agents.md) |
| **How-To Guides** | Goal-oriented recipes for specific problems. | [Model Domain Logic](docs/src/how-to/how-to-custom-types.md) <br> [Native Training](docs/src/journeys/native-training.md) |
| **Explanations** | Theoretical deep-dives and architectural 'Why's. | [Compiler Architecture](docs/src/explanation/expl-architecture.md) <br> [AI Orchestration](docs/src/explanation/expl-ai-orchestration.md) |
| **Reference** | Authoritative lists, CLI maps, and type systems. | [CLI Surface](docs/src/reference/cli.md) <br> [Decorator Registry](docs/src/reference/ref-decorators.md) |
| **Architecture** | Single-Source-of-Truth (SSOT) planning and ADRs. | [Master Arch Index](docs/src/architecture/architecture-index.md) <br> [Contributor Hub](docs/src/contributors/contributor-hub.md) |
| **Operations & Quality** | Deployment runbooks, CI constraints, and Docker topology. | [Docker Deployment](docs/src/reference/deployment-compose.md) <br> [CI Runner Contract](docs/src/ci/runner-contract.md) |

> **Looking to contribute?** We actively track undocumented surfaces. Check our [Known Documentation Gaps & Backlog](docs/src/api/DOC_GAPS.md) to see where the community needs help.

---

## Architectural Guardrails

Vox applies the same philosophy to itself that it applies to user code: machine-verifiable constraints over style-guide suggestions. The rules below aren't enforced through code review — they fail CI. Each one exists because we've seen what happens without it.

### No skeleton code (`vox-toestub`)

`todo!()`, `unimplemented!()`, empty function bodies, and hollow arrow functions in production paths are a build blocker. The `vox-toestub` crate runs a suite of detectors — `StubDetector`, `EmptyBodyDetector`, `HollowFnDetector`, `ReachabilityDetector`, and others — as part of every CI matrix pass under `vox ci toestub-scoped`.

**Why it matters for AI codebases:** AI agents produce plausible-looking scaffolding. An agent that returns a `todo!()` didn't finish the job — it silently deferred it. TOESTUB makes that deferral a build failure rather than a runtime surprise. The `VictoryClaimDetector` goes further, flagging comments like "implementation complete" adjacent to `unimplemented!()` calls.

```bash
vox stub-check --path crates/my-crate   # run locally before pushing
vox ci toestub-scoped                   # full workspace scan in CI
```

### Complexity bounds (`GodObjectDetector`, `SprawlDetector`)

No struct or impl block may exceed 500 lines or 12 methods. No directory may contain more than 20 files. Both limits are enforced by dedicated detectors in `vox-toestub`.

**Why it matters:** An LLM's ability to reason about a module degrades sharply when the module exceeds its coherent processing window. The 500-line limit isn't aesthetic — it's calibrated so the entire struct fits comfortably within a 32K-token context window alongside the surrounding codebase. The 20-file directory limit forces domain decomposition before a module becomes a grab-bag. The `vox-orchestrator` crate documents this explicitly in its own module comment: *"decomposed from the original god-object."*

### All credentials routed through Clavis (`secret-env-guard`, `operator-env-guard`)

Direct `std::env::var` calls for secrets are a CI failure. All credentials are declared as `SecretId` variants in `crates/vox-clavis/src/lib.rs` and resolved via `vox_clavis::resolve_secret(...)`. The `vox ci secret-env-guard` command scans changed files for raw environment reads and fails the build if any are found outside a strict allowlist.

**Why it matters:** Hidden environment variables cause deployment drift and make it impossible to audit what capabilities an application possesses. When an agent introduces a new API key, it must go through Clavis — which means it appears in `vox clavis doctor`, gets picked up by `vox ci clavis-parity`, and is visible to every operator. There's no path for a credential to sneak in through a casual `env::var("SOME_API_KEY")`. The `SecretDetector` in `vox-toestub` catches hardcoded credentials as a separate failure class.

### Documentation is compiler-verified (`vox-doc-pipeline`, `SchemaComplianceDetector`)

```vox
// vox:skip
All `.vox` code blocks in `docs/src/` must either use `{{#include}}` to pull from a verified file in `examples/golden/`, or be marked `// vox:skip`. Loose code snippets that can't be compiled are a CI failure via `SchemaComplianceDetector`.
```

**Why it matters:** Documentation that silently diverges from working code is worse than no documentation — it actively misleads both human readers and AI agents that use docs as retrieval context. The golden file pipeline (`examples/golden/`) means every snippet in this README and the docs site has been compiled against the current compiler before it shipped.

### Context isolation is centrally managed (`.voxignore` → `vox ci sync-ignore-files`)

`.voxignore` is the single source of truth for what files are excluded from AI context. Derived files (`.cursorignore`, `.aiignore`, `.aiexclude`) are regenerated automatically. Editing them directly causes a CI drift failure.

**Why it matters:** Generated artifacts, telemetry logs, and build outputs are noise that degrades model attention. Without a centrally managed exclusion surface, each tool gets its own ad-hoc ignore file that drifts out of sync, and agents start reading their own previous outputs as source of truth. Centralizing this in `.voxignore` means the boundary is enforced once, not maintained four times.

### No DRY violations, deprecated symbols, or unwired modules

`vox-toestub` ships additional detectors that catch structural debt before it accumulates: `DryViolationDetector` flags copy-pasted logic blocks; `DeprecatedUsageDetector` blocks use of retired crate names and environment variables (see the retired-symbols table in `AGENTS.md`); `UnwiredModuleDetector` catches modules declared but never imported. These run in CI alongside the structural checks above.

```bash
vox ci toestub-scoped --report    # full findings report with severity breakdown
```

---

## Acknowledgements & Lineage

Many of the design paradigms that underpin Vox are not entirely unique to this project. Beyond specific frameworks, Vox is heavily influenced by the philosophies that constitute timeless, robust software engineering. We stand on the shoulders of giants.

### Systems & Protocols
- **Durable Execution (`workflow`)**: The concept of writing long-running, fault-tolerant code that magically survives server restarts was pioneered by systems like Azure Durable Functions, and later Cadence & Temporal (created by Maxim Fateev and Samar Abbas)<sup>[1](#ref1)</sup>.
- **Islands Architecture (`@island`)**: The approach of sending static HTML and selectively hydrating dynamic "islands" of interactivity was coined by Katie Sylor-Miller at Etsy (2019) and popularized by Jason Miller (creator of Preact) in 2020<sup>[2](#ref2)</sup>. Modern frameworks like Astro further normalized this server-first approach.
- **Model Context Protocol (`@mcp.tool`)**: The standard providing AI models safe, authenticated access to tools and file systems was developed by Anthropic<sup>[3](#ref3)</sup>.
- **Unifying Distributed Logic**: The philosophy of treating a distributed system as a single cohesive program rather than disjointed microservices owes much of its modern exploration to projects like the Unison language<sup>[4](#ref4)</sup>.

### Foundational Philosophies
- **Accidental vs. Essential Complexity**: As outlined by Fred Brooks in *The Mythical Man-Month*, much of software engineering is bogged down by "accidental complexity"—the tooling, ORMs, and glue code required just to make systems talk to each other. Vox eliminates accidental complexity by natively generating the API and database boundaries, enabling humans and AI to focus squarely on the "essential complexity" of the application logic<sup>[5](#ref5)</sup>.
- **"Constraints Liberate"**: Echoing the philosophy of Tony Hoare and the design of strongly typed languages like ML, Haskell, and Rust, Vox relies on rigid schemas and compiler assertions to reject invalid states. By forcing an AI model into a mathematically verifiable corridor, we use constraints as a self-healing bounds loop, proving that strict rules unlock, rather than hinder, generative capability.
- **Data-Driven Architecture**: *"Show me your flowcharts and conceal your tables, and I shall continue to be mystified. Show me your tables... and they'll be obvious."* — Fred Brooks. Vox organizes its architecture explicitly around data definitions (`@table`), radiating logic out from the schema rather than trying to reconcile an ORM with an arbitrary state hierarchy.
- **Fail-Fast & The Actor Model**: Joe Armstrong's "Let it crash" philosophy from Erlang/OTP informs Vox's durable execution and agent orchestration. Instead of attempting to anticipate and catch every possible local exception natively within an AI model, the system isolates execution into independent `activities` that can fail, report their status, and securely restart via a centralized orchestrator<sup>[6](#ref6)</sup>.

---

<!-- ANCHOR: community_license -->
## Community, Backing & License

### Backing Vox (Open Collective)

The Vox Foundation operates as a transparent, community-backed entity through **Open Collective**. Every dollar raised and spent is public. Sponsorship funds developer grants, CI hardware for MENS neural training, and academic bounties.

[Open Collective →](https://opencollective.com/vox-foundation)

### License

Vox is licensed under **Apache 2.0**. You can use it to build commercial or closed-source applications without opening your own code. Contributors grant explicit patent rights. You can modify the compiler, runtime, or standard library as long as you retain the original copyright notices.

[`LICENSE`](LICENSE) · [github.com/vox-foundation/vox](https://github.com/vox-foundation/vox)

### Get Involved

Vox Scientia is a publication pipeline for aggregating and surfacing community research — pulling from wherever developers are talking, not constraining where they talk. Roadmap decisions and architectural questions are tracked in GitHub Discussions because that's the format our tooling can index, parse, and feed back into the system. Come wherever you are.

- **[GitHub Discussions](https://github.com/vox-foundation/vox/issues)**: Architecture questions, language design feedback, and roadmap input.
- **RSS Feed**: [`voxlang.org/feed.xml`](https://voxlang.org/feed.xml) — changelogs and architectural decision records.
<!-- ANCHOR_END: community_license -->

---

## References

<a id="ref1"></a>**[1]** Fateev, M., & Abbas, S. (2019). *Temporal*. Temporal Technologies. <https://temporal.io>
<a id="ref2"></a>**[2]** Miller, J. (2020). *Islands Architecture*. JasonFormat. <https://jasonformat.com/islands-architecture/>
<a id="ref3"></a>**[3]** Anthropic. (2024). *Model Context Protocol*. <https://modelcontextprotocol.io>
<a id="ref4"></a>**[4]** Unison Computing. *Unison Language: A new approach to distributed programming*. <https://unison-lang.org>
<a id="ref5"></a>**[5]** Brooks, F. P. (1987). "No Silver Bullet—Essence and Accidents of Software Engineering." *IEEE Computer*, 20(4), 10-19. DOI: <https://doi.org/10.1109/MC.1987.1663532>
<a id="ref6"></a>**[6]** Armstrong, J. (2003). *Making reliable distributed systems in the presence of software errors* [Ph.D. thesis, Royal Institute of Technology, Stockholm]. <https://erlang.org/download/armstrong_thesis_2003.pdf>
<a id="ref7"></a>**[7]** Copeland, G., & Maier, D. (1984). "Making Smalltalk a Database System." *SIGMOD '84*, 316–325. DOI: <https://doi.org/10.1145/602259.602287>
<a id="ref8"></a>**[8]** Cunningham, W. (1992). "The WyCash Portfolio Management System." *Addendum to the proceedings of OOPSLA '92*, 29-30. DOI: <https://doi.org/10.1145/157709.157715>
<a id="ref9"></a>**[9]** Liu, N. F., Lin, K., Hewitt, J., Paranjape, A., Bevilacqua, M., Petroni, F., & Liang, P. (2023). "Lost in the Middle: How Language Models Use Long Contexts." *Transactions of the Association for Computational Linguistics*. arXiv: <https://arxiv.org/abs/2307.03172>
