# Harness Productization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Axis harness behaves like a coding-agent product: project DNA (AGENTS.md + rules on the **existing** loader), user hooks, deferred tools, isolated subagents. GUI remaining work is **Track 6**, not Task 8.

**Architecture:** Extend `load_project_context` in `crates/vox-orchestrator/src/memory/project_file.rs` (do **not** create `project_dna.rs`). VOX.md is already in `build_system_prompt_with_skill`. Load user hooks from `.vox/hooks.json` via `vox_process_run*`. Drop `DEFAULT_MAX_TOOLS` to 20 and pin `vox_tool_search` in `TurnContext::pin_names`. Worktrees: **not** `subagent_dispatch.rs` (pure router) — spawn path that sets cwd under `.vox/worktrees/<id>`. Checkpoint/thinking/attention/Compute honesty → Track 6.

**Depends on:** Track 1 gate (Apply + permission wire). Do not start until that gate is green.

**Closes:** H05 H06 H07 H08 H11 (prefix **and** cache-hit field) H13-remaining (accept_all+confidence) H14 H15 H18. H10 H12 H16-UI H17 G04–G18 → Track 6. Coverage: [`2026-08-31-platform-parity-id-coverage.md`](../specs/2026-08-31-platform-parity-id-coverage.md).

## Audit corrections (spec §9)

- `load_project_dna` / `project_dna.rs` is a plan-bug. Use `load_project_context`.
- `classify` returns `Option<ClassifyResult>` — no `Dispatch::Auto`. Do not invent Dispatch. Remaining H13: autodispatch gate.
- `subagent_dispatch.rs` has no I/O. Worktrees belong on `spawn_agent` / jail helpers.
- Hooks: `vox_process_run` / `vox_process_run_capture` in `vox-actor-runtime` builtins. Git worktree may use `Command` argv-first — do not wrap `cmd /c`.
- `always.inject` is not a codebase symbol.
- G06/G08–G09/G12/G15/H13 are false positives if rebuilt.

**Tech Stack:** Rust orchestrator + MCP, Tauri/React, git worktrees, JSON contracts.

**Spec:** [`docs/superpowers/specs/2026-08-31-platform-parity-design.md`](../specs/2026-08-31-platform-parity-design.md) H05–H08 H11 H14 H15 H18 (GUI → Track 6).

## Global Constraints

Inherit spec §6. Subprocess hooks go through `vox-actor-runtime` process primitives (telemetry-observable), never `std::process::Command` wrapped in `cmd /c`. No new crate edges. Worktrees live under the repo’s existing VCS isolation helpers if present (`rg worktree crates/vox-vcs crates/vox-orchestrator`).

---

## File map

| File | Role |
|---|---|
| Extend: `crates/vox-orchestrator/src/memory/project_file.rs` | AGENTS.md + `.vox/rules` into `load_project_context` |
| Create: `crates/vox-orchestrator/src/user_hooks.rs` | PreToolUse / PostToolUse / Stop |
| Create: `contracts/harness/hooks.v1.json` | Hook file schema |
| Modify: `crates/vox-orchestrator-mcp/src/llm_bridge/tool_selection.rs` | cap 20 |
| Modify: agent_loop `TurnContext::pin_names` | always `vox_tool_search` |
| Modify: spawn_agent / VCS isolation (rg `worktree` `spawn_agent`) | worktree cwd — **not** `subagent_dispatch.rs` |
| Modify: chat prompt builder | prefix order H11; AGENTS.md |

---

### Task 1: Project DNA — AGENTS.md + rules on existing loader (H06)

**Files:** `crates/vox-orchestrator/src/memory/project_file.rs` (`load_project_context`); `build_system_prompt_with_skill` in orchestrator-mcp chat_tools.

**Interfaces:**
- Consumes: files `VOX.md` (already injected), `AGENTS.md`, `.vox/rules/**/*.md`
- Produces: extend `load_project_context` — **do not** add `load_project_dna`

- [ ] **Step 1: Failing test** next to existing `load_project_context` tests (rg that name). Temp dir with `AGENTS.md` + `.vox/rules/rust.md`; assert concatenated output contains both and truncates at budget.

- [ ] **Step 2:** FAIL if AGENTS.md / rules absent from prefix.

- [ ] **Step 3:** Read AGENTS.md + rules sorted by path after VOX.md; join `\n\n`; truncate on char boundary (default 12_000). Wire into `build_system_prompt_with_skill` if not already pulling `load_project_context` fully.

- [ ] **Step 4:** PASS.

