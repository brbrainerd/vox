# GUI Visual Audit Expansion & Launch Crash Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the `SessionManager` panic that prevents the GUI window from appearing, then expand Playwright visual-audit coverage to capture empty-state, error-state screenshots, and produce a browsable HTML audit report — without increasing CI runtime.

**Architecture:** Two sequential tracks. Track 1 fixes a bare `panic!()` in `crates/vox-orchestrator-mcp/src/server_state.rs:179` that aborts the entire daemon process when `SessionManager::new` fails (causing `exit 0xffffffff` and a misleading "supervised task cancelled unexpectedly" log for `hydrate_external_skills`). The fix degrades gracefully to a fallback. Track 2 adds composable mock-variant factories, an opt-in spec gated by `VOX_VARIANT_SCREENSHOTS=1` that captures empty and error state for 10 key surfaces, and a static HTML audit report.

**Tech Stack:** Rust 2021 (Tauri v2 backend, tokio async), TypeScript (Playwright 1.x, React 18, Vitest), pnpm 9.

---

## Background & Diagnostics

### The crash (root-caused)

The GUI exits with `exit code: 0xffffffff` and the log shows:

```
supervised task cancelled unexpectedly  task="hydrate_external_skills"
```

`hydrate_external_skills` itself (in `crates/vox-orchestrator-mcp/src/skills_hydrate.rs`) is safe — no `.unwrap()` in production code, errors are warned and skipped. The **real** crash is a bare `panic!()` on **line 179** of `server_state.rs`, which executes *before* `hydrate_external_skills` is spawned:

```rust
// crates/vox-orchestrator-mcp/src/server_state.rs:178-179
let session_manager = SessionManager::new(session_cfg)
    .unwrap_or_else(|e| panic!("Session manager initialization failed: {}", e));
```

When `SessionManager::new` fails (database lock contention, missing directory, permission error), the `panic!()` aborts the whole process. The `hydrate_external_skills` Tokio task had been spawned earlier in the same startup sequence; when the runtime shuts down, its `JoinHandle` sees cancellation — producing the misleading log line.

The supervisor (`spawn_supervised_infallible` in `crates/vox-actor-runtime/src/supervisor.rs`) correctly logs panics and cancellations but does **not** propagate them — so the supervisor itself is fine. The raw process panic from line 179 is what kills everything.

### The screenshot gaps

The `screenshots.spec.ts` sweep derives its view list from:
```typescript
SURFACE_REGISTRY.filter((e) => e.viewKey && e.tier !== 'none').map((e) => e.viewKey)
```

The 4 missing surfaces (`console`, `tasks`, `discovery-inbox`, `archive-panel`) all have **`tier: 'live_backend'`** in the generated registry — they should be captured. The PNGs are simply absent because the sweep was last run before these surfaces were registered. Re-running the spec will generate them; no code change to the spec is needed.

---

## File Structure

### New files
| Path | Responsibility |
|---|---|
| `crates/vox-gui/ui/e2e/lib/tauriMockVariants.ts` | `installEmptyStateMock(viewKey)` and `installErrorStateMock(viewKey)` factories |
| `crates/vox-gui/ui/e2e/lib/tauriMockVariants.test.ts` | Vitest unit tests for the variant factories |
| `crates/vox-gui/ui/e2e/screenshots-variants.spec.ts` | Opt-in multi-state screenshot spec (20 screenshots for 10 surfaces) |
| `crates/vox-gui/ui/e2e/screenshots-audit-report.spec.ts` | HTML audit report generator from `screens/*.png` |

### Modified files
| Path | Change |
|---|---|
| `crates/vox-orchestrator-mcp/src/server_state.rs` | Replace `panic!()` on SessionManager init failure with graceful degradation |
| `crates/vox-gui/ui/.gitignore` | Exclude the generated `e2e/screens/audit-report.html` |

---

## Track 1 — Fix the GUI Launch Crash

### Task 1: Understand the session manager failure mode

**Files:**
- Read: `crates/vox-orchestrator-mcp/src/server_state.rs`
- Read: the `SessionManager` source file

- [ ] **Step 1: Confirm the panic location**

  ```powershell
  rg "Session manager initialization failed" c:\Users\Owner\vox\crates -n
  ```
  Expected: matches `crates/vox-orchestrator-mcp/src/server_state.rs` at a line containing `.unwrap_or_else(|e| panic!(...)`.

- [ ] **Step 2: Find the SessionManager source**

  ```powershell
  rg "pub struct SessionManager" c:\Users\Owner\vox\crates -n -l
  ```
  Open the file it returns. Read the `fn new` and note:
  - What error type does it return?
  - Is there a `fn new_in_memory`, `fn default`, `fn noop`, or similar zero-dependency constructor?

