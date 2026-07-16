# Axis GUI Remediation Phase 1 (Bug Fixes) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the eight Phase 1 correctness fixes from `docs/superpowers/specs/2026-07-16-axis-gui-audit-remediation-design.md` — secretary double-submit (C2), top-level ErrorBoundary (C3), CodeRabbit toast shape (B6), dead `ds/` stylesheet (B7), listener leaks / unhandled `listen()` rejections, Tasks copy honesty (B1 interim), the `gui-visual-review` `0000-00-00` date default, and the chat token-loss race — each as a small, independently revertable commit with its regression test.

**Architecture:** Tauri 2 desktop app. Rust backend commands live in `crates/vox-gui/src/commands/` (crate `vox-gui`); the React 19 + TypeScript frontend lives in `crates/vox-gui/ui/src/` (vitest + @testing-library for unit tests, pnpm-managed — never npm). The visual-review CLI is `crates/vox-orchestrator-mcp/src/bin/gui-visual-review.rs` backed by `crates/vox-orchestrator-mcp/src/visus_review/`. All tasks are working-tree edits on one branch; one commit per task.

**Tech Stack:** Rust (tokio, tauri, serde, chrono), TypeScript/React 19, vitest 3 + @testing-library/react, Tailwind 3 + generated design tokens, pnpm.

Test commands used throughout (verified against `crates/vox-gui/ui/package.json`, where `"test": "vitest run"` and `"typecheck": "tsc --noEmit"`):

- Frontend, single file: `pnpm --dir crates/vox-gui/ui test -- <path-under-ui>`
- Frontend, full suite: `pnpm --dir crates/vox-gui/ui test`
- Frontend typecheck: `pnpm --dir crates/vox-gui/ui typecheck`
- Rust: `cargo test -p vox-gui <filter>` and `cargo test -p vox-orchestrator-mcp <filter>`
- Formatting: `cargo fmt -p vox-gui` / `cargo fmt -p vox-orchestrator-mcp` only. **NEVER `cargo fmt --all`** (Windows arg-limit overflow). Never pipe cargo output to `head`/`grep` (orphaned-process leak on Windows); redirect to a file if you must page it.

Repo rules (AGENTS.md): no new `.ps1`/`.sh`/`.py` files (VoxScript only for automation — none is needed here), `.vox` files are Vox source, don't hand-edit generated docs.

---

## Task 0: Unstage and delete the placeholder visual-review report

The repo currently has `contracts/reports/gui-visual-review/0000-00-00.json` **staged** (git status shows `A`). It is a junk artifact from a local `gui-visual-review` run without `--date` (the bug fixed in Task 7). It must never be committed.

**Files:**
- Delete: `contracts/reports/gui-visual-review/0000-00-00.json` (unstage first)

**Steps:**

- [ ] Verify the file is staged:
  ```
  git -C C:/Users/Owner/vox status --short contracts/reports/gui-visual-review/0000-00-00.json
  ```
  Expected output: `A  contracts/reports/gui-visual-review/0000-00-00.json`
- [ ] Unstage it:
  ```
  git -C C:/Users/Owner/vox restore --staged contracts/reports/gui-visual-review/0000-00-00.json
  ```
- [ ] Delete the file from disk (PowerShell):
  ```
  Remove-Item C:/Users/Owner/vox/contracts/reports/gui-visual-review/0000-00-00.json
  ```
- [ ] Verify it is gone from both index and working tree:
  ```
  git -C C:/Users/Owner/vox status --short contracts/reports/gui-visual-review/
  ```
  Expected: no output mentioning `0000-00-00.json`. Do **not** touch `cache.v1.json` or `ledger.jsonl` in the same directory.
- [ ] No commit for this task (the file was staged-new, never committed; unstaging + deleting leaves nothing to record).

---

## Task 1: Secretary double-submit fix (C2)

Every actionable composer message currently triggers `SUBMIT_TASK` twice: `App.tsx` `handleLoquelaSubmit` calls `submit_orchestrator_task` explicitly, **and** `chat_append_message` (called on the same message to persist it) runs `vox_orchestrator::secretary::classify` and submits again. The loser of the race is refused as a near-duplicate (`task_id: null`), producing either a spurious "Secretary proposed" toast with `item_id: "unknown"` or a wrong "near-duplicate — submit anyway?" confirm dialog.

Fix (per spec): the composer marks its persist call `already_submitted: true` so the secretary skips classification; and when the daemon reply is a dedupe (`task_id: null`), no toast is emitted at all.

**Files:**
- Modify: `crates/vox-gui/src/commands/chat.rs` (`ChatAppendInput` at lines 121-130, secretary block at lines 168-224, tests module at lines 262-353)
- Create: `crates/vox-gui/ui/src/lib/composerSubmit.ts`
- Create: `crates/vox-gui/ui/src/lib/composerSubmit.test.ts` (payload-builder tests + F28 App.tsx wiring guard)
- Modify: `crates/vox-gui/ui/src/App.tsx` (composer persist call at lines 677-679; import block near line 14)

### Steps

- [ ] **Write the failing Rust tests.** In `crates/vox-gui/src/commands/chat.rs`, append to the existing `mod tests` block (after `chat_message_dto_model_id_absent_when_none`, before the closing `}` at line 353):

  ```rust
  #[test]
  fn secretary_skips_messages_the_composer_already_submitted() {
      // Precondition: this message IS actionable for the classifier.
      let msg =
          "fix the broken authentication flow in the login page it keeps redirecting users";
      assert!(
          vox_orchestrator::secretary::classify("user", msg).is_some(),
          "precondition: classifier finds this actionable"
      );
      // The composer already dispatched it -> secretary must stand down.
      assert!(secretary_candidate("user", msg, true).is_none());
      // Same message NOT pre-submitted -> secretary still classifies it.
      assert!(secretary_candidate("user", msg, false).is_some());
  }

  #[test]
  fn submitted_task_id_is_none_when_daemon_dedupes() {
      // Dedupe reply: null task_id + duplicate_of. No toast may be built from this.
      assert_eq!(
          submitted_task_id(&serde_json::json!({"task_id": null, "duplicate_of": 7})),
          None
      );
      assert_eq!(submitted_task_id(&serde_json::json!({})), None);
      assert_eq!(
          submitted_task_id(&serde_json::json!({"task_id": 42})),
          Some("42".to_string())
      );
  }

  #[test]
  fn chat_append_input_already_submitted_defaults_to_false() {
      let input: ChatAppendInput = serde_json::from_str(
          r#"{"session_id":"s","role":"user","content":"hi","task_id":null}"#,
      )
      .expect("older frontends omit the field");
      assert!(!input.already_submitted);
  }
  ```

