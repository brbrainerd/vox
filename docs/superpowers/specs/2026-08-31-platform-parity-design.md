---
title: "Platform Parity Program — Design Spec (Language, Runtime, Harness, Axis, Models)"
description: "Normative spec for closing the 82 ranked gaps vs Cursor Agent and Claude Code Desktop: shared types, ID inventory, file ownership, and track decomposition. Plans argue from this document."
category: "architecture"
status: "roadmap"
training_eligible: false
---

# Platform Parity Program — Design Spec

**Date:** 2026-08-31
**Inventory source:** Chat session diagnosis of Vox vs Cursor / Claude Code Desktop (originally 82 IDs L01–L20, R01–R12, H01–H18, G01–G18, M01–M14). Live-code re-verified that morning against `dispatch.rs`, `agent_loop.rs`, `pending_approvals.rs`, `test.rs`, `llm.rs`, `DiffReview.tsx`, `App.tsx` `/rollback`. **Adversarial re-audit the same evening** (six parallel codebase tracks: GUI/Axis, HITL/dispatch, language/codegen, harness+models, CI/policy, GUI IA) produced §2 + §9 corrections. Executors follow **§9 over any conflicting task text in a child plan**.

This spec is the contract the implementation plans implement. Executors read this file, **[ID coverage (original canvas)](2026-08-31-platform-parity-id-coverage.md)** (every L/R/H/G/M `gap`/`fix`), **and** the track plan they are assigned. Coverage v1 beats skip/optional language in a child plan. Do not invent types, env vars, or crate edges that are not named here.

## 0. Goal

A new user can (a) complete an agent coding turn in Vox Axis without an API key already in the environment, (b) see and asynchronously approve the actual diff, (c) run an agent loop long enough to edit-build-test, (d) author and verify `.vox` with closures and honest `@ai`/test tooling, (e) trust that Linux/Windows/macOS and wasm-vs-container claims are CI-gated, and (f) have `models::decide()` pick a model for reasons that change with the task and that cannot surprise-bill.

## 1. Non-goals

- Matching Cursor’s editor chrome or shipping a second VS Code. Axis remains the harness desktop; the extension may remain the inline-edit surface until Track 3 G07 decides otherwise **in that task**, not by sneaking in an editor rewrite.
- Internet personal mesh (v1.1). Track 5 may document LAN-mesh / `vox deploy --dry-run`; it must not claim global mesh.
- Rewriting the three-axis scorer into a fourth “locality” axis. Locality is a **filter**: soft via `prefer_local` / `select_local_first`, hard via `CandidateScope::LocalOnly`. Do not conflate those two (M05, M13).
- Rebuilding shipped Axis surfaces: ContextWindowMeter, IntentPanel, keybind capture, PlanPanel (session nodes), `vox_browser_*` (wrong name `vox_browser_snapshot`), secretary propose-only, VOX.md chat inject, `models::decide()` on the MCP chat path, scoreboard `* 0.15`, `daily_budget_usd` chat/infer guard, `cross-platform-check.yml` `pull_request` trigger, `HirExpr::Lambda` / `list.map`, `vox deploy --dry-run`, default determinism lint. See §2 / §9.
- New workspace crate dependency edges. Exceptions are USER-AUTHORIZED-ONLY (`AGENTS.md` §Dependency Discipline). This spec introduces **no** new crate-to-crate edges. New modules land in existing crates listed in §4.
- Reading or editing `docs/src/archive/`.
- Full durable HTTP `emit_main_boot` route refactor. L19 in this program is a **hard diagnostic** (`durable-http-boot-unimplemented`) when that path is compiled; the refactor stays in `docs/src/architecture/http-runtime-extraction-2026.md`.

## 2. Already shipped or already planned (do not rebuild)

Live-code 2026-08-31:

