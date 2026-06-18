---
title: "Secretary Agent — Chat-Driven Task Orchestration"
description: "Implementation plan for a secretary agent that listens to chat messages, classifies intent, and submits tasks to the orchestrator hopper without interrupting the user's conversational flow."
category: "plans"
status: "current"
---

# Secretary Agent Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When a user sends a chat message, a lightweight "secretary" module classifies whether it contains actionable intent, and if so, submits a new task to the orchestrator hopper — all without requiring the user to navigate to the Tasks view.

**Architecture:** A new `SecretaryClassifier` (pure Rust, no LLM for Phase 1) applies keyword heuristics to incoming chat messages. When actionable intent is detected, it calls `submit_orchestrator_task` internally and emits a `"vox://secretary-proposed-task"` Tauri event. The frontend shows a dismissable toast banner confirming the task was submitted. Phase 1 is deliberately heuristic-only (DRY, YAGNI) — LLM-based intent extraction is a later phase.

**Tech Stack:** Rust (`vox-orchestrator` crate, no new dependencies), TypeScript/React (new `SecretaryToast` component, `@tauri-apps/api/event`). Depends on Plan A (real-time task push) being shipped first — the event bus that Plan A sets up carries the confirmation back to the UI.

---

## Background: What "Secretary" Means Here

The secretary intercepts outgoing chat messages (from the `chat_append_message` Tauri command). It runs a fast heuristic check:
- Does the message contain action verbs + a target? ("fix", "add", "update", "create", "remove", "refactor", "write", "implement")
- Is the message from the `user` role (not `assistant`)?
- Is the message longer than 10 words?

If yes: submit an `IntakeItem` to the hopper with `source = IntakeSource::Agent` and `intent = <cleaned message>`. Then emit `"vox://secretary-proposed-task"` with the task description and hopper item ID so the frontend can show a toast.

This approach is intentionally conservative: it only acts on explicit action-verb messages, never interprets ambiguous chat. False negatives (missed tasks) are safe. False positives (spurious tasks) are correctable because the user sees the toast and can cancel.

---

## File Map

| File | Change | Responsibility |
|---|---|---|
| `crates/vox-orchestrator/src/secretary.rs` | **Create** | `SecretaryClassifier` — pure heuristic intent classifier |
| `crates/vox-orchestrator/src/lib.rs` | **Modify** | Re-export `secretary` module |
| `crates/vox-gui/src/commands/chat.rs` | **Modify** | Call `SecretaryClassifier::classify` after `chat_append_message`, emit event |
| `crates/vox-gui/src/commands/orchestrator.rs` | **Modify** | Add `SECRETARY_PROPOSED_EVENT` constant |
| `crates/vox-gui/ui/src/components/surfaces/Chat/SecretaryToast.tsx` | **Create** | Dismissable toast shown when secretary submits a task |
| `crates/vox-gui/ui/src/components/surfaces/Chat/SecretaryToast.test.tsx` | **Create** | Unit tests for toast rendering and dismiss |
| `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx` | **Modify** | Subscribe to `vox://secretary-proposed-task`, render `<SecretaryToast>` |

---

## Task 1: Build `SecretaryClassifier` with unit tests

**Context:** This is pure Rust, no Tauri, no async. The classifier receives a `role: &str` and `content: &str`, and returns `Option<String>` — `Some(cleaned_intent)` if actionable, `None` if not. The "cleaned intent" is the first 200 characters of the content with leading whitespace and common filler words stripped.

This module goes in `crates/vox-orchestrator/src/secretary.rs`. The orchestrator crate already has `IntakeItem` and `IntakeSource` in the `hopper` module — we use those directly.

**Files:**
- Create: `crates/vox-orchestrator/src/secretary.rs`

- [ ] **Step 1.1: Create the test module (write tests first)**

Create `crates/vox-orchestrator/src/secretary.rs` with the tests only:

