# MCP / Skills Code Review Cleanup — HANDOFF (start here)

> **You do NOT need any prior conversation.** Read this document first. Authoritative task checklist lives in [`.cursor/plans/mcp_skills_review_cleanup_3b12660f.plan.md`](../../../.cursor/plans/mcp_skills_review_cleanup_3b12660f.plan.md) (do **not** edit the plan file). For branch-wide context (vault, graphify, GUI WIP), see [`2026-06-16-feat-vault-decryption-recovery-SESSION-HANDOFF.md`](2026-06-16-feat-vault-decryption-recovery-SESSION-HANDOFF.md). SSOT for federation semantics: [`docs/src/architecture/mcp-vox-language-exposure.md`](../../src/architecture/mcp-vox-language-exposure.md).

**Branch:** `feat/vault-decryption-recovery` (MCP/skills work is **interleaved** with unrelated tracks — prefer a **scoped MCP/skills commit or PR** when shipping)  
**Plan tasks 1–10:** **implemented in working tree** (mostly **uncommitted**)  
**Plan tasks 0 & 11:** **NOT done** — hygiene audit + full verification close-out  
**Last updated:** 2026-06-16

**Do NOT commit unless the human operator explicitly asks.**

---

## Human intent

Close out the **tracks G–N MCP/Skills/E2E parity code review** by fixing federation, skill permissions, CI gates, and tests so workspace `@tool` / `@resource` declarations and skill allowlists behave consistently in `vox-mcp`.

**Architecture locked in (do not redesign without explicit approval):**

| Decision | Choice |
|----------|--------|
| Federation model | **Option A** — `WorkspaceMcpLoader` → `WorkspaceMcpSurface` → `merged_tool_registry` + dispatch |
| Empty `manifest.tools` | **Unrestricted** allowlist (documented in SSOT) |
| Infrastructure tools while skill active | **`vox_skill_*`**, **`vox_workspace_mcp_refresh`**, **`vox_chat_*`** exempt from allowlist |
| Federated tool tier meta | `vox_tier: "workspace"` on merged workspace tools |
| Skill macro tier meta | `vox_tier: "skill"` in `list_tools` |
| Loader failures | **Partial success** — per-file errors in `WorkspaceMcpLoadResult.errors`; scan continues |

**Explicit deferrals (out of scope unless expanded):**

- Per-call compile cache for workspace dispatch (performance)
- Full hybrid `vox-search` replacement for BM25 skill search
- Making golden `read_file` actually read disk (behavior change)
- `skill_mcp_sandbox_test` wired into CI (exists as untracked test file)

---

## Original code review findings → task mapping

| ID | Finding | Task | Status in tree |
|----|---------|------|----------------|
| H1 | Workspace resources not served | 2 | `server.rs` list/read + `dispatch_workspace_resource` |
| H2 | Allowlist blocks infrastructure tools | 3 | `skill_permissions.rs` exempt set + tests |
| H3 | Chat `active_skill` no allowlist parity | 4 | `activate_skill_for_id_or_name` + `chat/message.rs` |
| H4 | One bad file aborts scan | 1 | `WorkspaceMcpLoadResult`, loader `continue` |
| M5–M6 | Shadow/duplicate silent | 1 | `shadowed`, `duplicate_tools`, `duplicate_resources` + warn logs |
| M7–M8 | Narrow parity gate / registry bypass | 8 | `mcp_vox_surface_parity.rs` + fixtures |
| M9–M10 | Daemon skips hydrate / search race | 5 | `spawn_external_skill_hydration` in `new_full` + `new_for_daemon` |
| M11–M12 | Empty tools / skill_run trust | 10 | SSOT doc sections |
| M13 | Tier bypass | 6 | `registry.rs` + `server.rs` meta |
| M14 | Missing integration tests | 9 | `skill_mcp_permissions_test.rs`, `workspace_mcp_federation_test.rs` |
| L15–L19 | initialize count, skill_list source, optional params, etc. | 7, 10 | See files below |

