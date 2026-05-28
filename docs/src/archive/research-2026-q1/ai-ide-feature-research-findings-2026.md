---
title: "AI IDE feature research findings 2026"
description: "Evidence-backed comparison of modern AI IDE features and a Vox difficulty, LOC, necessity, GUI, and Mens gap analysis."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-03-31"
training_eligible: false
training_rationale: "Synthesizes architecture constraints and findings for implementation waves."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# AI IDE feature research findings 2026

## Purpose

This document is the research dossier for the modern AI IDE and coding-agent market, with a specific goal:

- identify the features developers most repeatedly value because they save real time,
- compare the strongest current products using documented evidence,
- map those same features against the current Vox codebase,
- estimate likely Vox implementation difficulty and rough LOC bands,
- recommend what Vox should build next inside the existing VS Code extension and supporting core crates.

This page is research, not a claim that Vox or any external product fully ships every capability mentioned below.

The machine-readable companion artifact for future AI-assisted analysis is:

- [`docs/agents/ai-ide-feature-matrix-2026.json`](../../agents/ai-ide-feature-matrix-2026.json)

## Executive summary

The strongest pattern across modern AI IDEs is not “better autocomplete.” It is a bundled workflow:

1. an agent can read and edit multiple files,
2. it can run tools like terminal, browser, or diagnostics,
3. it can show a plan before action when needed,
4. it leaves behind checkpoints, diffs, and review controls,
5. it remembers durable repo guidance through rules, memories, skills, or workflows,
6. it gives the user enough transparency that autonomy feels safe instead of reckless.

The most loved features are the ones that reduce friction in repeated loops:

- very fast inline completion and edits,
- strong plan or ask modes,
- easy rollback and checkpoint restore,
- visible multi-file review,
- explicit context targeting with `@`-style files, search, or repo indexing,
- reusable rules, workflows, and skills,
- tool transparency and approvals,
- automation of validation, tests, and lint-fix loops.

The most important Vox conclusion is that the repo already has more backend capability than its current product feel suggests. Vox is not starting from zero. It already has:

- MCP-first tool surfaces and registry discipline,
- orchestrator tasking and agent lifecycle machinery,
- snapshot and workspace primitives,
- browser tooling,
- memory and retrieval infrastructure,
- voice-adjacent Oratio surfaces,
- planning, plan adequacy, and context lifecycle work.

The biggest gap is productization, not sheer capability count. In practical terms, Vox should prioritize:

1. review, checkpoint, and diff UX on top of existing snapshot infrastructure,
2. repo-visible rules, workflows, and reusable agent guidance,
3. better context targeting and retrieval ergonomics,
4. clearer ask / plan / execute / debug mode boundaries,
5. stronger verification and autofix loops in the extension UI.

Vox should defer or sharply limit investment in the most expensive “full platform” ambitions until the single-user editor loop feels excellent:

- deep Git/PR/worktree parity with Codex and GitHub Copilot,
- highly visible multi-agent orchestration UX,
- cloud-manager surfaces that duplicate what premium hosted tools already sell.

Mens should support this roadmap, not lead it. The best Mens-aligned opportunities are:

- lower-latency completion and edit routing,
- better retrieval and context ranking,
- voice-to-code quality,
- eventual personalization of workflow suggestions and memory retrieval once deterministic controls exist.

## Methodology

Primary evidence was gathered from official docs, official release notes, official changelogs, and official product pages where possible. The comparison set mixes full IDEs and influential coding-agent products because developer expectations are shaped by both.

Important constraints:

- not every vendor documents every feature with equal precision,
- some products publish polished docs while others rely more on launch posts,
- `Antigravity` currently has weaker evidence quality than the rest of the set and is therefore treated with lower confidence.

### Comparison set

Core named tools:

- Cursor
- Windsurf
- Antigravity
- Claude Code
- ChatGPT desktop plus Codex app workflow
- Gemini Code Assist

Additional comparators:

- GitHub Copilot coding agent
- Zed AI
- Aider
- Cline
- Roo Code
- Replit Agent
- Devin
- Continue

### Scoring notes

The product composite scores below are synthesized from documented feature coverage in the categories that repeatedly correlate with developer time savings:

- inline generation and edits,
- agentic multi-file execution,
- safety and review,
- rules or memory,
- extensibility,
- context controls,
- verification loops,
- multimodal and GUI support.

They are not benchmark scores and should not be confused with SWE-bench or vendor model claims.

### Support legend

