---
title: "Skill Ecosystem Interop & Model-Awareness Research (2026-06-12)"
description: "Verified audit of Vox's skill system vs. the agentskills.io ecosystem: internal architecture, GUI/chat surfacing, LLM-layer gaps, external format landscape, and a license-verified catalog of bundleable third-party skills."
category: architecture
---

# Skill Ecosystem Interop & Model-Awareness Research (2026-06-12)

Four-agent codebase audit + web ecosystem survey + 103-agent adversarially-verified
deep-research pass (25/25 claims confirmed, 0 refuted). Companion plan:
[`docs/superpowers/plans/2026-06-12-skill-ecosystem-interop-and-awareness.md`](../../superpowers/plans/2026-06-12-skill-ecosystem-interop-and-awareness.md).

## 1. Executive summary

Vox already has a **production-grade skill spine** (six crates, SKILL.md format, CI
gates, 9 first-party skill plugins, sandbox runtimes, GUI marketplace surface). The
audit found five real gaps between that spine and the goals (ecosystem interop, GUI
/slash chat integration, MENS/OpenRouter model awareness, SSOT):

| # | Gap | Severity | Status |
|---|-----|----------|--------|
| G1 | Frontmatter parser was **TOML-only**; agentskills.io mandates **YAML** — no real-world skill (superpowers, anthropics/skills, Cursor, Codex) could be imported | Blocker for interop | **FIXED** — commit `3df43e7316` (YAML parse with TOML legacy fallback, 16/16 tests) |
| G2 | Discovery only walks `Plugin.toml` plugin roots; bare `SKILL.md` skill directories (the universal convention: `.claude/skills/`, `.agents/skills/`) are invisible | Blocker for interop | Planned (Track A) |
| G3 | Models are never told skills exist: `build_system_prompt()` injects VOX.md/memory/policy riders but **no skill catalog**, and there is no tier-2 "load skill body" tool. MENS path is prompt-only (no tool-calling), OpenRouter path has tools but nothing skill-shaped to call | Blocker for model awareness | Planned (Track B) |
| G4 | GUI chat (Loquela) slash commands are **8 hardcoded entries** (`Loquela.tsx:24-33`); skills don't appear as `/skill-name`, don't appear in Cmd+K palette results | High | Planned (Track C) |
| G5 | No skill usage/performance tracking — permissions, dependencies, version-conflict logic also unenforced | Medium | Planned (Track B4 + later phases) |

The strategic call this research supports: **converge on the open agentskills.io
SKILL.md contract as the single source of truth**, discover from the vendor-neutral
directories every major harness now reads, and replicate the spec's three-tier
progressive disclosure for *all* models (catalog injection for prompt-only MENS,
catalog + `vox_skill_use` tool for tool-calling OpenRouter models).

## 2. Internal audit — what exists today

### 2.1 Skill system core (verified file:line)

- **Crates:** `vox-skills` (marketplace API, sandbox, hooks), `vox-plugin-host`
  (discovery/loading/registry, `skill_parser.rs`), `vox-plugin-types`
  (`SkillManifest`/`SkillCategory`/`SkillPermission` SSOT), `vox-plugin-api` (ABI),
  `vox-skill-runtime` (WASM-vs-container runtime trait), `vox-plugin-catalog`
  (first-party catalog SSOT at `crates/vox-plugin-catalog/catalog.toml`).
- **Format:** `<name>.skill.md` with frontmatter → `VoxSkillBundle`
  (`crates/vox-plugin-host/src/skill_bundle.rs:27-47`). Parser:
  `crates/vox-plugin-host/src/skill_parser.rs` — now TOML *and* YAML (G1 fix).
  Vox-specific fields ride in the spec's `metadata` map as `vox-*` keys
  (`vox-id`, `vox-version`, `vox-category`, `vox-tools`, `vox-permissions`, …).
- **Discovery:** `crates/vox-plugin-host/src/discover.rs:16-109` walks an install
  root for `Plugin.toml`, eagerly reads referenced SKILL.md, installs into the
  in-memory `SkillRegistry` (`skill_registry.rs:131-147`, optional DB backend).
- **CI gates:** `vox ci agentskills-compliance`
  (`crates/vox-cli/src/commands/ci/agentskills_compliance.rs` — name regex
  `^[a-z0-9][a-z0-9-]{0,63}$`, description ≤ 1024, name==crate short-name) and
  `vox ci plugin-skill-parity` (SKILL.md `vox-tools` ⇄ `Plugin.toml` `exposes`,
  SKILL.md is authoritative, `--write` autocorrects).
- **CLI:** `vox skill list|install|uninstall|search|info|create|run|eval-task|promote|context-assemble|discover`
  (`crates/vox-cli/src/commands/extras/skill_cmd.rs`, dispatch to `extras/ars/*`,
  feature `ars`). `ars::discover` walks the workspace (depth 6) for `.skill.md`.
