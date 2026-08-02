---
title: "Skill Ecosystem Audit — All 50 Skill Files, Read in Full (2026-08-01)"
description: "Per-file findings across every SKILL.md and *.skill.md in the repo (Claude-format vendored bundle, Vox-native superpowers ports, Vox-native plugin skills), plus architecture-level gaps in selection, review, graphify wiring, and GUI surfacing that feed the improvement roadmap."
category: "architecture"
status: "current"
training_eligible: false
---

# Skill Ecosystem Audit — All 50 Skill Files, Read in Full (2026-08-01)

> **Provenance.** Four parallel research agents read the full content of every skill file in
> the repo, the 18 prior skill-ecosystem docs, and the relevant selection/GUI/graphify code.
> This doc consolidates their findings. No file was summarized from its filename alone.

Companion design spec: [`../../superpowers/specs/2026-08-01-skill-ecosystem-improvements-design.md`](../../superpowers/specs/2026-08-01-skill-ecosystem-improvements-design.md).
Prior art this audit builds on and does not repeat: [`skill-discovery-and-induction-research-2026-07-30.md`](skill-discovery-and-induction-research-2026-07-30.md), [`skill-marketplace-security-and-provenance-research-2026-07-30.md`](skill-marketplace-security-and-provenance-research-2026-07-30.md), [`skill-registry-trust-and-curation-research-2026-07-30.md`](skill-registry-trust-and-curation-research-2026-07-30.md), [`skill-code-marketplace-research-and-audit-2026-06-18.md`](skill-code-marketplace-research-and-audit-2026-06-18.md), [`skill-ecosystem-interop-research-2026-06-12.md`](skill-ecosystem-interop-research-2026-06-12.md).

## 0. Scale and normative baseline

**50 skill files, 49 distinct** (one is a byte-identical duplicate — see §2.4):

| Location | Count | Format |
|---|---|---|
| `assets/skills/**/SKILL.md` | 27 | agentskills.io YAML, vendored |
| `crates/vox-skills/skills/**/*.skill.md` | 14 | mixed (see below) |
| `crates/vox-plugin-skill-*/*.skill.md` | 8 | legacy `[metadata]`-nested TOML |
| `crates/vox-plugin-populi-mesh/populi.skill.md` | 1 | legacy `[metadata]`-nested TOML |

**AGENTS.md §"Agent Skills (Required, SSOT)" already settles two open questions this audit
would otherwise have to adjudicate:**

1. **One canonical format.** "New skills MUST use spec-compliant YAML frontmatter (`name`
   matching the directory name, `description` 1–1024 chars); Vox extensions ride in `metadata`
   `vox-*` keys." The parser SSOT is `vox-plugin-host::skill_parser` (YAML + **legacy** TOML —
   TOML is explicitly the back-compat path, not a co-equal option).
2. **Search goes through `vox-search`.** "Richer skill retrieval... MUST be built on the
   existing `vox-search` hybrid stack (tantivy lexical + semantic indexer + RRF fusion)...
   **not a bespoke skill index.**"

Both are violated today by code already in the tree (§2.4, §3.1). These aren't judgment calls
for the roadmap to make — they're policy the roadmap has to bring the codebase into compliance
with.

---

## 1. Claude-format skills (`assets/skills/`, 27 files)

Vendored, not hand-authored, from two pinned upstream repos declared in
`assets/skills/SOURCES.toml`: **anthropics/skills** (12 skills, Apache-2.0) and
**obra/superpowers** (14 skills, MIT). `vox-graph` is the one hand-authored exception — no
`SOURCES.toml` entry, Vox-specific. Sync is manual (`scripts/vendor-skills.vox`), not
auto-updating.

