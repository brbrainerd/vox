---
title: "Agent Instruction Files: Per-Platform Enhancements + Token-Waste & Commit-Churn Audit (June 2026)"
description: "What each agent platform (Claude Code, Antigravity/Gemini 3.5 Flash, GitHub Copilot, Cursor) supports for instruction files as of mid-2026, the capabilities our overlay set under-uses, and a git-history audit of token waste and commit-message-vs-diff drift with concrete remediations."
category: "Architecture"
---

# Agent Instruction Files: Per-Platform Enhancements + Token-Waste & Commit-Churn Audit

> **Status:** Research + internal audit (no code changes). Feeds a follow-on hygiene/enhancement plan.
> **Date:** 2026-06-18
> **Method:** Targeted primary-source verification (direct fetches of Cursor docs, GitHub Copilot docs, Google Antigravity codelab) + live git-history audit of this repo. A broader deep-research fan-out was attempted but its verification phase was **rate-limited to inconclusive**; only directly-fetched facts are marked verified below.

## 0. Confidence legend
- **[V]** verified this session against a primary source (direct fetch).
- **[S]** sourced from a primary doc but not independently re-verified (deep-research rate-limited); high plausibility, confirm before relying.
- **[A]** from this session's live git audit of the repo (high reliability for "what our history shows").

---

## 1. Headline finding: all four tools now natively support `AGENTS.md`

This **validates the repo's hub-and-spoke instruction architecture** and changes what the overlays are *for*:

- **Cursor [V]:** "AGENTS.md is a simple markdown file for defining agent instructions. Place it in your project root as an alternative to `.cursor/rules`." Supports **nested AGENTS.md** across subdirectories.
- **GitHub Copilot [V]:** "the nearest AGENTS.md file in the directory tree will take precedence"; `CLAUDE.md`/`GEMINI.md` accepted as root alternatives.
- **Antigravity [S, prior]:** reads `AGENTS.md` + `GEMINI.md`.
- **Claude Code [S]:** reads `CLAUDE.md` (enterprise/project/user levels) and `AGENTS.md`.

**Implication:** the overlays (`GEMINI.md`, `CLAUDE.md`, `.github/copilot-instructions.md`, `.cursor/rules/`) should hold **only tool-unique capabilities** — not restated policy. Policy belongs in `AGENTS.md`, which every tool reads. (Our recent hygiene pass already trimmed CLAUDE.md toward this; the same trimming applies to the others.)

---

## 2. Per-platform capabilities and what our overlays under-use

### 2.1 GitHub Copilot [V]
**Supports:** (a) repo-wide `.github/copilot-instructions.md`; (b) **path-specific** `*.instructions.md` files under `.github/instructions/`, each with frontmatter `applyTo: '<glob>'` (e.g. `applyTo: 'app/models/**/*.rb'`) and an optional `excludeAgent: code-review|cloud-agent`; (c) native `AGENTS.md`.

**GAP (we under-use):** we have a single flat `.github/copilot-instructions.md`. We do **not** use path-specific `.github/instructions/*.instructions.md`. High-value scopings for this repo:
- `applyTo: 'crates/**/*.rs'` → Rust-only invariants (no `std::env::var` → `vox_secrets`; no stubs; `cargo fmt -p`).
- `applyTo: 'crates/vox-gui/**'` → the Tauri/clippy caveats.
- `applyTo: 'docs/src/**/*.md'` → frontmatter requirement.
- `applyTo: 'scripts/**'` → VoxScript-only automation.
Since Copilot now reads `AGENTS.md` natively, `copilot-instructions.md` can shrink to a pointer + the path-scoped files.

### 2.2 Cursor [V]
**Supports:** `.cursor/rules/*.mdc` with four application types: **Always Apply** (`alwaysApply: true`), **Intelligent** (description-driven), **File-pattern** (`globs:`), **Manual** (`@my-rule`). Recommends **rules < 500 lines**, split into composable rules. Native `AGENTS.md` (+ nested).