- **First-party skills (9):** `crates/vox-plugin-skill-{compiler,git,memory,orchestrator,rag,testing,testing-validate,v0}`
  + built-in superpowers subset at `crates/vox-skills/skills/superpowers/`
  (writing-plans, subagent-driven-development, systematic-debugging,
  test-driven-development).
- **Runtime:** registration + prompt-injection-ready bodies exist; sandbox execution
  (`vox-skills/src/sandbox/`, WASM/container via `vox-skill-runtime`) is designed
  and reachable via `vox skill run`, but **the orchestrator does not auto-invoke
  skills**. Permissions are declared, not enforced. `dependencies` unused.
  MCP CRUD tools exist: `vox_skill_{list,search,info,install,uninstall,parse}`
  (`contracts/mcp/tool-registry.canonical.yaml`; `vox-mcp-registry/src/lib.rs:40-48`).

### 2.2 GUI & chat surfacing

- **Skills surface:** `crates/vox-gui/ui/src/components/surfaces/SkillsPlugins/SkillsPluginsView.tsx`
  (installed + marketplace tabs, routed through MCP `vox_skill_*` tools; install
  passes the daemon HITL approval gate). Registered in
  `contracts/gui/surface-registry.v1.yaml`.
- **Chat composer (Loquela):** `crates/vox-gui/ui/src/components/surfaces/Loquela/Loquela.tsx`
  — modal composer with mode (plan/act/verify), runtime tier (live `listModels`),
  **skill selector dropdown** (default `auto`), budget/doubt/stream toggles, voice.
  Slash commands are **hardcoded** (`LQ_SLASH`, lines 24-33; parse at line 145:
  `trimmed.startsWith("/")`); the backend receives slash text verbatim — there is
  no server-side slash registry.
- **Command palette:** `CommandPalette.tsx` searches agents + backend
  (`vox_search_query`); result locator kinds include `'skill'` but skills are not
  fed into results today.
- **Transport:** Tauri `invoke` + MCP tool bridge + HTTP gateway
  (`crates/vox-orchestrator-mcp/src/http_gateway/`, REST `/api/v2/*`, WS `/v1/ws`,
  default port 3921).

### 2.3 LLM layer (the model-awareness seam)

- **Facade:** `vox_actor_runtime::llm` — `LlmConfig` has `tools: Option<Vec<LlmToolDef>>`
  + `tool_choice` (`types.rs:32-75`), streaming, embeddings. **No system-prompt
  field** — callers assemble prompts.