| Skill | Lines | Purpose | Portability to Vox Axis / MENS | Flags |
|---|---|---|---|---|
| brainstorming | 164 | Pre-implementation design dialogue | Needs Skill-tool + TodoWrite equivalents | Ships `spec-document-reviewer-prompt.md`; nothing references it |
| writing-plans | 152 | Spec → bite-sized task plan | Same | Ships `plan-document-reviewer-prompt.md`; nothing references it |
| executing-plans | 70 | Execute a plan in a fresh session | TodoWrite | Thin; mostly a router |
| subagent-driven-development | 277 | Per-task subagent + two-stage review | Task tool, TodoWrite | Overlaps with dispatching-parallel-agents by design (documented alternates) |
| dispatching-parallel-agents | 182 | Fan out independent work | Task tool | — |
| systematic-debugging | 296 | Root-cause-first debugging | — (pure discipline, portable) | Ships `CREATION-LOG.md` — an authoring log, not a runtime reference; shouldn't ship |
| test-driven-development | 371 | Red-green-refactor | — (portable) | — |
| requesting-code-review | 105 | Dispatch `superpowers:code-reviewer` subagent | Named agent type doesn't exist outside CC | Review logic lives in an external agent definition, not this file |
| receiving-code-review | 213 | Evaluate/act on review feedback | — (portable) | — |
| using-git-worktrees | 218 | Isolated worktree setup | POSIX bash embedded in body | Conflicts with this repo's own no-new-`.sh`/VoxScript-only policy |
| finishing-a-development-branch | 200 | Merge/PR/keep/discard menu | `gh` CLI | — |
| verification-before-completion | 139 | Evidence before completion claims | — (portable) | — |
| skill-creator | 485 | Build/eval/iterate on skills | Heavy CC/claude.ai coupling (`present_files`, TodoWrite, Python eval scripts) | Least portable file in the set |
| writing-skills | 655 | TDD-for-skills methodology | `@`-link syntax is CC-specific | Longest file; all its cross-refs resolve (not broken) |
| **vox-graph** | 42 | Graph-first search over Rust query engine | **Already Vox-native** — MCP tool names only | Calls `vox_search_query`/`vox_discover`/`vox_search_path`/`vox_search_status`; cross-check against §3.2 — these may be stale post-rename |
| frontend-design | 55 | Aesthetic direction prose | — (portable) | — |
| canvas-design | 129 | Poster/art via philosophy-doc template | claude.ai-artifact-flavored | Near-duplicate template structure to algorithmic-art |
| algorithmic-art | 404 | p5.js generative art | References present, resolve fine | Near-duplicate template structure to canvas-design |
| theme-factory | 59 | Apply one of 10 artifact themes | claude.ai-artifact-oriented | — |
| brand-guidelines | 73 | Anthropic-branded deck styling | python-pptx specific | Low relevance to this repo; candidate for drop |
| web-artifacts-builder | 73 | React/Tailwind bundling via shell scripts | Node/Vite toolchain baked in | Not portable |
| webapp-testing | 95 | Playwright via Python helpers | Python-specific | — |
| mcp-builder | 236 | Build MCP servers (TS/Python) | WebFetch-heavy, mostly doc pointers | Portable |
| slack-gif-creator | 254 | Animated GIFs for Slack | Python/PIL | Low relevance to this repo |
| internal-comms | 32 | Status-report/newsletter templates | — | Thin, generic, low relevance |
| claude-api | 356 (+~65 ref files) | Claude/Anthropic SDK reference | Deeply Claude-specific by design | **Stale**: pricing/model table cached 2026-06-04, ~2 months old; model IDs drift fastest of anything in the set |
| using-superpowers | 117 | Meta-skill: mandates skill invocation | Explicitly CC-tool-named (`Skill`, `TodoWrite`, `Task`) | Ships `references/{codex,copilot,gemini}-tools.md` — **no `vox-axis-tools.md`**, the direct blocker to running this skill set under MENS |

**No standalone "code review" skill and no "review a plan/spec" skill exist in this set.**
`requesting-code-review`/`receiving-code-review` are a self-review-before-commit pair, not peer
or plan review.

## 2. Vox-native skills (`crates/vox-skills/skills/`, 14 files)