| ID | Reality | Action |
|---|---|---|
| L02 (partial) | `ai_schema_ctx::schema_for` emits a JSON Schema **body** when the return typedef is registered; unknown types still emit **name-only**. | Track 2 finishes the fallback: hard error, not silent name-only. |
| L05 (partial) | `vox test` has a global `--json` envelope in `commands/test.rs`; it still shells `cargo test` with inherited stdio and **no per-test JSONL**. | Track 2 adds per-test records. |
| H07 (partial) | `tool_search.rs` implements `vox_tool_search`; `select_tools_for_turn` caps at `DEFAULT_MAX_TOOLS = 40`. | Track 3 drops the default cap to 20 and makes `vox_tool_search` always in the core set. |
| H09 (partial) | `PermissionMode::Plan` auto-approves nothing; `select_tools_for_turn` restricts Plan to `http_read_role_eligible`. Dispatch still **executes** a write if the model names it. | Track 1: `plan_blocks_execution` **and** MCP `vox_exit_plan_mode` (only promotion). Track 6 PlanPanel Approve calls it. |
| M01, M02, G02 | `crates/vox-oauth-pkce` exists; full wizard+budget plan already written. | **Execute** `docs/superpowers/plans/2026-08-01-free-tier-onboarding.md`. Do not duplicate those tasks. |
| chat_turn | Unification plan exists. | **Execute remaining work** in `docs/superpowers/plans/2026-08-28-chat-harness-unification.md`. |
| R01 (partial) | `cross-platform-check.yml` already runs on `pull_request` + `merge_group` + schedule. Per-PR: `cargo check --workspace` on Win/mac/Linux. Deep nextest is merge_group/schedule. Docs that say “Win/mac only on merge_group” are **stale**. | Track 5: **do not add `pull_request`**. Promote the existing PR job as a required check (admin). **Required:** merge_group/schedule `vox-compiler --lib` on one hosted OS **and** `vox run --interp` golden (Task 1b). |
| H13 | Secretary is already propose-only (`classify` → `SecretaryProposed` toast; confirm via `secretary_confirm_task`). There is **no** `Dispatch::Auto`. | Track 3 Task 5b: autodispatch only `accept_all` + `confidence >= 0.9` (default 0 = never). Do not invent `Dispatch`. |
| H06 (partial) | `load_project_context` (`project_file.rs`) + `build_system_prompt_with_skill` already inject root `VOX.md`. AGENTS.md is an MCP resource, **not** in the chat prefix. No `.vox/rules` loader. | Track 3: **extend** `load_project_context`. Do **not** create `project_dna.rs`. |
| G06 | Meter is wired; compaction still invisible | Track 6 **must** add 70% warn, Compact button, dropped-tokens string (original G06 fix). Do not rebuild the meter. |
| G08–G09 (partial) | IntentPanel exists | Track 6: persist intent on `chat_turn` + Plan-mode panel default-on. Do not skip. |
| G12 (partial) | Keybind capture exists | Track 6: action ids send, plan, accept hunk, reject, compact, new session + handlers. |
| G10 (partial) | `PlanPanel` is live against `list_plan_nodes` / `update_plan_node`. It is **not** CLI `vox plan` JSON. | Track 6 Task 21: Approve invokes `vox_exit_plan_mode`. Empty state names session nodes SSOT. Do not skip. |
| G15 (partial) | Full `vox_browser_*` suite exists. **No** `vox_browser_snapshot`. | Allowlist live names (`screenshot` / `page_info` / `text` / `click`) and **gate** mutating browser tools. |
| M03 (partial) | `scoreboard_feedback_boost` already ends `* 0.15`. No `history_boost()` / `HISTORY_PRIOR_MAX`. | Track 4: named const + fixture-registry winner test. Do not write a “FAIL unbounded boost” test against HEAD. |
| M09 (partial) | GUI `chat_turn` relays to `vox_chat_message`; MCP `resolve_mcp_chat_model*` already calls `decide()`. | Track 4: regression on the MCP resolver, **not** a new `decide()` call in `chat_turn.rs`. |
| M01/M02 | `enforce_budget_guard` already refuses chat/infer when `daily_budget_usd` exceeded. | Track 0A remainder: wizard/PKCE. Track 6 T29: rail `$ remaining`. Track 4 T6: Exceeded → one `prefer_local` retry if T0A lacks it. Do not re-implement the guard. |
| L01 (partial) | Closures already ship as `fn(x) …` → `Expr::Lambda` → `HirExpr::Lambda`. `list`/`Option`/`Result` `.map` methods exist. Codegen already emits `move \|…\|`. | Track 2: **bar-literal sugar** `|x|` onto existing `Lambda`. Do **not** add `HirExpr::Closure`. |
| L18 | `CompileKind::Wasi` names `*.wasm`; native does not. The “both lanes emit `.wasm`” comment is stale. | Track 2 T9c: **prove** native artifact name (`compile_native_artifact_name`). Help/doctor must not advertise `--target server\|client\|fullstack` as shipped. |
| R11 | `determinism_lint::check_workflow_determinism` is already in default typeck parallel passes. | Track 5 T5d: fixture `workflow` + `time.now()` fails `vox check`. Track 6 T27: GUI “cannot replay”. |
| R12 | `vox deploy --dry-run` already exists (`DeployArgs.dry_run`). | Track 5: fixture exit-code contract (0 / 4), not the flag. |

## 3. Shared types (all tracks)

Copy these signatures. Later tasks must not rename them.

### 3.1 Approvals (Track 1)

`crates/vox-orchestrator-mcp/src/pending_approvals.rs` today:

```rust
pub struct PendingApprovalInfo {
    pub approval_id: ApprovalId, // "AP-000001"
    pub tool: String,
    pub summary: String,         // 200-char truncation — MUST go away
    pub requested_at_ms: u64,
}
```

Replace with:

```rust
pub struct PendingApprovalInfo {
    pub approval_id: ApprovalId,
    pub tool: String,
    pub summary: String, // one-line, ≤120 chars, NEVER the args dump
    pub args: serde_json::Value,
    pub unified_diff: Option<String>,
    pub risk_class: String, // from permission_modes::classify; "unknown" if None
    pub estimated_cost_usd: Option<f64>,
    pub requested_at_ms: u64,
}

pub fn register(
    &self,
    info: PendingApprovalInfoDraft, // tool, summary, args, unified_diff, risk_class, estimated_cost_usd, requested_at_ms
) -> (ApprovalId, tokio::sync::oneshot::Receiver<ApprovalOutcome>);
```

`vox_orchestrator::ApprovalOutcome` today has a unit `Modified` and is `Copy + Eq`. Change **in** `crates/vox-orchestrator/src/attention/budget.rs` to:

```rust
#[derive(Debug, Clone, Serialize, PartialEq)] // NO Copy, NO Eq — Value is not Eq
pub enum ApprovalOutcome {
    Approved,
    Rejected,
    Modified { args: serde_json::Value },
    AutoApproved,
    TimedOut,
}
```

Serde: `Modified` must be `{"Modified":{"args":{...}}}` so old unit `Modified` in the DB is **not** silently accepted. Add a custom deserializer that maps bare `"Modified"` to `Rejected` (fail closed) and logs once. Tests must cover that mapping.

Same commit **must** update every `Modified` site, including `daemon_extra.rs` `orch.resolve_approval` and `outcome_from_decision` (that helper must take `Option<Value>` so `modify` can carry args). GUI currently sends `outcome: 'approved'|'rejected'`; keep accepting that **and** `decision`. `decision=modify` **requires** `args` object or return an error JSON — do not construct empty `Modified`.

`vox_resolve_approval` args become:

```json
{
  "type": "object",
  "required": ["approval_id", "decision"],
  "properties": {
    "approval_id": { "type": "string" },
    "decision": { "enum": ["approve", "reject", "modify"] },
    "args": { "description": "required iff decision=modify" }
  }
}
```

Timeout: prefer a field on existing `HitlPolicy` (`crates/vox-config` `hitl_policy.rs`) named `approval_timeout_secs: u64` (default `86400`). `0` means never. **Do not invent** `VoxConfig.harness` — that struct does not exist. Remove the `const APPROVAL_TIMEOUT = 300s` in `dispatch.rs`. Register the env alias `VOX_APPROVAL_TIMEOUT_SECS` in `contracts/config/env-vars.v1.yaml`. Read timeout from the config already on `ServerState`, not by calling `VoxConfig::load()` on every park.

`risk_class` is a string `"mutating" | "destructive" | "unknown"`. `SafetyClass` has **no** `Display` — do not call `.to_string()` on it.

`reregister_after_restart` and `hitl_rehydrate.rs` must gain the new fields in the same change as `register`. HITL table lives in `crates/vox-db/src/schema/domains/execution.rs` (baseline version **90**). Additive nullable columns + `BASELINE_VERSION` bump; do **not** invent a `contracts/db` hitl YAML with its own `x-vox-version`.

### 3.2 Agent loop budgets (Track 1)

`crates/vox-orchestrator-mcp/src/chat_tools/chat/agent_loop.rs`:

```rust
pub const DEFAULT_MAX_ITERATIONS: usize = 8; // KEEP as Ask default
pub const MAX_ITERATIONS_PLAN: usize = 16;
pub const MAX_ITERATIONS_ACT: usize = 32;
pub const MAX_ITERATIONS_AGENT: usize = 128;
```

`max_iterations` is chosen from `permission_mode` (`ask`→8, `plan`→16, `accept_edits`→32, `accept_all`→128). Do not invent a parallel `SessionMode` enum; bind the composer’s Ask/Plan/Agent control to **existing** `PermissionMode` wire strings: Ask=`ask`, Plan=`plan`, Agent=`accept_edits` (not `accept_all` — Agent still parks `always_requires_approval` tools).

**End-to-end wire (false-negative if omitted):** `chat_tools/chat/message.rs` currently hardcodes `permission_mode: None` and `max_iterations = DEFAULT_MAX_ITERATIONS`. Composer + Track 1 budgets **never reach** `run_agent_turn` until `ChatMessageParams` / `try_run_agent_turn` thread the mode. Approvals Segment already writes `setPermissionMode` onto MCP invoke. One SSOT: transport `permission_mode` **and** `ChatMessageParams` must agree. Do not add `permission_mode` to `ChatTurnInput` as a *second* writer without updating `CHAT_TURN_KEYS` + `buildChatTurn.ts` in the same commit (Track 6 owns that React/IPC plumbing).