```rust
//! Secretary classifier: lightweight heuristic for detecting actionable intent
//! in chat messages.
//!
//! Phase 1 is purely heuristic — no LLM calls, no async. Phase 2 will add an
//! optional LLM fallback via `vox_actor_runtime::llm::llm_chat()` for messages
//! that pass the heuristic gate but whose intent is ambiguous.

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Outcome of classifying a single chat message.
#[derive(Debug, PartialEq, Eq)]
pub struct ClassifyResult {
    /// The extracted task intent (first 200 chars, normalised).
    pub intent: String,
    /// Heuristic confidence 0–100 (not a probability; just for logging).
    pub confidence_pct: u8,
}

/// Classify a single chat turn.
///
/// Returns `Some(ClassifyResult)` if the message contains actionable intent,
/// `None` if it should be ignored.
///
/// # Rules
/// - Only `role == "user"` messages are evaluated.
/// - Message must be ≥ 10 words.
/// - Message must contain at least one action verb (see `ACTION_VERBS`).
pub fn classify(role: &str, content: &str) -> Option<ClassifyResult> {
    if role != "user" {
        return None;
    }

    let words: Vec<&str> = content.split_whitespace().collect();
    if words.len() < 10 {
        return None;
    }

    let lower = content.to_lowercase();
    let matched_verb = ACTION_VERBS.iter().find(|&&v| lower.contains(v))?;

    // Trim to 200 chars, strip leading/trailing whitespace.
    let intent = content.chars().take(200).collect::<String>().trim().to_string();

    // Confidence is higher when the verb appears early in the message.
    let verb_pos = lower.find(matched_verb).unwrap_or(usize::MAX);
    let confidence_pct = if verb_pos < 20 { 85 } else { 60 };

    Some(ClassifyResult {
        intent,
        confidence_pct,
    })
}

/// Action verbs that signal the user wants something done.
const ACTION_VERBS: &[&str] = &[
    "fix", "add", "update", "create", "remove", "delete", "refactor",
    "write", "implement", "build", "migrate", "extract", "rename",
    "move", "replace", "rewrite", "upgrade", "configure", "setup",
    "install", "deploy", "test", "debug", "investigate",
];

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_assistant_messages() {
        let result = classify(
            "assistant",
            "fix the bug in the authentication module please it is broken",
        );
        assert!(result.is_none());
    }

    #[test]
    fn ignores_short_messages() {
        // Under 10 words — should be ignored
        let result = classify("user", "fix the bug please");
        assert!(result.is_none());
    }

    #[test]
    fn detects_fix_verb() {
        let result = classify(
            "user",
            "fix the authentication bug in the login module where users cannot sign in",
        );
        assert!(result.is_some());
        let r = result.unwrap();
        assert!(r.intent.contains("fix"));
        assert!(r.confidence_pct > 0);
    }

    #[test]
    fn detects_implement_verb() {
        let result = classify(
            "user",
            "implement the new retry logic for the HTTP client that currently fails on timeouts",
        );
        assert!(result.is_some());
    }

    #[test]
    fn ignores_no_action_verb() {
        let result = classify(
            "user",
            "the authentication module seems to be having some issues with the login flow currently",
        );
        assert!(result.is_none(), "no action verb should produce None");
    }

    #[test]
    fn intent_is_capped_at_200_chars() {
        let long = format!("fix the thing {}", "x".repeat(300));
        let result = classify("user", &long).unwrap();
        assert!(result.intent.len() <= 200);
    }

    #[test]
    fn early_verb_gets_higher_confidence() {
        let early = classify(
            "user",
            "fix the memory leak in the websocket handler it is causing the server to crash",
        )
        .unwrap();
        let late = classify(
            "user",
            "the websocket handler seems to be leaking memory please fix it before the release",
        )
        .unwrap();
        assert!(early.confidence_pct > late.confidence_pct);
    }
}
```