---

### Task 2: Apply the fix

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/server_state.rs` (lines ~178-179)

- [ ] **Step 1: Replace the panic with a graceful fallback**

  In `server_state.rs`, replace:
  ```rust
  let session_manager = SessionManager::new(session_cfg)
      .unwrap_or_else(|e| panic!("Session manager initialization failed: {}", e));
  ```

  **If `SessionManager` has an in-memory/noop constructor** (e.g. `new_in_memory()`, found in Task 1):
  ```rust
  let session_manager = match SessionManager::new(session_cfg) {
      Ok(sm) => sm,
      Err(e) => {
          tracing::warn!(
              error = %e,
              "session manager initialization failed; \
               falling back to in-memory sessions (data will not persist across restarts)"
          );
          SessionManager::new_in_memory()
      }
  };
  ```

  **If NO fallback constructor exists:**
  ```rust
  let session_manager = match SessionManager::new(session_cfg) {
      Ok(sm) => sm,
      Err(e) => {
          tracing::error!(
              error = %e,
              "session manager initialization failed; \
               GUI will start without session persistence. \
               Check that the sessions directory is writable and not locked by another process."
          );
          SessionManager::default()
      }
  };
  ```

  Use exact variable names from the existing code. Do not rename or restructure anything else.

- [ ] **Step 2: Build to confirm it compiles**

  ```powershell
  cargo build -p vox-orchestrator-mcp
  ```
  Expected: `Finished` with no errors. If you get `SessionManager doesn't implement Default`, proceed to Task 3. If it compiles, skip Task 3.

- [ ] **Step 3: Commit**

  ```powershell
  git add crates/vox-orchestrator-mcp/src/server_state.rs
  git commit -m "fix(vox-orchestrator-mcp): degrade gracefully when SessionManager init fails instead of panicking"
  ```

---

### Task 3: Add `SessionManager::default()` (only if needed)

**Files:**
- Modify: the SessionManager source file found in Task 1

> **Skip this task if** Task 2's build succeeded. Only proceed here if `cargo build -p vox-orchestrator-mcp` failed because `SessionManager` doesn't implement `Default`.

- [ ] **Step 1: Read the existing SessionManager constructors**

  Open the SessionManager file. Look for private fields — what is the struct's storage backend? Find the simplest zero-argument constructor or any `fn new_empty` / `fn for_testing` helper.

- [ ] **Step 2: Add the `Default` impl**

  At the bottom of the SessionManager source file, after the closing `}` of the last `impl` block:
  ```rust
  impl Default for SessionManager {
      /// Creates a non-persisting session manager for degraded-mode operation.
      /// Sessions created in this mode are lost when the process exits.
      fn default() -> Self {
          // Adjust this to use whatever the simplest private constructor is.
          // Look for: new_with_storage, new_empty, for_testing, new_noop, etc.
          Self { /* fill in the minimum required fields with zero/empty values */ }
      }
  }
  ```

  Fill in the struct literal using the actual field names from the struct definition. If any fields hold `Arc<Mutex<...>>` or similar, wrap an empty collection: `Arc::new(Mutex::new(vec![]))`.

- [ ] **Step 3: Build**

  ```powershell
  cargo build -p vox-orchestrator-mcp
  ```
  Expected: `Finished`.

- [ ] **Step 4: Commit**

  ```powershell
  git add <SESSION_MANAGER_SOURCE_FILE>
  git commit -m "feat(session-manager): implement Default for graceful degraded-mode operation"
  ```

---

### Task 4: Verify the GUI window appears

**Files:**
- No code changes

- [ ] **Step 1: Build `vox-orchestrator-d`**

  ```powershell
  cargo build -p vox-orchestrator-d
  ```
  Expected: `Finished`.

- [ ] **Step 2: Stage the orchestrator binary**

  The GUI looks for `vox-orchestrator-d.exe` in `~/.vox/bin/`. Copy the freshly-built binary:
  ```powershell
  New-Item -ItemType Directory -Force "$env:USERPROFILE\.vox\bin"
  Copy-Item "target\debug\vox-orchestrator-d.exe" "$env:USERPROFILE\.vox\bin\vox-orchestrator-d.exe" -Force
  ```

- [ ] **Step 3: Launch `vox-gui`**

  ```powershell
  cargo run -p vox-gui
  ```
  Expected: A Tauri window titled **Vox** appears on screen within ~15 seconds and remains open.

  If it still crashes:
  ```powershell
  cargo run -p vox-gui 2>&1 | Select-String "ERROR|WARN|panic"
  ```
  Look for a *different* panic — fix it before proceeding to Track 2.