Parallel dispatch: partition by original index; `join_all` **ungated** tools (concurrency cap 4–8); gated tools sequential in relative order; sort results by index before appending `role:tool` messages. Do not parallelize anything that mutates the same `ServerState` lock as a gated tool in the same turn. Do not parallelize writes.

### 3.3 Apply hunks (Track 1)

New Tauri command `apply_worktree_hunks` in `crates/vox-gui/src/commands/` (follow existing `chat_turn.rs` command registration):

```rust
pub struct ApplyHunksInput {
    pub file: String,           // repo-relative, UTF-8
    pub hunks: Vec<HunkSpec>,   // 1-based old_start as in unified diff
    pub action: ApplyAction,    // Accept | Reject
}
pub enum ApplyAction { Accept, Reject }
```

**Prerequisite (H19):** `vox_write_file` is gated in dispatch but has **no match arm** — after approve it returns `Unknown tool`. Track 1 Task 0 implements the handler (`path` + `content`, resolve under `repository.root` via `workspace_path.rs`) **before** Apply / Modified-exec tests that depend on a successful write.

Accept applies listed hunks via that handler (or a shared repo-scoped write helper). Canonicalize under `ServerState.repository.root`; reject `..`, absolute paths outside root, and symlink escape. Reject hunks revert against HEAD (no existing VCS hunk helper — implement in `apply_hunks.rs`). Regenerates `gui-surface-coverage` in the same commit. `tempfile` is already a normal dep of `vox-orchestrator-mcp` — do not re-add it.

`DiffReview.tsx` grows `onAcceptHunk(file, hunkIndex)` / `onRejectHunk` / `onAcceptFile` / `onRejectFile`. Parse unified diff in a pure `parseUnifiedDiff(text): FileDiff[]` module with vitest; no network.

### 3.4 Closures (Track 2)

Lexer already has `Token::Bar` for `|` and `Token::PipeOp` for `|>`. Do not add a new token. `Token::Bar` is used today **only** for ADT variant heads (`parser/descent/decl/mid.rs`). Parse `|x|` as a **prefix** in **expression** position only.

**Do not add `HirExpr::Closure`.** Closures already lower to:

```rust
HirExpr::Lambda(Vec<HirParam>, Option<HirType>, Box<HirExpr>, bool, Span)
```

`HirExpr::Block(Vec<HirStmt>, Span)` already exists. `HirParam` is `{ id, name, type_ann, default, span }`. `parse_script(tokens: Vec<Spanned>) -> Result<Module, …>`; `lower_module(&Module) -> HirModule` (**not** `Result` — do not `.unwrap_or_else` on it). Copy the pipeline from `crates/vox-compiler/tests/emission_ladder_test.rs`.

Grammar sugar (the actual L01 gap): `|` param-list `|` (expr | `{` stmt* expr? `}`) → existing `Expr::Lambda` / `HirExpr::Lambda`. Empty `||` needs a special case (two `Bar`s). Capture: immutable by default; `mut` param opt-in. Codegen already emits `move |params|` for `fn(x) …` lambdas.

Stdlib: `list.map` / `filter`, `Option.map`, `Result.map` / `map_err` / `and_then` are **already registered** in `typeck/builtins.rs`. Track 2 Task 3 is a **bar-syntax golden** (`xs.map(|x| x * 2)`), not a method-table rewrite. `xs.map(fn(x: int) to int { … })` must keep compiling.

### 3.5 `@ai` honesty (Track 2)

- Unknown structured-output type: **compile error**, not name-only `response_format`.
- `HirFn` gains `doc_comment: Option<String>` populated from the `//` / `///` trivia immediately above the `fn` (parser already sees comments — wire them). Prompt = doc_comment (or `@prompt` body if present) + typed args, **never** `"Implement the function: {name}"` as the sole user message.
- `EffectAnnotation::Llm` added in `crates/vox-ast/src/decl/effect.rs`. `@ai` implies `Llm`. `@pure` + `@ai` is a hard typeck error. Keyword: `llm` in `uses` clauses (`from_keyword("llm")`).
- Budget fields on HIR that are set must be read by codegen or typeck must error `budget-annotation-dropped`. No silent drop.
- Model pin: typeck looks up `vox_orchestrator::models` registry **or** a compile-time snapshot in `contracts/models/known-slugs.v1.json` generated by `vox ci` — **do not** add a `vox-compiler` → `vox-orchestrator` crate edge. Use the JSON snapshot (L0 data). Track 2 generates it; Track 4 owns refreshing it.

### 3.6 Test loop (Track 2)

