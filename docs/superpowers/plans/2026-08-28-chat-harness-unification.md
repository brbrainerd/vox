# Chat Harness Unification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every composer control reach the backend through one `chat_turn` contract; make model selection honest about what was resolved; build the three backends delegation depends on; gate plans on approval; and replace a constant labelled "quality" with measured Efficiency, Responsiveness, and Intelligence.

**Architecture:** One Tauri command owns the sync-vs-background routing decision, taking one `ChatTurnInput` whose routing fields are held at parity with `SubmitTaskInput` by a test that reflects over **both** structs. The frontend keeps its two store lifecycles — `chatPending`/`chatReplySettled` for sync, `submit`/`submitResolved`/`agentEvent` for background — because `submitResolved` is the only writer of `taskToSession`, the map that routes every live agent event to a bubble.

**Tech Stack:** Rust (Tauri 2, serde, tokio), TypeScript/React 19, vitest, Playwright, `vox-db` (libSQL), orchestrator daemon JSON-RPC.

**Spec:** [`docs/superpowers/specs/2026-08-28-chat-harness-unification-design.md`](../specs/2026-08-28-chat-harness-unification-design.md)

## Global Constraints

- **Task 0 is mandatory and comes first.** `cargo test -p vox-gui` dies in `tauri-build` on a missing `externalBin` sidecar in any fresh worktree. Every agent working in its own worktree runs it.
- **Test-first.** Every task starts with a failing test.
- **Every task states its observable.** A test that only inspects a payload it constructed is not acceptance. Seven such tests were caught in review; see the spec's §14.
- **No new workspace crate edges.** (Verified: this plan introduces none. `vox-actor-runtime` is already a `vox-gui` dep.)
- **All new serde fields are `#[serde(default)]`.**
- **Never run `cargo fmt --all`** (Windows `os error 206`). Use `vox run scripts/fmt.vox` or `cargo fmt -p <crate>`.
- **Regenerate SSOT in the same commit** as any new Tauri command: `cargo run -p vox-cli -- ci gui-surface-coverage --write`. Otherwise `ssot-drift` fails the *fast* pre-push tier.
- **Batch commits per phase.** lefthook runs workspace-wide fmt + whole-repo toestub on every commit (~1–3 min each).
- **DOM tests need `// @vitest-environment jsdom` on line 1.** The default environment is `node`.
- Rust: `cargo test -p <crate> <filter>`. TS: `pnpm -C crates/vox-gui/ui exec vitest run <path>` (after `pnpm -C crates/vox-gui/ui install --prefer-offline`).
- Verification gate is `vox ci pre-push --full` (`--complete` runs **no tests**) plus the §Task V manual checklist.

---

## Task 0: Worktree prerequisites

**Files:** none (build only).

- [ ] **Step 1: Build the Tauri sidecar**

```bash
vox run scripts/gui-build.vox
```

- [ ] **Step 2: Install UI deps**

```bash
pnpm -C crates/vox-gui/ui install --prefer-offline
```

- [ ] **Step 3: Confirm the baseline compiles**

Run: `cargo test -p vox-gui --no-run`
Expected: builds. If it fails inside `tauri-build` with "resource path … doesn't exist", Step 1 did not complete.

---

# Phase A — One dispatch

## Task A1: `chat_turn` (input, both branches, registration)

Tasks 1–3 of the first draft are merged: the command cannot compile with only one branch.

**Files:**
- Create: `crates/vox-gui/src/commands/chat_turn.rs`
- Modify: `crates/vox-gui/src/commands/mod.rs`, `control_plane.rs`, `chat.rs`, `main.rs`
- Regenerate: `contracts/reports/gui-surface-coverage.v1.json`

**Interfaces:**
- Consumes: `SubmitTaskInput`, `submit_orchestrator_task` (`control_plane.rs`); `parse_chat_message_envelope`, `ParsedChatReply`, `persist_assistant_reply` (`chat.rs`)
- Produces: `ChatTurnInput`, `ChatTurnDto`, `Execution`, `sync_tool_args`, `background_input`, `chat_turn`

**Observable:** with a model picked in the composer and "Quick chat" selected, the args sent to `vox_chat_message` contain `model_override` and the resolved `context_files`. Before this task they contain neither.

- [ ] **Step 1: Make the reused items reachable**

In `crates/vox-gui/src/commands/chat.rs`, raise visibility (all three are crate-local today, so nothing else changes):

```rust
pub(crate) struct ParsedChatReply {
    pub(crate) content: String,
    pub(crate) model_id: Option<String>,
    pub(crate) latency_ms: Option<u64>,
    pub(crate) selection_reason: Option<String>,
}
```

and `pub(crate) fn parse_chat_message_envelope`, `pub(crate) async fn persist_assistant_reply`.