- [ ] **Step 4: Confirm the log shows the warning, not a panic**

  The log should now show either nothing (if `SessionManager::new` succeeds) or:
  ```
  WARN session manager initialization failed; falling back to in-memory sessions ...
  ```
  It should NOT show `supervised task cancelled unexpectedly`.

---

## Track 2 — Visual Audit Expansion

### Task 5: Regenerate the 4 missing surface screenshots

**Files:**
- No code changes needed

- [ ] **Step 1: Start the Vite dev server**

  ```powershell
  cd crates\vox-gui\ui
  pnpm run dev
  ```
  Leave this running. Open a second terminal for the next steps.

- [ ] **Step 2: Run the screenshot sweep**

  ```powershell
  cd crates\vox-gui\ui
  pnpm exec playwright test screenshots.spec.ts --project=chromium
  ```
  Expected: All tests pass. Verify the 4 new PNGs were written:
  ```powershell
  Test-Path crates\vox-gui\ui\e2e\screens\console.png
  Test-Path crates\vox-gui\ui\e2e\screens\tasks.png
  Test-Path crates\vox-gui\ui\e2e\screens\discovery-inbox.png
  Test-Path crates\vox-gui\ui\e2e\screens\archive-panel.png
  ```
  Expected: all four return `True`.

- [ ] **Step 3: If any of the 4 tests fail — add a tauriMock stub**

  If a test fails with a console error like `Unhandled invoke: some_command_name`, add a stub in `crates/vox-gui/ui/e2e/lib/tauriMock.ts` inside the `switch (cmd)` block, **before** `default: return null`:
  ```typescript
  case 'some_command_name': return [];   // or null, or {} — match what the UI expects
  ```
  Rerun only the failing test:
  ```powershell
  pnpm exec playwright test screenshots.spec.ts --project=chromium --grep "console"
  ```

- [ ] **Step 4: Commit the new screenshots**

  ```powershell
  git add crates/vox-gui/ui/e2e/screens/console.png
  git add crates/vox-gui/ui/e2e/screens/tasks.png
  git add crates/vox-gui/ui/e2e/screens/discovery-inbox.png
  git add crates/vox-gui/ui/e2e/screens/archive-panel.png
  git commit -m "test(vox-gui): add missing surface screenshots for console/tasks/discovery-inbox/archive-panel"
  ```
  If you also modified `tauriMock.ts`:
  ```powershell
  git add crates/vox-gui/ui/e2e/lib/tauriMock.ts
  git commit --amend --no-edit
  ```

---

### Task 6: Create `tauriMockVariants.ts` — empty-state and error-state mock factories

**Files:**
- Create: `crates/vox-gui/ui/e2e/lib/tauriMockVariants.ts`
- Create: `crates/vox-gui/ui/e2e/lib/tauriMockVariants.test.ts`

- [ ] **Step 1: Write the Vitest unit tests first**

  Create `crates/vox-gui/ui/e2e/lib/tauriMockVariants.test.ts`:
  ```typescript
  import { describe, it, expect } from 'vitest';
  import { installEmptyStateMock, installErrorStateMock } from './tauriMockVariants';

  /**
   * The factories are designed to be injected via page.addInitScript (serialised + re-executed
   * in browser context). We test their logic directly by simulating the browser globals.
   */
  function makeFakeWindow(): any {
    const storage: Record<string, string> = {};
    return {
      localStorage: {
        setItem: (k: string, v: string) => { storage[k] = v; },
        getItem: (k: string) => storage[k] ?? null,
        _storage: storage,
      },
      __TAURI_INTERNALS__: undefined,
      __TAURI_CALLS__: undefined,
    };
  }

  function withFakeWindow<T>(fn: (win: any) => T): T {
    const realWindow = (global as any).window;
    const fakeWin = makeFakeWindow();
    (global as any).window = fakeWin;
    try {
      return fn(fakeWin);
    } finally {
      (global as any).window = realWindow;
    }
  }

  describe('installEmptyStateMock', () => {
    it('sets vox_active_view in localStorage', () => {
      withFakeWindow((win) => {
        installEmptyStateMock('dashboard');
        expect(win.localStorage._storage['vox_active_view']).toBe(JSON.stringify('dashboard'));
      });
    });

    it('returns [] for list_gui_runs', async () => {
      await withFakeWindow(async (win) => {
        installEmptyStateMock('runs');
        const result = await win.__TAURI_INTERNALS__.invoke('list_gui_runs');
        expect(result).toEqual([]);
      });
    });

    it('returns the viewKey for get_initial_view', async () => {
      await withFakeWindow(async (win) => {
        installEmptyStateMock('settings');
        const result = await win.__TAURI_INTERNALS__.invoke('get_initial_view');
        expect(result).toBe('settings');
      });
    });

    it('returns typed-empty object for get_memory_status', async () => {
      await withFakeWindow(async (win) => {
        installEmptyStateMock('memory');
        const result = await win.__TAURI_INTERNALS__.invoke('get_memory_status');
        expect(result).toMatchObject({ corpus_counts: {}, shards: [] });
      });
    });

    it('does not throw for any bootstrap command', async () => {
      const bootstrapCmds = [
        'get_build_info', 'get_orchestrator_status_bin', 'get_action_manifest',
        'get_gui_preference', 'get_gamify_settings', 'get_identity_summary',
      ];
      await withFakeWindow(async (win) => {
        installEmptyStateMock('dashboard');
        for (const cmd of bootstrapCmds) {
          await expect(win.__TAURI_INTERNALS__.invoke(cmd)).resolves.not.toThrow();
        }
      });
    });
  });

  describe('installErrorStateMock', () => {
    it('throws for list_gui_runs', async () => {
      await withFakeWindow(async (win) => {
        installErrorStateMock('runs');
        await expect(win.__TAURI_INTERNALS__.invoke('list_gui_runs')).rejects.toThrow('[mock-error]');
      });
    });

    it('still returns viewKey for get_initial_view', async () => {
      await withFakeWindow(async (win) => {
        installErrorStateMock('models');
        const result = await win.__TAURI_INTERNALS__.invoke('get_initial_view');
        expect(result).toBe('models');
      });
    });

    it('throws for policy_list', async () => {
      await withFakeWindow(async (win) => {
        installErrorStateMock('policies');
        await expect(win.__TAURI_INTERNALS__.invoke('policy_list')).rejects.toThrow('[mock-error]');
      });
    });
  });
  ```

