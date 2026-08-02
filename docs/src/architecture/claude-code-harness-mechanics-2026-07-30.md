---
title: "Claude Code Harness Mechanics — Verified Research 2026-07-30"
description: "Adversarially verified research into how the Claude Code harness works internally — agentic loop, context compaction, skill discovery via progressive disclosure, tool-result budgeting, the six-step permission pipeline, ~30 lifecycle hooks, and subagent context isolation — with an explicit ledger of refuted claims and open questions."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---

# Claude Code Harness Mechanics — Verified Research (2026-07-30)

> **Provenance.** Two-part evidence base.
>
> 1. A `deep-research` workflow run: 5 search angles → 26 sources fetched → 125 claims
>    extracted → top 25 put through **3-vote adversarial verification** (each verifier
>    prompted to *refute*; 2-of-3 refutes kills the claim). Result: **17 confirmed,
>    8 refuted, 0 unverified**, synthesized to 11 findings. 108 agent calls,
>    7.3M subagent tokens, 0 errors. Run id `wf_47ddb85a-ed3`.
> 2. Direct primary-source fetches by the authoring session against
>    `platform.claude.com` and `code.claude.com` (2026-07-30) for the Skills and
>    hooks surfaces, which the workflow covered thinly.
>
> **Read the [Refuted Ledger](#12-refuted-ledger--do-not-restate-these) before citing
> anything from this document elsewhere.** Eight plausible, widely-repeated claims about
> Claude Code did **not** survive verification. Several of them appear in our own prior
> architecture docs.

Companion documents:

- [`vox-harness-graph-audit-2026-07-30.md`](vox-harness-graph-audit-2026-07-30.md) — what
  Vox actually does today, measured against this baseline with Graphify evidence.
- [`vox-harness-parity-plan-2026-07-30.md`](vox-harness-parity-plan-2026-07-30.md) — the
  sequenced remediation.
- [`orchestrator-gui-dispatch-audit-2026-07-02.md`](orchestrator-gui-dispatch-audit-2026-07-02.md)
  — the prior audit this supersedes in part.

---

## 0. The one-paragraph answer

Claude Code is **a plain LLM-with-tools loop wrapped in a thick layer of predefined code
paths whose single organizing concern is the finite context window.** There is no clever
orchestration graph, no vector index, no planner/executor split. What produces the
"it just works" feeling is that every way the loop can fail — context exhaustion, a tool
returning a megabyte, an unsafe write, a skill that should have fired and didn't — has a
specific, boring, individually-verifiable mechanism attached to it. The intelligence is in
the model; the engineering is in the guardrails. **This is the single most important
architectural lesson for Vox**, because Vox's harness has inverted it: elaborate
orchestration machinery around a chat surface that never calls a model at all.

---

## 1. The loop

### 1.1 Anthropic's own definition (confirmed, 2-1)

From *Building Effective Agents* (Dec 2024), verified verbatim:

- Agents are "systems where LLMs dynamically direct their own processes and tool usage."
- Workflows are "systems where LLMs and tools are orchestrated through predefined code paths."
- "Agents are typically just LLMs using tools based on environmental feedback in a loop."

The Claude Agent SDK is the same loop extracted from the CLI: *"The Agent SDK gives you the
same tools, agent loop, and context management that power Claude Code."*

### 1.2 The four-phase framing vs. the five-step mechanical loop

The SDK blog frames it as **gather context → take action → verify work → repeat**. But the
mechanical loop documented at `/agent-sdk/agent-loop` has **five steps and no distinct
verification stage**:

1. Receive prompt
2. Evaluate / respond
3. Execute tools
4. Repeat
5. Return result

Verification is not a phase. It is achieved by *tools the agent chooses to call* (run the
test, read the file back) plus *optional `Stop` / `PostToolUse` hooks*. This distinction
matters enormously for Vox: **do not build a verification phase into the loop.** Build
verification *tools* and a `Stop`-equivalent hook, and let the model decide when to verify.

### 1.3 Claude Code is not a "pure agent" by Anthropic's own taxonomy

An important nuance the verification pass surfaced: permission gating, compaction triggers,
hook execution, and dynamic-workflow orchestration are all **predefined code paths**. Under
Anthropic's own agent/workflow dichotomy, Claude Code is a hybrid — an agent loop with a
workflow shell. The shell is where all the reliability lives.

> **Refuted:** the characterization of the loop as "a simple while-loop alternating LLM API
> calls and tool calls" was voted **0-3**. Do not repeat it. The loop's *shape* is simple;
> its *implementation* is not, and Anthropic never described it that way.

---

## 2. Context gathering: agentic search, not RAG

### 2.1 The finding (confirmed, 3-0)

Claude issues shell/tool calls (`Glob`, `Grep`, `Read`, `Bash` with `head`/`tail`) to
selectively load only relevant file content **at runtime**. The filesystem layout is itself
a context-engineering surface: *"The folder and file structure of an agent becomes a form of
context engineering."*

**Anthropic removed an early vector-DB/RAG implementation from Claude Code because agentic
search outperformed it.** Boris Cherny (Claude Code creator), on Hacker News:

> "Early versions of Claude Code used RAG + a local vector db, but we found pretty quickly
> that agentic search generally works better."

A second Anthropic engineer added that agentic search "outperformed [it] by a lot, and this
was surprising." Independent write-ups confirmed no indexing as recently as March 2026.

The SDK post ranks the approaches explicitly: semantic search is *"usually faster than
agentic search, but less accurate, more difficult to maintain, and less transparent,"* with
the recommendation to *"start with agentic search, and only add semantic search if you need
faster results."*

### 2.2 The precision caveat

The absolutist reading ("nothing is pre-loaded") is **overstated**. Two things *are*
pre-loaded into the cached system-prompt prefix:

- `CLAUDE.md` contents.
- A working-directory listing at session start.

The accurate statement: **agentic search is the primary mechanism for pulling file
*content*; embeddings are deprioritized, not forbidden.**

### 2.3 Why this matters for Vox

Vox has invested heavily in Graphify — a structural code graph with communities, lenses,
coverage classification, and blast-radius analysis. The Claude Code evidence does **not**
say "delete it." It says:

- Do not make Graphify a *mandatory pre-load* into the model's context.
- Expose Graphify as a **tool the agent may call** (`graph query`, `graph explain`,
  `graph path`), competing on equal footing with grep.
- Graphify's genuine edge over grep is *relational* queries grep cannot answer
  ("who calls this", "what's the blast radius", "is this reachable"). That is where it
  should be pointed — see §2.4 of the audit doc, where exactly those queries produced
  findings a grep-only audit missed.

> **Refuted:** the framing "Claude Code's context strategy is just-in-time retrieval where
> the agent holds lightweight identifiers and loads data at runtime" was voted **0-3** —
> superseded by the agentic-search finding, which is the one that survived.

---

## 3. Context compaction

### 3.1 The confirmed core (3-0)

From Anthropic's *Effective context engineering for AI agents* (2025-09-29), attributing
this to Claude Code **by name**:

> "In Claude Code, for example, we implement this by passing the message history to the
> model to summarize and compress the most critical details. The model preserves
> architectural decisions, unresolved bugs, and implementation details while discarding
> redundant tool outputs or messages. The agent can then continue with this compressed
> context plus the five most recently accessed files."

Three things are load-bearing here and each is independently implementable:

| Element | Detail | Vox implication |
|---|---|---|
| **Model-driven, not rule-driven** | The *model* summarizes; there is no heuristic truncation policy deciding what matters. | Vox's `context_budget_manager.rs` must call an LLM, not a token-counting rule. |
| **An explicit preserve-list** | architectural decisions, unresolved bugs, implementation details. | This is a *prompt*, and it is tunable. Anthropic says to tune it on complex agent traces. |
| **An explicit discard-list** | redundant tool outputs, redundant messages. | Tool output is the compressible bulk, not conversation. |
| **Recency backstop** | the five most recently accessed files are re-attached post-compaction. | Cheap, high-value, and trivially portable. |

Anthropic's own caveat, quoted: *"overly aggressive compaction can result in the loss of
subtle but critical context."*

### 3.2 The tiered scheme (secondary sources, corroborating)

Reverse-engineering teardowns describe a **tiered** rather than single-shot design. These
are *secondary* sources — used as corroboration only, and their numbers disagree with each
other. Treat the *shape* as informative and the *numbers* as unverified:

| Tier | Name | Mechanism | Cost |
|---|---|---|---|
| 1 | Microcompact | Rearranges content via `cache_editing` to preserve prompt-cache hits. No data loss. | Zero; no LLM call |
| 2 | Snip | LRU archival — oldest messages moved to separate storage with lightweight markers. | No LLM call |
| 3 | Collapse | Grouped section summarization, applied incrementally (90% → 92% → 94%) for "progressive degradation rather than cliff-edge behavior." | LLM |
| 4 | Auto | Forks a sub-agent sharing the parent's model *and cached prefix*, producing a nine-section structured summary with `<analysis>` scratchpad and `<summary>` output. Only the summary enters context. | LLM |
| 5 | Reactive | Emergency recovery on API 413. Preserves the last 4 messages, summarizes the rest. One-attempt guard. | LLM |

Additional mechanics reported by the same teardown class:

- **Recursion guard:** the compaction sub-agent's `querySource` is set to `'compact'`, and
  the trigger checks that flag before firing — preventing compaction-triggering-compaction.
  *(Direct analogue: Vox's orchestrator has no such guard on its continuation nudges.)*
- **Token counting heuristic:** `t̂ = ⌈len/4⌉ + 1` — trading ~15% accuracy for speed versus
  an exact tokenizer call.
- **Warning escalation:** 60–75% (shorten outputs) → 75–90% (aggressive summarization) →
  90%+ (auto-compact).
- **Fixed overhead:** system prompt + tools + reminders consume ~20–25K tokens of a 200K
  window before any history.
- **Cache economics:** static/dynamic system-prompt split saves up to 90% on system-prompt
  cost; system prompt (~12–15K) cached at 5-minute TTL; **system reminders are injected
  into the message stream rather than the system prompt, specifically to avoid breaking
  byte-identity of the cached prefix.** This is a subtle and important trick.
- **`tokenSaverOutput` on BashTool:** full output goes to the UI, a compressed version goes
  to the model — "tens of thousands of tokens per session."

> **Numeric disagreement, deliberately not asserted:** third-party teardowns put the
> auto-compact trigger at ~83.5% of context in one account and ~92–95% in another. A
> separate widely-cited figure is `effectiveContextWindow − 13K`, with a claim that the
> buffer was reduced from 45K to 33K in v2.0.64. **We assert none of these.**

### 3.3 Cross-session state: compaction is explicitly declared insufficient (3-0)

From *Effective harnesses for long-running agents* (2025-11-26):

> "The core challenge of long-running agents is that they must work in discrete sessions,
> and each new session begins with no memory of what came before."

> "The key insight here was finding a way for agents to quickly understand the state of work
> when starting with a fresh context window, which is accomplished with the
> `claude-progress.txt` file alongside the git history."

The post explicitly says compaction exists in the SDK but *"isn't sufficient"* and
*"doesn't always pass perfectly clear instructions to the next agent."* Externalized
artifacts **complement** compaction; they do not replace it.

> **Scope caveat (important).** This is a case study of one Agent SDK harness experiment,
> not documentation of the shipped CLI. `claude-progress.txt` is that experiment's
> convention, **not a CLI feature**. The CLI's own cross-session mechanisms are
> auto-compaction and `--resume`.

> **Refuted (1-2):** the claim that the harness uses a formal two-role split — a specially
> prompted "initializer" session creating scaffolding, then "coding agent" sessions
> consuming it. Do not assert this.

---

## 4. Skills: progressive disclosure and automatic activation

*(This section is from direct primary-source fetches on 2026-07-30, not the workflow.)*

This is the mechanism the user asked about most directly, and it is **far simpler than it is
usually assumed to be.** There is no classifier, no embedding index, no routing model.

### 4.1 The three levels

| Level | When loaded | Token cost | Content |
|---|---|---|---|
| **1 — Metadata** | Always, at startup | **~100 tokens per skill** | `name` + `description` from YAML frontmatter |
| **2 — Instructions** | When the skill is triggered | Under 5K tokens | The SKILL.md body |
| **3+ — Resources** | As referenced | **Zero until accessed** | Bundled files; scripts run via bash, only their *output* enters context |

### 4.2 How activation actually works

> "Claude loads this metadata at startup and includes it in the system prompt. The
> `description` is what Claude matches your request against when determining whether to
> trigger the Skill, so it must say both what the Skill does and when to use it."

**That is the entire mechanism.** The model reads a list of `name: description` pairs in its
system prompt and decides, in-context, whether one applies. When it decides yes, it runs
`cat skill/SKILL.md` — an ordinary bash call — and the instructions enter context.

The documented worked example:

1. **Startup:** system prompt includes `pdf-processing - Extract text and tables from PDF files, fill forms, merge documents. Use when working with PDF files or when the user mentions PDFs, forms, or document extraction.`
2. **User:** "Extract the text from this PDF and summarize it"
3. **Claude invokes:** `bash: cat pdf-processing/SKILL.md`
4. **Claude determines:** form filling not needed → `FORMS.md` not read
5. **Claude executes** using the loaded instructions

**Consequences for Vox, stated plainly:**

- Automatic skill detection does **not** require vox-similarity, embeddings, a classifier,
  or a router. It requires (a) putting skill descriptions in the system prompt and (b)
  having a system prompt at all. Vox's chat path has neither.
- "You can install many Skills without context penalty" is the entire scaling story. At
  ~100 tokens each, 200 skills ≈ 20K tokens — which is why the metadata block must stay
  metadata and never inline bodies.
- The `description` is the **only** discovery surface. A skill with a vague description is
  invisible regardless of how good its body is.

### 4.3 Frontmatter contract (normative)

`name`:
- Maximum 64 characters
- Lowercase letters, numbers, hyphens only
- No XML tags
- Cannot contain reserved words `anthropic` or `claude`

`description`:
- Non-empty
- **Maximum 1,024 characters**
- No XML tags
- Must state **both** what the skill does **and** when to use it

Discovery paths in Claude Code: `~/.claude/skills/` (personal), `.claude/skills/` (project).
Filesystem-based; no API upload. Skills do **not** sync across surfaces (claude.ai / API /
Claude Code are three separate registries).

### 4.4 Description authoring rules (these are the high-leverage bits)

**Always write in third person.** The docs carry this as a warning, with the stated reason:
*"The description is injected into the system prompt, and inconsistent point-of-view can
cause discovery problems."*

- Good: `Processes Excel files and generates reports`
- Avoid: `I can help you process Excel files`
- Avoid: `You can use this to process Excel files`

**Be specific and include key terms.** Worked examples from the docs:

```yaml
description: Extract text and tables from PDF files, fill forms, merge documents. Use when working with PDF files or when the user mentions PDFs, forms, or document extraction.
```
```yaml
description: Analyze Excel spreadsheets, create pivot tables, generate charts. Use when analyzing Excel files, spreadsheets, tabular data, or .xlsx files.
```
```yaml
description: Generate descriptive commit messages by analyzing git diffs. Use when the user asks for help writing commit messages or reviewing staged changes.
```

Anti-examples: `Helps with documents`, `Processes data`, `Does stuff with files`.

The structural pattern is consistent: **`<verb phrase listing concrete capabilities>. Use
when <concrete trigger conditions, including literal words the user might type>.`**

### 4.5 Body authoring rules

- **Keep SKILL.md under 500 lines.** Split when approaching the limit.
- **"The context window is a public good."** Only add context the model doesn't already
  have. Challenge each paragraph: does it justify its token cost?
- **Set appropriate degrees of freedom** — matched to task fragility:
  - *High freedom* (prose instructions): multiple valid approaches, context-dependent
    decisions. Analogy: "open field with no hazards."
  - *Medium freedom* (pseudocode, parameterized scripts): a preferred pattern exists.
  - *Low freedom* (exact scripts, no parameters): fragile, error-prone, consistency
    critical. Analogy: "narrow bridge with cliffs on both sides." Example: *"Run exactly
    this script … Do not modify the command or add additional flags."*
- **Keep references one level deep from SKILL.md.** Nested references cause partial reads:
  "Claude might use commands like `head -100` to preview content rather than reading entire
  files, resulting in incomplete information."
- **Reference files >100 lines need a table of contents** so a partial read still reveals
  scope.
- **Domain-partition reference files** so a sales question loads `sales.md` and nothing else.
- **Avoid time-sensitive content**; use a collapsed "Old patterns" section instead.
- **Use consistent terminology** — one term per concept, throughout.
- **Don't offer too many options.** Provide a default with an escape hatch, not a menu.
- **Forward slashes only** in paths, even on Windows.
- **MCP tools need fully-qualified names** (`ServerName:tool_name`) or they won't resolve.

### 4.6 Evaluation-driven skill development

The docs are unusually prescriptive here, and it maps directly onto what Vox's skill
marketplace lacks:

> "**Create evaluations BEFORE writing extensive documentation.** This ensures your Skill
> solves real problems rather than documenting imagined ones."

The loop: identify gaps (run without the skill, document failures) → build 3 scenarios →
establish baseline → write minimal instructions → iterate.

Evaluation record shape:

```json
{
  "skills": ["pdf-processing"],
  "query": "Extract all text from this PDF file and save it to output.txt",
  "files": ["test-files/document.pdf"],
  "expected_behavior": [
    "Successfully reads the PDF file using an appropriate PDF processing library or command-line tool",
    "Extracts text content from all pages in the document without missing any pages",
    "Saves the extracted text to a file named output.txt in a clear, readable format"
  ]
}
```

Note the docs' own admission: *"There is not currently a built-in way to run these
evaluations."* **This is an open lane for Vox** — a skill-eval runner is a genuine
differentiator, not catch-up.

### 4.7 The Claude-A / Claude-B development pattern

Anthropic's recommended authoring loop, which is directly implementable as a Vox workflow:

- **Claude A** — the expert instance that writes and refines the skill.
- **Claude B** — a fresh instance with the skill loaded, doing real work.
- Observe B's behavior → bring specific failures back to A → A restructures (e.g. promotes
  a rule's prominence, strengthens "always" to "MUST") → re-test with B.

Signals to watch when observing B:

| Signal | Diagnosis |
|---|---|
| Reads files in an unanticipated order | Structure isn't as intuitive as assumed |
| Fails to follow references | Links need to be more explicit/prominent |
| Repeatedly reads the same file | That content belongs in SKILL.md |
| Never accesses a bundled file | It's unnecessary, or poorly signaled |

**Test across model tiers.** "What works perfectly for Opus might need more detail for
Haiku." Directly relevant to Vox's multi-model ambitions: a skill authored against a
frontier model may silently fail on a local 7B.

---

## 5. Tool design

### 5.1 The 25,000-token cap (confirmed, 3-0)

> "For Claude Code, we restrict tool responses to 25,000 tokens by default."

Re-verified against live MCP docs on 2026-07-30: a **10,000-token warning threshold**, a
**25,000-token default maximum**, and `MAX_MCP_OUTPUT_TOKENS` to adjust it.

The cap is enforced on first-party tools too. GitHub issue #4002 shows `Read` erroring:

> `File content (28375 tokens) exceeds maximum allowed tokens (25000). Please use offset and limit parameters.`

**Note the shape of that error.** It states the limit, states the actual value, and names
the recovery mechanism. That is the error-message design principle applied to its own cap.

### 5.2 Per-tool budgeting defaults

Anthropic's guidance: *"implement some combination of pagination, range selection, filtering,
and/or truncation with sensible default parameter values for any tool responses that could
use up lots of context"* — and, critically, *"If you choose to truncate responses, be sure
to steer agents with helpful instructions."*

Each technique maps to a live parameter:

| Tool | Technique | Default |
|---|---|---|
| `Read` | Range selection | `offset`/`limit`, 2000-line default |
| `Grep` | Truncation | `head_limit` default 250 |
| `Bash` | Character truncation | 30,000 chars in the live schema; `BASH_MAX_OUTPUT_LENGTH` env var |
| MCP tools | Declared cap | `anthropic/maxResultSizeChars` overrides the token cap for text |

The critique worth internalizing (dev.to, arXiv 2511.22729): **silent** truncation makes
agents confidently summarize content they never saw. Anthropic pre-empts this by *requiring*
steering instructions in the truncated result. Vox must do the same — a truncated tool
result that doesn't say it was truncated is a correctness bug, not a UX nit.

### 5.3 Tool-set design principles

From *Writing tools for agents*:

- **"More tools don't always lead to better outcomes."** Build "a few thoughtful tools
  targeting specific high-impact workflows" rather than wrapping every API endpoint.
- **Namespace by service and resource** (`asana_search`, `asana_projects_search`). The post
  notes that even the choice between prefix- and suffix-based namespacing produces
  *measurable* evaluation differences.
- **Error messages must guide, not report.** "Clearly communicate specific and actionable
  improvements" instead of "opaque error codes or tracebacks." Good errors "steer agents
  towards more token-efficient tool-use behaviors."
- **Offer a `ResponseFormat` enum** (`concise` | `detailed`) so the agent can request the
  verbosity it needs.
- **Evaluate with realistic multi-call tasks** — "potentially dozens" of tool calls, not
  single operations. Have agents read their own transcripts to find where they got stumped.
- **The failure mode to design against:** an address-book tool returning *all* contacts
  forces the agent to read "token-by-token" through irrelevant data — brute-force search
  burning the context window.

> **Refuted (1-2):** the widely-repeated claim that Anthropic spent more engineering effort
> on tool definitions than on the overall prompt for their SWE-bench agent. Do not assert.

---

## 6. Permissions

### 6.1 The six-step pipeline (confirmed, 3-0)

The docs render "How permissions are evaluated" as a six-item ordered list, and the flow
diagram's alt text calls it "the six-step permission evaluation flow" — so **"six-step" is
the vendor's own framing**, not an inference.

1. **Hooks**
2. **Deny rules**
3. **Ask rules**
4. **Permission mode**
5. **Allow rules**
6. **`canUseTool` callback**

Key per-step semantics, quoted:

- Deny rules block *"even in bypassPermissions mode."*
- Ask rules *"fall through to your canUseTool callback."*
- *"bypassPermissions approves everything that reaches this step."*
- In `dontAsk` mode, step 6 *"is skipped and the tool is denied."*

### 6.2 The six modes

`default`, `dontAsk`, `acceptEdits`, `bypassPermissions`, `plan`, `auto`.

`acceptEdits` auto-approves:
- File edits (`Edit`, `Write`)
- Filesystem commands: `mkdir`, `touch`, `rm`, `rmdir`, `mv`, `cp`, `sed`

…and **only** for paths inside the working directory or `additionalDirectories`. Paths
outside that scope, and writes to protected paths, still prompt. Non-filesystem `Bash` still
requires normal permissions.

### 6.3 Precision notes that matter

- **"Fixed" means the sequence is fixed, not that steps never short-circuit.**
  `AskUserQuestion`, MCP tools with `_meta["anthropic/requiresUserInteraction"]`, and
  org-set ask connector tools always reach the callback even under an allow rule. Plan mode
  routes file-edit/shell-write tools to the callback regardless of allow rules. Bare-name
  deny rules strip the tool *before* evaluation begins.
- **Several secondary write-ups report a different order ("Deny → Allow → Ask").** These
  are derivative and contradicted by the primary doc.
- `dontAsk` is settings/flag-only and absent from the CLI's Shift+Tab cycle, so the
  *user-facing* CLI mode count is smaller than six.

### 6.4 Hooks as the universal programmable interception point (confirmed, 3-0)

Bolded in the permissions doc:

> "**Auto-approved tools never reach `canUseTool`.** A tool call approved at any earlier
> step, by `acceptEdits` or `bypassPermissions`, or by an allow rule, skips your
> `canUseTool` callback… For checks that must run on every tool call, use a `PreToolUse`
> hook: hooks run before every other step, and a hook deny applies even in
> `bypassPermissions` mode."

And from the hooks doc: *"If any hook returns deny, the operation is blocked regardless of
other hooks"*; *"deny takes priority over defer, which takes priority over ask, which takes
priority over allow."*

Three qualifications the verification pass insisted on:

1. "Only universal interception point" **overstates it** — deny rules also survive
   `bypassPermissions`. Hooks are the only *programmable* one.
2. Hooks are **matcher-filtered**, so universality depends on registration.
3. A hook returning `allow` does **not** short-circuit deny/ask rules.

### 6.5 The security critique

Worth carrying forward because it applies verbatim to Vox's `harness_trust_guard`: security
researchers demonstrated that the permission layer pattern-matches on **command strings, not
capabilities** — reaching a denied binary via `/proc/self/root/usr/bin/npx` (same binary,
different path string). **Any deny-list built on string matching is a speed bump, not a
boundary.**

---

## 7. Hooks: the loop published as an extension surface

### 7.1 The finding (confirmed, 3-0)

Claude Code exposes **~30 lifecycle hook events that mirror the harness's internal loop
structure**, effectively publishing the agentic loop as a public extension surface. The
docs' framing: hooks are *"user-defined shell commands, HTTP endpoints, or LLM prompts that
execute automatically at specific points in Claude Code's lifecycle,"* receiving JSON
context on stdin.

`PostToolBatch` being defined as *"After a full batch of parallel tool calls resolves,
before the next model call"* is direct evidence that loop internals are exposed.

### 7.2 The full event table

*(Fetched directly from `code.claude.com/docs/en/hooks`, 2026-07-30.)*

| Event | Trigger | Blocks? | Key control fields |
|---|---|---|---|
| `SessionStart` | Session begins/resumes | No | `additionalContext`, `initialUserMessage`, `watchPaths`, `sessionTitle`, `reloadSkills` |
| `Setup` | `--init-only` / `--init` / `--maintenance` | No | as `SessionStart` |
| `UserPromptSubmit` | User submits prompt, before processing | **Yes** | `decision: "block"`, `reason`, `additionalContext` |
| `UserPromptExpansion` | Slash command expands | **Yes** | `decision: "block"`, `reason` |
| `PreToolUse` | Before tool executes | **Yes** | `permissionDecision`, `permissionDecisionReason`, `updatedInput`, `additionalContext` |
| `PermissionRequest` | Tool needs a permission decision | **Yes** | `decision.behavior`, `updatedInput`, `decision.permissionRules` (with `ttl`) |
| `PermissionDenied` | Denied by auto-mode classifier | No | `retry: true` |
| `PostToolUse` | After tool succeeds | **Yes** | `decision: "block"`, `updatedToolOutput`, `additionalContext` |
| `PostToolUseFailure` | After tool fails | **Yes** | as `PostToolUse`; adds `tool_error` |
| `PostToolBatch` | After a parallel batch, before next model call | **Yes** | as `PostToolUse`; `tool_uses[]`, `batch_size` |
| `Stop` | Claude finishes responding | **Yes** | `decision: "block"`, `reason`, `additionalContext` |
| `StopFailure` | Turn ends due to API error | No | output ignored; `error_type`, `error_message` |
| `Notification` | Claude Code sends a notification | No | `systemMessage`, `terminalSequence` |
| `MessageDisplay` | Assistant text displays | No | `displayContent` (display-only; transcript unchanged) |
| `SubagentStart` | Subagent spawned | No | `agent_type`, `agent_id` |
| `SubagentStop` | Subagent finishes | **Yes** | as `Stop` |
| `TaskCreated` | Task created via `TaskCreate` | **Yes** (rollback) | exit 2 / `continue: false` |
| `TaskCompleted` | Task marked completed | **Yes** | exit 2 / `continue: false` |
| `TeammateIdle` | Agent-team teammate going idle | **Yes** | exit 2 prevents idling |
| `InstructionsLoaded` | CLAUDE.md / `.claude/rules/*.md` loaded | No | `file_path`, `load_reason`, `content` |
| `ConfigChange` | Config file changes mid-session | **Yes** | `config_source`, `changed_keys` |
| `CwdChanged` | Working directory changes | No | `old_cwd`, `new_cwd` |
| `FileChanged` | Watched file changes on disk | No | `file_path`, `change_type` |
| `WorktreeCreate` | Worktree created | **Yes** | path on stdout / `worktreePath` |
| `WorktreeRemove` | Worktree removed | No | failures logged in debug only |
| `PreCompact` | Before compaction | **Yes** | `trigger_type: manual\|auto` |
| `PostCompact` | After compaction | No | side effects only |
| `Elicitation` | MCP server requests user input | **Yes** | `action`, `content` |
| `ElicitationResult` | User responds to elicitation | **Yes** | `action`, `content` |
| `SessionEnd` | Session terminates | No | `end_reason` |

**Matcher support varies by event** and is itself informative: `SessionStart` matches on
`startup|resume|clear|compact|fork`; `PreToolUse` on tool name (regex, e.g.
`mcp__memory__.*`); `SubagentStart/Stop` on agent type; `PreCompact` on `manual|auto`;
`InstructionsLoaded` on load reason; `ConfigChange` on config source; `FileChanged` on
literal filenames. `UserPromptSubmit`, `Stop`, `PostToolBatch`, `CwdChanged` have **no**
matcher — they always fire.

### 7.3 Universal output fields

Available on exit 0 across all events:

```json
{
  "continue": false,
  "stopReason": "Why stopped",
  "suppressOutput": false,
  "systemMessage": "Warning shown to the user",
  "terminalSequence": "OSC escape for notifications/titles",
  "hookSpecificOutput": {
    "hookEventName": "EventName",
    "additionalContext": "Context injected for Claude"
  }
}
```

`UserPromptSubmit` has a documented **30-second default timeout, lowered from 600** — a
telling detail: a blocking pre-prompt hook is on the latency-critical path and Anthropic
tightened it.

### 7.4 ⚠️ Two hook claims that FAILED verification

Both are widely repeated, including in our own tooling docs. **Do not assert them without
re-verification:**

1. **Hook exit-code semantics** (`0` = success with JSON from stdout, `2` = blocking error
   with stderr fed to Claude, other = non-blocking) — voted **1-2**.
2. **`PreToolUse` `permissionDecision` taking exactly four values** (`allow`/`deny`/`ask`/
   `defer`) — voted **0-3**.

The direct doc fetch in §7.2 *does* show both patterns. The verification pass disagreed on
whether the sources support them as stated with the claimed universality. The honest position:
**the patterns are real and documented for specific events, but "fixed semantics across all
hooks" is not established.** Vox should design its hook contract explicitly rather than
copying an assumed one.

*(This is exactly the kind of thing our `feedback_verify_audit_retirement_claims` memory
warns about — plausible, everywhere, and not actually verified.)*

---

## 8. Subagents

### 8.1 The finding (confirmed, 3-0)

Verbatim from the SDK blog:

> "Subagents are useful for two main reasons. First, they enable parallelization… Second,
> they help manage context: subagents use their own isolated context windows, and only send
> relevant information back to the orchestrator, rather than their full context."

And from the context-engineering post:

> "Each subagent might explore extensively, using tens of thousands of tokens or more, but
> returns only a condensed, distilled summary of its work (often 1,000-2,000 tokens)."

Implementation-level detail from current SDK docs:

- *"A subagent's context window starts fresh, with no parent conversation… The only content
  you pass from parent to subagent is the Agent tool's prompt string."*
- *"Intermediate tool calls and results stay inside the subagent; only its final message
  returns to the parent."*
- *"When the main conversation compacts, subagent transcripts are unaffected. They're stored
  in separate files."*

### 8.2 Qualifications

- The docs list **four** benefits, not two: parallelization, context isolation, *specialized
  instructions/knowledge*, and *tool restrictions* (e.g. limiting a subagent to
  `Read`/`Grep`/`Glob`). The blog's "two main reasons" is an incomplete enumeration.
- **1,000–2,000 tokens is an illustrative design point, not a measured distribution or an
  enforced bound.** The real ceiling is the subagent's output cap (reported variously as 8K
  and 32K across versions).
- Critics correctly note that isolation hides work-in-progress but does not prevent a
  verbose final result, and that N subagents multiply *total* token spend even while
  reducing *orchestrator* context.

### 8.3 The orchestrator-worker shape

From *How we built our multi-agent research system*: a lead agent decomposes the query and
spawns 3–5 parallel subagents, each given **an objective, an output format, tool/source
guidance, and explicit task boundaries.** Those four elements are the subagent prompt
contract — and they are exactly what Vox's `dispatch` path does not supply.

---

## 9. API-level context primitives (adjacent, not the CLI)

Confirmed 3-0, but with a **critical scope caveat**: there is **no evidence the Claude Code
CLI is implemented on top of these.** Claude Code's auto-compaction predates the Jan 2026
beta and is client-side.

### 9.1 Server-side compaction — `compact_20260112` (beta)

- Header `anthropic-beta: compact-2026-01-12`
- Trigger `{"type":"input_tokens","value":150000}`, documented **minimum 50,000**
- *"The API automatically drops all content blocks prior to the compaction block, continuing
  the conversation from the summary."*
- Summary produced server-side, returned as a typed compaction content block. User messages,
  assistant messages, tool calls, tool results, and even prior compaction blocks are
  flattened.
- Optional `instructions` override; `pause_after_compaction` flag.

### 9.2 Tool-result clearing — `clear_tool_uses_20250919`

> "Tool-result clearing, by contrast, is a *sub-transcript* operation. It walks the message
> list and surgically replaces `tool_result` content blocks, leaving everything else — user
> messages, assistant reasoning, the `tool_use` record — untouched."

Placeholder literal: `[cleared to save context]`. A companion `clear_thinking_20251015`
strategy exists. `tool_use` records survive only under the default `clear_tool_inputs: false`.

**This is the highest-leverage cheap win available to Vox**: tool results are the
compressible bulk, and clearing them is a pure data-structure operation with no LLM call.

### 9.3 The memory tool — `memory_20250818` (medium confidence)

- Config is just `{"type":"memory_20250818","name":"memory"}`
- **Client-side executed**: *"Claude requests file operations, and your application executes
  them."*
- Exactly **six** operations: `view`, `create`, `str_replace`, `insert`, `delete`, `rename`
- Rooted at a **virtual** `/memories` path — *"a prefix that your handler maps onto real
  storage, such as a per-user directory or keys in a database."*
- The API **auto-injects a system prompt**, quoted verbatim in the docs, beginning
  `IMPORTANT: ALWAYS VIEW YOUR MEMORY DIRECTORY BEFORE DOING ANYTHING ELSE` and including
  `ASSUME INTERRUPTION: Your context window might be reset at any moment.`
- GA — no beta header. Third-party posts still citing `context-management-2025-06-27` are
  stale.

Confidence is *medium* only because of scope: this is the API's memory tool, adjacent to
rather than part of the CLI harness.

---

## 10. Synthesis: what actually produces "it just works"

Ranked by leverage, based on what survived verification:

1. **A real loop with real tools.** Everything else is scaffolding around this. If the loop
   doesn't exist, no amount of scaffolding produces agency. *(This is Vox's #1 gap.)*
2. **A system prompt that lists available capabilities cheaply.** ~100 tokens per skill is
   what makes automatic activation possible. No system prompt → no automatic anything.
3. **Hard, enforced, well-signposted budgets on tool output.** 25K cap; per-tool
   pagination/truncation defaults; errors that name the recovery mechanism.
4. **Model-driven compaction with an explicit preserve/discard list** plus a recency
   backstop (five most-recent files).
5. **Context isolation via subagents** with a four-part prompt contract (objective, output
   format, tool guidance, boundaries).
6. **A deterministic, ordered permission pipeline** where the programmable layer (hooks)
   runs first and its deny is absolute.
7. **The loop published as ~30 lifecycle hook events**, so users extend the harness without
   forking it.
8. **Externalized cross-session state on disk** — because compaction is explicitly declared
   insufficient.
9. **Cache-preserving discipline** — system reminders injected into the message stream, not
   the system prompt, to keep the cached prefix byte-identical.

The unifying principle: **every failure mode gets a specific, boring, verifiable mechanism.**
Not a smarter prompt. Not a bigger model. A mechanism.

---

## 11. Where Vox can exceed, not just match

Genuine gaps in the Claude Code design that Vox's existing assets already address:

| Gap in Claude Code | Vox asset | Note |
|---|---|---|
| No built-in skill-evaluation runner ("There is not currently a built-in way to run these evaluations") | `vox-eval`, `vox-test-harness` | Ship the eval runner Anthropic documents but doesn't provide |
| Single-provider by construction | `models::decide()` with `SelectionAxes` + `CandidateScope` | Vox's selector is genuinely more expressive — it is just not wired to chat |
| No local-inference lane | `PopuliLocal` / `PopuliMesh` / Ollama routes | Present in `chat_route_to_llm_config`; no user-facing policy |
| Relational code queries are grep-shaped | Graphify (29K nodes, communities, blast-radius, coverage lenses) | Expose as *tools*, not as a pre-load |
| Skills don't grow from observed usage | `vox-skill-discovery` (`code_miner`, `op_miner`) + `skill_reliability` table | The organic-growth loop Anthropic doesn't have |
| Deny-lists are string-matched (demonstrated bypass) | `vox-capability-registry`, `vox-bounded-fs` | Capability-based enforcement beats pattern matching |

---

## 12. Refuted ledger — do not restate these

Eight claims failed 3-vote adversarial verification. Several appear in prior Vox docs and in
widely-shared blog posts.

| # | Claim | Vote |
|---|---|---|
| 1 | Claude Code's context strategy is "just-in-time retrieval with lightweight identifiers" | **0-3** |
| 2 | Anthropic characterizes the agentic loop as "a simple while-loop" | **0-3** |
| 3 | `PreToolUse` `permissionDecision` takes exactly four values (allow/deny/ask/defer) | **0-3** |
| 4 | Hook exit codes have fixed semantics (0 = success, 2 = blocking, other = non-blocking) | **1-2** |
| 5 | A formal two-role initializer/coding-agent split | **1-2** |
| 6 | Anthropic spent more effort on tool definitions than on the prompt for their SWE-bench agent | **1-2** |
| 7 | Anthropic's guidance *requires* ground truth from the environment at each loop step | **1-2** |
| 8 | Context exhaustion is handled by "automatic compaction… rather than hard-truncating" (as framed by the SDK blog) | **1-2** |

**Overstatement watch** — surviving claims whose usual phrasing is slightly too strong:

- "only universal interception point" → deny rules also survive `bypassPermissions`;
  hooks are the only *programmable* one.
- "fixed" pipeline → the *sequence* is fixed; individual steps short-circuit.
- "nothing is pre-loaded" → CLAUDE.md and a cwd listing are pre-loaded.
- "subagents return 1,000–2,000 tokens" → illustrative design point, not a bound.

---

## 13. Open questions

Carried forward deliberately; none of these are answered by available sources.

1. **What is the actual structure of the Claude Code CLI system prompt** — its sections,
   ordering, what is cached in the prefix, where CLAUDE.md and the cwd listing are injected,
   and how tool definitions sit relative to instructions? *No verified source addresses
   this*, despite it being central to the question. Treat any account of it as unverified.
2. **Does the shipped CLI use `compact_20260112` / `clear_tool_uses_20250919`, or its own
   client-side path?** Related: what is the real auto-compact threshold, given teardowns
   disagree (~83.5% vs ~92–95%)?
3. **What are the correct `PreToolUse` return semantics** — the exact `permissionDecision`
   value set and the exact exit-code contract? Both claims here were voted down, leaving the
   most operationally important part of the hook system unverified.
4. **How do the CLI's user-facing permission surfaces map onto the SDK's six-mode model** —
   which modes are reachable via Shift+Tab, how `settings.json` rules compile into the
   pipeline, and whether the CLI adds steps (e.g. the "auto mode classifier" implied by the
   `PermissionDenied` hook) that SDK docs don't describe?
5. **What are the concrete limits of subagent isolation** — the actual output-token cap
   (8K vs 32K reported), what happens when a subagent's own context fills, and whether
   subagents can nest?

---

## 14. Methodology and its limits

**What was done.** 5 decomposed search angles; 26 sources fetched and claim-extracted
(125 claims); top 25 claims each put to 3 independent verifiers prompted to *refute*, with
2-of-3 refutes killing the claim; survivors synthesized with source attribution. Plus direct
primary-source fetches for the Skills and hooks surfaces.

**What that does not buy.**

- **No source describes the actual CLI internals at code level.** Everything is vendor
  documentation, vendor engineering blogs, or third-party inference. Reverse-engineering
  teardowns were used only as corroboration, never as sole support.
- **Scope conflation is the biggest risk in this evidence base.** Three distinct systems are
  easy to blur: (1) the Claude Code CLI, (2) the Claude Agent SDK, (3) Anthropic API
  primitives. Permission-pipeline findings come from **SDK** docs. Compaction/memory
  primitives come from **API** docs with no established link to the CLI.
- **Time sensitivity.** Engineering blog posts are 8–19 months old (Dec 2024 – Nov 2025).
  Docs pages are current (version markers to v2.1.219, fetched 2026-07-30). **Where blog and
  docs diverge, docs win** — e.g. the SDK's mechanical loop has no verification phase despite
  the blog's four-phase framing; subagents have four documented benefits despite the blog's
  "two main reasons."
- **Re-verified as not drifted:** the 25,000-token cap and the six permission modes.