**GAP:** confirm our `.cursor/rules/*.mdc` (`build-environment.mdc`, `retired-surfaces.mdc`) use the right type — e.g. `retired-surfaces` should be `alwaysApply` (it's an LLM guard), while Rust/GUI rules should be `globs`-scoped so they only load on relevant files (token savings). Cursor reading `AGENTS.md` natively means `.cursor/rules` can be thin + glob-scoped, not a policy restatement.

### 2.3 Google Antigravity + Gemini 3.5 Flash [V for surfaces / S, prior for model]
**Supports beyond GEMINI.md:** `.agents/skills/*.md` (modular instruction "manuals" — e.g. `write_specs.md`, `generate_code.md`, `audit_code.md`, `deploy_app.md`); `.agents/agents.md` **personas** with structured **Goals / Traits / Constraints**; `.agents/workflows/*.md` **slash-command** pipelines (e.g. `/startcycle` chains personas+skills).

**GAP (biggest leverage we miss):** our `GEMINI.md` is a flat overlay that now merely *points* at `.agents/skills/`. We do **not** define:
- `.agents/agents.md` personas — a **code-author** + **adversarial-reviewer** + **plan-executor** persona trio with Constraints encoding our hard rules ("MUST end each task green + committed"; "MUST NOT call `std::env::var`"; "MUST pause for approval on `contracts/`").
- `.agents/workflows/*.md` — a `/run-plan` workflow that chains *read-handoff-doc → execute-task → verify → commit* for the hub-and-spoke plan, instead of a hand-pasted prompt.
Given Gemini 3.5 Flash's weak long-context recall, **persona Constraints + per-skill manuals are exactly the structural mitigation** the limitations doc calls for — more durable than a long GEMINI.md.

### 2.4 Claude Code [S]
**Supports:** `CLAUDE.md` auto-load (3 levels), skills, **hooks**, MCP, memory.

**GAP:** we lean on CLAUDE.md + skills + memory but likely under-use **hooks** as machine enforcement. Several "rules" currently stated as prose (no `cargo fmt --all`, no stubs, frontmatter on `docs/src`) are better as a `PreToolUse`/`Stop` hook that blocks the action — converting advisory text into enforcement (the layering model's "machine enforcement" tier). Claude Code lacks Copilot-style path-instructions, so path-scoping stays in CI/hooks.

---

## 3. Token-waste audit (external best-practice + our repo)

### 3.1 Generated-file diff suppression
**Best practice [S]:** mark tool-regenerated files `linguist-generated -diff` in `.gitattributes` so they collapse in diffs/PRs and don't bloat agent context.
**Our state [A]:** `.gitattributes` correctly collapses `Cargo.lock`, `gui-surface-coverage.v1.json`, `gui-surface-registry.v1.json`, `docs/src/**/*.generated.md`, and the CLI-catalog baseline. **GAP:** generated TypeScript is **not** covered — `crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts` (19 commits) and any sibling `*.generated.ts` bloat every diff. **Fix:** add `**/*.generated.ts linguist-generated -diff` (and audit `crates/vox-gui/ui/src/generated/` for other uncovered artifacts).

### 3.2 High-churn committed generated artifacts
**Our state [A]:** `gui-surface-coverage.v1.json` (**54 commits**) and `gui-surface-registry.v1.json` (**25**) are regenerated and re-committed constantly. They're diff-collapsed (good) but 79 commits of regenerated JSON is churn. **Decision to make:** `.gitignore` + regenerate-in-CI, vs keep-as-committed-SSOT. If a contract consumer reads them at build time, keep; otherwise stop committing them.

### 3.3 Prompt caching as a token-waste lever
**Best practice [S]:** Anthropic prompt-cache reads cost ~0.1× base input (≈90% off reused context), 5-minute TTL refreshed on use, optional 1-hour cache. **Implication for instruction files:** keep always-loaded files (`AGENTS.md`, overlays) **stable** so the cached prefix stays warm — frequent churn of these files invalidates the cache. Our overlays churn little (good); avoid gratuitous edits. (This also matches the ScheduleWakeup cache-window guidance used elsewhere.)

### 3.4 Sub-agent context isolation & retrieval
**Best practice [S]:** isolate sub-agent contexts (one task per window) and prefer retrieval over full-file loads to bound context. We already use this (workflow sub-agents, `where-things-live.md` lookup, graphify). No gap; keep large-fan-out batched (the rate-limit lesson recurs in this very session).

---

## 4. Commit-churn / message-vs-diff drift (external + our repo)

### 4.1 Best practice [S]
- **commitlint** enforces Conventional Commits, runnable as a `commit-msg` hook **and** in CI.
- **CodeFuse-CommitEval** (Nov 2025) is a benchmark for detecting commit-message-vs-diff inconsistency; ML/LLM scorers can emit a 0–1 agreement score with a reject threshold. (Treat as a direction, not a dependency.)

### 4.2 Our state [A] — drift is real and unguarded
Trivial-typed commits hiding large diffs in recent history:
| Lines changed | Type | Subject (truncated) |
|---|---|---|
| 49,219 / 49,159 | `chore: merge …` | merge commits — message says nothing about scope |
| 26,508 / 16,047 | `test(semcov…)` | golden/snapshot dumps |
| 12,434 | `chore(deps): upgrade candle…` | large dependency bump |

The prior remediation artifacts (`COMMIT_MESSAGE_REWRITE_PLAN.md`, `graphify-out/commit_audit.json`) are **no longer in the tree** — that work never landed. **Nothing currently flags a `chore:` carrying a 49K-line change.**

**Fix (resurrect as a gate, not a one-shot rewrite):** a `vox ci commit-lint` check that (a) enforces Conventional Commits, and (b) **warns/fails when a `chore`/`docs`/`style`/`ci`/`test`-typed commit exceeds an N-line threshold** unless whitelisted (merges, vendored deps, golden regen). Wire into the fast pre-push tier. Deterministic line-count threshold first; LLM scoring optional later.

---

## 5. Gesture toward an enhancement plan (buildable pieces, not a plan)

Ordered by leverage ÷ effort:
1. **`.gitattributes`: add `**/*.generated.ts linguist-generated -diff`** — one line, immediate diff/context savings. (§3.1)
2. **`vox ci commit-lint`** — threshold gate for trivial-typed large commits + Conventional Commits enforcement. (§4.2)
3. **Copilot path-instructions** — split `.github/instructions/*.instructions.md` with `applyTo` globs (Rust / GUI / docs / scripts). (§2.1)
4. **Antigravity `.agents/agents.md` + `.agents/workflows/run-plan.md`** — persona trio with hard-rule Constraints + a plan-execution workflow; the durable fix for Gemini 3.5 Flash recall. (§2.3)
5. **Cursor rule-type/glob audit** — scope `.cursor/rules/*.mdc` by `globs`, keep guards `alwaysApply`. (§2.2)
6. **Claude Code hooks** — convert "no `cargo fmt --all` / no stubs / docs frontmatter" prose into blocking hooks. (§2.4)
7. **Decide on high-churn generated JSON** — gitignore+CI-regen vs committed-SSOT. (§3.2)
8. **Overlay shrink pass** — now that all four tools read `AGENTS.md`, reduce every overlay to tool-unique features + a pointer. (§1)

**Sequencing:** items 1–2 are cheap, high-value, machine-enforced — do first. Items 3–6 are per-platform leverage. Item 8 is the cleanup that the AGENTS.md-everywhere finding unlocks.

---

## 6. Sources
**Verified this session (direct fetch):**
- Cursor rules — https://cursor.com/docs/rules
- GitHub Copilot custom instructions — https://docs.github.com/en/copilot/customizing-copilot/adding-repository-custom-instructions-for-github-copilot
- Antigravity pipelines/skills/personas/workflows — https://codelabs.developers.google.com/autonomous-ai-developer-pipelines-antigravity

**Sourced but rate-limited (re-verify before relying):**
- Anthropic prompt caching — https://platform.claude.com/docs/en/build-with-claude/prompt-caching
- CodeFuse-CommitEval — https://arxiv.org/pdf/2511.19875
- commitlint / Conventional Commits — conventionalcommits.org, commitlint.js.org
- Antigravity skills codelab — https://codelabs.developers.google.com/getting-started-with-antigravity-skills

**Internal git audit (this session):** commit-type distribution, trivial-message large-diff scan, file-churn ranking, `.gitattributes` coverage — all from `git log` over the last 500–1000 commits of this repo.