`contracts/reports/vox-test-run.v1.json` (new):

```json
{
  "schema_version": "1.0.0",
  "file": "examples/foo.vox",
  "exit_code": 0,
  "tests": [
    { "name": "it_works", "status": "passed", "duration_ms": 12, "message": null, "source": { "file": "examples/foo.vox", "line": 40 } }
  ]
}
```

`vox test --json` prints this object (keep the existing envelope **or** replace it — do not print two competing schemas). Source maps: codegen writes `target/generated/.vox-sourcemap.json` mapping `target/generated/src/**` lines → `.vox` spans. `vox test` remaps cargo failure paths before printing.

`--coverage`: either invoke `llvm-cov` when present and emit lcov, or **delete the flag** from clap. Same for `--update-snapshots` and `--forall-iterations`. Honesty over flags.

Exit codes: `0` ok, `2` compile fail, `3` test fail, `4` infra (cargo missing). Document in `docs/src/reference/cli.md` in the same commit.

### 3.7 Config / env (all tracks)

New `VOX_*` names **must** be added to `contracts/config/env-vars.v1.yaml` in the same commit:

| Var | Meaning |
|---|---|
| `VOX_APPROVAL_TIMEOUT_SECS` | 0 = never; default 86400 |
| `VOX_AGENT_MAX_ITERATIONS` | optional override of the mode default |
| `VOX_LLM_REPLAY` | path to cassette JSONL; empty = live |
| `VOX_PREFER_LOCAL` | **not** in `env-vars.v1.yaml` today (`prefer_local` is a `SelectionIntent` field). Track 4 **adds** the env if the GUI clutch exposes it, same commit as the registry row |
| `VOX_REQUIRE_SANDBOX` | Track 5 R03: doctor nonzero on `warning-only` |
| `VOX_REQUIRE_CUDA` | Track 5 R10 fail-loud doctor |
| `VOX_BEST_OF_N` | Track 4 M12 opt-in |

### 3.8 Persistence tier

Pending approval **payloads** (args, diff) are audit/event data. Do **not** `std::fs::write` the diff.

Tier A (`hitl_approval_record` in `vox-db`, table in `schema/domains/execution.rs`): store `args_json` (small) plus `diff_sha256` + `diff_bytes` for size accounting. **Blobs > 4 KiB** must not live as unbounded TEXT on the row (`data-storage-ssot-2026.md` §4.3) — put large diffs in the existing artifact/CAS path and keep a digest on the row. In-memory `PendingApprovalInfo.unified_diff` may still hold the full text for the live UI waiter.

Bump `BASELINE_VERSION` (currently 90) in the same commit. Additive nullable columns only.

Checkpoints / oplog already exist. GUI list tool is **`vox_oplog`** (not `vox_oplog_list`). List entries use field **`id`** (e.g. `"OP-000007"`), not `operation_id`. `vox_undo` still requires arg **`operation_id`** (int or `OP-XXXXXX`). Track 1 `/rollback` must list then undo; never `vox_undo` with `{}`.

## 4. File ownership (locked)

| Area | Create | Modify | Must not touch |
|---|---|---|---|
| Approvals | `crates/vox-orchestrator-mcp/src/approval_diff.rs` | `pending_approvals.rs` **including** `reregister_after_restart`, `hitl_rehydrate.rs`, `dispatch.rs`, `daemon_extra.rs`, `input_schemas.rs`, `budget.rs`, `vox-db` `execution.rs` + `BASELINE_VERSION`, GUI `ApprovalsView.tsx` **and** `NeedsYou` / `useAttentionInbox` | `std::fs` for diffs; inventing `VoxConfig.harness`; `SafetyClass::to_string()` |
| Agent loop | — | `agent_loop.rs`, `permission_modes.rs`, **`chat_tools/chat/message.rs`**, `ChatMessageParams` | Changing MCP tool schemas except resolve/undo/write |
| Write tool | — | `dispatch.rs` `vox_write_file` arm (H19) | Unknown-tool after approve |
| Apply | `parseUnifiedDiff.ts`, `apply_hunks.rs`, `rollbackLast.ts` | `DiffReview.tsx`, command registry, `App.tsx` `/rollback` **and** `App.test.tsx` (locks `args: {}` today) | Gamify, Mercatus; MCP name `vox_oplog_list` |
| Closures | compiler tests `crates/vox-compiler/tests/closures_*.rs` | parser **expr** descent only → existing `HirExpr::Lambda` | New `HirExpr::Closure`; `lower_module` as `Result`; method-table rewrite |
| `@ai` / effects | — | `effect.rs`, typeck, `llm.rs` emit, HirFn | Direct vendor HTTP |
| `vox test` | `contracts/reports/vox-test-run.v1.json` | `commands/test.rs`, codegen sourcemap emit | `cargo fmt --all` |
| VOX.md / hooks | `crates/vox-orchestrator/src/user_hooks.rs` | **`memory/project_file.rs` `load_project_context`** (do not create `project_dna.rs`), chat prompt builder, GUI Settings | Copying Cursor proprietary format |
| Models | — | `scoreboard_feedback_boost` named const, `select.rs` fixture tests, MCP `resolve_mcp_chat_model*`, GUI clutch | Binding `decide()` a second time in `chat_turn.rs`; new scoring axes |
| CI | docs + required-check note | `cross-platform-check.yml` **depth policy** (do not add a second `pull_request`), `github-hosted-exceptions.md`, `runner-autoscaling.md` | Full nextest-on-three-OS on PR; merge_group-only required check |
| GUI product | `sessionMode.ts`, CheckpointDrawer, honesty badges, `queue_background_turn` | Loquela composer, Approvals DTO, six keybind actions, Compact, PlanPanel Approve, chat footer axes/$ | Dumping G04–G18 as one Track 3 task; rented cloud VMs |

