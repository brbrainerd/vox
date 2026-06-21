# Soft-HITL — Gemini Flash Handoff Prompt (2026-06-19)

This is the copy-paste brief for Gemini Flash in Antigravity. It is committed so
the runner can read it and every doc it references. **Precondition:** the runner
must have this branch checked out (`claude/auto-gui-debug-plans-2026-06-18`); if
the runner is remote/cloud, push the branch first (the rev-2 commits are
local-only as of writing).

---

## ── COPY-PASTE BELOW THIS LINE ──

You are implementing the **attention-aware soft human-in-the-loop** feature for the
Vox repo. All design and step-by-step plans are committed in this checkout. Read
them from disk — do not rely on this message for detail.

### Read first (in this order)
1. `docs/src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md` — your own operating limits.
2. `docs/superpowers/specs/2026-06-19-attention-aware-soft-hitl-design.md` — the design SSOT (rev 2). Read §2 (locked decisions) and §8 (what changed) before any code.
3. The three plans, executed **in order**:
   - `docs/superpowers/plans/2026-06-19-soft-hitl-phase0-attention-strip.md`
   - `docs/superpowers/plans/2026-06-19-soft-hitl-phase1-feedback-gating-backend.md`
   - `docs/superpowers/plans/2026-06-19-soft-hitl-phase2-needs-you-surface.md`

Each plan opens with a **🤖 EXECUTION TARGET**, **Operating Rules**, and a **Flash
Execution Addendum** (pre-flight `rg` gates + a task-split table). Follow them
exactly. The addendum's pre-flight gates are **blocking**: run each `rg`, paste the
output, and if reality differs from the plan, **STOP and report** — do not guess or
"fix" the design.

### Execution order & independence
- **Phase 0** (GUI strip) is self-contained and ships on its own — start here.
- **Phase 1** (Rust backend) is the foundation for Phase 2 — do it second, fully green, before any Phase-2 task.
- **Phase 2** (GUI surface + overlay) depends on Phase 1's MCP tools (`vox_feedback_list`, `vox_resolve_feedback`) and events existing.
- Within a phase, run `[PARALLEL-SAFE]` tasks concurrently only if your subagents work in **separate files** (never two subagents on one file). `[SEQUENTIAL]` tasks edit a shared file — run them one at a time. The task-split tables list which is which.

### Non-negotiable invariants (do NOT regress these — they are the result of a 96-finding audit)
1. **Gate by `TaskId(u64)`**, never `HopperItemId` (a UUID string; the hopper→task map is one-way).
2. **No hopper mutation.** Do NOT add `ItemState::Blocked`, do NOT touch `crates/vox-orchestrator/src/hopper/`, do NOT gate the dispatcher. "Blocked" is a computed GUI overlay only.
3. **Doubts are non-gating.** `doubt_task` is synchronous and self-resolving; project doubts via the async EventBus sink (Phase 1 Task 9), and make the Needs-You **Overrule** button dispatch the real `OVERRULE_TASK` (Phase 1 Task 7b).
4. **One `FeedbackStore`** — created on the `Orchestrator`, `Arc`-cloned into `ServerState`. Never construct it twice.
5. **Transport = the existing `invoke_mcp_tool` command**; **reactivity = `vox://agent-events`** (NOT `vox://activity-appended`, which has no emitter). No new Tauri commands.
6. **Reuse `AttentionBudgetMeter`** (Phase 0) — the budget is already rendered; do not write a second parser.
7. MCP tools must be added to `contracts/mcp/tool-registry.canonical.yaml` (SSOT) and the `http_gateway` allowlist; any file calling `evaluate_*` must also call `state.record_attention_event(..)` in the same file (the `attention_ledger_parity` CI gate); the new GUI surface needs the literal `'needs-you'` in `App.tsx` (the `gui-surface-registry` wiring gate).

### Verification ritual (per task, before you commit)
- **Rust (Phase 1):** `cargo test -p <crate> <filter>` → `cargo clippy -p <crate> -- -D warnings` → `cargo fmt -p <crate>`. Never `cargo fmt --all` (Windows arg-limit). Run `vox stub-check`. No stubs, ever.
- **GUI TS (Phase 0/2):** `npx vitest run <path>` → `npx tsc --noEmit`. Every new component test starts with `// @vitest-environment jsdom`.
- **vox-gui Rust:** build **lib-only** — `cargo clippy -p vox-gui --lib -- -D warnings` (`--all-targets` breaks on the Tauri build script).
- A task is done only when its tests are green **and** committed. If a step fails twice, stop and report (two-strike rule).

### Working location
Use a dedicated git worktree off this branch for isolation (the repo uses
`.claude/worktrees/`); do not work directly in the main checkout if other sessions
are active.

### When done (each phase)
Append a one-line entry to `docs/superpowers/antigravity-handoff-ledger.md` (the
append-only loop ledger) recording: phase, commits, test counts (green), and any
deviation from the plan or pre-flight-gate mismatch you hit. This is how the
Claude-Code review loop picks the work back up.

## ── COPY-PASTE ABOVE THIS LINE ──

---

### Notes for the human (not part of the prompt)
- If the Antigravity runner is **local** (Flash editing this checkout), the
  committed docs are already present — paste the block and go.
- If the runner is **remote/cloud**, push `claude/auto-gui-debug-plans-2026-06-18`
  first so the runner's clone has the rev-2 spec/plans.
- Suggested cadence: hand off **Phase 0 alone** first (lowest risk, proves the
  loop + the runner's environment), review its ledger entry, then hand off Phase 1,
  then Phase 2. Don't hand all three at once.