- [ ] Run and confirm the expected failure (compile errors — `secretary_candidate` and `submitted_task_id` don't exist yet, `already_submitted` is not a field):
  ```
  cargo test -p vox-gui chat::tests
  ```
  Expected: compilation failure naming `secretary_candidate`, `submitted_task_id`, `already_submitted`.

- [ ] **Implement the Rust side.** Three edits in `crates/vox-gui/src/commands/chat.rs`:

  1. Add the field to `ChatAppendInput`. Current code (lines 121-130):
     ```rust
     #[derive(Debug, Deserialize)]
     pub struct ChatAppendInput {
         pub session_id: String,
         pub role: String,
         pub content: String,
         pub task_id: Option<String>,
         /// Optional model id to record in the message payload (e.g. for assistant messages).
         #[serde(default)]
         pub model_id: Option<String>,
     }
     ```
     becomes:
     ```rust
     #[derive(Debug, Deserialize)]
     pub struct ChatAppendInput {
         pub session_id: String,
         pub role: String,
         pub content: String,
         pub task_id: Option<String>,
         /// Optional model id to record in the message payload (e.g. for assistant messages).
         #[serde(default)]
         pub model_id: Option<String>,
         /// True when the composer already dispatched this message as a task
         /// (`submit_orchestrator_task`). The secretary must not submit it again
         /// (C2: every actionable composer message used to be SUBMIT_TASK'd twice).
         #[serde(default)]
         pub already_submitted: bool,
     }
     ```

  2. Add two pure helpers directly above the `#[cfg(test)]` line (currently line 262):
     ```rust
     /// Secretary gate: never classify a message the composer already submitted
     /// as a task — that path caused every actionable composer message to be
     /// SUBMIT_TASK'd twice (explicit submit + secretary re-submit).
     fn secretary_candidate(
         role: &str,
         content: &str,
         already_submitted: bool,
     ) -> Option<vox_orchestrator::secretary::ClassifyResult> {
         if already_submitted {
             return None;
         }
         vox_orchestrator::secretary::classify(role, content)
     }

     /// Task id from a `SUBMIT_TASK` daemon reply. `None` means the daemon
     /// deduped the submission (`task_id: null` + `duplicate_of`): nothing new
     /// was created, so no "Secretary proposed a task" toast may be emitted.
     fn submitted_task_id(raw: &serde_json::Value) -> Option<String> {
         raw.get("task_id")
             .and_then(|v| v.as_u64())
             .map(|v| v.to_string())
     }
     ```

  3. Rewire `chat_append_message`. Current classify gate (line 170):
     ```rust
     if let Some(classified) = vox_orchestrator::secretary::classify(&input.role, &input.content) {
     ```
     becomes:
     ```rust
     if let Some(classified) =
         secretary_candidate(&input.role, &input.content, input.already_submitted)
     {
     ```
     Current `Ok(raw)` arm (lines 201-217):
     ```rust
     Ok(raw) => {
         let item_id = raw
             .get("task_id")
             .and_then(|v| v.as_u64())
             .map(|v| v.to_string())
             .unwrap_or_else(|| "unknown".to_string());
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
     ```
     becomes:
     ```rust
     Ok(raw) => {
         if let Some(item_id) = submitted_task_id(&raw) {
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
         } else {
             // Daemon deduped (task_id null, duplicate_of set): nothing new
             // was enqueued, so no toast and no tasks-changed ping.
             tracing::debug!("secretary: daemon deduped near-duplicate; suppressing toast");
         }
     }
     ```

  4. Fix the now-broken existing test constructor. In `chat_append_rejects_empty_session_id` (lines 273-279), the struct literal
     ```rust
     let input = ChatAppendInput {
         session_id: "   ".to_string(),
         role: "user".to_string(),
         content: "hi".to_string(),
         task_id: None,
         model_id: None,
     };
     ```
     gains one line:
     ```rust
     let input = ChatAppendInput {
         session_id: "   ".to_string(),
         role: "user".to_string(),
         content: "hi".to_string(),
         task_id: None,
         model_id: None,
         already_submitted: false,
     };
     ```

- [ ] Run and confirm all Rust tests pass:
  ```
  cargo test -p vox-gui chat::tests
  ```
  Expected: all tests in `commands::chat::tests` pass, including the three new ones.

- [ ] **Write the failing frontend test.** Create `crates/vox-gui/ui/src/lib/composerSubmit.test.ts`:

  ```ts
  import { describe, expect, it } from 'vitest';
  import { readFileSync } from 'node:fs';
  import { resolve } from 'node:path';
  import { userAppendInput } from './composerSubmit';

  describe('userAppendInput', () => {
    it('marks composer messages as already submitted so the backend secretary skips them', () => {
      const input = userAppendInput(
        'sess-1',
        'refactor the login flow so it stops redirecting users after auth',
      );
      expect(input.already_submitted).toBe(true);
      expect(input.session_id).toBe('sess-1');
      expect(input.role).toBe('user');
      expect(input.task_id).toBeNull();
      expect(input.content).toBe('refactor the login flow so it stops redirecting users after auth');
    });

    it('stringifies missing descriptions to the empty string', () => {
      expect(userAppendInput('sess-1', undefined).content).toBe('');
      expect(userAppendInput('sess-1', null).content).toBe('');
    });
  });

  // F28 wiring guard: the payload-builder tests above cannot catch a reverted
  // or skipped call-site edit — revert the App.tsx change and they stay green
  // while the double-submit returns. Mirror the Task 2 idiom (ErrorBoundary
  // .test.tsx reads main.tsx via readFileSync) and pin the composer persist
  // call to the new payload builder.
  describe('App.tsx composer persist wiring (C2)', () => {
    it('routes the chat_append_message payload through userAppendInput', () => {
      const app = readFileSync(resolve(__dirname, '../App.tsx'), 'utf8');
      const call = app.indexOf("invoke('chat_append_message'");
      expect(call).toBeGreaterThan(-1);
      // The FIRST chat_append_message invoke in App.tsx is the composer
      // user-persist path (the later one at ~849 persists assistant replies);
      // its input must come from userAppendInput(...).
      expect(app.slice(call, call + 220)).toContain('userAppendInput(sessionId');
      // The old inline payload (which never carried already_submitted) is gone.
      expect(app).not.toContain("{ session_id: sessionId, role: 'user'");
    });
  });
  ```

  No Rust-side source-guard test is added for the `chat_append_message` gate rewiring (impl step 3): `secretary_candidate` and `submitted_task_id` are private fns, so skipping or reverting that edit leaves them referenced only from `#[cfg(test)]` code, and this task's `cargo clippy -p vox-gui -- -D warnings` step fails on `dead_code` — the clippy gate already covers that seam.

- [ ] Run and confirm the expected failure (module does not exist):
  ```
  pnpm --dir crates/vox-gui/ui test -- src/lib/composerSubmit.test.ts
  ```
  Expected: `Cannot find module './composerSubmit'` (or equivalent resolve error). (After `composerSubmit.ts` exists but before the App.tsx edit below, the file loads and the two payload-builder tests pass while the `App.tsx composer persist wiring (C2)` test fails — `userAppendInput(sessionId` not found in App.tsx.)

- [ ] **Implement the frontend side.** Create `crates/vox-gui/ui/src/lib/composerSubmit.ts`:

  ```ts
  /**
   * Payload builder for persisting a composer-submitted user message via the
   * `chat_append_message` Tauri command.
   *
   * `already_submitted: true` tells the backend secretary (chat.rs
   * `chat_append_message`) that this message was ALREADY dispatched as a task
   * by the composer (`submit_orchestrator_task`). Without it, every actionable
   * composer message was submitted twice (audit finding C2), producing a
   * spurious "Secretary proposed a task" toast or a wrong near-duplicate
   * confirm dialog depending on which submit lost the race.
   */
  export interface ComposerUserAppendInput {
    session_id: string;
    role: 'user';
    content: string;
    task_id: null;
    already_submitted: true;
  }

  export function userAppendInput(
    sessionId: string,
    description: unknown,
  ): ComposerUserAppendInput {
    return {
      session_id: sessionId,
      role: 'user',
      content: String(description ?? ''),
      task_id: null,
      already_submitted: true,
    };
  }
  ```

  Then in `crates/vox-gui/ui/src/App.tsx`, replace the composer persist call. Current code (lines 677-679):
  ```ts
  invoke('chat_append_message', {
    input: { session_id: sessionId, role: 'user', content: String(payload.description ?? ''), task_id: null },
  }).catch((err) => pushToast({ tone: 'warn', title: 'Message not saved', body: String(err), cause: 'backend-error' }));
  ```
  becomes:
  ```ts
  invoke('chat_append_message', {
    input: userAppendInput(sessionId, payload.description),
  }).catch((err) => pushToast({ tone: 'warn', title: 'Message not saved', body: String(err), cause: 'backend-error' }));
  ```
  And add the import next to the existing UI imports. Current line 14:
  ```ts
  import { Toasts, ToastItem } from './components/ui/Toasts';
  ```
  becomes:
  ```ts
  import { Toasts, ToastItem } from './components/ui/Toasts';
  import { userAppendInput } from './lib/composerSubmit';
  ```
  Do **not** touch the assistant-persist call at App.tsx:849 — `classify()` returns `None` for non-`user` roles, so it cannot double-submit.

- [ ] Run and confirm the frontend passes:
  ```
  pnpm --dir crates/vox-gui/ui test -- src/lib/composerSubmit.test.ts
  pnpm --dir crates/vox-gui/ui typecheck
  ```
  Expected: 3 tests pass (2 payload-builder + 1 App.tsx wiring guard); typecheck clean.

- [ ] Format, lint, commit:
  ```
  cargo fmt -p vox-gui
  cargo clippy -p vox-gui -- -D warnings
  git -C C:/Users/Owner/vox add crates/vox-gui/src/commands/chat.rs crates/vox-gui/ui/src/lib/composerSubmit.ts crates/vox-gui/ui/src/lib/composerSubmit.test.ts crates/vox-gui/ui/src/App.tsx
  git -C C:/Users/Owner/vox commit -m "fix(gui): stop secretary double-submit of composer messages (C2)"
  ```

---

## Task 2: Wrap `<App/>` in the existing ErrorBoundary (C3)

`main.tsx` renders `<App/>` bare; any throw in App/Sidebar/StatusBar/DockShell white-screens the window. The full-screen recovery component `components/ErrorBoundary.tsx` exists (renders "Display Runtime Error" + a "Recover State" button) and is imported nowhere.

**Files:**
- Create: `crates/vox-gui/ui/src/components/ErrorBoundary.test.tsx`
- Modify: `crates/vox-gui/ui/src/main.tsx` (imports at lines 1-9, render tree at lines 40-48)

### Steps

- [ ] **Write the failing test.** Create `crates/vox-gui/ui/src/components/ErrorBoundary.test.tsx` (idiom copied from `components/dashboard/WidgetErrorBoundary.test.tsx`):

  ```tsx
  // @vitest-environment jsdom
  import { describe, expect, it, vi, beforeEach, afterEach } from 'vitest';
  import { render, screen } from '@testing-library/react';
  import { readFileSync } from 'node:fs';
  import { resolve } from 'node:path';
  import React from 'react';
  import { ErrorBoundary } from './ErrorBoundary';

  function Boom(): React.ReactElement {
    throw new Error('kaboom in shell');
  }

  describe('ErrorBoundary', () => {
    // React logs the caught error to console.error; silence it for a clean run.
    let spy: ReturnType<typeof vi.spyOn>;
    beforeEach(() => { spy = vi.spyOn(console, 'error').mockImplementation(() => {}); });
    afterEach(() => { spy.mockRestore(); });

    it('renders the recovery screen instead of white-screening when a child throws', () => {
      render(
        <ErrorBoundary>
          <Boom />
        </ErrorBoundary>,
      );
      expect(screen.getByText('Display Runtime Error')).toBeTruthy();
      expect(screen.getByText('kaboom in shell')).toBeTruthy();
      expect(screen.getByRole('button', { name: /recover state/i })).toBeTruthy();
    });

    it('renders children unchanged when nothing throws', () => {
      render(
        <ErrorBoundary>
          <div data-testid="ok-body">fine</div>
        </ErrorBoundary>,
      );
      expect(screen.getByTestId('ok-body')).toBeTruthy();
    });
  });

  describe('main.tsx wiring (C3 regression: boundary existed but was imported nowhere)', () => {
    it('wraps the app tree in ErrorBoundary', () => {
      const main = readFileSync(resolve(__dirname, '../main.tsx'), 'utf8');
      expect(main).toContain("import { ErrorBoundary } from './components/ErrorBoundary'");
      const open = main.indexOf('<ErrorBoundary>');
      expect(open).toBeGreaterThan(-1);
      expect(open).toBeLessThan(main.indexOf('<App />'));
      expect(main).toContain('</ErrorBoundary>');
    });
  });
  ```

- [ ] Run and confirm the expected failure:
  ```
  pnpm --dir crates/vox-gui/ui test -- src/components/ErrorBoundary.test.tsx
  ```
  Expected: the two component tests **pass** (the component itself already works); the `main.tsx wiring` test **fails** (no ErrorBoundary import in main.tsx).

- [ ] **Implement.** In `crates/vox-gui/ui/src/main.tsx`, add the import. Current line 5:
  ```ts
  import App from './App'
  ```
  becomes:
  ```ts
  import App from './App'
  import { ErrorBoundary } from './components/ErrorBoundary'
  ```
  Then wrap the render tree. Current code (lines 40-48):
  ```tsx
  ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
    <React.StrictMode>
      <LanguageProvider>
        <QueryClientProvider client={queryClient}>
          <App />
        </QueryClientProvider>
      </LanguageProvider>
    </React.StrictMode>,
  )
  ```
  becomes:
  ```tsx
  ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
    <React.StrictMode>
      <ErrorBoundary>
        <LanguageProvider>
          <QueryClientProvider client={queryClient}>
            <App />
          </QueryClientProvider>
        </LanguageProvider>
      </ErrorBoundary>
    </React.StrictMode>,
  )
  ```
  The boundary sits **outside** `LanguageProvider`/`QueryClientProvider` so a throw inside either provider is also caught. `ErrorBoundary` itself uses no context, so this is safe.

- [ ] Run and confirm all pass:
  ```
  pnpm --dir crates/vox-gui/ui test -- src/components/ErrorBoundary.test.tsx
  pnpm --dir crates/vox-gui/ui typecheck
  ```
  Expected: 3 tests pass; typecheck clean.

- [ ] Commit:
  ```
  git -C C:/Users/Owner/vox add crates/vox-gui/ui/src/main.tsx crates/vox-gui/ui/src/components/ErrorBoundary.test.tsx
  git -C C:/Users/Owner/vox commit -m "fix(gui): wrap App in top-level ErrorBoundary (C3)"
  ```

---

## Task 3: CodeRabbit toast shape fix (B6)

`CodeRabbitView.tsx` pushes toasts as `{ kind, message }`, but `Toasts.tsx` renders `{ tone, title, body, cause }` (`ToastItem`, backed by the `Toast` type in `types/tauri.ts` where `cause` is **required**). Result: blank toast cards. It type-checks today only because `CodeRabbitViewProps` declares `pushToast: (t: any) => void` — the loose signature the spec says to tighten.

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/CodeRabbit/CodeRabbitView.test.tsx`
- Modify: `crates/vox-gui/ui/src/components/surfaces/CodeRabbit/CodeRabbitView.tsx` (props at lines 5-8, toast pushes at lines 73-78, 102, 112, 115)

### Steps

- [ ] **Write the failing test.** Create `crates/vox-gui/ui/src/components/surfaces/CodeRabbit/CodeRabbitView.test.tsx`:

  ```tsx
  // @vitest-environment jsdom
  import { describe, it, expect, vi, beforeEach } from 'vitest';
  import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
  import React from 'react';

  type ProgressEvent = { payload: { status: string; error?: string } };
  let progressCb: ((e: ProgressEvent) => void) | null = null;
  const listenMock = vi.fn();
  vi.mock('@tauri-apps/api/event', () => ({
    listen: (...a: unknown[]) => (listenMock as (...a: unknown[]) => unknown)(...a),
  }));

  const planMock = vi.fn();
  vi.mock('../../../transport', () => ({
    codeRabbitPlan: (...a: unknown[]) => planMock(...a),
    codeRabbitReport: vi.fn().mockResolvedValue({}),
    codeRabbitRunAsync: vi.fn().mockResolvedValue(undefined),
    codeRabbitTokenPresent: vi.fn().mockResolvedValue(true),
  }));

  import { CodeRabbitView } from './CodeRabbitView';

  describe('CodeRabbitView toast shape (B6)', () => {
    beforeEach(() => {
      progressCb = null;
      planMock.mockReset();
      listenMock.mockReset();
      listenMock.mockImplementation((_evt: string, cb: (e: ProgressEvent) => void) => {
        progressCb = cb;
        return Promise.resolve(() => {});
      });
    });

    it('pushes a Toast-shaped success toast when a run finishes', async () => {
      const pushToast = vi.fn();
      render(<CodeRabbitView pushToast={pushToast} />);
      await waitFor(() => expect(progressCb).toBeTruthy());
      await act(async () => {
        progressCb!({ payload: { status: 'ok' } });
      });
      expect(pushToast).toHaveBeenCalledWith({
        tone: 'ok',
        title: 'CodeRabbit run finished',
        cause: 'backend-ok',
      });
    });

    it('pushes a Toast-shaped warn toast when a run fails', async () => {
      const pushToast = vi.fn();
      render(<CodeRabbitView pushToast={pushToast} />);
      await waitFor(() => expect(progressCb).toBeTruthy());
      await act(async () => {
        progressCb!({ payload: { status: 'error', error: 'rate limited' } });
      });
      expect(pushToast).toHaveBeenCalledWith({
        tone: 'warn',
        title: 'CodeRabbit run failed',
        body: 'rate limited',
        cause: 'backend-error',
      });
    });

    it('pushes a Toast-shaped warn toast when planning fails', async () => {
      planMock.mockRejectedValue(new Error('boom'));
      const pushToast = vi.fn();
      render(<CodeRabbitView pushToast={pushToast} />);
      fireEvent.click(screen.getByRole('button', { name: /plan sweep/i }));
      await waitFor(() => expect(pushToast).toHaveBeenCalled());
      const toast = pushToast.mock.calls[0][0];
      expect(toast.tone).toBe('warn');
      expect(toast.title).toBe('Plan failed');
      expect(String(toast.body)).toContain('boom');
      expect(toast.cause).toBe('backend-error');
      expect(toast).not.toHaveProperty('kind');
      expect(toast).not.toHaveProperty('message');
    });
  });
  ```

- [ ] Run and confirm the expected failure:
  ```
  pnpm --dir crates/vox-gui/ui test -- src/components/surfaces/CodeRabbit/CodeRabbitView.test.tsx
  ```
  Expected: all three tests fail — `pushToast` is called with `{ kind: ..., message: ... }`, not the Toast shape.

- [ ] **Implement.** Four edits in `crates/vox-gui/ui/src/components/surfaces/CodeRabbit/CodeRabbitView.tsx`:

  1. Tighten the loose signature. Current code (lines 1-8):
     ```tsx
     import React, { useCallback, useEffect, useState } from 'react';
     import { listen, type UnlistenFn } from '@tauri-apps/api/event';
     import { codeRabbitPlan, codeRabbitReport, codeRabbitRunAsync, codeRabbitTokenPresent } from '../../../transport';

     interface CodeRabbitViewProps {
       pushToast: (t: any) => void;
       gamifyEnabled?: boolean;
     }
     ```
     becomes:
     ```tsx
     import React, { useCallback, useEffect, useState } from 'react';
     import { listen, type UnlistenFn } from '@tauri-apps/api/event';
     import { codeRabbitPlan, codeRabbitReport, codeRabbitRunAsync, codeRabbitTokenPresent } from '../../../transport';
     import type { Toast } from '../../../types/tauri';

     interface CodeRabbitViewProps {
       pushToast: (t: Toast) => void;
       gamifyEnabled?: boolean;
     }
     ```
     (`Toast` requires `cause`, so any future `{kind, message}` regression fails `tsc --noEmit`.)

  2. Progress-listener toast. Current code (lines 75-78):
     ```tsx
     _props.pushToast({
       kind: e.payload.status === 'error' ? 'error' : 'success',
       message: e.payload.status === 'error' ? `CodeRabbit run failed: ${e.payload.error}` : 'CodeRabbit run finished',
     });
     ```
     becomes:
     ```tsx
     _props.pushToast(
       e.payload.status === 'error'
         ? { tone: 'warn', title: 'CodeRabbit run failed', body: e.payload.error, cause: 'backend-error' }
         : { tone: 'ok', title: 'CodeRabbit run finished', cause: 'backend-ok' },
     );
     ```

  3. Plan-failure toast. Current code (line 102):
     ```tsx
     _props.pushToast({ kind: 'error', message: `Plan failed: ${err}` });
     ```
     becomes:
     ```tsx
     _props.pushToast({ tone: 'warn', title: 'Plan failed', body: String(err), cause: 'backend-error' });
     ```

  4. Run toasts. Current code (lines 112 and 115):
     ```tsx
     _props.pushToast({ kind: 'info', message: 'CodeRabbit sweep started' });
     ```
     becomes:
     ```tsx
     _props.pushToast({ tone: 'info', title: 'CodeRabbit sweep started', cause: 'backend-ok' });
     ```
     and:
     ```tsx
     _props.pushToast({ kind: 'error', message: `Run failed: ${err}` });
     ```
     becomes:
     ```tsx
     _props.pushToast({ tone: 'warn', title: 'Run failed', body: String(err), cause: 'backend-error' });
     ```

- [ ] Run and confirm all pass:
  ```
  pnpm --dir crates/vox-gui/ui test -- src/components/surfaces/CodeRabbit/CodeRabbitView.test.tsx
  pnpm --dir crates/vox-gui/ui typecheck
  ```
  Expected: 3 tests pass; typecheck clean (the tightened prop matches App's `pushToast: (t: Toast) => void` handed down via `surfaceComponents.tsx:167`).

- [ ] Commit:
  ```
  git -C C:/Users/Owner/vox add crates/vox-gui/ui/src/components/surfaces/CodeRabbit/CodeRabbitView.tsx crates/vox-gui/ui/src/components/surfaces/CodeRabbit/CodeRabbitView.test.tsx
  git -C C:/Users/Owner/vox commit -m "fix(gui): CodeRabbit toasts use the real Toast shape and a typed pushToast prop (B6)"
  ```

---

## Task 4: Make `ds-section-head` styling real (B7)

`.ds-section-head` (Cinzel display font + underline divider — the project's "dividers underline headings" rule) is defined in `crates/vox-gui/ds/components.css:38-47`, which is imported only by `crates/vox-gui/ds/styles.css` — and **nothing under `ui/` imports either file**. So `MissionControlPanel.tsx` (4 usages), `VoxGraphStatusPanel.tsx:97`, and `NeedsYouSurface.tsx:121,157` render those headings unstyled.

**Decision (made here after diffing both sheets): migrate the single used class into `ui/src/index.css` — do NOT import `ds/styles.css`.** Rationale, verified against file contents:

- The only `.ds-*` class referenced anywhere under `ui/src/` is `ds-section-head` (ripgrep across all `ds-` class names: 7 hits, 3 files, all `ds-section-head`).
- `ds/styles.css` is the standalone Claude Design bundle: it `@import`s its own fonts (`ds/fonts/fonts.css`, duplicating `ui/src/styles/fonts.css`), re-declares the full `--color-*` token set from a **parallel** token build (`ds/tokens/tokens.basalt.css` vs the app's generated `ui/src/styles/tokens.generated.css` — a drift hazard), and sets `:root { color-scheme; background; color; font-family }`, which would fight the app's theme switching (`applyTheme`).
- Every variable `.ds-section-head` uses already exists in the app's generated tokens: `--font-family-display` (`tokens.generated.css:77`), `--color-text-muted` (`:46`), `--color-border-subtle` (`:47`).

**Files:**
- Modify: `crates/vox-gui/ui/src/index.css` (`@layer components` block, after `.vox-display` at line 61)
- Modify: `crates/vox-gui/ui/src/index.css.test.ts` (append a describe block)
- No change: `crates/vox-gui/ds/components.css` stays as-is (it serves the standalone design bundle; add a sync note only).

### Steps

- [ ] **Write the failing test.** Append to `crates/vox-gui/ui/src/index.css.test.ts` (after the `STATUS_TONE` describe block):

  ```ts
  describe('ds-section-head (B7: migrated from ds/components.css so it actually loads)', () => {
    it('is defined in the app stylesheet', () => {
      expect(css).toContain('.ds-section-head');
    });

    it('underlines the heading (divider below, never a cap above)', () => {
      const start = css.indexOf('.ds-section-head');
      const body = css.slice(start, css.indexOf('}', start));
      expect(body).toContain('border-bottom: 1px solid var(--color-border-subtle)');
      expect(body).toContain('font-family: var(--font-family-display)');
      expect(body).not.toContain('border-top');
    });

    it('does not pull in the standalone ds bundle (token/font double-load hazard)', () => {
      expect(css).not.toContain("@import '../ds");
      expect(css).not.toContain('ds/styles.css');
    });
  });
  ```

- [ ] Run and confirm the expected failure:
  ```
  pnpm --dir crates/vox-gui/ui test -- src/index.css.test.ts
  ```
  Expected: the first two new tests fail (`.ds-section-head` not present in index.css); the third passes.

- [ ] **Implement.** In `crates/vox-gui/ui/src/index.css`, inside the `@layer components { ... }` block, directly after the `.vox-display` rule (line 61):
  ```css
  .vox-display { font-family: var(--font-family-display); letter-spacing: 0.13em; text-transform: uppercase; }
  ```
  add:
  ```css
  /* Section heading — migrated from ds/components.css `.ds-section-head` (B7:
     the ds/ sheet is not imported by the app, so the class was dead). A divider
     that introduces a section UNDERLINES its heading (border-bottom beneath the
     label) — it never caps the section with a top border above the first label.
     Keep in sync with ds/components.css. */
  .ds-section-head {
    font-family: var(--font-family-display);
    font-size: 9px;
    letter-spacing: 0.28em;
    text-transform: uppercase;
    color: var(--color-text-muted);
    padding-bottom: 4px;
    margin-bottom: 6px;
    border-bottom: 1px solid var(--color-border-subtle);
  }
  ```
  (Byte-for-byte the same declarations as `ds/components.css:38-47`.)

- [ ] Add the reciprocal sync note in `crates/vox-gui/ds/components.css`. Current comment block (lines 33-37):
  ```css
  /* --- section heading ---------------------------------------------------- *
   * A divider that introduces a section UNDERLINES its heading (border-bottom
   * beneath the label) — it never caps the section with a top border above the
   * first label, which crowds the text and hurts readability. Items follow below.
   */
  ```
  becomes:
  ```css
  /* --- section heading ---------------------------------------------------- *
   * A divider that introduces a section UNDERLINES its heading (border-bottom
   * beneath the label) — it never caps the section with a top border above the
   * first label, which crowds the text and hurts readability. Items follow below.
   * NOTE: the app copy of this rule lives in ui/src/index.css (`.ds-section-head`);
   * keep the two in sync when editing.
   */
  ```

- [ ] Run and confirm all pass, then verify nothing else regressed:
  ```
  pnpm --dir crates/vox-gui/ui test -- src/index.css.test.ts
  pnpm --dir crates/vox-gui/ui test -- src/components/surfaces/MissionControl src/components/surfaces/VoxGraph src/components/surfaces/NeedsYou
  pnpm --dir crates/vox-gui/ui typecheck
  ```
  Expected: all pass. (Visual confirmation on the NeedsYou/VoxGraph surfaces happens in the post-merge screenshot sweep — spec Phase 3 — no action here.)

- [ ] Commit:
  ```
  git -C C:/Users/Owner/vox add crates/vox-gui/ui/src/index.css crates/vox-gui/ui/src/index.css.test.ts crates/vox-gui/ds/components.css
  git -C C:/Users/Owner/vox commit -m "fix(gui): make ds-section-head real by migrating it into index.css (B7)"
  ```

---

## Task 5: Listener leaks, unhandled `listen()` rejections, `useLocalStorage` warn

Six independent listener micro-fixes with one shared theme: async `listen()` subscriptions must survive unmount races (disposed-flag pattern, reference implementation `components/surfaces/Console/AgentTab.tsx:16-37`) and must never leave an unhandled promise rejection when the event bridge is unavailable (vite preview, tests, headless capture) — SubAgentsView, NeedsYouSurface, TasksView, SettingsView, CodeRabbitView, and ActivitySurface (F1: two unguarded chains at `ActivitySurface.tsx:283-303`, same shape as SettingsView — no `.catch` anywhere, cleanup `.then((unlisten) => unlisten())`). Plus: `useLocalStorage` currently logs storage errors via `console.log`, hiding them from warning filters. Plus (F18): `ChatSurface.tsx:130-132` re-triggers session hydration that `App.tsx:652-654` already owns — a redundant second `chat_get_messages` (limit 500) fetch on every session switch — so the ChatSurface trigger and its now-dead `onHydrateSession` prop are deleted. Scope note (F2): `BrowserView.tsx`'s three listener effects (`listenAgentEvents`/`listenBrowserFrames`/`listenPreviewAvailable`, `BrowserView.tsx:175-238`) are deliberately OUT of scope — each already `.catch()`es its `listen()` chain, so the residual risk there is leak-only (no disposed flag if unmount wins the race against `listen()` resolving), never an unhandled rejection; that lower-priority conversion is deferred, so do not assume this task fully closes the leak class.

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/SubAgents/SubAgentsView.tsx` (lines 32-42)
- Modify: `crates/vox-gui/ui/src/components/surfaces/SubAgents/SubAgentsView.test.tsx` (mock at lines 5-9, beforeEach, new test)
- Modify: `crates/vox-gui/ui/src/components/surfaces/NeedsYou/NeedsYouSurface.tsx` (lines 48-60)
- Modify: `crates/vox-gui/ui/src/components/surfaces/NeedsYou/__tests__/NeedsYouSurface.test.tsx` (new test)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.tsx` (lines 82-93)
- Create: `crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.listeners.test.tsx`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx` (lines 759-762)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.test.tsx` (new test)
- Modify: `crates/vox-gui/ui/src/components/surfaces/CodeRabbit/CodeRabbitView.tsx` (lines 80-88)
- Modify: `crates/vox-gui/ui/src/components/surfaces/CodeRabbit/CodeRabbitView.test.tsx` (new test)
- Modify: `crates/vox-gui/ui/src/hooks/useLocalStorage.ts` (lines 14, 23)
- Create: `crates/vox-gui/ui/src/hooks/useLocalStorage.test.ts`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Activity/ActivitySurface.tsx` (lines 283-303)
- Create: `crates/vox-gui/ui/src/components/surfaces/Activity/ActivitySurface.listeners.test.tsx`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx` (prop at lines 36 and 59, hydrate effect at lines 130-132)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx` (imports; new hydration-ownership describe)
- Modify: `crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx` (lines 60, 192)
- Modify: `crates/vox-gui/ui/src/App.tsx` (line 1104; live line is 1105 after Task 1 added one import line)

### Steps

- [ ] **Write the failing tests (all sites, then implement all).**

  **(a)** In `crates/vox-gui/ui/src/components/surfaces/SubAgents/SubAgentsView.test.tsx`, first make the `listenActivity` mock overridable. Current code (lines 5-9):
  ```ts
  const fetchTreeMock = vi.fn();
  vi.mock('./subAgentClient', () => ({
    fetchTree: (...a: unknown[]) => fetchTreeMock(...a),
    listenActivity: vi.fn().mockRejectedValue(new Error('not tauri')),
  }));
  ```
  becomes:
  ```ts
  const fetchTreeMock = vi.fn();
  const listenActivityMock = vi.fn();
  vi.mock('./subAgentClient', () => ({
    fetchTree: (...a: unknown[]) => fetchTreeMock(...a),
    listenActivity: (...a: unknown[]) => listenActivityMock(...a),
  }));
  ```
  and inside the existing `beforeEach` (after `fetchTreeMock.mockResolvedValue(...)`), add:
  ```ts
  listenActivityMock.mockReset();
  listenActivityMock.mockRejectedValue(new Error('not tauri'));
  ```
  Then append the new test inside the `describe('SubAgentsView', ...)` block:
  ```ts
  it('unlistens immediately when unmounted before listenActivity resolves (leak guard)', async () => {
    const unlisten = vi.fn();
    let resolveListen!: (u: () => void) => void;
    listenActivityMock.mockReset();
    listenActivityMock.mockImplementation(
      () => new Promise<() => void>((res) => { resolveListen = res; }),
    );
    const { unmount } = render(<SubAgentsView pushToast={() => {}} />);
    unmount();
    // Subscription resolves only AFTER unmount — the disposed flag must fire it.
    resolveListen(unlisten);
    await waitFor(() => expect(unlisten).toHaveBeenCalledTimes(1));
  });
  ```

  **(b)** In `crates/vox-gui/ui/src/components/surfaces/NeedsYou/__tests__/NeedsYouSurface.test.tsx`, append inside the `describe('NeedsYouSurface', ...)` block:
  ```ts
  it('unlistens immediately when unmounted before listenFeedbackChanged resolves (leak guard)', async () => {
    const unlisten = vi.fn();
    let resolveListen!: (u: () => void) => void;
    vi.spyOn(transport, 'listenFeedbackChanged').mockImplementation(
      () => new Promise((res) => { resolveListen = res; }),
    );
    const { unmount } = render(
      <LanguageProvider>
        <NeedsYouSurface onOpenContext={() => {}} pushToast={() => {}} />
      </LanguageProvider>,
    );
    await waitFor(() => expect(resolveListen).toBeTruthy());
    unmount();
    resolveListen(unlisten);
    await waitFor(() => expect(unlisten).toHaveBeenCalledTimes(1));
  });
  ```

  **(c)** Create `crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.listeners.test.tsx`:
  ```tsx
  // @vitest-environment jsdom
  import { describe, it, expect, vi } from 'vitest';
  import { render, screen, waitFor } from '@testing-library/react';
  import React from 'react';

  // Simulate the bare-browser case: the Tauri event bridge is unavailable and
  // every listen() rejects. Vitest fails the run on unhandled rejections, so
  // this test is red until the .catch guards exist.
  vi.mock('@tauri-apps/api/event', () => ({
    listen: vi.fn().mockRejectedValue(new Error('event bridge unavailable')),
  }));
  vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn().mockResolvedValue(null),
  }));

  import * as transport from '../../../transport';
  import { TasksView } from './TasksView';

  describe('TasksView listener guards', () => {
    it('mounts and unmounts without unhandled rejections when listen() rejects', async () => {
      vi.spyOn(transport, 'hopperList').mockResolvedValue([]);
      vi.spyOn(transport, 'feedbackList').mockResolvedValue({ needsYou: [], withheld: [] });
      vi.spyOn(transport, 'listenFeedbackChanged').mockRejectedValue(
        new Error('event bridge unavailable'),
      );
      const { unmount } = render(<TasksView />);
      await waitFor(() => expect(screen.getByText('Tasks')).toBeTruthy());
      unmount();
      // Flush microtasks so any dangling rejection surfaces and fails the run.
      await new Promise((r) => setTimeout(r, 0));
    });
  });
  ```

  **(d)** In `crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.test.tsx`, the module mock at lines 69-71 is `listen: vi.fn().mockResolvedValue(() => {})`. Append inside `describe('SettingsView', ...)`:
  ```ts
  it('mounts and unmounts without unhandled rejections when the config-changed listener fails', async () => {
    const { listen } = await import('@tauri-apps/api/event');
    (listen as unknown as ReturnType<typeof vi.fn>).mockRejectedValueOnce(
      new Error('event bridge unavailable'),
    );
    const { unmount } = render(<SettingsView pushToast={vi.fn()} />, { wrapper });
    await screen.findByLabelText('Search settings');
    unmount();
    await new Promise((r) => setTimeout(r, 0));
  });
  ```

  **(e)** In `crates/vox-gui/ui/src/components/surfaces/CodeRabbit/CodeRabbitView.test.tsx` (created in Task 3), append inside the describe block:
  ```ts
  it('mounts and unmounts without unhandled rejections when the progress listener fails', async () => {
    listenMock.mockReset();
    listenMock.mockRejectedValue(new Error('event bridge unavailable'));
    const { unmount } = render(<CodeRabbitView pushToast={vi.fn()} />);
    await new Promise((r) => setTimeout(r, 0));
    unmount();
    await new Promise((r) => setTimeout(r, 0));
  });
  ```

  **(f)** Create `crates/vox-gui/ui/src/hooks/useLocalStorage.test.ts`:
  ```ts
  // @vitest-environment jsdom
  import { describe, it, expect, vi, afterEach } from 'vitest';
  import { act, renderHook } from '@testing-library/react';
  import { useLocalStorage } from './useLocalStorage';

  describe('useLocalStorage error reporting', () => {
    afterEach(() => vi.restoreAllMocks());

    it('warns (not console.log) and falls back when reading throws', () => {
      const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
      const log = vi.spyOn(console, 'log').mockImplementation(() => {});
      vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
        throw new Error('storage disabled');
      });
      const { result } = renderHook(() => useLocalStorage('lk-read', 'fallback'));
      expect(result.current[0]).toBe('fallback');
      expect(warn).toHaveBeenCalled();
      expect(log).not.toHaveBeenCalled();
    });

    it('warns (not console.log) when writing throws', () => {
      const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
      const log = vi.spyOn(console, 'log').mockImplementation(() => {});
      vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
        throw new Error('quota exceeded');
      });
      const { result } = renderHook(() => useLocalStorage('lk-write', 'v'));
      act(() => { result.current[1]('next'); });
      expect(warn).toHaveBeenCalled();
      expect(log).not.toHaveBeenCalled();
    });
  });
  ```

  **(g)** Create `crates/vox-gui/ui/src/components/surfaces/Activity/ActivitySurface.listeners.test.tsx` (mirrors the TasksView idiom in (c); kept separate from the existing `ActivitySurface.container.test.tsx`, whose module-level transport mock resolves its listeners):
  ```tsx
  // @vitest-environment jsdom
  import { describe, it, expect, vi } from 'vitest';
  import { render, screen, waitFor } from '@testing-library/react';
  import React from 'react';

  // Simulate the bare-browser case (F1): both Activity listener chains reject
  // when the Tauri event bridge is unavailable. Vitest fails the run on
  // unhandled rejections, so this test is red until the .catch guards exist.
  vi.mock('../../../transport', () => ({
    activityQuery: vi.fn().mockResolvedValue([]),
    listenActivityAppended: vi.fn().mockRejectedValue(new Error('event bridge unavailable')),
    listenAgentEvents: vi.fn().mockRejectedValue(new Error('event bridge unavailable')),
  }));

  import { ActivitySurface } from './ActivitySurface';

  describe('ActivitySurface listener guards', () => {
    it('mounts and unmounts without unhandled rejections when listen() rejects', async () => {
      const { unmount } = render(<ActivitySurface pushToast={() => {}} />);
      await waitFor(() => expect(screen.getByText(/agent activity timeline/i)).toBeTruthy());
      unmount();
      // Flush microtasks so any dangling rejection surfaces and fails the run.
      await new Promise((r) => setTimeout(r, 0));
    });
  });
  ```

  **(h)** In `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx`, add the fs imports next to the existing vitest imports. Current code (lines 1-4):
  ```tsx
  // @vitest-environment jsdom
  import { describe, it, expect, vi, beforeEach } from 'vitest';
  import { render, screen, waitFor, act } from '@testing-library/react';
  import React from 'react';
  ```
  becomes:
  ```tsx
  // @vitest-environment jsdom
  import { describe, it, expect, vi, beforeEach } from 'vitest';
  import { render, screen, waitFor, act } from '@testing-library/react';
  import { readFileSync } from 'node:fs';
  import { resolve } from 'node:path';
  import React from 'react';
  ```
  Then append a new top-level describe after the closing `});` of `describe('ChatSurface', ...)` (end of file), mirroring the Task 2 readFileSync wiring idiom:
  ```tsx
  describe('session hydration ownership (F18: redundant double hydrate per session switch)', () => {
    it('ChatSurface has no hydrate trigger — App.tsx owns hydration', () => {
      const surface = readFileSync(resolve(__dirname, './ChatSurface.tsx'), 'utf8');
      // The redundant effect (`if (activeId && onHydrateSession) onHydrateSession(activeId)`)
      // and its prop are gone — App's activeSessionId effect is the only trigger.
      expect(surface).not.toContain('onHydrateSession');
      const surfaces = readFileSync(resolve(__dirname, '../../layout/surfaceComponents.tsx'), 'utf8');
      expect(surfaces).not.toContain('onHydrateChatSession');
      const app = readFileSync(resolve(__dirname, '../../../App.tsx'), 'utf8');
      expect(app).toContain('hydrateChatSession(activeSessionId)');
    });
  });
  ```

- [ ] Run and confirm the expected failures:
  ```
  pnpm --dir crates/vox-gui/ui test -- src/components/surfaces/SubAgents/SubAgentsView.test.tsx src/components/surfaces/NeedsYou/__tests__/NeedsYouSurface.test.tsx src/components/surfaces/Tasks/TasksView.listeners.test.tsx src/components/surfaces/Settings/SettingsView.test.tsx src/components/surfaces/CodeRabbit/CodeRabbitView.test.tsx src/hooks/useLocalStorage.test.ts src/components/surfaces/Activity/ActivitySurface.listeners.test.tsx src/components/surfaces/Chat/ChatSurface.test.tsx
  ```
  Expected: SubAgents leak-guard test fails (unlisten never called); NeedsYou leak-guard test fails; TasksView and SettingsView tests fail with reported unhandled rejections; CodeRabbitView rejection test fails with an unhandled rejection; both useLocalStorage tests fail (`console.log` used, `console.warn` not); the ActivitySurface listener test fails with reported unhandled rejections (from both unguarded chains); the ChatSurface hydration-ownership test fails (`onHydrateSession` still present in ChatSurface.tsx). ChatSurface's seven pre-existing tests stay green.

- [ ] **Implement all sites.**

  **(a)** `SubAgentsView.tsx`, the listener effect. Current code (lines 32-42):
  ```ts
  useEffect(() => {
    let un: (() => void) | undefined;
    listenActivity((e) => {
      // Backend does not yet stamp window_id on agent events (audit correction #1):
      // route by window_id when present, else attribute to the selected window.
      const w = (e.kind as { window_id?: string }).window_id
        ?? useSubAgentStore.getState().selectedWindowId;
      if (w) pushEvent(w, e);
    }).then((u) => { un = u; }).catch(() => {});
    return () => un?.();
  }, [pushEvent]);
  ```
  becomes (disposed-flag pattern from `AgentTab.tsx:16-37`):
  ```ts
  useEffect(() => {
    let disposed = false;
    let un: (() => void) | undefined;
    listenActivity((e) => {
      // Backend does not yet stamp window_id on agent events (audit correction #1):
      // route by window_id when present, else attribute to the selected window.
      const w = (e.kind as { window_id?: string }).window_id
        ?? useSubAgentStore.getState().selectedWindowId;
      if (w) pushEvent(w, e);
    })
      .then((u) => {
        // Unmount may win the race against the async subscription resolving.
        if (disposed) u();
        else un = u;
      })
      .catch(() => {});
    return () => {
      disposed = true;
      un?.();
    };
  }, [pushEvent]);
  ```

  **(b)** `NeedsYouSurface.tsx`, the self-fetch effect body. Current code (lines 48-60):
  ```ts
    let unlisten: (() => void) | null = null;
    listenFeedbackChanged(() => {
      refresh();
    }).then((un) => {
      unlisten = un;
    });

    const timer = setInterval(refresh, 5000);

    return () => {
      if (unlisten) unlisten();
      clearInterval(timer);
    };
  ```
  becomes:
  ```ts
    let disposed = false;
    let unlisten: (() => void) | null = null;
    listenFeedbackChanged(() => {
      refresh();
    })
      .then((un) => {
        // Unmount may win the race against the async subscription resolving.
        if (disposed) un();
        else unlisten = un;
      })
      .catch(() => { /* event bridge unavailable (bare browser/tests) — poll still runs */ });

    const timer = setInterval(refresh, 5000);

    return () => {
      disposed = true;
      if (unlisten) unlisten();
      clearInterval(timer);
    };
  ```

  **(c)** `TasksView.tsx`, the self-fetch effect. Current code (lines 82-93):
  ```ts
    selfRefresh();
    const sub = listen<void>('vox://tasks-changed', () => {
      selfRefresh();
    });
    const subFeedback = listenFeedbackChanged(() => {
      selfRefresh();
    });
    return () => {
      mounted.current = false;
      sub.then((fn) => fn());
      subFeedback.then((fn) => fn());
    };
  ```
  becomes:
  ```ts
    selfRefresh();
    // listen() rejects when the Tauri event bridge is unavailable (bare
    // browser, tests) — guard so nothing leaks an unhandled rejection and
    // cleanup still resolves.
    const sub = listen<void>('vox://tasks-changed', () => {
      selfRefresh();
    }).catch(() => undefined);
    const subFeedback = listenFeedbackChanged(() => {
      selfRefresh();
    }).catch(() => undefined);
    return () => {
      mounted.current = false;
      sub.then((fn) => fn?.());
      subFeedback.then((fn) => fn?.());
    };
  ```

  **(d)** `SettingsView.tsx`, the config-changed listener. Current code (lines 759-762):
  ```ts
  useEffect(() => {
    const un = listen('vox://llm-config-changed', () => { reload(); });
    return () => { un.then((f) => f()); };
  }, [reload]);
  ```
  becomes:
  ```ts
  useEffect(() => {
    // Guarded: listen() rejects outside Tauri (bare browser/tests).
    const un = listen('vox://llm-config-changed', () => { reload(); }).catch(() => undefined);
    return () => { un.then((f) => f?.()); };
  }, [reload]);
  ```

  **(e)** `CodeRabbitView.tsx`, the progress listener. Current code (lines 73-88, after Task 3's toast change):
  ```ts
    listen<{ status: string; error?: string }>('coderabbit://progress', (e) => {
      ...
    }).then((u) => {
      // If we unmounted before listen() resolved, unlisten immediately (no leak).
      if (cancelled) u();
      else un = u;
    });
  ```
  — append a `.catch` to that chain:
  ```ts
    }).then((u) => {
      // If we unmounted before listen() resolved, unlisten immediately (no leak).
      if (cancelled) u();
      else un = u;
    }).catch(() => { /* event bridge unavailable — no progress toasts */ });
  ```

  **(f)** `useLocalStorage.ts`: change both `console.log(error);` occurrences (lines 14 and 23) to `console.warn(error);`.

  **(g)** `ActivitySurface.tsx`, both listener effects (F1) — same `.catch(() => undefined)` + `fn?.()` treatment as TasksView in (c). Current code (lines 282-303):
  ```ts
  // Reactive updates on "vox://activity-appended"
  useEffect(() => {
    const unlistenPromise = listenActivityAppended(() => {
      fetchLogs();
    });
    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, [fetchLogs]);

  // Also refresh on "vox://agent-events" — this event IS already emitted by the
  // Rust daemon bridge (spawn_agent_event_stream), whereas "vox://activity-appended"
  // has no Rust emitter yet. This makes the timeline update live without any new
  // backend work (Option B: lazy reactive refresh).
  useEffect(() => {
    const unlistenPromise = listenAgentEvents(() => {
      fetchLogs();
    });
    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, [fetchLogs]);
  ```
  becomes:
  ```ts
  // Reactive updates on "vox://activity-appended"
  useEffect(() => {
    // listen() rejects when the Tauri event bridge is unavailable (bare
    // browser, tests, headless capture) — guard so nothing leaks an
    // unhandled rejection and cleanup still resolves.
    const unlistenPromise = listenActivityAppended(() => {
      fetchLogs();
    }).catch(() => undefined);
    return () => {
      unlistenPromise.then((unlisten) => unlisten?.());
    };
  }, [fetchLogs]);

  // Also refresh on "vox://agent-events" — this event IS already emitted by the
  // Rust daemon bridge (spawn_agent_event_stream), whereas "vox://activity-appended"
  // has no Rust emitter yet. This makes the timeline update live without any new
  // backend work (Option B: lazy reactive refresh).
  useEffect(() => {
    // Guarded like the effect above: listen() rejects outside Tauri.
    const unlistenPromise = listenAgentEvents(() => {
      fetchLogs();
    }).catch(() => undefined);
    return () => {
      unlistenPromise.then((unlisten) => unlisten?.());
    };
  }, [fetchLogs]);
  ```

  **(h)** Delete the redundant hydrate trigger and its dead prop (F18). The prop's only consumer is the effect being deleted, its only wiring is `surfaceComponents.tsx:192` (fed from `App.tsx` `onHydrateChatSession: hydrateChatSession`), and no test references either name — so the whole thread comes out; `App.tsx:652-654` (the effect that fires `hydrateChatSession(activeSessionId)` on every session switch) remains the single hydration owner. Four edits:

  1. `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx` — drop the effect. Current code (lines 130-132):
     ```ts
     useEffect(() => {
       if (activeId && onHydrateSession) onHydrateSession(activeId);
     }, [activeId, onHydrateSession]);
     ```
     Delete these three lines (and the blank line that follows, keeping one blank line between the neighboring effects).
  2. Same file — drop the prop. In `ChatSurfaceProps` (line 36), delete:
     ```ts
       onHydrateSession?: (sessionId: string) => void;
     ```
     and in the destructuring parameter list (line 59), delete:
     ```ts
       onHydrateSession,
     ```
  3. `crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx` — delete the `SurfaceProps` field (line 60):
     ```ts
       onHydrateChatSession?: (sessionId: string) => void;
     ```
     and the JSX pass-through inside `case 'chat'` (line 192):
     ```tsx
               onHydrateSession={props.onHydrateChatSession}
     ```
  4. `crates/vox-gui/ui/src/App.tsx` — delete the supplier line (line 1104 pre-plan; 1105 after Task 1's import). Current code:
     ```ts
         onHydrateChatSession: hydrateChatSession,
     ```
     `hydrateChatSession` itself stays: its remaining caller is the App-level effect (`if (activeSessionId) hydrateChatSession(activeSessionId);`), which is exactly the single owner this fix leaves in place. Removing all four sites together is required for a clean `tsc --noEmit` (the object literal at App.tsx:1104 would fail the excess-property check once the `SurfaceProps` field is gone).

- [ ] Run and confirm all pass:
  ```
  pnpm --dir crates/vox-gui/ui test -- src/components/surfaces/SubAgents/SubAgentsView.test.tsx src/components/surfaces/NeedsYou/__tests__/NeedsYouSurface.test.tsx src/components/surfaces/Tasks/TasksView.listeners.test.tsx src/components/surfaces/Settings/SettingsView.test.tsx src/components/surfaces/CodeRabbit/CodeRabbitView.test.tsx src/hooks/useLocalStorage.test.ts src/components/surfaces/Activity/ActivitySurface.listeners.test.tsx src/components/surfaces/Chat/ChatSurface.test.tsx
  pnpm --dir crates/vox-gui/ui typecheck
  ```
  Expected: all pass, no unhandled-rejection reports in the vitest summary.

- [ ] Commit:
  ```
  git -C C:/Users/Owner/vox add crates/vox-gui/ui/src/components/surfaces/SubAgents/SubAgentsView.tsx crates/vox-gui/ui/src/components/surfaces/SubAgents/SubAgentsView.test.tsx crates/vox-gui/ui/src/components/surfaces/NeedsYou/NeedsYouSurface.tsx "crates/vox-gui/ui/src/components/surfaces/NeedsYou/__tests__/NeedsYouSurface.test.tsx" crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.tsx crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.listeners.test.tsx crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.test.tsx crates/vox-gui/ui/src/components/surfaces/CodeRabbit/CodeRabbitView.tsx crates/vox-gui/ui/src/components/surfaces/CodeRabbit/CodeRabbitView.test.tsx crates/vox-gui/ui/src/hooks/useLocalStorage.ts crates/vox-gui/ui/src/hooks/useLocalStorage.test.ts crates/vox-gui/ui/src/components/surfaces/Activity/ActivitySurface.tsx crates/vox-gui/ui/src/components/surfaces/Activity/ActivitySurface.listeners.test.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx crates/vox-gui/ui/src/App.tsx
  git -C C:/Users/Owner/vox commit -m "fix(gui): listener leak/rejection guards, useLocalStorage warns, drop double chat hydrate"
  ```

---

## Task 6: Tasks copy honesty (B1 interim)

Chat submissions go to the orchestrator task graph (`submit_orchestrator_task` → daemon `SUBMIT_TASK`); the Tasks surface lists the SQLite hopper (`hopper_list`). Two lies must go: the TasksView subtitle "Chat submissions land here." and the chat.rs comment "submit to hopper". The full merge-view is Phase 2 (fork F1) — this task only makes the words true.

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.copy.test.tsx`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.tsx` (lines 252-254)
- Modify: `crates/vox-gui/src/commands/chat.rs` (comment at lines 168-169 — no test possible for a comment; verified by review)

### Steps

- [ ] **Write the failing test.** Create `crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.copy.test.tsx`:

  ```tsx
  // @vitest-environment jsdom
  import { describe, it, expect, vi } from 'vitest';
  import { render, screen } from '@testing-library/react';
  import React from 'react';

  vi.mock('@tauri-apps/api/event', () => ({
    listen: vi.fn().mockResolvedValue(() => {}),
  }));
  vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn().mockResolvedValue(null),
  }));

  import { TasksView } from './TasksView';
  import type { AttentionInbox } from '../../../hooks/useAttentionInbox';

  // Shared-inbox mode: no self-fetching, so no transport spies needed.
  const attention: AttentionInbox = {
    approvals: [],
    needsYou: [],
    withheld: [],
    blockedTasksCount: 0,
    hopperTasks: [],
    totalCount: 0,
    refresh: vi.fn().mockResolvedValue(undefined),
    resolveApproval: vi.fn().mockResolvedValue(undefined),
    resolveFeedback: vi.fn().mockResolvedValue(undefined),
  };

  describe('TasksView copy honesty (B1 interim)', () => {
    it('does not claim chat submissions land here, and says where rows really come from', () => {
      render(<TasksView attention={attention} />);
      // The old lie: chat submissions go to the orchestrator task graph, not the hopper.
      expect(screen.queryByText(/chat submissions land here/i)).toBeNull();
      // The honest replacement names both stores.
      expect(screen.getByText(/hopper/i)).toBeTruthy();
      expect(screen.getByText(/orchestrator task graph/i)).toBeTruthy();
    });
  });
  ```

- [ ] Run and confirm the expected failure:
  ```
  pnpm --dir crates/vox-gui/ui test -- src/components/surfaces/Tasks/TasksView.copy.test.tsx
  ```
  Expected: fails on `queryByText(/chat submissions land here/i)` being non-null (and on the missing honest copy).

- [ ] **Implement.** In `crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.tsx`, current subtitle (lines 252-254):
  ```tsx
          <p className="text-[11px] text-text-muted">
            Everything queued or running across the agent fleet. Chat submissions land here.
          </p>
  ```
  becomes:
  ```tsx
          <p className="text-[11px] text-text-muted">
            The hopper to-do queue — items added from the composer below. Chat
            submissions run in the orchestrator task graph and are not listed here yet.
          </p>
  ```
  Then in `crates/vox-gui/src/commands/chat.rs`, the current comment (lines 168-169):
  ```rust
      // Secretary: detect actionable intent in user messages and submit to hopper.
      // Fire-and-forget — errors here must never fail the chat message save.
  ```
  becomes:
  ```rust
      // Secretary: detect actionable intent in user messages and submit it to the
      // orchestrator daemon task graph (SUBMIT_TASK) — NOT the SQLite hopper that
      // the Tasks surface lists (store unification is Phase 2, fork F1).
      // Fire-and-forget — errors here must never fail the chat message save.
  ```

- [ ] Run and confirm passes (and that the comment-only Rust edit still compiles):
  ```
  pnpm --dir crates/vox-gui/ui test -- src/components/surfaces/Tasks/TasksView.copy.test.tsx
  cargo test -p vox-gui chat::tests
  ```
  Expected: frontend test passes; Rust tests unchanged and green.

- [ ] Commit:
  ```
  git -C C:/Users/Owner/vox add crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.tsx crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.copy.test.tsx crates/vox-gui/src/commands/chat.rs
  git -C C:/Users/Owner/vox commit -m "fix(gui): honest Tasks subtitle and secretary comment (B1 interim)"
  ```

---

## Task 7: `gui-visual-review` date fix

`crates/vox-orchestrator-mcp/src/bin/gui-visual-review.rs:21` defaults `--date` to the literal `"0000-00-00"`, which is how the junk report removed in Task 0 got created. Fix both layers: the CLI defaults to the real system UTC date, and `write_report` refuses any date that is not a real `YYYY-MM-DD` (belt-and-suspenders so the file can never recur). `chrono` is already a dependency of `vox-orchestrator-mcp` (Cargo.toml:117) and `tempfile` is available for tests (Cargo.toml:132); `chrono::Utc::now().format("%Y-%m-%d")` is the crate's established idiom (`agy_tools.rs:60`).

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/visus_review/mod.rs` (`write_report` at lines 409-418; new fn + tests)
- Modify: `crates/vox-orchestrator-mcp/src/bin/gui-visual-review.rs` (import at line 3, `--date` default at line 21)

### Steps

- [ ] **Write the failing tests.** In `crates/vox-orchestrator-mcp/src/visus_review/mod.rs`, append after `write_report` (end of file):

  ```rust
  #[cfg(test)]
  mod report_date_tests {
      use super::*;

      fn empty_report() -> RunReport {
          RunReport {
              schema_version: 1,
              generated_at: "t".into(),
              default_model: "m".into(),
              surfaces: vec![],
              total_capture_ms: 0,
              total_review_ms: 0,
              surfaces_reviewed: 0,
              surfaces_cached: 0,
              surfaces_deferred: 0,
              spiked: false,
              spike_detail: String::new(),
          }
      }

      #[test]
      fn default_report_date_is_a_real_utc_date() {
          let d = default_report_date();
          assert_ne!(d, "0000-00-00");
          assert!(
              chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").is_ok(),
              "not a real YYYY-MM-DD date: {d}"
          );
      }

      #[test]
      fn write_report_refuses_the_zero_date_placeholder() {
          let dir = tempfile::tempdir().unwrap();
          let err = write_report(dir.path(), "0000-00-00", &empty_report())
              .expect_err("0000-00-00 must be refused");
          assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
          assert!(!dir.path().join("0000-00-00.json").exists());
      }

      #[test]
      fn write_report_refuses_non_date_strings() {
          let dir = tempfile::tempdir().unwrap();
          assert!(write_report(dir.path(), "not-a-date", &empty_report()).is_err());
          assert!(write_report(dir.path(), "", &empty_report()).is_err());
      }

      #[test]
      fn write_report_accepts_a_real_date() {
          let dir = tempfile::tempdir().unwrap();
          let p = write_report(dir.path(), "2026-07-16", &empty_report())
              .expect("real date accepted");
          assert!(p.exists());
          assert!(p.ends_with("2026-07-16.json"));
      }
  }
  ```

- [ ] Run and confirm the expected failure:
  ```
  cargo test -p vox-orchestrator-mcp visus_review::report_date_tests
  ```
  Expected: compile error — `default_report_date` does not exist; after adding only that fn, `write_report_refuses_*` fail because `write_report` happily writes any string.

- [ ] **Implement.** In `crates/vox-orchestrator-mcp/src/visus_review/mod.rs`, replace `write_report` (currently lines 409-418):
  ```rust
  pub fn write_report(
      report_dir: &Path,
      date: &str,
      report: &RunReport,
  ) -> std::io::Result<std::path::PathBuf> {
      std::fs::create_dir_all(report_dir)?;
      let path = report_dir.join(format!("{date}.json"));
      std::fs::write(&path, serde_json::to_string_pretty(report).unwrap() + "\n")?;
      Ok(path)
  }
  ```
  with:
  ```rust
  /// Default report date for the CLI: today's UTC date. Replaces the historical
  /// `--date`-absent behavior of writing a junk `0000-00-00.json` report.
  pub fn default_report_date() -> String {
      chrono::Utc::now().format("%Y-%m-%d").to_string()
  }

  pub fn write_report(
      report_dir: &Path,
      date: &str,
      report: &RunReport,
  ) -> std::io::Result<std::path::PathBuf> {
      // Refuse placeholder/garbage dates ("0000-00-00" has month 0 and fails the
      // parse) so a stray report file can never be produced again.
      if chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err() {
          return Err(std::io::Error::new(
              std::io::ErrorKind::InvalidInput,
              format!("refusing to write report: {date:?} is not a real YYYY-MM-DD date"),
          ));
      }
      std::fs::create_dir_all(report_dir)?;
      let path = report_dir.join(format!("{date}.json"));
      std::fs::write(&path, serde_json::to_string_pretty(report).unwrap() + "\n")?;
      Ok(path)
  }
  ```
  Then in `crates/vox-orchestrator-mcp/src/bin/gui-visual-review.rs`, current import (line 3):
  ```rust
  use vox_orchestrator_mcp::visus_review::{RunArgs, run, write_report};
  ```
  becomes:
  ```rust
  use vox_orchestrator_mcp::visus_review::{RunArgs, default_report_date, run, write_report};
  ```
  and the current default (line 21):
  ```rust
      let date = get("--date").unwrap_or_else(|| "0000-00-00".into());
  ```
  becomes:
  ```rust
      let date = get("--date").unwrap_or_else(default_report_date);
  ```
  (The bin already prints a `::warning::` and exits 0 when `write_report` errs — advisory contract preserved.)

- [ ] Run and confirm all pass:
  ```
  cargo test -p vox-orchestrator-mcp visus_review
  ```
  Expected: the 4 new tests pass alongside the existing `verdict_tests`/`decide_tests` and the other visus_review module tests.

- [ ] Format, lint, commit:
  ```
  cargo fmt -p vox-orchestrator-mcp
  cargo clippy -p vox-orchestrator-mcp -- -D warnings
  git -C C:/Users/Owner/vox add crates/vox-orchestrator-mcp/src/visus_review/mod.rs crates/vox-orchestrator-mcp/src/bin/gui-visual-review.rs
  git -C C:/Users/Owner/vox commit -m "fix(ci): gui-visual-review defaults --date to real UTC date and refuses invalid dates"
  ```

---

## Task 8: Chat token-loss race — buffer unroutable frames, replay after submitResolved

`sessionChatStore.ts` drops any `agentEvent` whose session cannot be resolved (`if (!sessionId) return base;` at line 143). When `task_started` (without a `session_id` field) or early `token_streamed` frames arrive **before** the `submit_orchestrator_task` invoke resolves (`submitResolved` is what seeds `taskToSession`), those frames — including real streamed tokens — are silently lost. Fix entirely inside `sessionChatStore.ts`: buffer unroutable `token_streamed`/`task_started` frames (bounded by a 30 s window relative to the newest frame, plus a 200-frame hard cap) and replay them through the reducer once `submitResolved` lands. `chatCorrelation.ts` needs **no** change: once the routing layer delivers the replayed `task_started` to the right session, `chatReducer` seeds `agentToTask` and the replayed tokens append normally.

**Files:**
- Modify: `crates/vox-gui/ui/src/lib/sessionChatStore.ts` (interface at lines 13-22, `submitResolved` case at lines 101-114, `agentEvent` case at lines 127-148)
- Modify: `crates/vox-gui/ui/src/lib/sessionChatStore.test.ts` (existing literal at lines 107-110 gains `pending: []`; three new tests)

### Steps

- [ ] **Write the failing tests.** In `crates/vox-gui/ui/src/lib/sessionChatStore.test.ts`, first fix the store literal in `resolveSessionForEvent prefers taskToSession` (lines 107-110). Current code:
  ```ts
    const store = {
      sessions: {},
      taskToSession: { '42': 'sess-x' },
    };
  ```
  becomes:
  ```ts
    const store = {
      sessions: {},
      taskToSession: { '42': 'sess-x' },
      pending: [],
    };
  ```
  Then append three tests inside `describe('sessionChatStore', ...)`:
  ```ts
  it('buffers frames that race ahead of submitResolved and replays them (token-loss fix)', () => {
    let store = sessionChatReducer(initialSessionChatStore, {
      type: 'submit',
      sessionId: 'sess-a',
      runId: 'R1',
      prompt: 'q',
    });
    // task_started arrives BEFORE submitResolved and carries no session_id —
    // unroutable at this point.
    store = sessionChatReducer(store, {
      type: 'agentEvent',
      event: evt({ type: 'task_started', task_id: 7, agent_id: 3 }),
    });
    store = sessionChatReducer(store, {
      type: 'agentEvent',
      event: evt({ type: 'token_streamed', agent_id: 3, text: 'Hi' }),
    });
    // Nothing routed yet; both frames are held, not dropped.
    expect(getSessionMessages(store, 'sess-a').find(m => m.role === 'assistant')?.text).toBe('');
    expect(store.pending.length).toBe(2);
    // The submit resolves — buffered frames replay in order.
    store = sessionChatReducer(store, {
      type: 'submitResolved',
      sessionId: 'sess-a',
      runId: 'R1',
      taskId: '7',
    });
    const assistant = getSessionMessages(store, 'sess-a').find(m => m.role === 'assistant');
    expect(assistant?.text).toBe('Hi');
    expect(assistant?.status).toBe('streaming');
    expect(store.pending.length).toBe(0);
  });

  it('evicts buffered frames older than the replay window', () => {
    let store = sessionChatReducer(initialSessionChatStore, {
      type: 'agentEvent',
      event: { id: 1, timestamp_ms: 1_000, kind: { type: 'token_streamed', agent_id: 9, text: 'stale' } },
    });
    // 60s later — the stale frame is outside the 30s window and gets evicted.
    store = sessionChatReducer(store, {
      type: 'agentEvent',
      event: { id: 2, timestamp_ms: 61_000, kind: { type: 'token_streamed', agent_id: 9, text: 'fresh' } },
    });
    expect(store.pending.map(f => f.id)).toEqual([2]);
  });

  it('does not buffer unroutable frame types outside the race (task_completed etc.)', () => {
    const store = sessionChatReducer(initialSessionChatStore, {
      type: 'agentEvent',
      event: evt({ type: 'task_completed', task_id: 999 }),
    });
    expect(store.pending.length).toBe(0);
  });
  ```

- [ ] Run and confirm the expected failure:
  ```
  pnpm --dir crates/vox-gui/ui test -- src/lib/sessionChatStore.test.ts
  ```
  Expected: the three new tests fail (`store.pending` is `undefined`; assistant text stays `''` after `submitResolved`). Existing tests still pass.

- [ ] **Implement.** Four edits in `crates/vox-gui/ui/src/lib/sessionChatStore.ts`:

  1. Extend the store shape. Current code (lines 13-22):
     ```ts
     export interface SessionChatStore {
       sessions: Record<string, ChatState>;
       /** task_id → session_id for routing agent events */
       taskToSession: Record<string, string>;
     }

     export const initialSessionChatStore: SessionChatStore = {
       sessions: {},
       taskToSession: {},
     };
     ```
     becomes:
     ```ts
     export interface SessionChatStore {
       sessions: Record<string, ChatState>;
       /** task_id → session_id for routing agent events */
       taskToSession: Record<string, string>;
       /** Unroutable token_streamed/task_started frames buffered until the
        *  submit that owns them resolves; replayed in `submitResolved`. Fixes
        *  the token-loss race where task_started precedes submitResolved. */
       pending: AgentEventFrame[];
     }

     export const initialSessionChatStore: SessionChatStore = {
       sessions: {},
       taskToSession: {},
       pending: [],
     };
     ```

  2. Add the buffering helper below `withSession` (after line 44):
     ```ts
     /** Replay window for unroutable frames: anything older than this (relative
      *  to the newest buffered frame) is a lost cause, not a race, and is evicted. */
     const PENDING_REPLAY_WINDOW_MS = 30_000;
     /** Hard cap so a runaway stream cannot grow the buffer without bound. */
     const PENDING_MAX_FRAMES = 200;
     /** Frame types that participate in the submit race and are worth holding. */
     const BUFFERABLE_TYPES = new Set(['token_streamed', 'task_started']);

     function bufferPending(
       pending: AgentEventFrame[],
       event: AgentEventFrame,
     ): AgentEventFrame[] {
       const cutoff = event.timestamp_ms - PENDING_REPLAY_WINDOW_MS;
       return [...pending.filter((f) => f.timestamp_ms >= cutoff), event].slice(
         -PENDING_MAX_FRAMES,
       );
     }
     ```

  3. Buffer instead of dropping in the `agentEvent` case. Current code (lines 141-147):
     ```ts
           const base = { ...store, taskToSession };
           const sessionId = resolveSessionForEvent(base, action.event);
           if (!sessionId) return base;

           const prev = ensureSession(base, sessionId);
           const next = chatReducer(prev, { type: 'agentEvent', event: action.event });
           return withSession(base, sessionId, next);
     ```
     becomes:
     ```ts
           const base = { ...store, taskToSession };
           const sessionId = resolveSessionForEvent(base, action.event);
           if (!sessionId) {
             // Race: token_streamed/task_started can precede submitResolved (no
             // task→session mapping yet, no session_id on the frame). Buffer
             // instead of dropping; `submitResolved` replays the queue.
             if (BUFFERABLE_TYPES.has(kind.type)) {
               return { ...base, pending: bufferPending(base.pending, action.event) };
             }
             return base;
           }

           const prev = ensureSession(base, sessionId);
           const next = chatReducer(prev, { type: 'agentEvent', event: action.event });
           return withSession(base, sessionId, next);
     ```

  4. Replay in `submitResolved`. Current code (lines 101-114):
     ```ts
         case 'submitResolved': {
           const sid = action.sessionId;
           const taskId = String(action.taskId);
           const prev = ensureSession(store, sid);
           const next = chatReducer(prev, {
             type: 'submitResolved',
             runId: action.runId,
             taskId,
           });
           return {
             ...withSession(store, sid, next),
             taskToSession: { ...store.taskToSession, [taskId]: sid },
           };
         }
     ```
     becomes:
     ```ts
         case 'submitResolved': {
           const sid = action.sessionId;
           const taskId = String(action.taskId);
           const prev = ensureSession(store, sid);
           const resolved = chatReducer(prev, {
             type: 'submitResolved',
             runId: action.runId,
             taskId,
           });
           let next: SessionChatStore = {
             ...withSession(store, sid, resolved),
             taskToSession: { ...store.taskToSession, [taskId]: sid },
           };
           // Replay frames that raced ahead of this resolution, in arrival
           // order. Frames that are STILL unroutable re-buffer themselves via
           // the agentEvent case, so nothing is lost or reordered.
           const queued = next.pending;
           if (queued.length > 0) {
             next = { ...next, pending: [] };
             for (const event of queued) {
               next = sessionChatReducer(next, { type: 'agentEvent', event });
             }
           }
           return next;
         }
     ```

- [ ] Run and confirm all pass (including the untouched neighbors that exercise routing):
  ```
  pnpm --dir crates/vox-gui/ui test -- src/lib/sessionChatStore.test.ts src/lib/chatCorrelation.test.ts
  pnpm --dir crates/vox-gui/ui typecheck
  ```
  Expected: all pass. `chatCorrelation.test.ts` is untouched and must stay green (no changes were made to `chatCorrelation.ts`).

- [ ] Commit:
  ```
  git -C C:/Users/Owner/vox add crates/vox-gui/ui/src/lib/sessionChatStore.ts crates/vox-gui/ui/src/lib/sessionChatStore.test.ts
  git -C C:/Users/Owner/vox commit -m "fix(gui): buffer unroutable chat frames and replay after submitResolved (token-loss race)"
  ```

---

## Final verification (whole plan)

- [ ] Full frontend suite: `pnpm --dir crates/vox-gui/ui test` — expected: green, no unhandled-rejection reports.
- [ ] Frontend typecheck/build gate: `pnpm --dir crates/vox-gui/ui typecheck` — expected: clean.
- [ ] Rust: `cargo test -p vox-gui` and `cargo test -p vox-orchestrator-mcp visus_review` — expected: green. (Do not run workspace-wide `clippy --all-targets`; vox-gui's buildscript breaks it — use the per-crate clippy commands already run in Tasks 1 and 7.)
- [ ] `git -C C:/Users/Owner/vox status --short contracts/reports/gui-visual-review/` — expected: nothing staged, no `0000-00-00.json`.
- [ ] `git log --oneline -9` shows the eight commits (Tasks 1-8; Task 0 has no commit) on top of the starting commit, each independently revertable.