| Skill | Lines | Purpose | Frontmatter dialect |
|---|---|---|---|
| populi.skill.md | 34 | Mens worker labels ↔ orchestrator routing hints | Flat TOML (`id`, `name`, `tools[]`, `permissions[]` top-level) |
| superpowers/antigravity-pipeline | 63 | Claude→Gemini delegation loop (author→execute→review→merge-gate) | Plain YAML (`name`+`description` only) |
| superpowers/brainstorming | 40 | Design-decision gate | Plain YAML |
| superpowers/deep-research | 41 | Adversarially-verified research report | Plain YAML |
| superpowers/delegate-gemini | 53 | Single-task Gemini/agy delegation | Plain YAML |
| superpowers/dispatching-parallel-agents | 39 | Decompose+dispatch concurrent work | Plain YAML |
| superpowers/executing-plans | 71 | Execute a plan with checkpoints | Plain YAML |
| superpowers/requesting-code-review | 28 | Self-review checklist pre-commit | Plain YAML |
| superpowers/research | 33 | Lightweight web research | Plain YAML |
| superpowers/subagent-driven-development | 278 | Fresh-subagent-per-task + two-stage review | Plain YAML; embeds Graphviz `dot` diagrams |
| superpowers/test-driven-development | 372 | Strict TDD | Plain YAML |
| superpowers/using-git-worktrees | 44 | Isolate work in worktrees | Plain YAML |
| superpowers/verification-before-completion | 33 | Evidence-before-assertion | Plain YAML |
| superpowers/writing-plans | 152 | Bite-sized, no-placeholder plans | Plain YAML |

### 2.1 Format split

Three dialects coexist in the Vox-native set alone: (a) flat TOML (`populi.skill.md`), (b)
plain YAML matching the Claude Code convention almost verbatim (all 13 `superpowers/*` ports),
(c) `[metadata]`-nested TOML with `vox-*`-prefixed keys (all 9 plugin skills, §3). Per AGENTS.md
§0, only YAML-with-`metadata.vox-*`-keys is canonical; TOML (both flat and nested) is legacy.

### 2.2 Vox-native tool/permission fields

Vox-native manifests declare a machine-readable `tools` array (MCP tool IDs, `vox_snake_case`
convention) and a `permissions` array — fields the Claude-format `SKILL.md` frontmatter has no
first-class equivalent for. This is a real capability gap in the vendored format, not a defect;
worth preserving when consolidating onto the canonical dialect.

### 2.3 Graphify — read/query side is Rust and wired; construction side is not

- Rust crate: **`vox-graph-reader`** (`GraphifyReaderError`) — BFS/shortest-path, Leiden
  clustering (`leiden-rs`), tree-sitter AST parsing (Rust/TS/Python). Mid-rename from
  `vox-graphify-reader` per `docs/superpowers/plans/2026-06-27-vox-graph-rename-and-manifest-plan-vg1.md`.
- MCP surface: `crates/vox-orchestrator-mcp/src/graph_tools.rs` exposes
  `vox_graphify_status`, `vox_graphify_search`, `vox_graphify_query`,
  `vox_graphify_callers/callees/path/compare/rebuild`, etc. — **still named `vox_graphify_*`**.
- CLI surface: `vox graphify {status,query,ingest,rebuild,coverage,index,refresh,gc,crate-map,...}`
  (`crates/vox-cli/src/commands/graphify/mod.rs`, 1506 lines).
- The Claude-format `vox-graph/SKILL.md` (§1) instructs agents to call
  **`vox_search_query`/`vox_discover`/`vox_search_path`/`vox_search_status`** — the *renamed*
  names, not the ones the MCP layer currently exposes. **This is very likely a live bug**: the
  skill either calls tools that don't exist yet, or the MCP layer has aliases not surfaced to
  this audit. Needs a direct verification pass in Phase 1/2, not an assumption either way.
- Construction (parsing→extraction→Leiden clustering as a *pipeline*, as opposed to the
  Rust reader library) is still Python: `scripts/graphify-refresh.vox → rebuild_full_graph.py
  → python -m graphify`, using PyPI `graphifyy` + `networkx` + `graspologic`, with multimodal
  extractors calling out via OpenRouter (`~/.graphify/providers.json`). A Rust-native
  construction roadmap exists in `docs/src/architecture/graphify-python-free-findings-2026.md`
  but is `status: research`, not executed.