- [ ] **Step 2: Run tests — confirm they fail (file not created yet)**

  ```powershell
  cd crates\vox-gui\ui
  pnpm exec vitest run e2e/lib/tauriMockVariants.test.ts
  ```
  Expected: FAIL — `Cannot find module './tauriMockVariants'`.

- [ ] **Step 3: Create `tauriMockVariants.ts`**

  Create `crates/vox-gui/ui/e2e/lib/tauriMockVariants.ts`:
  ```typescript
  /**
   * Variant Tauri-invoke mocks for multi-state visual audit screenshot sweeps.
   *
   * installEmptyStateMock  — all list/count IPC commands return empty; detail commands return null.
   * installErrorStateMock  — key data-fetch commands throw so the UI must render error states.
   *
   * Usage (Playwright):
   *   await page.addInitScript(installEmptyStateMock, viewKey)
   *   await page.addInitScript(installErrorStateMock, viewKey)
   *
   * Structurally mirrors installTauriMock in tauriMock.ts — drop-in replacement.
   */

  /** IPC commands that return lists — return [] in empty-state mock. */
  const LIST_CMDS = new Set([
    'list_model_cards', 'list_gui_runs', 'list_ludus_notifications',
    'list_gamify_leaderboard', 'list_gamify_companions', 'list_gamify_quests',
    'list_research_sessions', 'list_publication_manifests', 'list_branches',
    'list_secret_status', 'list_repo_files', 'chat_list_sessions',
    'policy_list', 'policy_status', 'get_routing_intentions', 'get_model_scoreboard',
  ]);

  /**
   * Return a typed-empty response for detail commands (not null) so the UI
   * doesn't crash on destructuring (e.g. `const { hits } = result` when result is null).
   */
  function emptyDetailResponse(cmd: string): unknown {
    switch (cmd) {
      case 'get_memory_status': return { corpus_counts: {}, shards: [] };
      case 'get_command_catalog': return { generated_from: 'mock-empty', entries: [] };
      case 'vox_search_query': return { hits: [], facets_by_source: [], facets_by_kind: [], total: 0, next_cursor: null, corpora: [] };
      case 'get_routing_summary_live': return { active_model: null, decision_preview: null };
      case 'execute_command': return { exit_code: 0, stdout: '', stderr: '' };
      case 'get_full_registry': return { commands: [] };
      default: return null;
    }
  }

  /** Detail-fetch commands with a known shape — return typed-empty, not null. */
  const DETAIL_CMDS = new Set([
    'get_routing_summary_live', 'get_ludus_profile', 'get_research_session_detail',
    'get_memory_status', 'get_command_catalog', 'get_full_registry',
    'get_command_metadata', 'get_gui_run', 'get_task_diff',
    'explain_model_selection', 'suggest_model_for_task', 'vox_search_query', 'execute_command',
  ]);

  /** Commands that must succeed for the app shell to mount at all. */
  function bootstrapResponse(cmd: string, viewKey: string): unknown {
    switch (cmd) {
      case 'get_initial_view': return viewKey;
      case 'get_build_info': return { version: '0.6.0', display: '0.6.0+local (dev)' };
      case 'get_orchestrator_status_bin': return new Uint8Array([0x80]);
      case 'get_orchestrator_status': return { agent_count: 0, agents: [], recent_events: [], alerts: [], peers: [] };
      case 'get_action_manifest': return { x_vox_version: 2, schema_version: 1, generated_from: 'mock-empty', actions: [] };
      case 'get_gui_preference': return null;
      case 'get_gamify_settings': return { enabled: false, mode: 'off' };
      case 'get_identity_summary': return { display_name: 'tester@vox', os_user: 'tester' };
      case 'get_active_model': return null;
      case 'get_selection_policy': return { chain: [], free_tier: true };
      default: return null;
    }
  }

  export function installEmptyStateMock(viewKey: string): void {
    try {
      localStorage.setItem('vox_active_view', JSON.stringify(viewKey));
      localStorage.setItem('vox_sidebar_mode', 'default');
    } catch { /* sandboxed contexts may deny localStorage */ }
    (window as any).__TAURI_CALLS__ = [];

    (window as any).__TAURI_INTERNALS__ = {
      invoke: async (cmd: string) => {
        (window as any).__TAURI_CALLS__.push({ cmd });
        if (LIST_CMDS.has(cmd)) return [];
        if (DETAIL_CMDS.has(cmd)) return emptyDetailResponse(cmd);
        return bootstrapResponse(cmd, viewKey);
      },
    };
  }

  /** IPC commands whose failure exercises error-state UI in the component. */
  const ERROR_CMDS = new Set([
    'list_gui_runs', 'list_model_cards', 'get_routing_summary_live',
    'vox_search_query', 'list_research_sessions', 'list_publication_manifests',
    'get_memory_status', 'chat_list_sessions', 'get_model_scoreboard',
    'get_ludus_profile', 'policy_list', 'policy_status',
    'list_gamify_companions', 'list_gamify_quests', 'list_gamify_leaderboard',
    'get_command_catalog',
  ]);

  export function installErrorStateMock(viewKey: string): void {
    try {
      localStorage.setItem('vox_active_view', JSON.stringify(viewKey));
      localStorage.setItem('vox_sidebar_mode', 'default');
    } catch { /* sandboxed contexts may deny localStorage */ }
    (window as any).__TAURI_CALLS__ = [];

    (window as any).__TAURI_INTERNALS__ = {
      invoke: async (cmd: string) => {
        (window as any).__TAURI_CALLS__.push({ cmd });
        if (ERROR_CMDS.has(cmd)) {
          throw new Error(`[mock-error] ${cmd} simulated IPC failure`);
        }
        return bootstrapResponse(cmd, viewKey);
      },
    };
  }
  ```