- **Egress paths (dual, convergence to `vox-llm-egress` still unbuilt):**
  1. `vox-actor-runtime` → OpenRouter (`chat.rs`, `wire.rs`; tools serialized;
     key from `vox_secrets::SecretId::OpenRouterApiKey`).
  2. `vox-orchestrator-mcp/src/llm_bridge/provider_adapter.rs` — adapters for
     Google/Ollama/Anthropic-native/OpenAI-compat/**VoxLocal (MENS)**. The
     `InferRequest` envelope *does* carry `system_prompt` + `tools`.
     **MENS (`llm_bridge/mod.rs:46-114`, `VOX_LOCAL_ENDPOINT`, default
     `127.0.0.1:7863/generate`) is prompt-only — no tool-calling.**
- **Model selection:** `vox-orchestrator/src/models/{select,registry,autonomic}.rs`
  with capability flags incl. `supports_tool_use` (`spec.rs:38-85`) — the natural
  predicate for "tool-based vs prompt-injected" skill delivery per model.
- **Prompt assembly:** `vox-orchestrator-mcp/src/chat_tools/mod.rs:54-144`
  (`build_system_prompt`) — VOX.md, MEMORY.md, environment, anti-laziness,
  completion-policy, day-stable temporal context (deliberately cache-friendly),
  budget signal, Socrates rider, operating mode. **No skill catalog section.**
  The skill registry is *already reachable from the orchestrator*
  (`vox-orchestrator/src/monitor.rs` holds `Arc<vox_skills::SkillRegistry>`).

## 3. External ecosystem — verified landscape (mid-2026)

The agentskills.io SKILL.md format is the converged standard, adopted by **41
clients** on the official showcase (OpenAI Codex and GitHub Copilot independently
verified against first-party docs). Spec: directory + `SKILL.md`, YAML frontmatter,
required fields exactly `name` (1-64 chars, `[a-z0-9-]`, **must match directory
name**) and `description` (1-1024); optional `license`, `compatibility`,
`metadata` (string→string), `allowed-tools` (experimental). Three-tier progressive
disclosure is normative: (1) ~100 tokens name+description at startup, (2) body
<5000 tokens on activation, (3) bundled `scripts/`/`references/`/`assets/` on
demand.

| Ecosystem | Directories read | Trigger |
|---|---|---|
| Claude Code | `~/.claude/skills`, `.claude/skills` (+nested), plugins, enterprise | Skill tool by description; `/skill-name` |
| Cursor ≥2.4 | `.cursor/skills`, `.agents/skills`, `~/…`, **compat: `.claude/skills`, `.codex/skills`** | auto by description; `/` menu; `paths` globs |
| OpenAI Codex | `.agents/skills` (cwd→root), `~/.agents/skills`, `/etc/codex/skills` | implicit; `/skills`, `$skill-name` |
| GitHub Copilot | `.github/skills`, `.claude/skills`, `.agents/skills`; `~/.copilot/skills`, `~/.agents/skills` | auto by description |
| Gemini CLI | `~/.gemini/skills`, `.gemini/skills` (+ `.agents/skills` aliases) | auto by description |
| Windsurf | `.windsurf/skills` | auto by relevance |
| OpenCode | `~/.config/opencode/skills`, `~/.claude/skills`, `~/.agents/skills`, repo | auto by description |
| Amp | `.agents/skills`, `~/.config/agents/skills`, `.claude/skills` compat (+ skill-bundled `mcp.json`) | lazy body load |
| AGENTS.md | repo root + nested (Linux Foundation stewarded) | always-on ambient (complementary, not a skill mechanism) |
| MCP | SEP-2640 draft: skills over Resources, `skill://<name>/SKILL.md`, `skill://index.json` | host-side disclosure; format defers to agentskills.io |

Implications: **`.agents/skills/` is the vendor-neutral convention; `.claude/skills/`
is the most widely honored compatibility path.** Vox should read both (plus its own
`.vox/skills/`) and *publish* bundled skills in spec form so every other harness
picks them up from a Vox repo unchanged. Non-Claude models are made skill-aware via
two documented patterns: catalog injection into the system prompt (works for any
model, incl. MENS) and tool-based on-demand body loading (needs tool-calling —
gate on `supports_tool_use`).

## 4. License-verified bundling catalog (deep-research, 25/25 claims confirmed)

**Bundleable (~26 skills):**

- **obra/superpowers — MIT (LICENSE verified at 3 levels; active, pushed 2026-06-11).**
  14 process skills: test-driven-development, systematic-debugging, writing-plans,
  executing-plans, verification-before-completion, requesting-code-review,
  receiving-code-review, brainstorming, subagent-driven-development,
  dispatching-parallel-agents, using-git-worktrees, finishing-a-development-branch,
  writing-skills, using-superpowers. (Four are already vendored at
  `crates/vox-skills/skills/superpowers/` — refresh + complete the set.)
- **obra/superpowers-lab — MIT.** 4 experimental: finding-duplicate-functions,
  mcp-cli, using-tmux-for-interactive-commands, windows-vm (orchestrates
  externally-licensed components; skill files themselves MIT).
- **obra/superpowers-skills — MIT but ARCHIVED 2025-10-27** (frozen snapshot, 8
  categories; mine selectively, point at live repo).
- **anthropics/skills — Apache-2.0 subset only:** claude-api skill, mcp-builder,
  skill-creator, creative/design/enterprise example sets (~14 of ~18 skills).

**NOT bundleable (license landmines):**

- anthropics/skills **docx / pdf / pptx / xlsx** — source-available, proprietary;
  LICENSE.txt explicitly prohibits distribution/sublicensing/derivatives.
- obra/superpowers-developing-for-claude-code — README asserts MIT but **no LICENSE
  file exists** (GitHub API `license: null`); needs upstream resolution first.
- General rule (spec-confirmed): the frontmatter `license` field is optional and
  free-form — **repo-level LICENSE files are the only acceptable evidence**, and
  the bundling pipeline must record per-skill provenance + license.

Open questions from research: awesome-claude-skills lists, Vercel open-agents, and
skillport/openskills registries were *not* verified (no surviving claims) — future
sourcing passes, not blockers.

## 5. Recommended architecture (consumed by the plan)

1. **SSOT stays where it is:** `SkillManifest` in `vox-plugin-types`, parser in
   `vox-plugin-host`, registry in `vox-skills`, first-party catalog in
   `vox-plugin-catalog/catalog.toml`. Interop is additive — spec YAML is the
   exchange format; `metadata.vox-*` carries Vox extensions losslessly.
2. **Discovery roots (Track A):** workspace `.vox/skills` → `.agents/skills` →
   `.claude/skills`; user `~/.vox/skills` → `~/.agents/skills` → `~/.claude/skills`;
   plus the bundled set. First-wins dedup by id in that order.
3. **Model awareness (Track B):** tier-1 catalog section in `build_system_prompt`
   (day-stable, alphabetical, capped — preserves prompt-prefix caching); tier-2
   `vox_skill_use` MCP tool for tool-calling models; direct body injection for an
   explicitly selected skill (works on MENS); activation telemetry for performance
   tracking keyed by (skill, model, task-category, outcome).
4. **GUI (Track C):** Loquela slash menu = hardcoded verbs ∪ `/{skill-name}` from
   the live registry; palette feeds skills into results; SkillsPlugins shows
   discovered-but-uninstalled skills with one-click install.
5. **Distribution (later):** MCP SEP-2640 `skill://` resources once merged;
   registry HTTP bridge (`vox-skills::registry_api`) stays feature-gated.