- [ ] **Step 1.2: Run tests to verify they FAIL (module doesn't exist yet)**

```
cargo test -p vox-orchestrator -- secretary::tests
```

Expected: FAIL — module not declared yet. This is correct.

- [ ] **Step 1.3: Declare the module in `lib.rs`**

Open `crates/vox-orchestrator/src/lib.rs`. Find the section where modules are declared (they are listed alphabetically). Add:

```rust
pub mod secretary;
```

alongside the existing `pub mod` declarations.

- [ ] **Step 1.4: Run tests again to verify they PASS**

```
cargo test -p vox-orchestrator -- secretary::tests
```

Expected: 7 PASS.

- [ ] **Step 1.5: Commit**

```
git add crates/vox-orchestrator/src/secretary.rs crates/vox-orchestrator/src/lib.rs
git commit -m "feat(orchestrator): add SecretaryClassifier with heuristic intent detection"
```

---

## Task 2: Add `SECRETARY_PROPOSED_EVENT` and emit helper

**Context:** The GUI backend needs to notify the frontend when the secretary submits a task. We add a new event constant and a typed payload struct to `orchestrator.rs` — same location as the existing event constants.

**Files:**
- Modify: `crates/vox-gui/src/commands/orchestrator.rs`

- [ ] **Step 2.1: Add the test for the payload struct**

Append to the `budget_tests` module (or add a new `secretary_tests` module) in `orchestrator.rs`:

```rust
#[cfg(test)]
mod secretary_tests {
    use super::*;

    #[test]
    fn secretary_proposed_payload_serializes() {
        let payload = SecretaryProposedPayload {
            item_id: "abc123".to_string(),
            intent: "Fix the auth bug in login module".to_string(),
            confidence_pct: 85,
        };
        let json = serde_json::to_value(&payload).expect("serialize");
        assert_eq!(json["item_id"], "abc123");
        assert_eq!(json["confidence_pct"], 85);
    }
}
```

- [ ] **Step 2.2: Run to verify it FAILS**

```
cargo test -p vox-gui -- secretary_tests
```

Expected: FAIL — `SecretaryProposedPayload` not defined yet.

- [ ] **Step 2.3: Add the constant, struct, and emit helper**

Append to `crates/vox-gui/src/commands/orchestrator.rs` (after `emit_tasks_changed`):

```rust
/// Tauri event emitted when the secretary auto-submits a task from chat.
/// Payload: `SecretaryProposedPayload`.
pub const SECRETARY_PROPOSED_EVENT: &str = "vox://secretary-proposed-task";

/// Payload for the [`SECRETARY_PROPOSED_EVENT`] Tauri event.
#[derive(Debug, serde::Serialize, Clone)]
pub struct SecretaryProposedPayload {
    /// Hopper item ID assigned to the submitted task.
    pub item_id: String,
    /// Cleaned intent text that was submitted as the task description.
    pub intent: String,
    /// Classifier confidence 0–100 (for UI display only; not a guarantee).
    pub confidence_pct: u8,
}

/// Emit [`SECRETARY_PROPOSED_EVENT`] to all webview windows.
pub fn emit_secretary_proposed(app_handle: &tauri::AppHandle, payload: SecretaryProposedPayload) {
    let _ = app_handle.emit(SECRETARY_PROPOSED_EVENT, payload);
}
```

- [ ] **Step 2.4: Run tests to verify they PASS**

```
cargo test -p vox-gui -- secretary_tests
```

Expected: 1 PASS.

- [ ] **Step 2.5: Compile check**

```
cargo check -p vox-gui
```

Expected: no errors.

- [ ] **Step 2.6: Commit**

```
git add crates/vox-gui/src/commands/orchestrator.rs
git commit -m "feat(gui): add SECRETARY_PROPOSED_EVENT and emit_secretary_proposed helper"
```

---

## Task 3: Wire secretary into `chat_append_message`

**Context:** `chat_append_message` in `crates/vox-gui/src/commands/chat.rs` is called every time a message is saved to the DB. After saving the message successfully, we run `classify(role, content)`. If the result is `Some`, we submit a task to the orchestrator daemon and emit the secretary event.

The task submission uses `call_daemon` with `orch_daemon_method::ENQUEUE` — the same path `submit_orchestrator_task` uses. We call it directly because `chat_append_message` doesn't have an `AppHandle` yet (we'll add it).

**Files:**
- Modify: `crates/vox-gui/src/commands/chat.rs`

- [ ] **Step 3.1: Update the tests in `chat.rs`**

The existing tests in `chat.rs` use `VoxDb::connect(DbConfig::Memory)` directly and don't go through the Tauri command. They will continue to pass because we only add secretary logic to `chat_append_message` the Tauri command, not the underlying `db.chat_append_message` method.

Add one new test to verify the classifier is exercised (this will be a unit test of the classify function indirectly, since `chat_append_message` cannot be called without a running daemon in tests):