---

## What was implemented (Tasks 1–10)

### Core federation (`crates/vox-orchestrator-mcp/src/workspace_mcp/`)

| File | Responsibility |
|------|----------------|
| `mod.rs` | `WorkspaceMcpSurface`, `WorkspaceMcpLoadResult`, `WorkspaceMcpLoadError`, duplicate/shadow fields |
| `loader.rs` | Resilient scan; `load_skips_invalid_file_and_keeps_valid_tools` test |
| `dispatch.rs` | `dispatch_workspace_tool`, `dispatch_workspace_resource`; unit tests for golden tool + `vox://golden/mcp-status` |
| `schema.rs` | `param_required_in_schema()` — optional params when HIR default present |

### Server / dispatch wiring

| File | Change |
|------|--------|
| `server_state.rs` | `load_workspace_mcp()`, `spawn_external_skill_hydration()` shared by `new_full` / `new_for_daemon` |
| `server.rs` | Workspace resources in `list_resources` / `read_resource`; skill macro `vox_tier: "skill"`; `initialize()` uses merged registry len |
| `dispatch.rs` | **Critical:** workspace tool routing before static match; `vox_workspace_mcp_refresh`; `vox_skill_run` arm |
| `registry.rs` | `merged_tool_registry` adds `vox_tier: "workspace"`; `merged_workspace_tools_carry_workspace_tier_meta` (**must** use `#[tokio::test]`) |
| `skill_permissions.rs` | Allowlist + infrastructure exemption + unit tests |
| `skills_tools.rs` | `activate_skill_for_id_or_name`, `skill_run`, `manifest_source_label` for list/search/info |
| `chat_tools/chat/message.rs` | Calls activate when `params.skill` set |

### CI / contracts

| Path | Status |
|------|--------|
| `crates/vox-cli/src/commands/ci/mcp_vox_surface_parity.rs` | **Untracked** — wired in `ci/mod.rs`, `run_body.rs`, `docs.rs` ssot-drift |
| `contracts/mcp/workspace-tool-fixtures.v1.json` | **Untracked** — tool + `vox://golden/mcp-status` resource fixture |
| `contracts/mcp/workspace-mcp-surface.v1.yaml` | **Untracked** — scan globs SSOT |
| `docs/src/architecture/mcp-vox-language-exposure.md` | **Untracked** — federation SSOT |

### Integration tests (both **untracked**)

| File | Covers |
|------|--------|
| `crates/vox-integration-tests/tests/skill_mcp_permissions_test.rs` | Allowlist after `skill_use`; infrastructure `vox_skill_run` |
| `crates/vox-integration-tests/tests/workspace_mcp_federation_test.rs` | `read_file`, resource read, refresh diagnostics, loader resource |
| `crates/vox-integration-tests/Cargo.toml` | Added `vox-plugin-api` dev-dep |

Golden fixture source: `examples/golden/mcp_tools.vox` (repo root scan via `load_scan_config`).

---

## Session-specific fixes (easy to miss)

These were **missing on the branch** and caused integration failures even after Tasks 1–10 landed elsewhere:

1. **`dispatch.rs` had no workspace tool routing** — `handle_tool_call_inner` must consult `state.workspace_mcp` and call `dispatch_workspace_tool` **before** the static `match` (otherwise `read_file` → `Unknown tool`).
2. **`vox_workspace_mcp_refresh` match arm missing** — refresh test failed with unknown tool.
3. **`vox_skill_run` match arm missing** — permission test failed after allowlist fix.
4. **`merged_workspace_tools_carry_workspace_tier_meta`** used sync `#[test]` but calls `ServerState::new_full` → needs `#[tokio::test]`.
5. **Integration test skill id collision** — `git-skill` name/id collides with hydrated external skills; use **`integration-test-git-skill`** and assert `registry.get().tools`.
6. **Do not use `vox_run_shell` for allowlist denial tests** — it hits **300s HITL approval timeout** before returning; use **`vox_git_diff`** and assert `"allowlist"` in error JSON.