- **Not wired as a Vox-native skill at all.** Grepping `crates/vox-skills` and
  `crates/vox-plugin-skill-*` for "graphify" returns nothing — MENS/Vox Axis can reach graphify
  only via raw MCP tool calls or the CLI, with no `SKILL.md`-level description/trigger telling
  a chat-routed model it exists or when to prefer it over grep. This is the direct gap behind
  the "make sure graphify exists as a skill in both harnesses" ask.

### 2.4 Duplication

`populi.skill.md` exists **twice**, byte-different only in frontmatter shape, identical body:
`crates/vox-skills/skills/populi.skill.md` (flat TOML) and
`crates/vox-plugin-populi-mesh/populi.skill.md` (`[metadata]`-nested TOML). Two SSOTs for one
skill — pick one location, delete the other.

### 2.5 Review gate

No "review a plan/spec" skill and no general code-review skill exist here either.
`superpowers/requesting-code-review.skill.md` is, like its Claude-format counterpart, a
self-review checklist — not peer or plan review.

## 3. Vox-native plugin skills (`crates/vox-plugin-skill-*/`, 8 files + populi-mesh, 9 total)

| Skill | Lines | Purpose |
|---|---|---|
| compiler.skill.md | 29 | Compile/build-check the workspace (`vox_validate_file`, `vox_check_workspace`) |
| git.skill.md | 32 | Git workflow + custom file-ownership/claim locking |
| memory.skill.md | 33 | Persistent agent memory: facts, logs, knowledge graph, sessions |
| orchestrator.skill.md | 40 | Multi-agent task orchestration (submit/status/rebalance/budget) |
| rag.skill.md | 31 | Visual/multimodal RAG proxy to external VLM backend |
| testing.skill.md | 29 | Run tests/coverage for Vox crates |
| testing.validate.skill.md | 51 | 5-stage AI delivery gate with self-healing loop (max 5 iterations) |
| v0.skill.md | 39 | UI components via v0.dev API (or local stub) |
| populi-mesh/populi.skill.md | 36 | Duplicate of §2's populi.skill.md (see §2.4) |

### 3.1 Selection mechanism — policy violation already in the tree

`crates/vox-orchestrator-mcp/src/chat_tools/skill_catalog.rs` implements a genuine three-tier
progressive-disclosure system, functionally equivalent to Claude Code's deferred-tool/ToolSearch
pattern:

- **Tier 1**: `render_skill_catalog(&skill_entries, 64)` — name+description only, capped at 64,
  sorted by `reliability_scores` descending with alphabetical fallback (deterministic for
  prompt-cache stability). Called at `chat_tools/mod.rs:168`.
- **Tier 2**: `vox_skill_use` MCP tool (`skills_tools.rs`) — loads the full body on demand.
- **Pinned**: `render_pinned_skill` — full body injected directly (32KB cap) when a skill is
  explicitly selected, so prompt-only models like MENS get it without a tool round-trip.

This is the right shape. The problem: **selection within the cap is alphabetical/reliability-sort
truncation, not relevance ranking** — confirmed independently by the 2026-07-30 research
("right cap, wrong mechanism") and by direct code reading this session.