- [ ] **Step 5:** commit `feat: inject AGENTS.md and .vox/rules into the agent system prefix`

---

### Task 2: User hooks (H05)

**Files:** `user_hooks.rs`; `contracts/harness/hooks.v1.json`; dispatch pre/post in `handle_tool_call_with_mode`.

**Interfaces:**

```rust
pub struct HookEvent<'a> {
    pub hook: &'static str, // "PreToolUse" | "PostToolUse" | "Stop"
    pub tool: &'a str,
    pub args_json: &'a str,
}
pub fn run_hooks(root: &Path, event: HookEvent<'_>) -> Result<(), String>;
```

JSON file `.vox/hooks.json`:

```json
{ "PreToolUse": [{ "command": "vox", "args": ["ci", "queue", "--hook-guard"] }] }
```

- [ ] **Step 1: Failing test** — a hook command `["echo"]` / on Windows `["cmd", "/c", "echo"]` is **forbidden** by policy. Test instead with a `Command` that is the `vox` binary path skipping: use a relative `scripts` no — use `#[cfg(unix)]` `/bin/true` and `#[cfg(windows)]` `where.exe` as the command with args `["cmd"]` that exits 0. Assert PreToolUse runs before a stubbed tool.

Simpler unit test: `parse_hooks_json` rejects `shell_exec`. `run_hooks` with empty file is Ok.

```rust
#[test]
fn missing_hooks_file_is_ok() {
    let dir = std::env::temp_dir();
    run_hooks(&dir, HookEvent { hook: "PreToolUse", tool: "x", args_json: "{}" }).unwrap();
}

#[test]
fn pretool_nonzero_blocks() {
    // hooks.json command that exits 1 — run_hooks returns Err
}
```

- [ ] **Step 2:** FAIL. **Step 3:** parse JSON; spawn argv via existing process helper (`rg "Command::new" crates/vox-actor-runtime/src` for the blessed API). Nonzero PreToolUse → dispatch returns error JSON `hook blocked`. **Step 4:** PASS. **Step 5:** commit `feat: user-defined PreToolUse/PostToolUse/Stop hooks`

---

### Task 3: Tool-search default cap (H07)

**Files:** `tool_selection.rs` (`DEFAULT_MAX_TOOLS = 40` → `20`); ensure `vox_tool_search` is never filtered out.

- [ ] **Step 1:**

```rust
#[test]
fn cap_is_twenty() {
    assert_eq!(DEFAULT_MAX_TOOLS, 20);
}

#[test]
fn tool_search_survives_cap() {
    let ctx = TurnContext { /* copy a fixture from existing tests in this file */ };
    let tools = select_tools_for_turn(&ctx);
    assert!(tools.iter().any(|t| t.name == "vox_tool_search"));
    assert!(tools.len() <= 20);
}
```

Copy `TurnContext` construction from `tool_selection.rs` existing tests (`rg "TurnContext" crates/vox-orchestrator-mcp/src/llm_bridge/tool_selection.rs`).

- [ ] **Step 2:** FAIL (cap 40). **Step 3:** change constant; pin `vox_tool_search` before truncation. **Step 4:** PASS. **Step 5:** commit `fix: default tool cap 20 with vox_tool_search always present`

---

### Task 4: Subagent worktrees (H08)

**Files:** spawn path (`rg spawn_agent` under orchestrator / openclaw-runtime). **Do not** edit `subagent_dispatch.rs` for cwd — it is a pure `DispatchRouter`.

- [ ] **Step 1:**

```rust
#[test]
fn spawn_sets_cwd_under_worktree_when_flag_on() {
    let cwd = spawn_isolated_cwd(Path::new(".")).unwrap();
    assert!(cwd.to_string_lossy().contains(".vox/worktrees") || cwd != PathBuf::from("."));
}
```

Put the helper next to the actual spawn, not the router.

**GUI cwd (original H08):** SubAgents list must show `cwd` from the spawn struct. Test: `SubAgentsView` fixture with `cwd: ".vox/worktrees/abc"` renders that path. If the view lives in `vox-gui`, add the field to the existing IPC payload in the same commit (`gui-surface-coverage --write` if a new command). Default flag stays false until tests pass; **then default-on in this track** if green (do not leave the feature flag forever-off).

- [ ] **Step 2:** FAIL. **Step 3:** `git worktree add --detach <root>/.vox/worktrees/<id> HEAD` argv-first (`Command` for git is OK; hooks still use `vox_process_run*`). Record cwd on the subagent struct. **Step 4:** PASS. **Step 5:** commit `feat: isolate subagents in git worktrees`

---