## 5. Track decomposition

| Track | Plan file | IDs | Depends on |
|---|---|---|---|
| 0 Existing | execute as-is | M01 M02 G02 (free-tier plan); remaining chat_turn unification | none |
| 1 Trust loop | `docs/superpowers/plans/2026-08-31-trust-loop-approvals-apply-agent.md` | H01 H02 H03 H04 H09 H16 H19 G01 G03 | none (parallel with 0). Task 0 = `vox_write_file` handler |
| 2 Language | `docs/superpowers/plans/2026-08-31-language-ai-authorship.md` | L01–L20 | none (parallel with 0/1 except L11 snapshot consumed by 4). L01 = bar sugar on `Lambda` |
| 3 Harness product | `docs/superpowers/plans/2026-08-31-harness-productization.md` | H05–H08 H11 H13-remaining H14 H15 H18 | Track 1 Apply + modes; Track 0 wizard |
| 4 Models | `docs/superpowers/plans/2026-08-31-model-router-local-cloud.md` | M03–M14 | Track 0 budgets already enforced on chat; L11 snapshot. M09 = MCP resolver regression |
| 5 Runtime | `docs/superpowers/plans/2026-08-31-runtime-cross-platform-honesty.md` | R01–R12 | none (parallel). R01 = required-check + T1b `--interp`; not add `pull_request` |
| 6 GUI product | `docs/superpowers/plans/2026-08-31-gui-product-axis.md` | G03 UI; G04 daemon; G05–G18 remaining original fixes; G19–G27; H01 rail; H10 H12 H16-UI H17; M02 $; R11 GUI | Gate A (Track 1 Apply + ExitPlanMode + rollback) |

Each track plan must produce working, testable software without the others, except where the Depends-on column says otherwise.

## 6. Global constraints (copied into every plan)

- Test-first: every new `pub fn` in `crates/*/src/**` gets a failing test in the same file (or the crate’s `tests/`) before implementation (`AGENTS.md` §Test-First).
- Never `cargo fmt --all` (Windows os error 206). Format with `vox run scripts/fmt.vox` or `cargo fmt -p <crate>`.
- Cargo from repo root. Windows: `& "$env:USERPROFILE\.cargo\bin\cargo.exe"`.
- Secrets: `vox_secrets::resolve_secret` only. After secret surface changes: `vox ci secret-env-guard` and `vox ci secrets-parity`.
- No new `VOX_*` env var without `contracts/config/env-vars.v1.yaml`.
- No `turso::Connection` outside `vox-db` / `vox-secrets` / `vox-test-harness`.
- No `std::fs::write` / `tokio::fs::write` for event/log data.
- LLM calls through `vox_actor_runtime::llm` only.
- GUI new Tauri commands: regenerate `gui-surface-coverage` in the same commit.
- Fresh worktree: `vox run scripts/gui-build.vox` before `cargo test -p vox-gui`.
- Do not add `exceptions` to crate-edges yourself.
- Commits: conventional (`feat:`, `fix:`, `test:`, `docs:`). No `--no-verify`. No amend unless the skill’s amend rules apply.
- Line endings LF except `*.ps1`.

## 7. Acceptance

The program is done when every ID in §8 is `done` or `wontfix-with-reason` in the ledger table of the master plan, and:

1. A zero-key machine with Ollama **or** completed OpenRouter PKCE can finish one Axis chat turn.
2. Approving a `vox_write_file` shows a unified diff, not 200 chars of JSON; `/rollback` without `operation_id` does not toast success.
3. `permission_mode=accept_edits` allows >8 tool iterations; independent reads in one model message run concurrently.
4. `examples/golden/` contains a closure `.map` that `vox check` accepts; `@ai` with an unknown return type fails typeck.
5. `vox test --json` lists per-test names; a failing test path mentions the `.vox` file.
6. `vox model explain` for a trivial vs complexity-9 codegen task is **not** byte-identical in the winner slug (history prior capped).
7. Required CI: the **existing** `cross-platform-check` PR job (`cargo check --workspace` on Win/mac/Linux) is a required check (admin). merge_group/schedule: `cargo test -p vox-compiler --lib` on **one** hosted OS **and** `vox run --interp` golden. Wasm skills that need subprocess fail `vox doctor` with a container recommendation. Do **not** claim a new PR trigger was added.

## 8. Full ID list (normative)

L01 closures · L02 schema body complete · L03 prompt channel · L04 budgets bind · L05 per-test JSON · L06 sourcemap · L07 llm effect · L08 no-op flags · L09 stub-check default · L10 cassette · L11 model-pin snapshot · L12 constrained-gen smoke · L13 project-wide check · L14 exit codes · L15 lsp dist · L16 repair facade · L17 diag URLs · L18 native vs wasm emit · L19 durable HTTP boot · L20 stdlib ratchet · R01 required OS smoke · R02 capability manifest · R03 sandbox matrix · R04 mobile claim honesty · R05 cloud-vs-model copy · R06 crates.io decision · R07 one skill runner · R08 installer signing · R09 triple matrix docs · R10 CUDA fail-loud · R11 determinism lint default · R12 deploy dry-run · H01 parallel + budgets · H02 approval diffs · H03 durable timeout · H04 modified + undo · H05 user hooks · H06 VOX.md · H07 tool search default · H08 worktrees · H09 plan dispatch deny + ExitPlanMode · H10 checkpoint UI · H11 prompt cache prefix + cache_hit · H12 thinking blocks · H13 secretary propose-only + accept_all confidence gate · H13 secretary propose-only · H14 harness eval · H15 ACI default-on mutating · H16 batch resolve · H17 attention badge · H18 provenance trailers · H19 `vox_write_file` dispatch handler · G01 hunk Apply · G02 onboarding (existing plan) · G03 Ask/Plan/Agent · G04 LAN daemon background turn · G05 Rules editor · G06 context meter · G07 editor-or-honest-position · G08 intent objects · G09 composer fields · G10 vox plan GUI · G11 analytical canvas · G12 keybinds · G13 subagent window_id · G14 Compute honesty · G15 browser as tools · G16 model explain in chat · G17 vision routing · G18 Bugbot analog · G19 mode-IA unification · G20 chat tool cards · G21 approval DTO + resolve schema tree · G22 approvals poll SSOT · G23 keybind actionHandlers · G24 harness honesty · G25 Matrix orphan · G26 decorator titles vs NAV_LABELS · G27 chat→review journey · M01 zero-key (existing) · M02 budget enforce (existing) · M03 history-prior cap · M04 coding benchmarks · M05 locality filter honesty · M06 direct providers · M07 autonomic cron · M08 prompt cache wire · M09 decide() in chat path · M10 no surprise premium · M11 live is_free · M12 best-of-N optional · M13 local/auto/cloud clutch · M14 429 failover.

## 9. Adversarial audit errata (2026-08-31 evening)

Six parallel codebase audits. **Child plans that contradict this section are wrong.**

### 9.1 False positives (do not rebuild)

| Claim in v1 plans | Live | Action |
|---|---|---|
| Closures / `list.map` missing | `HirExpr::Lambda`, method tables, `move \|` emit | Bar sugar only |
| `HirExpr::Closure` | Does not exist; would not compile | Use `Lambda` |
| ContextWindowMeter dead | Wired in `ChatExecutionRail` | Do not rebuild; still do G06 compact/warn/dropped |
| IntentPanel missing | Live `composeDescription` | Do not rebuild; still do G08 persist + G09 Plan default-on |
| Keybinds dead | `useKeybinds` + Settings | Add six action ids + handlers (G12) |
| PlanPanel dead / bind `vox plan` | Live plan-nodes IPC | Do not rebuild list; Approve → `vox_exit_plan_mode` |
| `vox_browser_snapshot` | Does not exist | Real `vox_browser_*` names |
| Secretary `Dispatch::Auto` | `classify` → `Option<ClassifyResult>` | Do not invent Dispatch; still do accept_all+confidence gate |
| `project_dna.rs` / `load_project_dna` | `load_project_context` | Extend, don’t fork |
| `subagent_dispatch.rs` worktrees | Pure router, no I/O | Wrong file |
| History boost unbounded | `scoreboard_feedback_boost * 0.15` | Named const + fixture test |
| `chat_turn` hardcoded slug | MCP `resolve_mcp_chat_model*` → `decide()` | Regression at resolver |
| `daily_budget_usd` unenforced | `enforce_budget_guard` on chat/infer | Skip guard rewrite |
| `cross-platform-check` not on PR | `pull_request` already present | Depth-policy test, not add trigger |
| R11/R12 greenfield | Determinism default-on; `deploy --dry-run` exists | Fixture tests |
| `VoxConfig.harness` | Does not exist | `HitlPolicy` |
| `vox_oplog_list` / `.operation_id` on list | Tool `vox_oplog`; list field `id` | Fix rollback helper |
| `ServerState.repository.root` missing | Exists | Do not invent cwd fallback as primary |
| Honesty-triage HIDE (meter, StreamCard, keybinds display) | July burn-down shipped KEEP | Do not re-HIDE |