The nearest built relevance mechanism is `skill_search_index.rs`, an **in-memory BM25 index**
(`SkillSearchIndex`) backing `vox_skill_search`. This is a **bespoke skill index**, which
AGENTS.md §0 explicitly prohibits ("richer skill retrieval... MUST be built on the existing
`vox-search` hybrid stack... not a bespoke skill index"). Two independent findings converge on
the same fix: retire the bespoke BM25 index and route both `vox_skill_search` and (new)
relevance-ranked tier-1 selection through `vox-search`'s tantivy+semantic+RRF stack.

No other `MAX_SKILLS`-style constant exists anywhere in `vox-skills`, `vox-skill-discovery`, or
`vox-skill-runtime`. `skill_list` (full `SkillInfo`, no cap) and the SEP-2640-style
`skill://index.json` resource (`skills_resources.rs`) are both uncapped — only the injected
chat-prompt catalog is capped.

### 3.2 Review-gate history

`vox-skill-review` (per `docs/superpowers/plans/2026-06-18-skill-review-gate.md`) — a
pre-publish gate checking frontmatter completeness, stub detection, MCP-SSOT drift, and dedup
against installed skills, with a `Pass`/`NeedsHuman` verdict — **was built once** (commits like
`6e2a055621` exist in git history) **and is now orphaned**: dangling, unreachable from any
branch, not merged to `origin/main`, and `crates/vox-skill-review` does not exist in the current
tree. This is a redo, not a from-scratch build — the prior commits are a reference
implementation to recover logic from, not a clean base to rebase onto.

### 3.3 GUI surface

`crates/vox-gui/ui/src/components/surfaces/SkillsPlugins/SkillsPluginsView.tsx` (648 lines) — 3
tabs: Installed / Marketplace / Discovered, routed through `invoke_mcp_tool` (HITL-gated).
Supporting: `SkillDetailPanel.tsx`, `hooks/useInstalledSkills.ts`, `lib/installedSkills.ts`,
`lib/federatedSearchIndex.ts`, omnibar/palette wiring (`paletteSources.ts`, `Sidebar`). **No
admin/audit panel** for per-skill permissions, trust level, or `skill_reliability` telemetry —
none of the 18 prior docs describe one either; the closest planned artifact was a "Discovered"
tab (built) and an unscoped telemetry-scoreboard idea (never became a GUI ticket).

## 4. Security/provenance research — status (context, not new research)

Both dated 2026-07-30, both verified via adversarial deep-research, both research-only (no
code):

- **Marketplace security/provenance**: review-time checks alone don't stop attacks that target
  the gap between what's reviewed and what runs (Tool Poisoning, Rug Pull, ATPA all documented
  PoCs; even OpenAI's real human+automated GPT Store review leaves 95%+ of listings
  exploitable). Recommended fix: Sigstore-style provenance binding + mandatory re-verification
  on any post-promotion body change, on top of the sandboxing Vox already has
  (`vox-skill-runtime`'s WASM/container tiers).
- **Registry trust/curation**: the official MCP Registry solves namespace ownership only
  (reverse-DNS/GitHub/DNS verification) and deliberately never delists for a disclosed
  vulnerability. Recommendation: Vox should copy the namespace model but be stricter — since
  Vox controls the whole stack (unlike the decentralized MCP registry), a `skill_reliability`
  failure signal should force `deprecated` status automatically, not just advise.

## 5. Summary of findings feeding the roadmap

Counted, not padded — this is what 50 files and the surrounding code actually contain:

- 1 duplicated skill file (2 locations, 2 dialects, identical body) → consolidate
- 3 competing frontmatter dialects in the Vox-native set vs. 1 canonical dialect per AGENTS.md → migrate 22 files (14 in §2 minus the already-canonical superpowers ports' near-miss, 9 in §3)
- 4 orphaned/dead files shipping with no referrer (`spec-document-reviewer-prompt.md`, `plan-document-reviewer-prompt.md`, `CREATION-LOG.md`, and the dangling `vox-skill-review` crate history)
- 1 confirmed-stale reference doc (`claude-api` pricing/model table)
- 1 missing portability file (`vox-axis-tools.md`) blocking 13+14 superpowers skills from running under MENS
- 1 likely-live bug (graphify tool-name mismatch between `vox-graph/SKILL.md` and `graph_tools.rs`)
- 1 graphify skill-wiring gap (construction/read engine exists, no Vox-native skill exposes it)
- 2 missing skills across both formats (plan/spec review, general code review)
- 1 policy-violating bespoke index (`skill_search_index.rs`'s BM25 engine) to retire in favor of `vox-search`
- 1 naive-selection mechanism (alphabetical/reliability truncation at cap-64) to replace with relevance ranking
- 1 orphaned review-gate crate to rebuild
- 2 unactioned research docs (security/provenance, registry trust) to turn into an executable gate
- 1 missing GUI surface (permissions/trust-level/reliability admin panel)

Individually itemized, per-file remediation steps (frontmatter migration line-items, dead-file
deletions, specific cross-reference fixes) are enumerated in the companion design spec's phase
task lists, where they're actionable rather than merely catalogued.