```rust
// Append to the #[cfg(test)] mod tests block in chat.rs:

    #[test]
    fn secretary_classify_short_user_message_returns_none() {
        // Verify that short messages don't trigger the secretary
        let result = vox_orchestrator::secretary::classify("user", "fix it");
        assert!(result.is_none(), "short message should return None");
    }

    #[test]
    fn secretary_classify_long_action_message_returns_some() {
        let result = vox_orchestrator::secretary::classify(
            "user",
            "fix the broken authentication flow in the login page it keeps redirecting users",
        );
        assert!(result.is_some());
    }
```

- [ ] **Step 3.2: Run tests to verify new tests PASS (they test `secretary::classify` directly)**

```
cargo test -p vox-gui -- chat::tests
```

Expected: 4 PASS (2 existing + 2 new).

- [ ] **Step 3.3: Add `AppHandle` parameter and secretary logic to `chat_append_message`**

Replace the `chat_append_message` function in `chat.rs` with this updated version. Read the existing function first to understand what you're adding to — the body is identical except for the new parameter and the appended secretary block:

```rust
#[tauri::command]
pub async fn chat_append_message(
    app_handle: tauri::AppHandle,
    input: ChatAppendInput,
) -> Result<i64, String> {
    if input.session_id.trim().is_empty() {
        return Err("session_id must not be empty".to_string());
    }
    if input.role.trim().is_empty() {
        return Err("role must not be empty".to_string());
    }
    let db = gui_db().await?;
    let conv_id = db
        .chat_ensure_gui_session(&input.session_id, "Chat")
        .await
        .map_err(|e| e.to_string())?;
    let payload = input
        .task_id
        .map(|t| serde_json::json!({ "task_id": t }).to_string());
    let msg_id = db
        .chat_append_message(conv_id, &input.role, &input.content, payload.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    // Secretary: detect actionable intent in user messages and submit to hopper.
    // Fire-and-forget — errors here must never fail the chat message save.
    if let Some(classified) =
        vox_orchestrator::secretary::classify(&input.role, &input.content)
    {
        let session_id = input.session_id.clone();
        let app_handle_clone = app_handle.clone();
        tokio::spawn(async move {
            use vox_cli_core::daemon_ipc::dispatch::call_daemon;
            use vox_foundation::protocol::orch_daemon_method;

            let params = serde_json::json!({
                "description": classified.intent,
                "file_manifest": [],
                "priority": null,
                "session_id": session_id,
                "allow_duplicate": false,
                "model_hint": null,
                "dry_run": null,
                "active_skill": null,
            });
            match call_daemon("vox-orchestrator-d", orch_daemon_method::ENQUEUE, params, false).await {
                Ok(raw) => {
                    let item_id = raw
                        .get("task_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    crate::commands::orchestrator::emit_secretary_proposed(
                        &app_handle_clone,
                        crate::commands::orchestrator::SecretaryProposedPayload {
                            item_id,
                            intent: classified.intent,
                            confidence_pct: classified.confidence_pct,
                        },
                    );
                    // Also ping the tasks list so it refreshes immediately.
                    crate::commands::orchestrator::emit_tasks_changed(&app_handle_clone);
                }
                Err(e) => {
                    // Daemon unavailable or rejected — log and move on.
                    tracing::debug!("secretary: failed to submit task: {e}");
                }
            }
        });
    }

    Ok(msg_id)
}
```

- [ ] **Step 3.4: Compile check**

```
cargo check -p vox-gui
```

Expected: no errors. If you see errors about `orch_daemon_method::ENQUEUE`, check that the constant name matches what's in `vox_foundation::protocol::orch_daemon_method` — it may be `ENQUEUE_TASK` or similar. Run `grep -r "ENQUEUE" crates/vox-foundation/` to find the correct name.

- [ ] **Step 3.5: Run existing tests to confirm they still pass**

```
cargo test -p vox-gui -- chat::tests
```

Expected: 4 PASS. (The `chat_append_message` Tauri command test still passes because it tests the empty-session-id guard, which runs before the secretary logic.)

- [ ] **Step 3.6: Commit**

```
git add crates/vox-gui/src/commands/chat.rs
git commit -m "feat(gui): wire SecretaryClassifier into chat_append_message"
```

---

## Task 4: Build `SecretaryToast` React component

**Context:** When the `"vox://secretary-proposed-task"` event arrives, the UI shows a small non-blocking toast at the bottom of the chat area. The toast shows:
- The task intent (first 80 characters)
- A "View task" button that navigates to the Tasks view
- A dismiss ("✕") button
- Auto-dismisses after 5 seconds