- `S` = strong documented support
- `P` = partial documented support
- `L` = limited or narrow documented support
- `N` = no meaningful evidence found in the sources used
- `U` = unclear or low-confidence evidence

## Evidence inventory

| Product | Official evidence used | Confidence | Notes |
| --- | --- | --- | --- |
| Cursor | [Agent mode](https://docs.cursor.com), [Features](https://cursor.com/en/features), [Subagents](https://docs.cursor.com/troubleshooting/common-issues) | High | Best-documented all-around AI IDE in this research pass. |
| Windsurf | [Cascade overview](https://docs.windsurf.com/windsurf/cascade/cascade), [Memories and rules](https://docs.windsurf.com/windsurf/cascade/memories), [Workflows](https://docs.windsurf.com/windsurf/cascade/workflows) | High | Particularly strong on repo-visible customization and workflow reuse. |
| Antigravity | [Google Developers blog](https://developers.googleblog.com/build-with-google-antigravity-our-new-agentic-development-platform), [Community documentation mirror](https://antigravity.im/documentation) | Low | Interesting directionally, but evidence quality is weaker than the rest of the set. |
| Claude Code | [Tools reference](http://code.claude.com/docs/en/tools-reference), [Subagents](https://code.claude.com/docs/en/sub-agents), [Hooks guide](https://code.claude.com/docs/en/hooks-guide) | High | Not a classic IDE, but a major reference for agent architecture. |
| ChatGPT desktop plus Codex | [ChatGPT macOS release notes](https://help.openai.com/en/articles/9703738-desktop-app-release-notes), [Codex app features](https://developers.openai.com/codex/app/features) | High | Strong on worktrees, terminal, voice, and Git review controls. |
| Gemini Code Assist | [Code overview](https://developers.google.com/gemini-code-assist/docs/code-overview), [Chat overview](https://developers.google.com/gemini-code-assist/docs/chat-overview), [Release notes](https://developers.google.com/gemini-code-assist/resources/release-notes) | High | Broad IDE feature set with strong enterprise positioning. |
| GitHub Copilot coding agent | [Copilot coding agent docs](https://docs.github.com/en/copilot/how-tos/use-copilot-agents/coding-agent) | High | Especially strong when the destination workflow is issue-to-PR. |
| Zed AI | [AI overview](https://zed.dev/docs/ai/overview), [Agent panel](https://zed.dev/docs/ai/agent-panel.html), [Tools](https://zed.dev/docs/ai/tools.html) | High | Strong editor-native reference with excellent review ergonomics. |
| Aider | [Git integration](https://aider.chat/docs/git.html), [Commands](https://aider.chat/docs/usage/commands.html), [Options](https://aider.chat/docs/config/options.html) | High | A key reference for Git-first safety and terminal power users. |
| Cline | [Plan and Act](https://docs.cline.bot/features/plan-and-act), [Checkpoints](https://docs.cline.bot/core-workflows/checkpoints), [MCP overview](https://docs.cline.bot/mcp/mcp-overview) | Medium | Strong for explicit planning and checkpoint behavior. |
| Roo Code | [Using modes](https://docs.roocode.com/basic-usage/using-modes/), [Boomerang tasks](https://docs.roocode.com/features/boomerang-tasks) | High | Good reference for mode design and orchestration isolation. |
| Replit Agent | [Replit Agent](https://docs.replit.com/replitai/agent-v2), [Checkpoints and rollbacks](https://docs.replit.com/core-concepts/agent/checkpoints-and-rollbacks) | High | Cloud-first, strong on checkpoints, app testing, and visual workflows. |
| Devin | [Interactive planning](https://docs.devin.ai/work-with-devin/interactive-planning), [Knowledge](https://docs.devin.ai/product-guides/knowledge), [First session](https://docs.devin.ai/get-started/first-run) | High | Strong on indexing, persistent knowledge, and long autonomous sessions. |
| Continue | [Configuring models, rules, tools](https://docs.continue.dev/guides/configuring-models-rules-tools), [MCP in Continue](https://docs.continue.dev/customize/deep-dives/mcp) | Medium | More configuration substrate than polished end-user product surface. |

## Product scoreboard

| Product | Composite / 100 | Agent depth | Safety and review | Rules or memory | Extensibility | Multimodal | Short read |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Cursor | 95 | 5 | 5 | 5 | 5 | 4 | Best current all-around benchmark for editor agent UX. |
| Windsurf | 91 | 5 | 4 | 5 | 4 | 4 | Strongest repo-visible rules and workflow customization reference. |
| Claude Code | 89 | 5 | 4 | 5 | 5 | 2 | Best architecture reference for tool loops, hooks, and subagents. |
| Devin | 88 | 5 | 4 | 5 | 3 | 3 | Strong planning and persistent knowledge reference. |
| Antigravity | 88 | 5 | 4 | 3 | 3 | 5 | Compelling, but confidence is low and details may drift. |
| Zed AI | 86 | 4 | 5 | 4 | 5 | 3 | Best editor-native reference for review and tool permissions. |
| ChatGPT desktop plus Codex | 85 | 4 | 5 | 4 | 5 | 5 | Strong desktop flow around worktrees, terminal, and voice. |
| Replit Agent | 84 | 5 | 5 | 3 | 3 | 5 | Strong cloud app-builder loop with rich checkpoints. |
| Gemini Code Assist | 83 | 4 | 4 | 4 | 3 | 3 | Broad practical IDE surface with good enterprise features. |
| GitHub Copilot coding agent | 82 | 4 | 5 | 4 | 5 | 3 | Best when the workflow ends as GitHub-native PR work. |
| Cline | 81 | 4 | 5 | 3 | 4 | 2 | Clear planning and checkpoint design. |
| Roo Code | 80 | 4 | 3 | 4 | 4 | 2 | Useful reference for mode separation and orchestration. |
| Aider | 74 | 3 | 5 | 2 | 2 | 3 | Git-first CLI benchmark, not a GUI IDE benchmark. |
| Continue | 72 | 3 | 2 | 5 | 5 | 1 | Powerful configuration substrate, weaker polished workflow. |

## Main feature matrix

This is the main comparison table requested for future planning. It mixes external support and Vox effort in one place so implementation decisions can be made row by row instead of tool by tool.

Column abbreviations:

- `Cur` Cursor
- `Win` Windsurf
- `Anti` Antigravity
- `Cla` Claude Code
- `Cod` ChatGPT desktop plus Codex
- `Gem` Gemini Code Assist
- `Cop` GitHub Copilot coding agent
- `Zed` Zed AI
- `Aid` Aider
- `Cli` Cline
- `Roo` Roo Code
- `Rep` Replit Agent
- `Dev` Devin
- `Con` Continue

| Feature | Why developers love it | Cur | Win | Anti | Cla | Cod | Gem | Cop | Zed | Aid | Cli | Roo | Rep | Dev | Con | Vox current state and likely owner | LOC | Diff | Need |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Inline edits and low-latency completion | Highest-frequency productivity loop; this is the feature people touch all day. | S | S | S | L | P | S | S | S | L | P | P | P | L | S | `partial`; [`GhostTextProvider`](../../../apps/editor/vox-vscode/src/inline/GhostTextProvider.ts), [`InlineEditController`](../../../apps/editor/vox-vscode/src/inline/InlineEditController.ts), [`ghost_text.rs`](../../../crates/vox-orchestrator/src/mcp_tools/tools/chat_tools/ghost_text.rs) | 200-800 | medium | critical |
| Agentic multi-file execution | Biggest step-change beyond autocomplete; entire tasks become executable. | S | S | S | S | S | S | S | S | P | S | S | S | S | P | `partial`; [`SidebarProvider`](../../../apps/editor/vox-vscode/src/SidebarProvider.ts), [`VoxMcpClient`](../../../apps/editor/vox-vscode/src/core/VoxMcpClient.ts), [`task_tools.rs`](../../../crates/vox-orchestrator/src/mcp_tools/tools/task_tools.rs) | 800-2500 | high | critical |
| Ask / plan / debug / execute mode separation | Trust rises when reading, planning, and acting are explicit. | S | S | S | S | L | P | P | P | P | S | S | S | S | L | `partial`; [`plan.rs`](../../../crates/vox-orchestrator/src/mcp_tools/tools/chat_tools/plan.rs), [`SidebarProvider`](../../../apps/editor/vox-vscode/src/SidebarProvider.ts) | 200-800 | medium | high |
| Checkpoints, revert, and review UX | Lowers the emotional cost of letting agents move fast. | S | S | P | P | S | S | S | S | S | S | L | S | P | L | `partial`; [`SnapshotProvider`](../../../apps/editor/vox-vscode/src/vcs/SnapshotProvider.ts), [`vcs_tools`](../../../crates/vox-orchestrator/src/mcp_tools/tools/vcs_tools), [`json_vcs_facade`](../../../crates/vox-orchestrator/src/json_vcs_facade.rs) | 800-2500 | high | critical |
| Tool transparency across terminal, browser, diagnostics, and web | Developers want autonomy with visibility. | S | S | S | S | S | P | P | S | P | S | P | S | S | P | `backend-only`; [`tool-registry.canonical.yaml`](../../../contracts/mcp/tool-registry.canonical.yaml), [`VoxMcpClient`](../../../apps/editor/vox-vscode/src/core/VoxMcpClient.ts) | 800-2500 | high | high |
| Subagents, parallelism, and orchestration | Separates serious agent systems from simple assistants. | S | S | S | S | L | L | P | S | N | L | S | S | P | L | `backend-only`; [`task_tools.rs`](../../../crates/vox-orchestrator/src/mcp_tools/tools/task_tools.rs), [`orchestrator`](../../../crates/vox-orchestrator/src/orchestrator), [`AgentController`](../../../apps/editor/vox-vscode/src/agents/AgentController.ts) | 2500-8000 | very high | medium |
| Context targeting, indexing, search, and mentions | Good context controls make AI faster and less error-prone. | S | S | P | P | S | S | S | S | L | P | P | P | S | P | `partial`; [`execution.rs`](../../../crates/vox-search/src/execution.rs), [`SidebarProvider`](../../../apps/editor/vox-vscode/src/SidebarProvider.ts), [`context_lifecycle.rs`](../../../crates/vox-orchestrator/src/context_lifecycle.rs) | 800-2500 | high | critical |
| Rules, memories, workflows, and skills | Turns one-off usefulness into repeatable team speed. | S | S | P | S | S | S | S | S | L | P | S | L | S | S | `partial`; [`handlers_memory.rs`](../../../crates/vox-orchestrator/src/mcp_tools/memory/handlers_memory.rs), [`capability-registry-ssot`](capability-registry-ssot.md), extension preferences and sidebar | 800-2500 | high | high |
| Extensibility via MCP, hooks, custom agents, or custom tools | Advanced teams want AI to plug into existing systems. | S | S | P | S | S | P | S | S | L | S | S | L | L | S | `shipped`; [`tool-registry.canonical.yaml`](../../../contracts/mcp/tool-registry.canonical.yaml), [`capability-registry-ssot`](capability-registry-ssot.md), [`mcpToolRegistry.generated.ts`](../../../apps/editor/vox-vscode/src/core/mcpToolRegistry.generated.ts) | 200-800 | medium | medium |
| Git, PR, and workspace isolation | Important once autonomous edits become common. | S | P | P | S | S | P | S | P | S | L | L | P | P | L | `partial`; [`workspaces.rs`](../../../crates/vox-orchestrator/src/mcp_tools/tools/vcs_tools/workspaces.rs), [`snapshots.rs`](../../../crates/vox-orchestrator/src/mcp_tools/tools/vcs_tools/snapshots.rs) | 2500-8000 | very high | medium |
| Multimodal input and GUI surfaces | Voice, images, visual review, and canvas flows make AI feel like a product. | S | S | S | L | S | P | P | P | P | L | L | S | P | L | `partial`; [`registerOratioSpeechCommands`](../../../apps/editor/vox-vscode/src/speech/registerOratioSpeechCommands.ts), [`VisualEditorPanel`](../../../apps/editor/vox-vscode/src/VisualEditorPanel.ts), [`webview-ui/components`](../../../apps/editor/vox-vscode/webview-ui/src/components) | 200-800 | medium | medium |
| Automated verification, diagnostics, and autofix loops | Developers care most about fast confident closure, not just generation. | S | S | S | S | S | P | P | S | P | P | P | S | S | P | `partial`; compiler and test tools under [`crates/vox-orchestrator/src/mcp_tools/tools`](../../../crates/vox-orchestrator/src/mcp_tools/tools), plus [`plan.rs`](../../../crates/vox-orchestrator/src/mcp_tools/tools/chat_tools/plan.rs) | 200-800 | medium | high |
| Collaboration, tracking, and shareability | Valuable after the core single-user loop is already excellent. | S | P | P | L | P | L | S | L | N | L | L | S | S | L | `partial`; [`AgentController`](../../../apps/editor/vox-vscode/src/agents/AgentController.ts), [`events.rs`](../../../crates/vox-orchestrator/src/events.rs) | 800-2500 | high | medium |

## What the market clearly values most

Across the tools with the strongest documentation and most coherent product direction, the most time-saving features cluster into five groups.

### 1. Fast local interaction loops

These are the features that create daily affection:

- tab or edit prediction,
- targeted inline transforms,
- lightweight explain or fix actions,
- low-friction model switching only when necessary.

This is why Cursor, Gemini, GitHub Copilot, and Zed feel sticky even before the user trusts full agent autonomy.

### 2. Safe autonomy

Developers like autonomy only when rollback is cheap.

The common winning ingredients are:

- visible diffs,
- restore checkpoints,
- approvals or profiles,
- isolated workspaces or worktrees,
- explicit plan-first modes.

This is why Cursor, Zed, Codex, Cline, Replit, and Aider feel safer than raw “chat that edits files.”

### 3. Persistent customization

Rules, memories, workflows, skills, and custom agents matter because they turn “one clever session” into “the way my team works every day.”

Windsurf is especially notable here because it exposes:

- rules,
- `AGENTS.md` inference,
- memories,
- workflows,
- skills.

That stack makes the product feel teachable and cumulative.

### 4. Tool visibility and execution breadth

The modern expectation is that an AI coding system can touch:

- files,
- terminal,
- diagnostics,
- browser or app automation,
- web search,
- external tools through MCP or similar extension systems.

The products that feel most advanced are the ones that treat these surfaces as one coherent workflow rather than a pile of disconnected buttons.

### 5. Context quality

The biggest quality improvements come from:

- explicit file and folder context,
- codebase search and indexing,
- thread or session reuse,
- rules and memory retrieval,
- summaries and context compaction.

This is where Devin, Cursor, Gemini, Windsurf, and Zed are especially instructive.

## Vox baseline: what already exists

The current Vox repo already contains strong building blocks for a serious AI IDE, especially compared with many projects that are still only chat wrappers.

### Extension and GUI surfaces

Important current extension surfaces include:

- [`apps/editor/vox-vscode/src/SidebarProvider.ts`](../../../apps/editor/vox-vscode/src/SidebarProvider.ts)
- [`apps/editor/vox-vscode/src/core/VoxMcpClient.ts`](../../../apps/editor/vox-vscode/src/core/VoxMcpClient.ts)
- [`apps/editor/vox-vscode/src/chat/ChatController.ts`](../../../apps/editor/vox-vscode/src/chat/ChatController.ts)
- [`apps/editor/vox-vscode/webview-ui/src/index.tsx`](../../../apps/editor/vox-vscode/webview-ui/src/index.tsx)
- [`apps/editor/vox-vscode/src/inline/InlineEditController.ts`](../../../apps/editor/vox-vscode/src/inline/InlineEditController.ts)
- [`apps/editor/vox-vscode/src/vcs/SnapshotProvider.ts`](../../../apps/editor/vox-vscode/src/vcs/SnapshotProvider.ts)
- [`apps/editor/vox-vscode/src/agents/AgentController.ts`](../../../apps/editor/vox-vscode/src/agents/AgentController.ts)

These already imply that Vox is trying to be more than a syntax extension. The extension has:

- a sidebar and multi-tab webview,
- chat history and metadata handling,
- composer flows,
- inspector and repo query affordances,
- browser actions,
- project init entry points,
- Ludus and orchestration visibility,
- voice and Oratio commands,
- snapshot and undo surfaces.

### Core MCP and orchestration surfaces

Important core surfaces include:

- [`contracts/mcp/tool-registry.canonical.yaml`](../../../contracts/mcp/tool-registry.canonical.yaml)
- [`crates/vox-orchestrator/src/mcp_tools/tools/chat_tools/plan.rs`](../../../crates/vox-orchestrator/src/mcp_tools/tools/chat_tools/plan.rs)
- [`crates/vox-orchestrator/src/mcp_tools/tools/chat_tools/ghost_text.rs`](../../../crates/vox-orchestrator/src/mcp_tools/tools/chat_tools/ghost_text.rs)
- [`crates/vox-orchestrator/src/mcp_tools/tools/task_tools.rs`](../../../crates/vox-orchestrator/src/mcp_tools/tools/task_tools.rs)
- [`crates/vox-orchestrator/src/context_lifecycle.rs`](../../../crates/vox-orchestrator/src/context_lifecycle.rs)
- [`crates/vox-search/src/execution.rs`](../../../crates/vox-search/src/execution.rs)

This means Vox already has:

- planning and plan-adequacy machinery,
- task submit and orchestration,
- browser tools,
- memory and context stores,
- snapshots and workspaces,
- retrieval and repo search,
- a disciplined MCP registry and capability model.

### Bottom line

The most important practical conclusion is this:

Vox does not need to invent a brand-new architecture before it can feel competitive. It mainly needs to expose and polish what it already has in ways developers immediately understand and trust.

## Recommended implementation order

### Tier 1: highest-value near-term work

1. Review and checkpoint UX
   The backend is already there. Build a better multi-file review flow, visible checkpoint restore, and clearer “accept / reject / regenerate / restore snapshot” interaction model inside the extension.
2. Rules, workflows, and repo-visible customization
   Give users a first-class place in Vox to teach the agent how to work in a repo, much closer to Windsurf rules plus workflows than to a hidden preference pane.
3. Context targeting and search ergonomics
   Add stronger file, folder, and symbol targeting in the UI, and make retrieval more visibly trustworthy.
4. Explicit mode surfaces
   Make ask, plan, execute, and debug feel like first-class modes rather than implicit or scattered affordances.
5. Verification-first loops
   Surface “run checks, summarize failures, fix what the AI just broke” as a core interaction pattern.

### Tier 2: valuable but after Tier 1

1. Better tool transparency and action logs
2. Stronger multimodal polish across Oratio, browser, and webview surfaces
3. Collaborative tracking and shareability

### Tier 3: important but expensive or not yet urgent

1. Full Git/PR/worktree parity
2. Highly visible multi-agent orchestration UX
3. Broad cloud-manager surfaces that duplicate hosted agent platforms

## GUI-specific critique and direction

The request explicitly called out the need for a GUI. Vox already has one, but it does not yet fully convert backend power into perceived capability.

### What should clearly live in the existing VS Code extension and webview

- ask / plan / execute / debug mode switcher,
- visible task queue and queued follow-up messages,
- checkpoint history and rollback buttons,
- rich multi-file diff review,
- context picker for files, folders, diagnostics, snapshots, previous plans, and previous threads,
- rules and workflow management,
- memory inspection and editing where appropriate,
- browser and Oratio actions as first-class side panels rather than hidden commands.

### What likely requires extension plus MCP work

- better agent transcript visibility for tool calls,
- stronger verification loops with test or lint summaries,
- context ranking and suggestion quality,
- more coherent skill and capability browsing.

### What is deep-core and should be justified carefully

- generalized multi-agent orchestration UX,
- remote execution and cloud-manager abstractions,
- Git-native PR generation and review parity,
- anything that would force a large new product surface before the core extension loop is already polished.

## What Vox should not over-prioritize yet

Some features look flashy but are not yet the highest leverage for Vox.

### 1. Competing head-on as a cloud IDE platform

Replit, Devin, Codex, and Antigravity all pull in platform assumptions that go beyond editor UX. Vox should learn from them, but not rush to copy them wholesale.

### 2. Broad external collaboration integrations

Slack, Jira, Linear, Azure Boards, and shared session surfaces matter, but they are second-order value until the single-user workflow is excellent.

### 3. Deep multi-agent theater

Subagents and orchestration are impressive, but exposing them before single-agent trust is nailed can make the product feel noisy rather than powerful.

## Mens implications

Mens should be treated as an amplifier for this roadmap, not as a substitute for product design.

### Best Mens-aligned opportunities

- low-latency completion and edit routing,
- better retrieval ranking and context selection,
- higher-quality voice-to-code,
- future personalization of rules or workflow suggestions,
- evaluation and telemetry loops for plan quality and completion quality.

### Poor Mens-first bets

- training before extension UX is coherent,
- model differentiation before review and rollback feel safe,
- “smart memory” before repo-visible deterministic rules exist.

In short, Mens is more valuable after Vox tightens the product loop around context, review, and rules.

## Final recommendations

If Vox wants the strongest return on implementation effort while staying inside its current architecture:

1. Build a much better review and rollback experience on top of snapshots and composer flows.
2. Create a first-class repo-visible rules and workflows system inside the extension.
3. Improve context targeting, search, and retrieval affordances before chasing more agent complexity.
4. Make plan and ask modes explicit and friendly.
5. Surface verification and autofix loops as part of the normal workflow, not as hidden tools.

If Vox does those well, it will already cover a large portion of what developers most consistently love in modern AI IDEs, without needing to change the Vox language or chase the most expensive hosted-platform features first.