In `crates/vox-gui/src/commands/control_plane.rs`, add `Serialize` to the derive and add the lineage field:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct SubmitTaskInput {
```

```rust
    /// Originating chat session, for delegation lineage (Phase D1). `None` for
    /// non-chat callers (Tasks surface, hopper).
    #[serde(default)]
    pub chat_session_id: Option<String>,
```

and in `submit_task_params`'s `json!` object:

```rust
        "chat_session_id": input.chat_session_id.filter(|s| !s.trim().is_empty()),
```

- [ ] **Step 2: Write the failing tests**

Create `crates/vox-gui/src/commands/chat_turn.rs` with only the test module for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn keys_of<T: serde::Serialize>(v: &T) -> BTreeSet<String> {
        serde_json::to_value(v)
            .expect("serializes")
            .as_object()
            .expect("object")
            .keys()
            .cloned()
            .collect()
    }

    /// Reflects over BOTH structs. The first draft's version filtered
    /// SubmitTaskInput's keys BY a hand-maintained constant and then compared
    /// the result to that same constant — a tautology that passed when a field
    /// was added to one struct only. This one cannot.
    #[test]
    fn routing_fields_match_submit_task_input() {
        let turn = keys_of(&ChatTurnInput::default());
        let submit = keys_of(&crate::commands::control_plane::SubmitTaskInput::default());
        let shared: BTreeSet<String> = ROUTING_FIELDS.iter().map(|s| s.to_string()).collect();
        let missing_from_turn: Vec<_> = shared.difference(&turn).collect();
        let missing_from_submit: Vec<_> = shared.difference(&submit).collect();
        assert!(
            missing_from_turn.is_empty(),
            "ChatTurnInput is missing routing fields: {missing_from_turn:?}"
        );
        assert!(
            missing_from_submit.is_empty(),
            "SubmitTaskInput is missing routing fields: {missing_from_submit:?}"
        );
    }

    #[test]
    fn execution_defaults_to_sync() {
        let input: ChatTurnInput =
            serde_json::from_value(serde_json::json!({"session_id":"s","content":"hi"}))
                .expect("minimal");
        assert_eq!(input.execution, Execution::Sync);
        assert!(input.context_files.is_empty());
    }

    /// The regression this whole plan exists for.
    #[test]
    fn sync_tool_args_carry_every_composer_control() {
        let input: ChatTurnInput = serde_json::from_value(serde_json::json!({
            "session_id": "s1", "content": "harden the crypto invariants",
            "model_override": "openrouter/anthropic/claude-opus-5",
            "tier": "cloud",
            "context_files": ["crates/vox-crypto/src/lib.rs"],
            "active_skill": "ponytail",
            "clutch": "genius", "risk": "low"
        })).expect("input");
        let args = sync_tool_args(&input);
        assert_eq!(args["prompt"], "harden the crypto invariants");
        assert_eq!(args["model_override"], "openrouter/anthropic/claude-opus-5");
        assert_eq!(args["tier"], "cloud");
        assert_eq!(args["context_files"][0], "crates/vox-crypto/src/lib.rs");
        assert_eq!(args["active_skill"], "ponytail");
        assert_eq!(args["clutch"], "genius");
        // `cognitive_profile` must NEVER be set from the tier: its values are
        // fast|reasoning|creative, and setting it switches the turn off the
        // agent loop onto mcp_infer_completion, killing tool calls and
        // selection_reason.
        assert!(args.get("cognitive_profile").is_none());
    }

    #[test]
    fn sync_tool_args_omit_blank_optionals() {
        let input: ChatTurnInput = serde_json::from_value(serde_json::json!({
            "session_id": "s1", "content": "hi", "model_override": "   ", "tier": ""
        })).expect("input");
        let args = sync_tool_args(&input);
        assert!(args.get("model_override").is_none());
        assert!(args.get("tier").is_none());
    }

    #[test]
    fn background_input_maps_every_routing_field() {
        let input: ChatTurnInput = serde_json::from_value(serde_json::json!({
            "session_id": "s1", "content": "refactor the parser",
            "execution": "background",
            "model_override": "m1", "tier": "mesh",
            "clutch": "efficiency", "risk": "moderate",
            "context_files": ["a.rs", "b.rs"], "priority": "urgent",
            "dry_run": true, "active_skill": "ponytail", "allow_duplicate": false
        })).expect("input");
        let out = background_input(&input);
        assert_eq!(out.description, "refactor the parser");
        assert_eq!(out.files, vec!["a.rs".to_string(), "b.rs".to_string()]);
        assert_eq!(out.model_override.as_deref(), Some("m1"));
        assert_eq!(out.tier.as_deref(), Some("mesh"));
        assert_eq!(out.clutch.as_deref(), Some("efficiency"));
        assert_eq!(out.priority.as_deref(), Some("urgent"));
        assert_eq!(out.allow_duplicate, Some(false));
        assert_eq!(out.chat_session_id.as_deref(), Some("s1"));
        assert!(out.task_category.is_none());
    }
}
```

- [ ] **Step 3: Run — FAIL**

Run: `cargo test -p vox-gui chat_turn::tests`
Expected: FAIL — module contents undefined.

- [ ] **Step 4: Implement**

Prepend to `chat_turn.rs`:

```rust
//! The single chat dispatch command.
//!
//! Before this module the composer forked in TypeScript: `task_category ==
//! 'chat'` early-returned to `chat_send_message` (4 fields) while everything
//! else went to `submit_orchestrator_task` (16). Half the composer's controls
//! therefore did nothing on a quick chat. The fork now lives here, as one
//! `match` over one struct.
//!
//! NOTE the frontend still branches — on *store lifecycle*, not on dispatch.
//! See the spec §6: `submitResolved` is the only writer of `taskToSession`,
//! which routes every live agent event to a bubble.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::control_plane::SubmitTaskInput;
use crate::commands::daemon::PersistentDaemon;   // NB: commands::daemon, not crate::daemon
use crate::commands::gui_db_pool::{GuiDbPool, map_db_err};

/// Routing fields that must exist on both input structs.
pub const ROUTING_FIELDS: &[&str] = &[
    "priority", "model_override", "tier", "dry_run", "active_skill",
    "clutch", "risk", "allow_duplicate", "grounding_check_enabled",
    "chat_session_id",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Execution {
    #[default]
    Sync,
    Background,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ChatTurnInput {
    pub session_id: String,
    pub content: String,
    #[serde(default)] pub execution: Execution,
    #[serde(default)] pub model_override: Option<String>,
    /// Composer "Run on" tier: local|mesh|cloud|auto. NOT `cognitive_profile`.
    #[serde(default)] pub tier: Option<String>,
    #[serde(default)] pub clutch: Option<String>,
    #[serde(default)] pub risk: Option<String>,
    #[serde(default)] pub context_files: Vec<String>,
    #[serde(default)] pub active_skill: Option<String>,
    #[serde(default)] pub skill_exclusions: Vec<String>,
    #[serde(default)] pub grounding_check_enabled: Option<bool>,
    #[serde(default)] pub priority: Option<String>,
    #[serde(default)] pub dry_run: Option<bool>,
    #[serde(default)] pub allow_duplicate: Option<bool>,
    #[serde(default)] pub chat_session_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ChatTurnDto {
    /// `0` on the background branch: that path persists no assistant row.
    pub id: i64,
    pub role: String,
    pub content: String,
    pub created_at: String,
    pub task_id: Option<String>,
    pub model_id: Option<String>,
    pub latency_ms: Option<u64>,
    pub selection_reason: Option<String>,
    pub grounding_flagged: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duplicate_of: Option<String>,
}

fn non_blank(v: &Option<String>) -> Option<&str> {
    v.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

/// `vox_chat_message` args. `ChatMessageParams` publishes
/// `additionalProperties: true` via a hand-written literal, so unknown keys are
/// tolerated — but they are also silently ignored until the struct AND the
/// literal at `input_schemas.rs:626-628` are updated (Task A2).
pub fn sync_tool_args(input: &ChatTurnInput) -> serde_json::Value {
    let mut args = serde_json::json!({
        "prompt": input.content,
        "session_id": input.session_id,
    });
    let obj = args.as_object_mut().expect("json! object");
    if !input.context_files.is_empty() {
        obj.insert("context_files".into(), serde_json::json!(input.context_files));
    }
    if !input.skill_exclusions.is_empty() {
        obj.insert("skill_exclusions".into(), serde_json::json!(input.skill_exclusions));
    }
    for (key, val) in [
        ("model_override", non_blank(&input.model_override)),
        ("tier", non_blank(&input.tier)),
        ("clutch", non_blank(&input.clutch)),
        ("risk", non_blank(&input.risk)),
        ("active_skill", non_blank(&input.active_skill)),
    ] {
        if let Some(v) = val {
            obj.insert(key.into(), serde_json::json!(v));
        }
    }
    args
}

/// Total mapping onto the existing background dispatch input.
pub fn background_input(input: &ChatTurnInput) -> SubmitTaskInput {
    SubmitTaskInput {
        description: input.content.clone(),
        files: input.context_files.clone(),
        priority: input.priority.clone(),
        session_id: Some(input.session_id.clone()),
        mode: None,
        tier: input.tier.clone(),
        // `model_hint` is dead on the wire: the daemon's SUBMIT_TASK handler
        // never reads it. Only `tier` -> enqueue_hints.model_preference works.
        model_hint: None,
        allow_duplicate: input.allow_duplicate,
        dry_run: input.dry_run,
        active_skill: input.active_skill.clone(),
        clutch: input.clutch.clone(),
        risk: input.risk.clone(),
        model_override: input.model_override.clone(),
        task_category: None,
        grounding_check_enabled: input.grounding_check_enabled,
        chat_session_id: Some(input.session_id.clone()),
    }
}

#[tauri::command]
pub async fn chat_turn(
    app_handle: tauri::AppHandle,
    input: ChatTurnInput,
    pool: State<'_, GuiDbPool>,
    daemon: State<'_, Arc<PersistentDaemon>>,
) -> Result<ChatTurnDto, String> {
    if input.session_id.trim().is_empty() {
        return Err("session_id must not be empty".to_string());
    }
    if input.content.trim().is_empty() {
        return Err("content must not be empty".to_string());
    }
    match input.execution {
        Execution::Sync => run_sync(input, pool, daemon).await,
        Execution::Background => run_background(app_handle, input, daemon).await,
    }
}

async fn run_sync(
    input: ChatTurnInput,
    pool: State<'_, GuiDbPool>,
    daemon: State<'_, Arc<PersistentDaemon>>,
) -> Result<ChatTurnDto, String> {
    let addr = daemon.ensure().await?;
    let client = match daemon.token().await {
        Some(token) => vox_orchestrator::orch_daemon::OrchDaemonClient::with_token(addr, token),
        None => vox_orchestrator::orch_daemon::OrchDaemonClient::new(addr),
    };
    let envelope = client
        .call(
            vox_foundation::protocol::orch_daemon_method::TOOL_CALL,
            serde_json::json!({ "name": "vox_chat_message", "args": sync_tool_args(&input) }),
        )
        .await
        .map_err(|e| e.to_string())?;
    let reply = crate::commands::chat::parse_chat_message_envelope(&envelope)?;
    let grounding_flagged = if input.grounding_check_enabled == Some(true) {
        Some(vox_orchestrator::grounding::assess_reply_confidence(&reply.content).flagged)
    } else {
        None
    };
    let db = pool.handle()?;
    let conv_id = db
        .chat_ensure_gui_session(&input.session_id, "Chat")
        .await
        .map_err(map_db_err)?;
    let dto = crate::commands::chat::persist_assistant_reply(
        &db, conv_id, &reply.content, reply.model_id.as_deref(),
        reply.latency_ms, reply.selection_reason.as_deref(), grounding_flagged,
    ).await?;
    Ok(ChatTurnDto {
        id: dto.id,
        role: dto.role,
        content: dto.content,
        created_at: dto.created_at,
        // Always None today: vox_chat_message's payload has no task_id.
        // Phase D adds one; do not build correlation on this yet.
        task_id: None,
        model_id: dto.model_id,
        latency_ms: dto.latency_ms,
        selection_reason: dto.selection_reason,
        grounding_flagged: dto.grounding_flagged,
        duplicate_of: None,
    })
}

/// Dispatch only. Deliberately persists NOTHING: today's background path writes
/// no assistant row, and a persisted "Dispatched as task #N" receipt would
/// hydrate on reload in place of the live task bubble.
async fn run_background(
    app_handle: tauri::AppHandle,
    input: ChatTurnInput,
    daemon: State<'_, Arc<PersistentDaemon>>,
) -> Result<ChatTurnDto, String> {
    let result = crate::commands::control_plane::submit_orchestrator_task(
        app_handle,
        background_input(&input),
        daemon,
    ).await?;
    Ok(ChatTurnDto {
        id: 0,
        role: "assistant".to_string(),
        content: String::new(),
        created_at: String::new(),
        task_id: result.task_id,
        model_id: None,
        latency_ms: None,
        selection_reason: None,
        grounding_flagged: None,
        duplicate_of: result.duplicate_of,
    })
}
```

Add `pub mod chat_turn;` to `commands/mod.rs`, and `commands::chat_turn::chat_turn,` to `main.rs`'s `generate_handler!` list after line 163.

- [ ] **Step 5: Run — PASS**

Run: `cargo test -p vox-gui chat_turn`
Expected: PASS (6 tests).

- [ ] **Step 6: Regenerate SSOT and commit**

```bash
cargo run -p vox-cli -- ci gui-surface-coverage --write
cargo fmt -p vox-gui
git add crates/vox-gui/src contracts/reports/gui-surface-coverage.v1.json
git commit -m "feat(gui): chat_turn — one dispatch carrying every composer control"
```

---

## Task A2: Make the daemon honour the new keys

Without this, A1's args are accepted and discarded — the test passes while the feature does nothing.

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/chat_tools/params.rs`
- Modify: `crates/vox-orchestrator-mcp/src/chat_tools/chat/message.rs`
- Modify: `crates/vox-orchestrator-mcp/src/input_schemas.rs` (the hand-written literal)

**Observable:** a `vox_chat_message` call carrying `model_override: "<id>"` returns `model_used == "<id>"`. Before this task it returns the auto-routed model.

- [ ] **Step 1: Write the failing test**

In `message.rs`'s existing `tests` module:

```rust
    #[test]
    fn request_override_beats_process_global() {
        assert_eq!(effective_model_pref(Some("req"), Some("glob")), Some("req".into()));
    }

    #[test]
    fn falls_back_to_process_global() {
        assert_eq!(effective_model_pref(None, Some("glob")), Some("glob".into()));
    }

    #[test]
    fn blank_request_override_is_not_a_pick() {
        // A blank must not shadow the global, and must never reach the
        // resolver — which treats "" as an explicit, unresolvable model id.
        assert_eq!(effective_model_pref(Some("  "), Some("glob")), Some("glob".into()));
        assert_eq!(effective_model_pref(Some(""), None), None);
    }

    #[test]
    fn tier_maps_to_resolution_not_cognitive_profile() {
        let r = resolution_for_tier(Some("local"), McpChatModelResolution::default());
        assert!(r.enforce_free_tier_only || r.allow_cheapest_fallback);
        let auto = resolution_for_tier(None, McpChatModelResolution::default());
        assert!(auto.allow_cheapest_fallback);
    }
```

- [ ] **Step 2: Run — FAIL.** `cargo test -p vox-orchestrator-mcp effective_model_pref`

- [ ] **Step 3: Implement**

Add to `ChatMessageParams` (after `skill`, `params.rs:88`):

```rust
    /// Per-request model pick from the GUI's ChatModelPicker. Takes precedence
    /// over the daemon's process-global `mcp_chat_model_override`, which stays
    /// for MCP clients with no per-request channel.
    #[serde(default)] pub model_override: Option<String>,
    /// Composer runtime tier: local|mesh|cloud|auto. Feeds
    /// `McpChatModelResolution`, NOT `cognitive_profile`.
    #[serde(default)] pub tier: Option<String>,
    #[serde(default)] pub clutch: Option<String>,
    #[serde(default)] pub risk: Option<String>,
    /// Session-scoped skills the user rejected in the transcript (Phase E).
    #[serde(default)] pub skill_exclusions: Vec<String>,
```

In `message.rs`:

```rust
/// Model preference for this turn: per-request pick, then the process-global
/// override, then nothing (auto-selector runs). Blanks are not picks.
pub fn effective_model_pref(request: Option<&str>, global: Option<&str>) -> Option<String> {
    request.map(str::trim).filter(|s| !s.is_empty())
        .or_else(|| global.map(str::trim).filter(|s| !s.is_empty()))
        .map(str::to_string)
}

/// Composer tier -> resolution constraints. Deliberately NOT `cognitive_profile`:
/// that field's values are fast|reasoning|creative and setting it switches the
/// turn off the agent loop onto `mcp_infer_completion`, losing tool calls and
/// the selection rationale.
pub fn resolution_for_tier(
    tier: Option<&str>,
    base: McpChatModelResolution,
) -> McpChatModelResolution {
    match tier.map(str::trim) {
        Some("local") => McpChatModelResolution { enforce_free_tier_only: true, ..base },
        Some("mesh")  => McpChatModelResolution { free_tier_latency_critical: true, ..base },
        Some("cloud") => McpChatModelResolution { allow_cheapest_fallback: false, ..base },
        _ => McpChatModelResolution { allow_cheapest_fallback: true, ..base },
    }
}
```

Thread `params.model_override.as_deref()` into `try_run_agent_turn` and replace its `pref` binding with `effective_model_pref(request_override, global.as_deref())`. Apply the same substitution at the other two `mcp_chat_model_override.read()` sites in `message.rs` (~520, ~567). Build the `resolution_template` via `resolution_for_tier(params.tier.as_deref(), …)`.

- [ ] **Step 4: Edit the published schema literal**

In `input_schemas.rs`, the `vox_chat_message` entry (~line 626) is a hand-written literal, not `derived_tool_schema!`. Add `model_override`, `tier`, `clutch`, `risk`, `skill_exclusions` to its `properties`. Nothing fails if you skip this — which is why it is an explicit step.

- [ ] **Step 5: Run — PASS.** `cargo test -p vox-orchestrator-mcp chat_tools::chat::message`

- [ ] **Step 6: Commit**

```bash
cargo fmt -p vox-orchestrator-mcp
git add crates/vox-orchestrator-mcp/src
git commit -m "feat(mcp): honour per-request model_override and composer tier"
```

---

## Task A3: Frontend — one payload, two lifecycles

**Files:**
- Create: `crates/vox-gui/ui/src/lib/buildChatTurn.ts` + `.test.ts`
- Modify: `transport.ts`, `lib/chatSend.ts`, `App.tsx`, `Loquela.tsx`, `App.test.tsx`

**Observable:** picking a model and sending a quick chat produces an IPC call whose payload contains that model id. Background sends still populate `taskToSession` and still stream tokens into their bubble.

- [ ] **Step 1: Write the failing tests**

Create `crates/vox-gui/ui/src/lib/buildChatTurn.test.ts` (pure logic — no jsdom pragma needed):

```typescript
import { describe, it, expect } from 'vitest';
import { buildChatTurn, CHAT_TURN_KEYS } from './buildChatTurn';

const full = {
  description: 'harden the crypto invariants',
  priority: 'urgent', active_skill: 'ponytail', tier: 'cloud',
  dry_run: false, clutch: 'genius', risk: 'low',
  context: [
    { kind: 'file', ref: 'crates/vox-crypto/src/lib.rs' },
    { kind: 'agent', ref: 'A-01' },
  ],
  execution_mode: 'chat' as const,
};

describe('buildChatTurn', () => {
  it('carries every composer control', () => {
    const out = buildChatTurn(full, {
      sessionId: 's1', modelOverride: 'openrouter/anthropic/claude-opus-5',
      groundingCheckEnabled: true,
    });
    expect(out.model_override).toBe('openrouter/anthropic/claude-opus-5');
    expect(out.tier).toBe('cloud');
    expect(out.clutch).toBe('genius');
    expect(out.risk).toBe('low');
    expect(out.priority).toBe('urgent');
    expect(out.grounding_check_enabled).toBe(true);
    // agent chips are not files
    expect(out.context_files).toEqual(['crates/vox-crypto/src/lib.rs']);
  });

  it('takes execution from the composer switch, not a legacy sentinel', () => {
    expect(buildChatTurn(full, { sessionId: 's1' }).execution).toBe('sync');
    expect(buildChatTurn({ ...full, execution_mode: 'task' }, { sessionId: 's1' }).execution)
      .toBe('background');
  });

  it('emits a stable key set', () => {
    const out = buildChatTurn(full, { sessionId: 's1' });
    expect(Object.keys(out).sort()).toEqual([...CHAT_TURN_KEYS].sort());
  });

  it('has no intent field', () => {
    // Loquela folds intent into `description` via composeDescription. A field
    // here would be permanently null, and double-counted if Loquela ever
    // emitted it without removing the fold.
    expect(CHAT_TURN_KEYS).not.toContain('intent');
  });
});
```

- [ ] **Step 2: Run — FAIL.** `pnpm -C crates/vox-gui/ui exec vitest run src/lib/buildChatTurn.test.ts`

- [ ] **Step 3: Implement the builder**

```typescript
// crates/vox-gui/ui/src/lib/buildChatTurn.ts
//
// The single seam between the composer and the backend. Before this file,
// App.tsx forked on `task_category === 'chat'` and the sync branch mapped four
// fields while the background branch mapped sixteen — so the model picker,
// tier, chips, priority, clutch, risk and dry-run silently did nothing on a
// quick chat. One mapping now, guarded by the key-set assertion.
import { contextRefsFromPayload } from './loquelaContext';
import type { ChatTurnInput } from '../transport';

export const CHAT_TURN_KEYS = [
  'session_id', 'content', 'execution', 'model_override', 'tier',
  'clutch', 'risk', 'context_files', 'active_skill', 'skill_exclusions',
  'grounding_check_enabled', 'priority', 'dry_run', 'allow_duplicate',
] as const;

export interface BuildChatTurnCtx {
  sessionId: string;
  modelOverride?: string | null;
  groundingCheckEnabled?: boolean;
  activeSkillId?: string | null;
  skillExclusions?: string[];
  allowDuplicate?: boolean;
}

export function buildChatTurn(
  payload: Record<string, unknown> & { description: string; execution_mode?: 'chat' | 'task' },
  ctx: BuildChatTurnCtx,
): ChatTurnInput {
  return {
    session_id: ctx.sessionId,
    content: payload.description,
    execution: payload.execution_mode === 'task' ? 'background' : 'sync',
    model_override: (payload.model_override as string) ?? ctx.modelOverride ?? null,
    tier: (payload.tier as string) ?? null,
    clutch: (payload.clutch as string) ?? null,
    risk: (payload.risk as string) ?? null,
    context_files: contextRefsFromPayload(payload),
    active_skill: (payload.active_skill as string) ?? ctx.activeSkillId ?? null,
    skill_exclusions: ctx.skillExclusions ?? [],
    grounding_check_enabled: ctx.groundingCheckEnabled ?? null,
    priority: (payload.priority as string) ?? null,
    dry_run: (payload.dry_run as boolean) ?? null,
    allow_duplicate: ctx.allowDuplicate ?? null,
  };
}
```

- [ ] **Step 4: Transport types**

Replace `ChatSendInput`/`chatSendMessage` in `transport.ts` with `ChatTurnInput`, `ChatTurnDto`, and `chatTurn()` using `safeInvoke('chat_turn', { input })`. Keep both inside the `__VOX_RAW_IPC_BEGIN__`/`END` markers if any raw `invoke`/`listen` is used (`transportIpcGuard` enforces this).

- [ ] **Step 5: `Loquela.send()` emits `execution_mode`**

Replace `task_category: executionMode === 'chat' ? 'chat' : undefined` with `execution_mode: executionMode`, deleting the sentinel in the same change so the new contract is not derived from the retiring one.

- [ ] **Step 6: `App.tsx` — one dispatch, two lifecycles**

Replace the two blocks (`App.tsx:962-1027` and `:1029-1108`) with:

```tsx
    const turn = buildChatTurn(payload, {
      sessionId,
      modelOverride: chatModelOverride,
      groundingCheckEnabled,
      activeSkillId: activeSkill?.id ?? null,
      skillExclusions,
      allowDuplicate: false,
    });

    if (turn.execution === 'background') {
      // Long-lived correlated stream. `submitResolved` is the ONLY writer of
      // taskToSession, which routes every task_*/token_streamed frame to this
      // bubble and replays the 30s pending buffer. Do not collapse this into
      // chatPending/chatReplySettled.
      let runId = '';
      try {
        const result = await executeIpcWithRun<ChatTurnDto>(
          'chat_turn', { input: turn }, 'gui.loquela.submit',
          (id) => {
            runId = id;
            dispatchSessionChat({ type: 'submit', sessionId, runId: id,
              prompt: String(payload.description ?? '') });
          },
        );
        if (result?.task_id == null && result?.duplicate_of) {
          if (runId) dispatchSessionChat({ type: 'failRun', sessionId, runId,
            error: `Skipped — near-duplicate of task #${result.duplicate_of}` });
          if (!window.confirm(`This looks like a near-duplicate of task #${result.duplicate_of}.\n\nSubmit it anyway?`)) {
            pushToast({ tone: 'info', title: 'Duplicate skipped',
              body: `Kept existing task #${result.duplicate_of}.`, cause: 'backend-ok' });
            return;
          }
          const retry = await executeIpcWithRun<ChatTurnDto>(
            'chat_turn', { input: { ...turn, allow_duplicate: true } },
            'gui.loquela.submit',
            (id) => { runId = id; dispatchSessionChat({ type: 'submit', sessionId, runId: id,
              prompt: String(payload.description ?? '') }); },
          );
          if (runId && retry?.task_id != null) {
            dispatchSessionChat({ type: 'submitResolved', sessionId, runId, taskId: String(retry.task_id) });
            recordGamifyGuiEvent('task_submitted', { session_id: sessionId, task_id: String(retry.task_id) },
              { enabled: gamifySettings.enabled });
          }
          checkBudgetWarn(sessionId);
          return;
        }
        if (runId && result?.task_id != null) {
          dispatchSessionChat({ type: 'submitResolved', sessionId, runId, taskId: String(result.task_id) });
          recordGamifyGuiEvent('task_submitted', { session_id: sessionId, task_id: String(result.task_id) },
            { enabled: gamifySettings.enabled });
        }
        checkBudgetWarn(sessionId);
      } catch (err) {
        pushToast(dispatchErrorToast(err, 'Dispatch Failed'));
      }
      return;
    }

    // Sync: terminal request/response.
    chatSendInFlightRef.current.add(sessionId);
    const tempId = nextGuiRunId();
    dispatchSessionChat({ type: 'chatPending', sessionId, tempId,
      userText: String(payload.description ?? '') });
    try {
      const reply = await sendChatTurn(turn);
      const persisted = persistedAssistantIdsRef.current.get(sessionId) ?? new Set<string>();
      persisted.add(reply.id);                    // ChatMessage.id is a string
      persistedAssistantIdsRef.current.set(sessionId, persisted);
      dispatchSessionChat({
        type: 'chatReplySettled', sessionId, tempId,
        result: { ok: true, message: {
          id: reply.id, role: 'assistant', text: reply.text, status: 'done',
          runId: tempId, modelId: reply.modelId, latencyMs: reply.latencyMs,
          selectionReason: reply.selectionReason,
          groundingFlagged: reply.groundingFlagged,
        } },
      });
      checkBudgetWarn(sessionId);
    } catch (err) {
      dispatchSessionChat({ type: 'chatReplySettled', sessionId, tempId,
        result: { ok: false, error: sanitizeErrorForToast(err) } });
      pushToast(dispatchErrorToast(err, 'Chat reply failed'));
    } finally {
      chatSendInFlightRef.current.delete(sessionId);
    }
```

Add `chatModelOverride`, `groundingCheckEnabled`, `skillExclusions` to the `useCallback` dependency array — the current deps are correct only because those values arrive as payload fields today.

- [ ] **Step 7: Rewrite the broken tests**

`App.test.tsx` has ~10 tests mocking `chat_send_message`, of which three assert the deleted fork by name (`'a plain chat send calls chat_send_message and not submit_orchestrator_task'` at `:577`, plus `:663`, `:733`). Those three are **rewritten, not repointed**: the new distinction is `execution: 'sync'` vs `'background'` on one command. The rest repoint to `chat_turn`.

Also update the two live Rust tests in `chat.rs` (`:796`, `:912`) — keep `chat_send_message` registered as a thin shim delegating to `chat_turn` with `execution: Sync`, so old frontends keep working, and point those tests at the shim.

- [ ] **Step 8: Run — PASS**

Run: `pnpm -C crates/vox-gui/ui exec vitest run src/lib/buildChatTurn.test.ts src/App.test.tsx && cargo test -p vox-gui chat`

- [ ] **Step 9: Sweep for orphaned references**

```bash
grep -rn "chat_send_message" docs/ contracts/ .github/ || true
```

Update any doc reference; a stale one trips `vox ci check-links`.

- [ ] **Step 10: Commit**

```bash
git add crates/vox-gui/ui/src crates/vox-gui/src/commands/chat.rs
git commit -m "feat(gui): single chat dispatch, two store lifecycles preserved"
```

---

# Phase B — Honest selection

## Task B1: `SelectionDto` classified from the resolved model

**Files:** `chat_tools/chat/message.rs`, `llm_bridge/model_route_policy/resolve.rs`, `chat_turn.rs`

**Observable:** pin a model, remove it from the registry, send a turn — the badge says "Fell back" and names the requested id. Before this, it says "Your pick" over a model the user did not get.

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn source_is_classified_from_the_resolved_model_not_the_request() {
        // The honesty bug: classifying from the request means a pinned model
        // that the resolver silently ignored still reads "Your pick".
        assert_eq!(SelectionSource::classify(Some("m"), Some("m"), None), SelectionSource::UserOverride);
        assert_eq!(SelectionSource::classify(Some("gone"), Some("other"), None), SelectionSource::Fallback);
        assert_eq!(SelectionSource::classify(None, Some("auto"), None), SelectionSource::AutoRouted);
        assert_eq!(SelectionSource::classify(None, Some("g"), Some("g")), SelectionSource::Global);
    }

    #[test]
    fn fallback_always_carries_a_rationale() {
        let d = SelectionDto::fallback("free-x", "requested `gone` is not in the registry");
        assert_eq!(d.source, SelectionSource::Fallback);
        assert!(d.rationale.expect("rationale").contains("not in the registry"));
    }
```

- [ ] **Step 2: Run — FAIL.** `cargo test -p vox-orchestrator-mcp selection`

- [ ] **Step 3: Implement**

`SelectionSource::classify(requested, resolved, global)` returns `UserOverride` only when `requested == resolved`; a requested-but-unresolved id yields `Fallback`. Serialize `SelectionDto` into the envelope at `message.rs:1058` beside the retained `selection_reason` key.

Give all **three** `None` sites truthful rationales:
- `:605` — `"cognitive profile '<p>' → complexity <n>; rationale not surfaced"` (not a fallback)
- `:633` — `"Fallback: cognitive-profile resolution failed (<err>)"`
- `:687` — `"Fallback: attachment present, or provider shape unmapped"`

In `resolve_mcp_chat_model_sync_inner:333-352`, set `rationale_out` on the unknown-`pref` path instead of falling through mutely; stop swallowing it in `try_run_agent_turn` when the pref was user-supplied.

- [ ] **Step 4: Run — PASS.** **Step 5: Commit.**

## Task B2: Badge and registered storage key

**Files:** `ModelBadge.tsx` + test, `ChatModelPicker.tsx`, `App.tsx`, `lib/shellPersistence.ts`, `contracts/gui/shell-persistence.v1.yaml`

**Observable:** the badge popover distinguishes "Your pick" / "Auto-routed" / "Fell back"; the pick survives reload; a pick whose model has left the registry is cleared on mount.

- [ ] **Step 1: Failing test** — `ModelBadge.test.tsx` with `// @vitest-environment jsdom` on line 1, asserting each `source` renders its distinct label and that `fallback` shows the rationale.
- [ ] **Step 2: Run — FAIL.**
- [ ] **Step 3:** Add `selection?: SelectionDto` to `ModelBadgeProps`; render the source line above the reason inside the existing popover.
- [ ] **Step 4:** Register `chatModelOverride: 'vox_chat_model.v1'` in `SHELL_PREFERENCE_KEYS` and `contracts/gui/shell-persistence.v1.yaml`; change `App.tsx:319` to `useLocalStorage`; validate against `listModels()` on mount and clear if absent. No picker-local storage.
- [ ] **Step 5: Run — PASS. Step 6: Commit.**

---

# Phase C — Typed errors from real strings

## Task C1: `ChatTurnError`

**Files:** `chat_turn.rs`, `App.tsx`, `lib/backendGuard.ts`

**Observable:** exceeding the session budget produces the "Budget limit reached" toast, and a free-tier cap produces "Free tier limit reached" — both of which the first draft would have silently downgraded to the generic toast.

- [ ] **Step 1: Write the failing test using fixtures taken from the emitters**

```rust
    #[test]
    fn classifies_the_strings_production_actually_emits() {
        // Every error is wrapped as `format!("LLM error: {e}")` before it
        // reaches the GUI — hence `contains`, not `starts_with`.
        assert!(matches!(
            classify_turn_error("LLM error: RATE_LIMITED: openrouter free tier 50/day"),
            ChatTurnError::RateLimited { .. }));
        assert!(matches!(
            classify_turn_error("LLM error: CONTEXT_LENGTH_EXCEEDED: 200000 > 128000"),
            ChatTurnError::ContextExceeded { .. }));
        // Real BudgetGuardError Display, NOT the invented "budget exceeded: …".
        assert!(matches!(
            classify_turn_error("Session budget of $5.00 exceeded (spent $5.10)"),
            ChatTurnError::BudgetExceeded { .. }));
        assert!(matches!(
            classify_turn_error("Daily budget of $20.00 exceeded (spent $20.03)"),
            ChatTurnError::BudgetExceeded { .. }));
        assert!(matches!(classify_turn_error("connection refused"), ChatTurnError::Backend { .. }));
    }
```

- [ ] **Step 2: Run — FAIL.**
- [ ] **Step 3:** Implement with a struct-variant enum (`#[serde(tag = "kind", rename_all = "snake_case")]`), matching after unwrapping the `"LLM error: "` prefix and using the same `^(Daily|Session) budget of \$` shape as `BUDGET_EXCEEDED_PATTERN`. Re-export the two prefix constants from `vox-actor-runtime`'s `llm/mod.rs` (`mod chat;` is private today) **or** inline the literals — there are zero other Rust consumers.
- [ ] **Step 4:** Frontend `dispatchErrorToast(err: unknown, …)` matches on `kind`, calling `sanitizeErrorForToast` only in the fallthrough. Pass the raw `err`, never the sanitized string.
- [ ] **Step 5: Run — PASS. Step 6: Commit.**

---

# Phase E — Skills

## Task E1: Events from tool results, in a new row component

**Files:** `agent_loop.rs`, `message.rs`, `chat_turn.rs`, new `ChatTurnEventRow.tsx` + test, `ChatTranscript.tsx`, `types/dashboard.ts`

**Observable:** a turn where the model loads a skill shows a chip naming it; a turn where the load is *denied* shows no such chip. `ChatAgentEventRow` and its three HITL controls are untouched.

- [ ] **Step 1: Failing test**

```rust
    #[test]
    fn skill_event_comes_from_the_result_not_the_call() {
        // Emitting from args would render "skill activated · X" for a call that
        // was denied, unknown, or errored — letting injected content assert in
        // system-styled UI that a trusted skill ran.
        assert!(turn_event_for_result("vox_skill_use", &json!({"id":"ponytail"}), true).is_some());
        assert!(turn_event_for_result("vox_skill_use", &json!({"id":"ponytail"}), false).is_none());
    }

    #[test]
    fn unknown_skill_ids_are_labelled_unknown() {
        let ev = turn_event_for_result("vox_skill_use", &json!({"id":"../../etc/passwd"}), true)
            .expect("event");
        assert_eq!(ev["skill_id"], "unknown");
    }
```

- [ ] **Step 2: Run — FAIL. Step 3:** Implement `turn_event_for_result`, resolving skill ids against the registry and truncating model-authored strings to 200 chars. Thread `events: Vec<Value>` onto `AgentTurnOutcome` → `AgentTurnResult`; the binding at `message.rs:540` becomes a 5-tuple across all four match arms (three supply `vec![]`).
- [ ] **Step 4:** Create `ChatTurnEventRow.tsx` for `TurnEventDto` — a **new** component — and add its render site in `ChatTranscript.tsx` **in this task**. Leave `ChatAgentEventRow` alone. Test file carries the jsdom pragma and asserts an unknown `kind` renders without throwing.
- [ ] **Step 5: Run — PASS. Step 6: Commit.**

## Task E2: `skill_exclusions` honoured, and "not this one" re-runs

**Observable:** clicking "not this one" immediately re-runs the turn and the excluded skill is absent from the system prompt.

- [ ] **Step 1:** Failing test in the skill-selection path: an excluded id is absent from the rendered catalog and cannot be pinned.
- [ ] **Step 2: Run — FAIL. Step 3:** Filter `skill_exclusions` in `render_skill_catalog`'s caller and in the pinned-skill path. This backend half is load-bearing; the first draft added the field and never read it.
- [ ] **Step 4:** Wire the chip's `onExcludeSkill` to append to session state **and** re-dispatch the turn.
- [ ] **Step 5: Run — PASS. Step 6: Commit.**

---

# Phase F — Planning

## Task F1: `/plan` issues `vox_plan`

**Observable:** `/plan add a health endpoint` renders plan nodes in `PlanPanel`. Before this it returns prose.

- [ ] **Step 1:** Failing test on `plan_tool_args`: produces `{goal, session_id, require_approval: true}` and **no** `mode`/`prompt` key (`vox_plan`'s schema is `additionalProperties: false`, so a stray key is a hard reject).
- [ ] **Step 2: Run — FAIL. Step 3:** `chat_turn` gains a `plan` execution or a `PlanTurnInput`; the GUI's `/plan` calls `vox_plan` with `{goal: <composer text>, session_id, require_approval: true}`. `ChatTurnDto` returns `plan_session_id`/`version`; `ChatSurface` points `PlanPanel` at it.
- [ ] **Step 4:** Remove `/plan` from `INTERNAL_MODE_SLASHES`, add to `APP_SLASH_COMMANDS`. Check `slashRouter.test.ts` for an existing `resolveInternalModeSlash('/plan') === 'plan'` assertion and delete it. Confirm `LoquelaModeId` still compiles with `'plan'` retained.
- [ ] **Step 5: Run — PASS. Step 6: Commit.**

## Task F2: Approval gate, opt-in, reusing `blocked_on_approval`

**Observable:** a GUI `/plan` renders "N steps awaiting approval" and dispatches nothing until approved. `vox plan` from the CLI behaves exactly as before.

- [ ] **Step 1: Write the failing test**

```rust
    #[tokio::test]
    async fn approve_flips_blocked_nodes_and_leaves_finished_ones() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("db");
        db.upsert_plan_node("s", 1, "n1", "a", "[]", "{}", "blocked_on_approval", None).await.unwrap();
        db.upsert_plan_node("s", 1, "n2", "b", "[]", "{}", "completed", None).await.unwrap();
        approve_plan_inner(&db, "s", 1).await.expect("approve");
        let rows = db.load_plan_nodes_with_status("s", 1).await.unwrap();
        let by = |id: &str| rows.iter().find(|r| r.node_id == id).unwrap().status.clone();
        assert_eq!(by("n1"), "pending");
        assert_eq!(by("n2"), "completed");
    }

    #[tokio::test]
    async fn non_gui_callers_still_get_runnable_plans() {
        // require_approval defaults false: ai.plan.execute, goal.rs's submit
        // path, and successor-node scheduling all break silently otherwise.
        assert_eq!(initial_node_status(false), "pending");
        assert_eq!(initial_node_status(true), "blocked_on_approval");
    }
```

- [ ] **Step 2: Run — FAIL.**
- [ ] **Step 3:** `approve_plan_inner` delegates to the existing `VoxDb::approve_all_blocked_plan_nodes` (`ops_planning.rs:349-368`, zero callers today) — one SQL statement, no new status vocabulary. `plan.rs:516` writes `initial_node_status(params.require_approval)`. Add `require_approval: bool` to `PlanParams` **and** to `vox_plan`'s schema literal (`additionalProperties: false` means an unlisted key is rejected).
- [ ] **Step 4:** Add an explicit `row_to_plan_node` arm for `blocked_on_approval` rather than letting it alias to `PlanStatus::Pending` (`schedule.rs:19-27`). Route `insert_plan_node` through the same gate so the GUI cannot create a runnable node inside an unapproved plan.
- [ ] **Step 5:** `PlanPanel` footer with Approve / Discard; do **not** add `add_plan_node` — `insert_plan_node` already ships.
- [ ] **Step 6: Run — PASS.**
- [ ] **Step 7: Write the rollback note into the commit message**

```
Rollback requires a data fix-up, not just a revert:
  UPDATE plan_nodes SET status='pending' WHERE status='blocked_on_approval';
```

---

# Phase D — Delegation (three prerequisites, in order)

> Do not start D3 before D0–D2 land. Until `orch.subagent_tree` has a handler,
> `pausedAgentForSession` returns `undefined` always, and Interrupt/Resume
> **disappears** rather than occasionally mis-targeting — strictly worse than
> today.

## Task D0: Tool reachability

**Observable:** a default chat turn's offered tool list contains `vox_spawn_agent`. Today it is index 166 of 188 and the cut is at 40.

- [ ] **Step 1:** Failing test — `select_tools_for_turn` with a default `TurnContext` returns a set containing the three delegation tools, and still returns exactly `max_tools` entries.
- [ ] **Step 2: Run — FAIL.**
- [ ] **Step 3:** Add `pin_names: Vec<&'static str>` to `TurnContext`, applied before `.take(max_tools)`, mirroring the existing `exclude_name_prefixes` hatch. Whitelist the three in `is_skill_infrastructure_tool` so a pinned skill cannot remove them. Record which three tools now fall off the end.
- [ ] **Step 4:** Resolve the `dispatch.rs:140-142` inconsistency (it checks the process-global `active_skill_id`, not the per-turn `params.skill`).
- [ ] **Step 5: Run — PASS. Step 6: Commit.**

## Task D1: Durable lineage

**Observable:** restart the daemon; the delegation edge for a prior spawn is still readable.

- [ ] Failing test → `chat_session_id`/`origin_turn_id` on `AgentDelegationBinding` and `DelegationEdge` (`topology.rs:78-100`), populated in `spawn_dynamic_agent_with_parent` (`spawn.rs:88-96`), **persisted** — today it is a HashMap that dies with the process. Add `chat_session_id` to `SpawnAgentParams` and `SubmitTaskParams` (the latter has `schemars` `deny_unknown_fields`, so the derive must land before any caller sends it). Carry it on `TaskEnqueueHints` rather than a new positional arg to `submit_task_with_agent`.
- [ ] Injection at `agent_loop.rs:264-272`: `let mut args = call.arguments.clone();` then overwrite `chat_session_id` — guarding `Value::Null`, using a key distinct from `session_id` (read by `dispatch.rs:63` for telemetry), and leaving the recorded assistant `calls` un-injected.

## Task D2: `SUBAGENT_TREE` handler

**Observable:** `list_subagent_tree` returns a non-empty tree after a spawn. Today it always returns empty because the method has no handler.

- [ ] Failing test → write the `SUBAGENT_TREE` arm in `orch_daemon/mod.rs` returning `{"tree": …}` from `agent_topology_snapshot`'s existing `delegation_edges` (`accessors.rs:345-394`).

## Task D3: Correlation surfaces

- [ ] DTO fields on `SubagentTreeNode`; `agentsForSession`; `pausedAgentForSession` replacing the fleet-wide guess; delegation event rows; `/spawn` rewritten to carry the user's actual text. Note `fetchTree()` returns `SubAgentNode[]`, not edges — Task D3 needs the edge list, so add a sibling accessor rather than mistyping it.

---

# Phase G — Streaming

## Task G1: Sync path streams over the existing channel

**Observable:** tokens appear progressively in a quick-chat bubble.

- [ ] Failing test → the sync path uses `llm_stream` and emits `AgentEventKind::TokenStreamed` carrying `session_id`; `resolveSessionForEvent` routes a frame carrying one directly. **No new channel** — the first draft's per-session delta channel had no turn-id correlation, raced settled bubbles, and leaked listeners.

---

# Phase M — Measurement

## Task M0: Repair the metrics that are wrong

**Observable:** the Runs scoreboard stops showing 1.0 for every model; p50/p99 change value.

- [ ] Failing tests, then:
  - real percentiles for `p50_latency_ms`/`p99_latency_ms` (currently `AVG`/`MAX`, `ops_scientia.rs:174-175`)
  - real elapsed time in `infer_with_retry` (currently hardcoded `0`, `chat.rs:397-408`)
  - credit the **served** model in `record_bandit_task_outcome` (currently the requested one, `success/mod.rs:468-471`)
  - `list_model_cards` calls `inject_scoreboard`, as `vox model explain` already does
  - delete `vox_eval::quality_proxy_score` (response-length bands)
  - stop rendering `quality_score` until M2 gives it meaning
  - fix the stale claim at `model-catalog-ssot-2026.md:106` (`quality_weights` **is** consumed, `scoring.rs:93-107`)

## Task M1: Live writer for the three-axis join

**Observable:** a normal chat turn produces a `model_selection_event` + `harness_eval_task_result` pair joinable on `(run_id, task_id)`.

- [ ] The single highest-value item in this phase: every other metric is decoration without a solved/not-solved denominator. The schema exists (`harness_eval.rs:28-57`) and is populated only by `vox harness eval --live`.

## Task M2: Completeness gate and TTFT/TPOT

- [ ] `completeness_ok = NOT(finish_reason=='length') AND NOT(unbalanced fence) AND NOT(user re-asked within 2 turns)`; `success = tests_passed AND diff_retained AND completeness_ok`. Capture TTFT/TPOT at the egress boundary — neither is measured anywhere today.
- [ ] Failing test must include the case that motivates the whole design: an empty reply scores `success = false`. (`assess_reply_confidence` currently gives it `confidence: 1.0, flagged: false`.)

## Task M3: Surface

- [ ] `cost_per_success` (Σ cost over **all** attempts ÷ successes; "insufficient data" below 10), p95 TTFT/TPOT + goodput, pass@1 and pass^3 side by side. Pareto set + budget-constrained argmax, not a weighted sum. Credible intervals; suppress ranks below minimum-N. 5% forced-exploration stream.

---

# Task V: Verification — the actual gate

Playwright here runs plain Vite with an injected fake backend; **no Rust executes**, and `gui-playwright-smoke` is tiered off `main`/`full-ci` so it gates nothing. An e2e assertion against a mock the author wrote cannot fail when the code is wrong.

- [ ] **Step 1:** `vox run scripts/gui-build.vox && cargo tauri dev` with a live orchestrator daemon.
- [ ] **Step 2: Execute this checklist against the running app, recording the observed value for each row.**

| # | Control | Action | Observable to read |
|---|---|---|---|
| 1 | Model picker | pick a non-default model, Quick chat | `ModelBadge` names that model; popover says "Your pick" |
| 2 | Model picker, stale | pin a model, remove from registry, send | popover says "Fell back", names the requested id |
| 3 | Tier | set "local", send | reply served by a free-tier model; badge rationale mentions the constraint |
| 4 | `@`-chips | attach a file, send | reply references file contents |
| 5 | Priority / clutch / risk | set each, Background | task row shows the values |
| 6 | Dry-run | enable, Background | no side effects |
| 7 | Grounding | enable, send a hedged reply | flagged indicator appears |
| 8 | Skill pin | pin a skill, send | reply follows it; chip shows it |
| 9 | Skill auto | send a turn the model resolves with a skill | activation chip appears; "not this one" re-runs without it |
| 10 | `/plan` | `/plan <goal>` | plan nodes render; "N steps awaiting approval" |
| 11 | Approve | click Approve | nodes dispatch; CLI `vox plan` unaffected |
| 12 | `/spawn` | `/spawn <text>` | task carries the typed text, not a hardcoded string |
| 13 | Delegation | a turn that fans out | delegation rows appear; SubAgents tree non-empty |
| 14 | Interrupt/Resume | two paused agents fleet-wide | acts on this session's agent |
| 15 | Streaming | Quick chat | tokens appear progressively |
| 16 | Budget error | exceed the session cap | "Budget limit reached" toast, not the generic one |
| 17 | Persistence | pick a model, reload | pick survives |

- [ ] **Step 3:** Extend `e2e/lib/tauriMock.ts` with `chat_turn`, `approve_plan`, `discard_plan` cases; demote the Playwright specs to regression-shape checks.
- [ ] **Step 4:** `vox ci pre-push --full`

---

## Parallel execution

Contended hub files force width 2–3, not 19 lanes. `chat_turn.rs` (A1, A2, B1, C1, E1), `App.tsx` (A3, B2, C1, D3, E2, F1), `main.rs` (A1, F2), mcp `message.rs` (A2, B1, E1).

| Batch | Parallel lanes | Notes |
|---|---|---|
| 0 | Task 0 | Serial. Per worktree. |
| 1 | **A1** ‖ **D0** ‖ **M0** | Disjoint. D0 and M0 have no Phase-A dependency. |
| 2 | **A2** ‖ **D1** | A2 owns mcp `message.rs`/`params.rs`; D1 owns `topology.rs`/`spawn.rs`. |
| 3 | **A3** ‖ **D2** | A3 owns `App.tsx` exclusively for its whole lane. |
| 4 | **B1** ‖ **E1** ‖ **F1** | B1 and E1 both touch `message.rs` — **serialize those two**, or merge. |
| 5 | **B2** → **C1** → **F1-frontend** (one `App.tsx` lane) ‖ **F2** ‖ **M1** | |
| 6 | **E2** ‖ **D3** ‖ **G1** | |
| 7 | **M2** → **M3** | |
| 8 | **Task V** + `vox ci gui-surface-coverage --write` + `vox ci pre-push --full` | Serial. |

One agent owns `App.tsx` for the duration of any lane touching it. Parallel worktrees each need Task 0 and are subject to the fmt drift AGENTS.md lists as a perennial — run `vox run scripts/fmt.vox` before merging assembled work.