This is a pure presentational component: it receives `intent`, `itemId`, and `onDismiss` + `onViewTask` callbacks as props. The `ChatSurface` parent manages the visible state.

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/Chat/SecretaryToast.tsx`
- Create: `crates/vox-gui/ui/src/components/surfaces/Chat/SecretaryToast.test.tsx`

- [ ] **Step 4.1: Write the tests first**

Create `crates/vox-gui/ui/src/components/surfaces/Chat/SecretaryToast.test.tsx`:

```typescript
// @vitest-environment jsdom
import React from 'react';
import { render, screen, act } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { vi, describe, it, expect, beforeEach, afterEach } from 'vitest';
import { SecretaryToast } from './SecretaryToast';

describe('SecretaryToast', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('renders the intent text', () => {
    render(
      <SecretaryToast
        intent="Fix the authentication bug in the login flow"
        itemId="item-1"
        onDismiss={vi.fn()}
        onViewTask={vi.fn()}
      />
    );
    expect(screen.getByText(/Fix the authentication bug/)).toBeInTheDocument();
  });

  it('calls onDismiss when ✕ button is clicked', async () => {
    const onDismiss = vi.fn();
    render(
      <SecretaryToast
        intent="Fix something important in the codebase today"
        itemId="item-2"
        onDismiss={onDismiss}
        onViewTask={vi.fn()}
      />
    );
    await userEvent.click(screen.getByRole('button', { name: /dismiss/i }));
    expect(onDismiss).toHaveBeenCalledOnce();
  });

  it('calls onViewTask when "View task" is clicked', async () => {
    const onViewTask = vi.fn();
    render(
      <SecretaryToast
        intent="Implement the new retry logic for HTTP client failures"
        itemId="item-3"
        onDismiss={vi.fn()}
        onViewTask={onViewTask}
      />
    );
    await userEvent.click(screen.getByRole('button', { name: /view task/i }));
    expect(onViewTask).toHaveBeenCalledOnce();
  });

  it('auto-dismisses after 5 seconds', () => {
    const onDismiss = vi.fn();
    render(
      <SecretaryToast
        intent="Fix the memory leak in the websocket handler today"
        itemId="item-4"
        onDismiss={onDismiss}
        onViewTask={vi.fn()}
      />
    );
    act(() => {
      vi.advanceTimersByTime(5001);
    });
    expect(onDismiss).toHaveBeenCalledOnce();
  });

  it('truncates long intent text to 80 characters', () => {
    const long = 'Fix the ' + 'very '.repeat(30) + 'long description';
    render(
      <SecretaryToast
        intent={long}
        itemId="item-5"
        onDismiss={vi.fn()}
        onViewTask={vi.fn()}
      />
    );
    // The rendered text should not exceed 80 chars + ellipsis
    const displayed = screen.getByTestId('secretary-toast-intent').textContent ?? '';
    expect(displayed.length).toBeLessThanOrEqual(83); // 80 + '...'
  });
});
```

- [ ] **Step 4.2: Run tests to verify they FAIL**

```
cd crates/vox-gui/ui
pnpm test SecretaryToast
```

Expected: FAIL — component doesn't exist yet.

- [ ] **Step 4.3: Create the component**

Create `crates/vox-gui/ui/src/components/surfaces/Chat/SecretaryToast.tsx`:

```typescript
import React, { useEffect } from 'react';

export interface SecretaryToastProps {
  /** The task intent text extracted from the chat message. */
  intent: string;
  /** The hopper item ID (for future cancel support). */
  itemId: string;
  /** Called when the toast should be dismissed. */
  onDismiss: () => void;
  /** Called when the user clicks "View task". */
  onViewTask: () => void;
}

const AUTO_DISMISS_MS = 5_000;
const MAX_INTENT_CHARS = 80;