---

## Verification status (as of 2026-06-16)

| Suite | Last known result | Notes |
|-------|-------------------|-------|
| `skill_mcp_permissions_test` | **2/2 PASS** | After fixes above |
| `workspace_mcp_federation_test` | **2/4 PASS** before dispatch wiring; **4/4 expected** after dispatch fixes | Re-run required |
| `vox-orchestrator-mcp --lib workspace_mcp` | **7/7 PASS** (earlier direct binary run) | Re-run after dispatch edits |
| `parity_gate_passes` | **Not confirmed** | Prior run hit `vox.exe` file lock (`Access is denied`) |
| **Background batch** (`target/verify-all-mcp.log`) | **All suites exit -1** | ~53 min run; no `test result` lines — builds interrupted mid-compile (file locks / killed process), not assertion failures |
| `vox ci pre-push --complete` | **Not run** | Task 11 |
| `operations-sync --target mcp --write` | **Not run** | Task 0 |

### Windows verification pitfalls

- **Do not run multiple `cargo test` in parallel** on this host — causes `LNK1104` / `Blocking waiting for file lock on build directory`.
- **Do not pipe cargo through `Out-String | Add-Content` in long batch scripts** — output may be truncated and `$LASTEXITCODE` can read `-1` when builds are killed or lock-blocked; run suites one at a time with direct terminal output instead.
- Use **`CARGO_BUILD_JOBS=1`** and **one sequential script**.
- Optional: `$env:CARGO_TARGET_DIR = "target-verify-mcp"` for isolation (cold compile ~25+ min first time).
- Never `cargo fmt --all` — use `vox run scripts/fmt.vox`.

### Recommended verification script (Task 11)

Run **sequentially** from repo root:

```powershell
$c = "$env:USERPROFILE\.cargo\bin\cargo.exe"
$env:CARGO_BUILD_JOBS = "1"

& $c test -p vox-orchestrator-mcp --lib workspace_mcp -j 1
& $c test -p vox-orchestrator-mcp --lib skill_permissions -j 1
& $c test -p vox-orchestrator-mcp --lib merged_workspace -j 1
& $c test -p vox-integration-tests --test skill_mcp_permissions_test -j 1
& $c test -p vox-integration-tests --test workspace_mcp_federation_test -j 1
& $c test -p vox-cli --lib parity_gate_passes -j 1

pwsh -File scripts/windows/vox-dev.ps1 ci mcp-vox-surface-parity
pwsh -File scripts/windows/vox-dev.ps1 ci pre-push --complete
```

If `vox` binary is locked, use `cargo run -p vox-cli -- ci …` instead of `./target/debug/vox.exe`.

---

## Remaining work

### Task 0 — PR hygiene

1. **Audit branch diff vs `main`** — isolate MCP/skills scope:

```powershell
git diff main --stat
git diff main -- crates/vox-orchestrator-mcp `
  crates/vox-cli/src/commands/ci/mcp_vox_surface_parity.rs `
  crates/vox-integration-tests/tests/workspace_mcp_federation_test.rs `
  crates/vox-integration-tests/tests/skill_mcp_permissions_test.rs `
  contracts/mcp `
  docs/src/architecture/mcp-vox-language-exposure.md `
  examples/golden/mcp_tools.vox
```

2. **Review unrelated hunks** on this branch (`vox-scientia`, `browser_tools`, `graphify_tools`, vault, etc.) — split to separate PR or document why required.

3. **Regenerate MCP catalog** if tool metadata changed:

```powershell
cargo build -p vox-cli -q
pwsh -File scripts/windows/vox-dev.ps1 ci operations-sync --target mcp --write
```