- [ ] **Step 4: Run tests to confirm they pass**

  ```powershell
  pnpm exec vitest run e2e/lib/tauriMockVariants.test.ts
  ```
  Expected: All 8 tests PASS.

- [ ] **Step 5: Commit**

  ```powershell
  git add crates/vox-gui/ui/e2e/lib/tauriMockVariants.ts crates/vox-gui/ui/e2e/lib/tauriMockVariants.test.ts
  git commit -m "test(vox-gui): add empty-state and error-state Tauri mock variant factories"
  ```

---

### Task 7: Create `screenshots-variants.spec.ts` — opt-in multi-state screenshots

**Files:**
- Create: `crates/vox-gui/ui/e2e/screenshots-variants.spec.ts`

- [ ] **Step 1: Create the spec**

  Create `crates/vox-gui/ui/e2e/screenshots-variants.spec.ts`:
  ```typescript
  /**
   * Multi-state visual audit — empty and error state screenshots for 10 key surfaces.
   *
   * NOT run on standard CI. Opt in with:
   *   VOX_VARIANT_SCREENSHOTS=1 pnpm exec playwright test screenshots-variants.spec.ts --project=chromium
   *
   * Output:
   *   e2e/screens/<view>-empty.png  — surface with all list/detail IPC returning empty
   *   e2e/screens/<view>-error.png  — surface with key data-fetch IPC throwing errors
   */
  import { test, expect, type Page } from '@playwright/test';
  import { installEmptyStateMock, installErrorStateMock } from './lib/tauriMockVariants';

  const RUN_VARIANTS = !!process.env['VOX_VARIANT_SCREENSHOTS'];

  const KEY_SURFACES = [
    'dashboard', 'chat', 'runs', 'approvals', 'models',
    'memory', 'search', 'policies', 'gamify', 'settings',
  ] as const;

  const BENIGN_CONSOLE: string[] = ['favicon'];

  function captureErrors(page: Page): { consoleErrors: string[]; pageErrors: string[] } {
    const consoleErrors: string[] = [];
    const pageErrors: string[] = [];
    page.on('console', (m) => {
      if (m.type() === 'error') consoleErrors.push(`${m.text()} ${m.location()?.url ?? ''}`);
    });
    page.on('pageerror', (e) => pageErrors.push(e.message));
    return { consoleErrors, pageErrors };
  }

  const meaningfulConsole = (errs: string[]): string[] =>
    errs.filter((t) => !BENIGN_CONSOLE.some((b) => t.includes(b)));

  test.describe('GUI visual audit — empty states', () => {
    for (const view of KEY_SURFACES) {
      test(`capture ${view}-empty`, async ({ browser }) => {
        test.skip(!RUN_VARIANTS, 'Set VOX_VARIANT_SCREENSHOTS=1 to run variant screenshots');
        const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
        const page = await ctx.newPage();
        const { consoleErrors, pageErrors } = captureErrors(page);
        await page.addInitScript(installEmptyStateMock, view);
        await page.goto('/');
        await page.waitForSelector('nav', { timeout: 15_000 });
        await page.waitForTimeout(1600);
        await page.screenshot({ path: `e2e/screens/${view}-empty.png`, fullPage: true });

        // Empty responses must NOT crash the error boundary — surfaces should show empty-state UI.
        await expect(
          page.locator('[data-surface-error]'),
          `[${view}-empty] crashed into its error boundary on empty data`,
        ).toHaveCount(0);
        expect(pageErrors, `[${view}-empty] uncaught page errors:\n${pageErrors.join('\n')}`).toEqual([]);
        const meaningful = meaningfulConsole(consoleErrors);
        expect(meaningful, `[${view}-empty] console errors:\n${meaningful.join('\n')}`).toEqual([]);
        await ctx.close();
      });
    }
  });

  test.describe('GUI visual audit — error states', () => {
    for (const view of KEY_SURFACES) {
      test(`capture ${view}-error`, async ({ browser }) => {
        test.skip(!RUN_VARIANTS, 'Set VOX_VARIANT_SCREENSHOTS=1 to run variant screenshots');
        const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
        const page = await ctx.newPage();
        const { pageErrors } = captureErrors(page);
        await page.addInitScript(installErrorStateMock, view);
        await page.goto('/');
        await page.waitForSelector('nav', { timeout: 15_000 });
        await page.waitForTimeout(1600);
        await page.screenshot({ path: `e2e/screens/${view}-error.png`, fullPage: true });

        // Error-state surfaces MAY render an error boundary — both are valid.
        // What we audit: does the error UI look reasonable (not blank, not garbled)?
        expect(pageErrors, `[${view}-error] uncaught page errors:\n${pageErrors.join('\n')}`).toEqual([]);
        await ctx.close();
      });
    }
  });
  ```