/** Dismissable toast shown when the secretary auto-submits a task from chat. */
export function SecretaryToast({ intent, itemId: _itemId, onDismiss, onViewTask }: SecretaryToastProps) {
  // Auto-dismiss after 5 seconds.
  useEffect(() => {
    const t = setTimeout(onDismiss, AUTO_DISMISS_MS);
    return () => clearTimeout(t);
  }, [onDismiss]);

  const displayed =
    intent.length > MAX_INTENT_CHARS
      ? intent.slice(0, MAX_INTENT_CHARS) + '...'
      : intent;

  return (
    <div
      role="status"
      aria-live="polite"
      className="flex items-center gap-2 rounded-lg border border-white/10 bg-zinc-900/95 px-3 py-2 shadow-lg backdrop-blur-sm"
    >
      {/* Secretary icon */}
      <span className="shrink-0 text-[10px] text-zinc-400" aria-hidden>📋</span>

      <div className="min-w-0 flex-1">
        <p className="text-[10px] text-zinc-500">Task added by secretary</p>
        <p
          data-testid="secretary-toast-intent"
          className="truncate text-[11px] text-zinc-200"
        >
          {displayed}
        </p>
      </div>

      {/* View task button */}
      <button
        type="button"
        aria-label="View task in Tasks panel"
        onClick={onViewTask}
        className="shrink-0 rounded px-2 py-0.5 text-[10px] text-brass hover:bg-white/[0.06] transition"
      >
        View task
      </button>

      {/* Dismiss button */}
      <button
        type="button"
        aria-label="Dismiss secretary toast"
        onClick={onDismiss}
        className="shrink-0 rounded p-0.5 text-zinc-500 hover:text-zinc-200 transition"
      >
        ✕
      </button>
    </div>
  );
}
```

- [ ] **Step 4.4: Run tests to verify they PASS**

```
cd crates/vox-gui/ui
pnpm test SecretaryToast
```

Expected: 5 PASS.

- [ ] **Step 4.5: Commit**

```
git add crates/vox-gui/ui/src/components/surfaces/Chat/SecretaryToast.tsx
git add crates/vox-gui/ui/src/components/surfaces/Chat/SecretaryToast.test.tsx
git commit -m "feat(gui): add SecretaryToast component with auto-dismiss"
```

---

## Task 5: Wire `SecretaryToast` into `ChatSurface`

**Context:** `ChatSurface.tsx` is the parent component. It needs to:
1. Subscribe to `"vox://secretary-proposed-task"` events on mount.
2. Store the latest toast payload in a `useState`.
3. Render `<SecretaryToast>` when payload is set.
4. Clear it on dismiss or view-task.

Look at the existing `ChatSurface.tsx` (89 lines) before editing — it renders `ChatSessionRail`, a messages area, and `ChatExecutionRail`. The toast overlays at the bottom.

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`

- [ ] **Step 5.1: Write the test addition for `ChatSurface.test.tsx`**

Add to the existing `ChatSurface.test.tsx`:

```typescript
// Add these imports and test to the existing describe block in ChatSurface.test.tsx

// At the top with other mocks:
const mockSecretaryPayload = {
  item_id: 'item-abc',
  intent: 'Fix the broken authentication flow in the login page today',
  confidence_pct: 85,
};

let secretaryEventHandler: ((event: { payload: typeof mockSecretaryPayload }) => void) | null = null;

// Update the @tauri-apps/api/event mock to capture the secretary handler:
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn((eventName: string, handler: (e: unknown) => void) => {
    if (eventName === 'vox://secretary-proposed-task') {
      secretaryEventHandler = handler as typeof secretaryEventHandler;
    }
    return Promise.resolve(() => {});
  }),
}));

// New test:
it('shows SecretaryToast when secretary-proposed-task event fires', async () => {
  render(<ChatSurface onNavigate={vi.fn()} />);
  // Wait for the listen subscriptions to be set up
  await waitFor(() => expect(secretaryEventHandler).not.toBeNull());
  // Simulate the event
  act(() => {
    secretaryEventHandler!({ payload: mockSecretaryPayload });
  });
  // Toast should appear
  await waitFor(() => {
    expect(screen.getByRole('status')).toBeInTheDocument();
    expect(screen.getByText(/Fix the broken authentication/)).toBeInTheDocument();
  });
});
```

- [ ] **Step 5.2: Run the test to verify it FAILS**

```
cd crates/vox-gui/ui
pnpm test ChatSurface
```

Expected: the new test FAILs (no toast rendered yet). Existing tests should still PASS.

- [ ] **Step 5.3: Update `ChatSurface.tsx` to subscribe and render the toast**

Open `ChatSurface.tsx`. Add these changes:

**Imports** (add after existing imports):

