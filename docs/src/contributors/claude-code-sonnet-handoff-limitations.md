---
title: "Claude Code handoff: Sonnet 4.6 limitations & operating envelope"
description: "Reusable reference for handing implementation plans off to Claude 4.6 Sonnet running in the Claude Code harness — its limits, this repo's gotchas, and the operating rules a handoff prompt must encode."
category: "Contributors"
status: "current"
training_eligible: true
training_rationale: "Durable, repo-specific operating envelope for agentic handoffs; high-value for any future plan execution."
---

# Claude Code handoff: Sonnet 4.6 limitations & operating envelope

Reusable across handoffs. When you hand any implementation plan to **Claude 4.6 Sonnet** (model id `claude-sonnet-4-6`) running in the Claude Code harness, paste the relevant rules below into the handoff prompt. These are the constraints that, left implicit, most often cause a plan to be executed wrongly.

## 1. Harness & sandbox limits

- **Subagents are read-only in this repo's worktree sandbox.** Dispatched agents get shell/write **DENIED**. *Consequence:* you cannot fan work out to parallel **writing** subagents and expect files to land. Pattern that works: a subagent *produces* a diff/analysis as text; the **main session writes, verifies, and commits**. If you need true parallel writers, use multiple git worktrees the **main session** owns.
- **Context window is finite.** Read only the files a task names — never whole crates. Vox crates like `vox-orchestrator` and `vox-cli` are large; blind reads blow the budget.
- **No interactive TUIs from tool calls.** `git rebase -i`, `git add -i`, and anything that opens an editor will hang. Use non-interactive flags.
- **Rate limits on web/deep-research.** Do **not** run the mass-verify deep-research workflow (110-agent fan-out trips the server rate-limit and mislabels `0-0 abstain` as "refuted"). Verify critical claims with **2–3 targeted `WebFetch`/`WebSearch`** calls instead.

## 2. Parallelism guidance

- **Sequential when tasks churn shared types.** A plan's foundation track (new crate, new public API) must run one task at a time — parallel agents conflict on the same files.
- **Parallel only after an API freezes**, and only across **disjoint files**, each worker in its **own worktree** (`isolation: "worktree"`).
- Multi-agent **Workflow** orchestration requires **explicit owner opt-in** ("use a workflow" / ultracode). Do not spin one up unprompted.

## 3. This repo's hard rules (from AGENTS.md / CLAUDE.md)

- **Read [`AGENTS.md`](../../../AGENTS.md) first**, then [`where-things-live.md`](../architecture/where-things-live.md) before adding code.
- **`cargo fmt --all` is BANNED.** Use `vox run scripts/fmt.vox` or `cargo fmt -p <crate>`.
- **No new `.ps1`/`.sh`/`.py` automation** — VoxScript (`.vox`) only.
- **`docs/src/` files need frontmatter** (`title`, `description`, `category`, `status`).
- **New crate ⇒ same-PR updates** to `crates/vox-arch-check/layers.toml` **and** `where-things-live.md`.
- **`.vox` files are Vox source** — not Rust/TS. Honor `// vox:skip`.

## 4. Build / verify gotchas (Windows-primary)

- **Never pipe `cargo` to `head`/`grep`/`tail`** — on Windows this orphans thousands of processes (~40 GB RAM). Redirect to a file: `cargo test -p x > out.txt 2>&1`. Recovery: `taskkill /F /IM cargo.exe` ×5.
- **`clippy` per crate:** `cargo clippy -p <crate> -- -D warnings`. Workspace `--all-targets` clippy breaks on `vox-gui` (Tauri build script) — exclude it.
- **Per-task verification gate:** `cargo test -p <crate>`, `cargo clippy -p <crate> -- -D warnings`, and `cargo run -p vox-arch-check` must pass before a task is "done." Evidence before assertions — show the command output.
- A stale installed `vox` may warn on git hooks (`schema_max` newer than binary); harmless for doc/plan commits.

## 5. Vox-language gotchas (if writing `.vox`)

- `Json.as_float()` has a native-lane bug — run scripts with `--mode interp`.
- No multi-line `+` expressions; no `list.set`; single-line fn signatures.

## 6. Handoff-prompt checklist

A good Sonnet 4.6 handoff prompt states, explicitly:

1. The plan file path and the **required sub-skill** (`superpowers:subagent-driven-development` or `executing-plans`).
2. Which track/task to start at, and that tasks are **strictly ordered** until the named API freeze.
3. The sandbox/parallelism limits from §1–2 (so it doesn't assume parallel writers).
4. The verification gate from §4 (so it doesn't claim done without evidence).
5. Any **UNVERIFIED caveats** from the plan's audit log — the files it must read before the affected task.
6. The decision/research **gates** it must stop at and ask the owner (never resolve unilaterally).