### Task 5: Prompt cache prefix + cache-hit field (H11)

**Files:** `build_system_prompt_with_skill` (no frozen `build_prefix_parts()` API today — assemble explicitly). Chat progress / footer payload for `cache_hit`.

**H11 prefix order (frozen):** identity → project DNA (`load_project_context`) → tool defs → memory → **mutating user/conversation suffix last**.

- [ ] **Step 1:** test that system prompt for a fixture workspace starts with identity and that conversation/user content is last. Do **not** assert `Dispatch::Auto`.

- [ ] **Step 2:** test `cache_hit_field_is_bool_or_honest_na` — if the provider returns a cache header, event JSON has `"cache_hit": true|false`; else `"cache_hit": null` and GUI must render `cache n/a` (Track 6 Task 14). Do not invent a dollar amount when the header is absent.

- [ ] **Step 3–5:** implement prefix order + field. Commit `feat: cache-stable chat system prefix; expose cache_hit`

### Task 5b: Secretary autodispatch only accept_all + high confidence (H13 remaining)

Propose-only **already shipped**. Original remaining fix: auto only `accept_all` + high confidence.

**Files:** secretary classify → `maybe_autodispatch` (`rg maybe_autodispatch` / `secretary_confirm`). Add `ClassifyResult.confidence: f32` default **0.0**.

- [ ] **Step 1:**

```rust
#[test]
fn already_fixed_never_autodispatches() {
    assert!(maybe_autodispatch("ask", &ClassifyResult { confidence: 0.95, .. }).is_none());
    assert!(maybe_autodispatch("accept_all", &ClassifyResult { confidence: 0.5, .. }).is_none());
}

#[test]
fn accept_all_and_high_confidence_may_autodispatch() {
    assert!(maybe_autodispatch("accept_all", &ClassifyResult { confidence: 0.9, .. }).is_some());
}
```

Copy `ClassifyResult` fields from live type (`rg struct ClassifyResult`). Default confidence 0 ⇒ never auto on existing call sites.

- [ ] **Step 2:** FAIL (no confidence / always None or always Some).
- [ ] **Step 3:** `maybe_autodispatch` returns None unless mode=`accept_all` **and** `confidence >= 0.9`. Do **not** add `Dispatch::Auto`.
- [ ] **Step 4:** PASS. Existing `does_not_substring_match_fix_inside_past_tense_fixed` still PASS.
- [ ] **Step 5:** commit `feat: secretary autodispatch only in accept_all at confidence 0.9`

### Task 6: ACI default-on + provenance (H15 H18). H10 H17 → Track 6

**H15:** `rg agentos_guardrail_kernel_enabled` — default `true` for mutating tools. Test: config default (`impl_default.rs` is `false` today).

**H18:** after successful `vox_write_file`, record model/cost on oplog if fields exist. Test: operation record includes `model` when `agent_loop` has `model_used`.

**H10 CheckpointDrawer / H17 attention formula:** Track 6 (`vox_oplog` not `vox_oplog_list`).

- [ ] Steps 1–5 for H15+H18 only. Commit `feat: ACI guardrail kernel default-on; edit provenance on oplog`

---

### Task 7: Harness eval gate (H14)

**Files:** `crates/vox-gui/src/commands/harness_eval.rs` already exists — wire a CI job **or** `vox harness eval` on a frozen transcript of 5 turns. Prefer extending existing eval rather than a new workflow.

- [ ] **Step 1:** `rg "harness eval" crates/vox-cli` — write a test that the eval binary returns nonzero when success rate < 0.8 on a fixture that always fails.
- [ ] **Step 2:** FAIL if no threshold.
- [ ] **Step 3:** add `--min-success 0.8`.
- [ ] **Step 4:** PASS.
- [ ] **Step 5:** commit `feat: harness eval fails below success threshold`

---

### Task 8: GUI IDs moved to Track 6

Do **not** implement G04–G18 here. See `docs/superpowers/plans/2026-08-31-gui-product-axis.md` **and** coverage v1 (G04 daemon queue, G06 compact/warn, G08 persist, G09 Plan default-on, G10 ExitPlanMode, G12 six action ids). Those are **open** on Track 6, not skips.

---

## Track 3 gate

HARD: `cargo test -p vox-orchestrator` for `load_project_context` AGENTS/rules test

HARD: `cargo test -p vox-orchestrator-mcp` `DEFAULT_MAX_TOOLS == 20` + `vox_tool_search` pinned

HARD: `maybe_autodispatch` tests from Task 5b

HARD: no `project_dna.rs`, no `Dispatch::Auto` introduced