```typescript
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { SecretaryToast } from './SecretaryToast';

interface SecretaryProposedPayload {
  item_id: string;
  intent: string;
  confidence_pct: number;
}
```

**State** (inside the component function, after other `useState` calls):

```typescript
  const [secretaryToast, setSecretaryToast] = useState<SecretaryProposedPayload | null>(null);
```

**Effect** (add a new `useEffect` after existing ones):

```typescript
  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    listen<SecretaryProposedPayload>('vox://secretary-proposed-task', (event) => {
      setSecretaryToast(event.payload);
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, []);
```

**JSX** (add the toast just before the closing `</div>` of the root element):

```typescript
        {secretaryToast && (
          <div className="absolute bottom-4 left-1/2 z-50 w-[480px] -translate-x-1/2">
            <SecretaryToast
              intent={secretaryToast.intent}
              itemId={secretaryToast.item_id}
              onDismiss={() => setSecretaryToast(null)}
              onViewTask={() => {
                setSecretaryToast(null);
                onNavigate('tasks');
              }}
            />
          </div>
        )}
```

- [ ] **Step 5.4: Run the test to verify it PASSES**

```
cd crates/vox-gui/ui
pnpm test ChatSurface
```

Expected: all PASS.

- [ ] **Step 5.5: TypeScript compile check**

```
cd crates/vox-gui/ui
pnpm tsc --noEmit
```

Expected: no errors.

- [ ] **Step 5.6: Commit**

```
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx
git commit -m "feat(gui): subscribe to secretary-proposed-task and render SecretaryToast"
```

---

## Task 6: Full integration smoke test

- [ ] **Step 6.1: Build and launch**

```
cargo run -p vox-gui
```

- [ ] **Step 6.2: Open the Chat surface, create a new session**

Navigate to the Chat view. Create or open an existing chat session.

- [ ] **Step 6.3: Send a short message (should NOT trigger secretary)**

Type: "what is the status?" — this is under 10 words with no action verb.

Expected: message is saved, no toast appears.

- [ ] **Step 6.4: Send an actionable message (should trigger secretary)**

Type: "fix the authentication bug in the login page it keeps redirecting users to the wrong page"

Expected within ~500ms: a toast appears at the bottom of the chat area saying "Task added by secretary" with the intent text. The task should also appear in the Tasks view immediately (because the `emit_tasks_changed` signal fires too).

- [ ] **Step 6.5: Click "View task"**

Expected: toast dismisses and the Tasks view opens with the submitted task visible.

- [ ] **Step 6.6: Send another actionable message, wait 5 seconds**

Expected: toast auto-dismisses after 5 seconds.

- [ ] **Step 6.7: Commit any fixups**

```
git add -A
git commit -m "fix(gui): smoke test fixups for secretary toast integration"
```

---

## Self-Review Checklist

- [x] **Spec coverage:** Research doc §5.2 "Chat → Task Pipeline" requires: (1) router classifies message, (2) context injection, (3) hopper submission, (4) feedback to user. Tasks 1 (classifier), 3 (hopper submission), 4+5 (toast feedback) implement steps 1, 3, 4. Step 2 (context injection) is omitted per YAGNI — Phase 1 uses simple heuristic, not context-aware LLM.
- [x] **No placeholders:** All code is complete. The fire-and-forget async block in Task 3 is complete Rust. The toast component is complete TSX.
- [x] **Type consistency:** `SecretaryProposedPayload` Rust struct fields (`item_id`, `intent`, `confidence_pct`) match the TypeScript `SecretaryProposedPayload` interface in Tasks 4 and 5. `ClassifyResult.intent` (Task 1) maps to `SecretaryProposedPayload.intent` (Task 2/3).
- [x] **Error safety:** Secretary logic is in a `tokio::spawn` fire-and-forget block. A daemon failure (Task 3) only logs a debug message — it never fails the `chat_append_message` call.
- [x] **YAGNI:** No LLM calls in Phase 1. The comment in `secretary.rs` documents Phase 2 LLM fallback as a future extension point.
- [x] **False positive safety:** The `allow_duplicate: false` flag in the hopper submission means if the user sends the same message twice, the daemon rejects the second submission as a near-duplicate.
- [x] **Cleanup:** All `listen()` subscriptions have corresponding `unlisten()` calls in `useEffect` cleanup.