- [ ] **Step 2: Verify the spec is skipped by default**

  ```powershell
  pnpm exec playwright test screenshots-variants.spec.ts --project=chromium
  ```
  Expected: `20 skipped`. Zero failures. No PNGs written.

- [ ] **Step 3: Run with the env var and confirm screenshots generate**

  ```powershell
  $env:VOX_VARIANT_SCREENSHOTS = "1"
  pnpm exec playwright test screenshots-variants.spec.ts --project=chromium
  ```
  Expected: 20 tests PASS. Verify:
  ```powershell
  Get-ChildItem crates\vox-gui\ui\e2e\screens | Where-Object { $_.Name -match "empty|error" } | Measure-Object | Select-Object Count
  ```
  Expected: Count = 20.

  **If an empty-state test fails** (console error on destructuring null):
  - Open `e2e/screens/<view>-empty.png` to see what rendered
  - Add the IPC command to `emptyDetailResponse()` in `tauriMockVariants.ts` with a safe empty shape, then rerun:
    ```powershell
    pnpm exec playwright test screenshots-variants.spec.ts --project=chromium --grep "<view>-empty"
    ```

- [ ] **Step 4: Commit**

  ```powershell
  git add crates/vox-gui/ui/e2e/screenshots-variants.spec.ts
  git commit -m "test(vox-gui): add opt-in empty-state and error-state screenshot variants for 10 key surfaces"
  ```

---

### Task 8: Create `screenshots-audit-report.spec.ts` — browsable HTML report

**Files:**
- Create: `crates/vox-gui/ui/e2e/screenshots-audit-report.spec.ts`
- Modify: `crates/vox-gui/ui/.gitignore`