Expect drift in `contracts/mcp/tool-registry.canonical.yaml` only if catalog entries changed.

4. **Stage untracked MCP files** before commit (see list in §What was implemented).

### Task 11 — Close-out

1. Run verification script above — all green.
2. Walk the [self-review table in the plan](../../../.cursor/plans/mcp_skills_review_cleanup_3b12660f.plan.md) — each H/M/L finding maps to a commit or file.
3. **PR description** should include:
   - Summary of federation + allowlist + CI gate
   - Test commands run (paste `test result` lines)
   - Known deferrals (compile cache, BM25→vox-search, read_file stub)
   - Note branch interleaving; recommend scoped review
4. Optional doc lint:

```powershell
cargo run -p vox-doc-pipeline -- --lint-only --paths docs/src/architecture/mcp-vox-language-exposure.md
```

---

## Key API shapes (for fresh agents)

### Loader

```rust
// Returns partial success; check .errors
let load = WorkspaceMcpLoader::load_repo(repo, &config)?;
let surface = load.surface;
```

Call sites must use **`load.surface`**, not assume `WorkspaceMcpSurface` directly from `load_repo`.

### Allowlist

```rust
check_skill_tool_permission(registry, active_skill_id.as_deref(), tool_name)
// None = allowed; Some(msg) = denied
```

Runs in `handle_tool_call` **after** scope/lock guards, **before** HITL approval for dangerous tools.

### Refresh response JSON (`vox_workspace_mcp_refresh`)

```json
{
  "tool_count": N,
  "resource_count": M,
  "shadowed": [...],
  "duplicate_tools": [...],
  "duplicate_resources": [...],
  "errors": [{ "path": "...", "message": "..." }]
}
```

---

## Untracked files to stage for MCP PR

```
contracts/mcp/workspace-mcp-surface.v1.yaml
contracts/mcp/workspace-tool-fixtures.v1.json
crates/vox-cli/src/commands/ci/mcp_vox_surface_parity.rs
crates/vox-integration-tests/tests/skill_mcp_permissions_test.rs
crates/vox-integration-tests/tests/workspace_mcp_federation_test.rs
crates/vox-orchestrator-mcp/src/skill_permissions.rs
crates/vox-orchestrator-mcp/src/workspace_mcp/
docs/src/architecture/mcp-vox-language-exposure.md
```

Also review whether `skill_mcp_sandbox_test.rs` should ship or stay local-only.

---

## Parallel agent guidance

Use **`dispatching-parallel-agents`** for **independent code domains** (e.g. docs vs tests vs dispatch), **not** for parallel `cargo test` on Windows.

Good split:

| Agent | Scope |
|-------|--------|
| A | Task 0 diff audit + operations-sync |
| B | Fix any failing integration test (single test file) |
| C | Parity gate + `vox ci ssot-drift` |

Run **one** cargo build/test chain after all agents finish editing.

---

## Related commands

| Command | Purpose |
|---------|---------|
| `vox ci mcp-vox-surface-parity` | CI gate over fixtures + load health |
| `vox ci ssot-drift` | Includes parity in docs drift bundle |
| `cargo test -p vox-orchestrator-mcp load_skips_invalid` | Loader resilience unit test |
| `cargo test -p vox-cli parity_gate_passes` | Parity unit test in `mcp_vox_surface_parity.rs` |

---

## Success criteria (definition of done)

- [ ] All verification commands in §Recommended verification script pass
- [ ] `operations-sync --target mcp --write` run if catalog metadata changed; no unexpected drift elsewhere
- [ ] Untracked MCP files committed in a **scoped** commit/PR (human approval)
- [ ] PR description maps every original review finding to code or explicit deferral
- [ ] No parallel cargo contention left mid-verify (`Get-Process cargo` → 0 before final run)

When done, mark plan todos **task-0-hygiene** and **task-11-closeout** complete.