### 9.2 False negatives (must add)

| Gap | Owner |
|---|---|
| `vox_write_file` gated, no dispatch handler (Unknown tool after approve) | Track 1 H19 Task 0 |
| `message.rs` hardcodes `permission_mode: None` + 8 iterations | Track 1 |
| `reregister_after_restart` / daemon `resolve_approval` ignore new fields/args | Track 1 |
| `ApprovalOutcome` is `Copy+Eq` — `Modified { args }` breaks the crate | Track 1 |
| HITL diffs >4 KiB as TEXT vs data-storage SSOT | Track 1 §3.8 |
| GUI resolve callers use `outcome` not `decision` (NeedsYou, attention, Approvals) | Track 6 G21 |
| `App.test.tsx` asserts `/rollback` `args: {}` | Track 1 + Track 6 |
| `ChatTurnInput` / `CHAT_TURN_KEYS` have no `permission_mode` | Track 6 G03 plumbing |
| DriveConsole clutch/risk ≠ PermissionMode — two mental models | Track 6 G19 |
| No CheckpointDrawer / thinking blocks / batch select / Compute honesty labels | Track 6 |
| AGENTS.md + `.vox/rules` not in chat prefix | Track 3 H06 |
| `vox_tool_search` not pinned in agent loop | Track 3 H07 |
| Worktree isolation (wrong module in v1 plan) | Track 3 H08 — spawn path, not `subagent_dispatch.rs` |
| `VOX_PREFER_LOCAL` claimed in spec, absent from env registry | Track 4 |
| `LlmConfig::anthropic` / `::ollama` missing | Track 4 M06 |
| Cascade used in research, not default chat | Track 4 M08 |
| CLI `vox skill run` is ARS echo stub vs MCP `SandboxedSkillRunner` | Track 5 R07 |
| Docs claim Win/mac check is merge_group-only | Track 5 docs |

### 9.3 Compile-breakers if an agent follows v1 plans literally

1. Match `HirExpr::Closure` / `lower_module` as `Result` / `parse_script(&tokens)` by ref.
2. `ApprovalOutcome` keep `Copy`/`Eq` with `Value` payload.
3. `SafetyClass::to_string()`.
4. `VoxConfig.harness.approval_timeout_secs`.
5. `register(draft)` without `reregister_after_restart` + tests.
6. `Dispatch::Auto`, `history_boost()`, `load_project_dna`, `vox_oplog_list`.
7. `permission_mode` on `ChatTurnInput` without `CHAT_TURN_KEYS`.
8. Dual `vox test --json` envelopes (`BuildLaneEnvelope` vs `vox-test-run.v1.json`) — **replace or nest, never both**.
9. `EffectAnnotation::Llm` without `HirCapability::Llm` exhaustiveness.
10. PR `cargo test -p vox-compiler` on hosted Win **and** mac every PR (anti-stacking).

## 10. Bug-prevention contracts (executors)

- After any `ApprovalOutcome` / `PendingApprovals::register` change: `cargo test -p vox-orchestrator -p vox-orchestrator-mcp` (exhaustiveness + `pending_approvals_tests` + daemon extra).
- After any new Tauri command: `cargo run -p vox-cli -- ci gui-surface-coverage --write` same commit; `vox run scripts/gui-build.vox` before `cargo test -p vox-gui` in a fresh worktree.
- After any new `VOX_*`: `contracts/config/env-vars.v1.yaml` same commit.
- After HITL schema: `BASELINE_VERSION` + `contracts/db/baseline-version-policy.yaml` parity.
- After GUI resolve schema: grep `outcome:` and `vox_resolve_approval` under `crates/vox-gui/ui` — update **all** callers.
- After `CHAT_TURN_KEYS` change: `buildChatTurn` assertion + Rust `ChatTurnInput` serde defaults.
- Closures: regression tests that `type A = | Foo | Bar` and `xs |> map` still parse.
- `vox test --json`: one schema SSOT; update envelope consumers in the same commit.