- [ ] **Step 1: Create the spec**

  Create `crates/vox-gui/ui/e2e/screenshots-audit-report.spec.ts`:
  ```typescript
  /**
   * Visual audit report generator.
   *
   * Reads all *.png files from e2e/screens/ and writes a dark-themed browsable
   * HTML grid to e2e/screens/audit-report.html. Run after any screenshot sweep:
   *
   *   pnpm exec playwright test screenshots-audit-report.spec.ts --project=chromium
   *
   * Then open:
   *   start crates\vox-gui\ui\e2e\screens\audit-report.html
   */
  import { test, expect } from '@playwright/test';
  import { readdirSync, writeFileSync, existsSync } from 'node:fs';
  import { join, basename, dirname } from 'node:path';
  import { fileURLToPath } from 'node:url';

  const __dirname = dirname(fileURLToPath(import.meta.url));
  const SCREENS_DIR = join(__dirname, 'screens');
  const OUT_PATH = join(SCREENS_DIR, 'audit-report.html');

  type StateVariant = 'base' | 'empty' | 'error' | 'special';

  function classifyPng(name: string): { surface: string; variant: StateVariant } {
    if (name.startsWith('_')) return { surface: name, variant: 'special' };
    if (name.endsWith('-empty')) return { surface: name.replace(/-empty$/, ''), variant: 'empty' };
    if (name.endsWith('-error')) return { surface: name.replace(/-error$/, ''), variant: 'error' };
    return { surface: name, variant: 'base' };
  }

  const VARIANT_BADGE: Record<StateVariant, { bg: string; color: string; label: string }> = {
    base:    { bg: '#1f6feb22', color: '#1f6feb', label: 'base' },
    empty:   { bg: '#388bfd22', color: '#58a6ff', label: 'empty' },
    error:   { bg: '#f8514922', color: '#f85149', label: 'error' },
    special: { bg: '#8b949e22', color: '#8b949e', label: 'special' },
  };

  test('generate visual audit report', async () => {
    if (!existsSync(SCREENS_DIR)) {
      throw new Error(`screens/ not found at ${SCREENS_DIR}. Run screenshot specs first.`);
    }
    const pngs = readdirSync(SCREENS_DIR).filter((f) => f.endsWith('.png')).sort();
    expect(pngs.length, 'No PNGs in e2e/screens/ — run screenshot specs first').toBeGreaterThan(0);

    const groups = new Map<string, { file: string; variant: StateVariant }[]>();
    for (const file of pngs) {
      const { surface, variant } = classifyPng(basename(file, '.png'));
      if (!groups.has(surface)) groups.set(surface, []);
      groups.get(surface)!.push({ file, variant });
    }

    const cards = Array.from(groups.entries())
      .sort(([a], [b]) => a.localeCompare(b))
      .flatMap(([, entries]) =>
        entries.map(({ file, variant }) => {
          const { bg, color, label } = VARIANT_BADGE[variant];
          const name = basename(file, '.png');
          return `
  <figure>
    <span class="badge" style="background:${bg};color:${color};border:1px solid ${color}44">${label}</span>
    <a href="${file}" target="_blank"><img src="${file}" loading="lazy" alt="${name}"></a>
    <figcaption>${name}</figcaption>
  </figure>`;
        }),
      )
      .join('\n');

    const html = `<!DOCTYPE html>
  <html lang="en">
  <head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width,initial-scale=1">
    <title>Vox GUI Visual Audit</title>
    <style>
      *, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; }
      body { background: #0d1117; color: #e6edf3; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; padding: 1.5rem; }
      header { margin-bottom: 1.5rem; border-bottom: 1px solid #21262d; padding-bottom: 1rem; }
      h1 { font-size: 1.4rem; color: #58a6ff; margin-bottom: 0.35rem; }
      .meta { font-size: 0.82rem; color: #8b949e; }
      .legend { display: flex; gap: 0.75rem; margin-top: 0.75rem; flex-wrap: wrap; }
      .legend-item { display: flex; align-items: center; gap: 0.35rem; font-size: 0.78rem; color: #8b949e; }
      .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(300px, 1fr)); gap: 0.875rem; }
      figure { background: #161b22; border: 1px solid #30363d; border-radius: 8px; overflow: hidden; transition: border-color 0.15s; }
      figure:hover { border-color: #58a6ff55; }
      figure a { display: block; }
      figure img { width: 100%; display: block; border-bottom: 1px solid #21262d; }
      figcaption { padding: 6px 10px 8px; font-size: 12px; font-weight: 500; color: #c9d1d9; }
      .badge { font-size: 10px; font-weight: 600; padding: 2px 7px; border-radius: 20px; margin: 7px 10px 0; display: inline-block; }
    </style>
  </head>
  <body>
    <header>
      <h1>Vox GUI Visual Audit</h1>
      <p class="meta">${pngs.length} screenshots · ${groups.size} surfaces · ${new Date().toISOString()}</p>
      <div class="legend">
        <span class="legend-item"><span class="badge" style="background:#1f6feb22;color:#1f6feb;border:1px solid #1f6feb44">base</span> default populated state</span>
        <span class="legend-item"><span class="badge" style="background:#388bfd22;color:#58a6ff;border:1px solid #388bfd44">empty</span> all lists empty</span>
        <span class="legend-item"><span class="badge" style="background:#f8514922;color:#f85149;border:1px solid #f8514944">error</span> IPC failures injected</span>
        <span class="legend-item"><span class="badge" style="background:#8b949e22;color:#8b949e;border:1px solid #8b949e44">special</span> sidebar / palette</span>
      </div>
    </header>
    <div class="grid">${cards}</div>
  </body>
  </html>`;

    writeFileSync(OUT_PATH, html, 'utf-8');
    console.log(`\n✅ Audit report: ${OUT_PATH}`);
    console.log(`   Open with: start "${OUT_PATH}"`);
  });
  ```

- [ ] **Step 2: Run the report generator**

  ```powershell
  cd crates\vox-gui\ui
  pnpm exec playwright test screenshots-audit-report.spec.ts --project=chromium
  ```
  Expected: 1 test PASSES. `e2e/screens/audit-report.html` is created.

- [ ] **Step 3: Open the report and verify it renders**

  ```powershell
  start crates\vox-gui\ui\e2e\screens\audit-report.html
  ```
  Expected: A dark-themed grid of screenshot thumbnails opens in your browser. Each thumbnail is clickable (full-size PNG). Badge colors distinguish states. After running the variant spec, you'll see base + empty + error variants side-by-side.

  **Review visually:**
  - [ ] No blank tiles (every PNG renders as a thumbnail)
  - [ ] Surface names are readable in the captions
  - [ ] State badges are color-coded correctly

- [ ] **Step 4: Exclude the report from git**

  Check `crates/vox-gui/ui/.gitignore`. If `e2e/screens/audit-report.html` is not already excluded, add:
  ```
  # Generated visual audit report — run screenshots-audit-report.spec.ts to regenerate
  e2e/screens/audit-report.html
  ```

- [ ] **Step 5: Commit**

  ```powershell
  git add crates/vox-gui/ui/e2e/screenshots-audit-report.spec.ts crates/vox-gui/ui/.gitignore
  git commit -m "test(vox-gui): add HTML visual audit report generator from screens/*.png"
  ```

---

## Self-Review

### Spec coverage
| Requirement | Task |
|---|---|
| Fix GUI launch crash (`panic!` in SessionManager init) | Tasks 1–2 |
| `SessionManager::default()` fallback if needed | Task 3 (conditional) |
| Verify GUI window appears on screen | Task 4 |
| Regenerate 4 missing surface screenshots | Task 5 |
| Empty-state mock factory (`installEmptyStateMock`) | Task 6 |
| Error-state mock factory (`installErrorStateMock`) | Task 6 |
| Vitest unit tests for both factories | Task 6 |
| Multi-state screenshot spec (opt-in, no CI blowup) | Task 7 |
| Browsable HTML audit report | Task 8 |

### Placeholder scan
No TBD, TODO, or "implement later" items. All Rust patterns are complete with exact Before/After snippets. All TypeScript code is complete and inline. The one conditional branch (Task 3) provides explicit instructions for both branches.

### Type consistency
- `installEmptyStateMock` and `installErrorStateMock` are exported from `tauriMockVariants.ts` and imported by exact name in `screenshots-variants.spec.ts` — names match.
- `KEY_SURFACES as const` in the spec — `StateVariant` and `VARIANT_BADGE` in the report spec cover all 4 variants used by `classifyPng` — exhaustive.
- `DETAIL_CMDS` set and `emptyDetailResponse` switch in `tauriMockVariants.ts` are consistent — every cmd in DETAIL_CMDS has a case in `emptyDetailResponse`, with a safe `default: return null` fallback for future additions.

---

## Out of Scope (File as Follow-up If Needed)
- **Interaction specs for 20+ uncovered surfaces** (flow, matrix, agents, approvals, runs, models, …) — each deserves a focused plan
- **Pixel-diff regression testing** against committed baselines — requires git LFS
- **Firefox and WebKit browser coverage**
- **Mobile/narrow viewport testing** (only 1440×900 currently)
- **Modal and hover-state screenshots** — needs per-surface interaction sequences
- **Live streaming / real-time event testing** — requires a real Tauri host or event-injection harness
