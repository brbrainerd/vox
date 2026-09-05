// @vitest-environment jsdom
import { describe, it, expect, vi, beforeAll, beforeEach, afterEach } from 'vitest';
import React from 'react';

// ── Tauri APIs ────────────────────────────────────────────────────────────────
const invokeMock = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

// ── msgpack ───────────────────────────────────────────────────────────────────
vi.mock('@msgpack/msgpack', () => ({
  decode: vi.fn().mockReturnValue({}),
}));

// ── xterm ─────────────────────────────────────────────────────────────────────
vi.mock('@xterm/xterm', () => ({
  Terminal: class {
    open() {}
    dispose() {}
    loadAddon() {}
    write() {}
  },
}));
vi.mock('@xterm/addon-fit', () => ({ FitAddon: class { fit() {}; activate() {} } }));
vi.mock('@xterm/addon-web-links', () => ({ WebLinksAddon: class { activate() {} } }));

// ── harness issue polling ────────────────────────────────────────────────────
const listHarnessIssuesMock = vi.hoisted(() => vi.fn());
// listHarnessIssuesForSession is also imported (by ChatTranscript, for the
// inline per-session summary strip) whenever the chat surface renders in
// this file's other tests — default it to an empty-issues resolution so
// those unrelated tests don't crash on an undefined mock export.
const listHarnessIssuesForSessionMock = vi.hoisted(() => vi.fn(() => Promise.resolve([])));
vi.mock('./components/surfaces/Scientia/harnessIssuesApi', () => ({
  listHarnessIssues: listHarnessIssuesMock,
  listHarnessIssuesForSession: listHarnessIssuesForSessionMock,
}));

// ── xyflow ────────────────────────────────────────────────────────────────────
vi.mock('@xyflow/react', () => ({
  ReactFlow: () => null,
  Background: () => null,
  Controls: () => null,
  MarkerType: {},
  Position: { Top: 'top', Bottom: 'bottom', Left: 'left', Right: 'right' },
  useNodesState: () => [[], vi.fn()],
  useEdgesState: () => [[], vi.fn()],
  useReactFlow: () => ({ fitView: vi.fn() }),
}));

import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import App from './App';
import { LanguageProvider } from './hooks/useLanguage';

// Polyfill APIs not in jsdom
beforeAll(() => {
  if (!('ResizeObserver' in globalThis)) {
    (globalThis as any).ResizeObserver = class {
      observe() {}
      unobserve() {}
      disconnect() {}
    };
  }
  // scrollIntoView is not implemented in jsdom
  if (!window.HTMLElement.prototype.scrollIntoView) {
    window.HTMLElement.prototype.scrollIntoView = vi.fn();
  }
});

beforeEach(() => {
  invokeMock.mockReset();
  invokeMock.mockResolvedValue(null);
  listHarnessIssuesMock.mockReset();
  listHarnessIssuesMock.mockResolvedValue([]);
});

afterEach(() => {
  window.location.hash = '';
});

describe('App shell', () => {
  const renderApp = () => {
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const wrapper = ({ children }: { children: React.ReactNode }) =>
      React.createElement(LanguageProvider, null,
        React.createElement(QueryClientProvider, { client: qc }, children));
    return render(<App />, { wrapper });
  };

  it('renders without throwing', () => {
    expect(() => renderApp()).not.toThrow();
  });

  // Task 5 (free-tier onboarding plan): a budget-exceeded dispatch error must
  // surface as a distinct "Budget limit reached" toast, not the generic
  // "Chat reply failed" toast used for every other backend-error class — see
  // `dispatchErrorToast` in App.tsx and `isBudgetExceededError` in
  // lib/backendGuard.ts. The error text matches `BudgetGuardError::Exceeded`'s
  // `Display` impl (`crates/vox-orchestrator-mcp/.../budget_guard.rs`).
  it('a budget-exceeded chat_turn error produces a distinct "Budget limit reached" toast', async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'chat_list_sessions') return Promise.resolve([]);
      if (cmd === 'get_memory_status') return Promise.resolve({ corpus_counts: {} });
      if (cmd === 'chat_create_session') return Promise.resolve({ session_id: 'gui-test-session' });
      if (cmd === 'chat_turn') {
        // Task C1: `chat_turn` returns `Result<ChatTurnDto, ChatTurnError>`
        // (crates/vox-gui/src/commands/chat_turn.rs) — a
        // `#[serde(tag = "kind", ...)]` enum — so Tauri v2 rejects invoke()
        // with the deserialized `{kind, message}` object, not a plain
        // string. `message` matches BudgetGuardError's Display text
        // propagated verbatim through enforce_budget_guard.
        return Promise.reject({ kind: 'budget_exceeded', message: 'Daily budget of $5.00 exceeded (spent $5.12)' });
      }
      return Promise.resolve(null);
    });
    window.location.hash = '#view=chat';
    renderApp();

    const composer = await screen.findByPlaceholderText(/describe a task/i);
    const user = userEvent.setup();
    await user.click(composer);
    await user.type(composer, 'hello there');
    await user.keyboard('{Enter}');

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('chat_turn', expect.anything()),
    );
    await waitFor(() => {
      expect(screen.getByText('Budget limit reached')).toBeInTheDocument();
    });
    expect(screen.queryByText('Chat reply failed')).toBeNull();
    // The budget message renders twice by design (failed chat bubble +
    // toast body) — assert presence, not uniqueness.
    expect(screen.getAllByText(/Daily budget of \$5\.00 exceeded/).length).toBeGreaterThan(0);
  });

  // Same distinct handling must apply to the other lifecycle a chat message
  // can take — the background execution (`/spawn`) — since the budget guard
  // is wired into both server-side, not just the synchronous one.
  it('a budget-exceeded background error produces a distinct "Budget limit reached" toast, not "Dispatch Failed"', async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'chat_list_sessions') return Promise.resolve([]);
      if (cmd === 'get_memory_status') return Promise.resolve({ corpus_counts: {} });
      if (cmd === 'chat_create_session') return Promise.resolve({ session_id: 'gui-test-session' });
      if (cmd === 'chat_turn') {
        return Promise.reject({ kind: 'budget_exceeded', message: 'Session budget of $2.00 exceeded (spent $2.01)' });
      }
      return Promise.resolve(null);
    });
    window.location.hash = '#view=chat';
    renderApp();

    const composer = await screen.findByPlaceholderText(/describe a task/i);
    const user = userEvent.setup();
    await user.click(composer);
    await user.type(composer, '/spawn');
    await user.keyboard('{Enter}');

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('chat_turn', expect.anything()),
    );
    await waitFor(() => {
      expect(screen.getByText('Budget limit reached')).toBeInTheDocument();
    });
    expect(screen.queryByText('Dispatch Failed')).toBeNull();
  });

  // Task 12b (free-tier onboarding plan): a rate-limited dispatch error (e.g.
  // OpenRouter's free-tier 50/day cap) must surface as a distinct "Free tier
  // limit reached" toast, not the generic "Chat reply failed" toast — see
  // `dispatchErrorToast` in App.tsx and `isRateLimitedError` in
  // lib/backendGuard.ts. The error text carries the `RATE_LIMITED_PREFIX`
  // marker prepended by `vox_actor_runtime::llm::chat::llm_chat`
  // (crates/vox-actor-runtime/src/llm/chat.rs), the funnel behind
  // `chat_turn`'s `try_run_agent_turn` tool-calling loop.
  it('a rate-limited chat_turn error produces a distinct "Free tier limit reached" toast', async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'chat_list_sessions') return Promise.resolve([]);
      if (cmd === 'get_memory_status') return Promise.resolve({ corpus_counts: {} });
      if (cmd === 'chat_create_session') return Promise.resolve({ session_id: 'gui-test-session' });
      if (cmd === 'chat_turn') {
        // Task C1: real shape is the deserialized ChatTurnError object, not a string.
        return Promise.reject({
          kind: 'rate_limited',
          message: 'RATE_LIMITED: OpenRouter rate limit exceeded, try again in 24h',
        });
      }
      return Promise.resolve(null);
    });
    window.location.hash = '#view=chat';
    renderApp();

    const composer = await screen.findByPlaceholderText(/describe a task/i);
    const user = userEvent.setup();
    await user.click(composer);
    await user.type(composer, 'hello there');
    await user.keyboard('{Enter}');

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('chat_turn', expect.anything()),
    );
    await waitFor(() => {
      expect(screen.getByText('Free tier limit reached')).toBeInTheDocument();
    });
    expect(screen.queryByText('Chat reply failed')).toBeNull();
    // The stripped, human-readable message renders twice by design (failed
    // chat bubble [raw, unstripped] + toast body [stripped]) — assert
    // presence, not uniqueness, mirroring the budget-exceeded test above.
    expect(screen.getAllByText(/OpenRouter rate limit exceeded/).length).toBeGreaterThan(0);
  });

  // Same distinct handling must apply to the other real dispatch mechanism a
  // chat message can take — the background execution (`/spawn`) — since
  // `mcp_infer_tool_completion` (crates/vox-orchestrator-mcp/src/llm_bridge/infer.rs)
  // is the separate HTTP-issuing funnel that path goes through, and it now
  // prepends the same `RATE_LIMITED_PREFIX` marker on its own terminal failure.
  it('a rate-limited background error produces a distinct "Free tier limit reached" toast, not "Dispatch Failed"', async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'chat_list_sessions') return Promise.resolve([]);
      if (cmd === 'get_memory_status') return Promise.resolve({ corpus_counts: {} });
      if (cmd === 'chat_create_session') return Promise.resolve({ session_id: 'gui-test-session' });
      if (cmd === 'chat_turn') {
        // Task C1: real shape is the deserialized ChatTurnError object, not a string.
        return Promise.reject({
          kind: 'rate_limited',
          message: 'RATE_LIMITED: OpenRouter rate limit exceeded, try again in 24h',
        });
      }
      return Promise.resolve(null);
    });
    window.location.hash = '#view=chat';
    renderApp();

    const composer = await screen.findByPlaceholderText(/describe a task/i);
    const user = userEvent.setup();
    await user.click(composer);
    await user.type(composer, '/spawn');
    await user.keyboard('{Enter}');

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('chat_turn', expect.anything()),
    );
    await waitFor(() => {
      expect(screen.getByText('Free tier limit reached')).toBeInTheDocument();
    });
    expect(screen.queryByText('Dispatch Failed')).toBeNull();
  });

  // Bug 4 (chat-harness review): the background dispatch branch mints its
  // pending bubble via `onRun` BEFORE the `chat_turn` invoke resolves. Its
  // catch block previously only called `pushToast` -- it never dispatched
  // `failRun` for that runId, so a dispatch failure left the bubble stuck
  // 'pending' with no terminal state until the multi-minute `pendingTimeout`
  // watchdog eventually flipped it to a GENERIC message (`PENDING_TIMEOUT_MESSAGE`
  // in lib/chatCorrelation.ts), discarding the real error. Proof here: the
  // "persist assistant transcript rows" effect only persists 'done'/'failed'
  // assistant messages (`assistantMessagesReadyToPersist`) -- so an immediate
  // `chat_append_message` carrying the REAL dispatch error means `failRun`
  // fired synchronously in the catch, not the watchdog minutes later.
  it('a background dispatch failure settles the pending bubble as failed (not stuck pending) with the real error', async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'chat_list_sessions') return Promise.resolve([]);
      if (cmd === 'get_memory_status') return Promise.resolve({ corpus_counts: {} });
      if (cmd === 'chat_create_session') return Promise.resolve({ session_id: 'gui-test-session' });
      if (cmd === 'chat_append_message') return Promise.resolve(1);
      if (cmd === 'chat_turn') {
        return Promise.reject('daemon unreachable: connection refused');
      }
      return Promise.resolve(null);
    });
    window.location.hash = '#view=chat';
    renderApp();

    const composer = await screen.findByPlaceholderText(/describe a task/i);
    const user = userEvent.setup();
    await user.click(composer);
    await user.type(composer, '/spawn');
    await user.keyboard('{Enter}');

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('chat_turn', expect.anything()),
    );
    await waitFor(() => {
      const persistCall = invokeMock.mock.calls.find(
        ([cmd, args]: [string, any]) =>
          cmd === 'chat_append_message' && args?.input?.role === 'assistant',
      );
      expect(persistCall).toBeDefined();
      expect(persistCall![1].input.content).toContain('daemon unreachable: connection refused');
    });
  });

  // Non-blocking budget-warn toast: distinct from "Budget limit reached"
  // (the hard-block toast above) — fires once spend crosses
  // `budget_warn_threshold_pct` but before the hard cap, after a SUCCESSFUL
  // chat_turn dispatch. See `checkBudgetWarn` in App.tsx.
  it('a successful chat_turn past the warn threshold produces an "Approaching budget limit" toast', async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'chat_list_sessions') return Promise.resolve([]);
      if (cmd === 'get_memory_status') return Promise.resolve({ corpus_counts: {} });
      if (cmd === 'chat_create_session') return Promise.resolve({ session_id: 'gui-test-session' });
      if (cmd === 'get_user_config') {
        return Promise.resolve([
          { key: 'budget_warn_threshold_pct', label: '', hint: '', group: '', kind: 'float', options: [], default: '0.8', currentValue: '0.8' },
        ]);
      }
      if (cmd === 'get_llm_spend') {
        return Promise.resolve({
          sessionUsd: 0, dayUsd: 4.25, totalUsd: 4.25, dailyBudgetUsd: 5.0, perSessionBudgetUsd: 2.0,
        });
      }
      if (cmd === 'chat_turn') {
        return Promise.resolve({ id: 'reply-1', text: 'hi there', modelId: null, latencyMs: 10, selectionReason: null });
      }
      return Promise.resolve(null);
    });
    window.location.hash = '#view=chat';
    renderApp();

    const composer = await screen.findByPlaceholderText(/describe a task/i);
    const user = userEvent.setup();
    await user.click(composer);
    await user.type(composer, 'hello there');
    await user.keyboard('{Enter}');

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('chat_turn', expect.anything()),
    );
    await waitFor(() => {
      expect(screen.getByText('Approaching budget limit')).toBeInTheDocument();
    });
    expect(screen.getByText(/85% of your daily budget/)).toBeInTheDocument();
  });

  it('a successful chat_turn below the warn threshold produces no budget-warn toast', async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'chat_list_sessions') return Promise.resolve([]);
      if (cmd === 'get_memory_status') return Promise.resolve({ corpus_counts: {} });
      if (cmd === 'chat_create_session') return Promise.resolve({ session_id: 'gui-test-session' });
      if (cmd === 'get_user_config') {
        return Promise.resolve([
          { key: 'budget_warn_threshold_pct', label: '', hint: '', group: '', kind: 'float', options: [], default: '0.8', currentValue: '0.8' },
        ]);
      }
      if (cmd === 'get_llm_spend') {
        return Promise.resolve({
          sessionUsd: 0, dayUsd: 1.0, totalUsd: 1.0, dailyBudgetUsd: 5.0, perSessionBudgetUsd: 2.0,
        });
      }
      if (cmd === 'chat_turn') {
        return Promise.resolve({ id: 'reply-1', text: 'hi there', modelId: null, latencyMs: 10, selectionReason: null });
      }
      return Promise.resolve(null);
    });
    window.location.hash = '#view=chat';
    renderApp();

    const composer = await screen.findByPlaceholderText(/describe a task/i);
    const user = userEvent.setup();
    await user.click(composer);
    await user.type(composer, 'hello there');
    await user.keyboard('{Enter}');

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('chat_turn', expect.anything()),
    );
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('get_llm_spend', expect.anything()),
    );
    expect(screen.queryByText('Approaching budget limit')).toBeNull();
  });

  it('the budget-warn toast fires only once even after a second message past the threshold', async () => {
    let replySeq = 0;
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'chat_list_sessions') return Promise.resolve([]);
      if (cmd === 'get_memory_status') return Promise.resolve({ corpus_counts: {} });
      if (cmd === 'chat_create_session') return Promise.resolve({ session_id: 'gui-test-session' });
      if (cmd === 'get_user_config') {
        return Promise.resolve([
          { key: 'budget_warn_threshold_pct', label: '', hint: '', group: '', kind: 'float', options: [], default: '0.8', currentValue: '0.8' },
        ]);
      }
      if (cmd === 'get_llm_spend') {
        return Promise.resolve({
          sessionUsd: 0, dayUsd: 4.25, totalUsd: 4.25, dailyBudgetUsd: 5.0, perSessionBudgetUsd: 2.0,
        });
      }
      if (cmd === 'chat_turn') {
        replySeq += 1;
        return Promise.resolve({ id: `reply-${replySeq}`, text: 'hi there', modelId: null, latencyMs: 10, selectionReason: null });
      }
      return Promise.resolve(null);
    });
    window.location.hash = '#view=chat';
    renderApp();

    const composer = await screen.findByPlaceholderText(/describe a task/i);
    const user = userEvent.setup();
    await user.click(composer);
    await user.type(composer, 'hello there');
    await user.keyboard('{Enter}');

    await waitFor(() => {
      expect(screen.getByText('Approaching budget limit')).toBeInTheDocument();
    });

    await user.click(composer);
    await user.type(composer, 'second message');
    await user.keyboard('{Enter}');

    await waitFor(() =>
      expect(invokeMock.mock.calls.filter((c) => c[0] === 'chat_turn').length).toBeGreaterThanOrEqual(2),
    );
    expect(screen.getAllByText('Approaching budget limit').length).toBe(1);
  });

  // Regression: ⌘. used to be displayed in Settings but wired to no handler.
  // It must now be claimed by the global keydown handler (preventDefault fires).
  it('handles ⌘. (Cmd+Period) globally', () => {
    renderApp();
    const ev = new KeyboardEvent('keydown', { key: '.', metaKey: true, cancelable: true, bubbles: true });
    window.dispatchEvent(ev);
    expect(ev.defaultPrevented).toBe(true);
  });

  // F-02: a null `chat_create_session` result used to throw inside the .then
  // handler (s.session_id on null), get caught, and surface a leaky
  // "Chat session" warn toast on every empty-history mount. The guard should
  // skip silently instead.
  it('null chat_create_session result produces no leaky "Chat session" toast (F-02)', async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'chat_list_sessions') return Promise.resolve([]);
      // get_memory_status has its own unrelated null-deref bug (useMemoryStatus.ts);
      // stub a shape it can consume so it doesn't produce an unhandled rejection
      // that would mask this test's actual signal.
      if (cmd === 'get_memory_status') return Promise.resolve({ corpus_counts: {} });
      return Promise.resolve(null); // chat_create_session -> null
    });
    renderApp();
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith('chat_create_session', expect.anything()));
    await waitFor(() => {
      expect(screen.queryByText('Chat session')).toBeNull();
    });
  });

  // F-02: the /rollback slash command reads `res.is_error` / `res.result` off
  // the invoke_mcp_tool response without a null guard. A null resolution used
  // to throw a caught TypeError whose text (mentioning `is_error`) leaked into
  // the failure toast body instead of an honest "no response" message.
  it('null MCP rollback result produces an honest failure toast, not a TypeError leak (F-02)', async () => {
    invokeMock.mockImplementation((cmd: string) => {
      // chat_list_sessions must resolve to an array or ChatSessionRail crashes on
      // an unrelated null-deref (out of scope here) and masks this test's signal.
      if (cmd === 'chat_list_sessions') return Promise.resolve([]);
      if (cmd === 'get_memory_status') return Promise.resolve({ corpus_counts: {} });
      return Promise.resolve(null); // every other backend call resolves null
    });
    window.location.hash = '#view=chat';
    renderApp();

    const composer = await screen.findByPlaceholderText(/describe a task/i);
    const user = userEvent.setup();
    await user.click(composer);
    await user.type(composer, '/rollback');
    await user.keyboard('{Enter}');

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith('invoke_mcp_tool', { tool: 'vox_undo', args: {} }));
    let toastRegion!: HTMLElement;
    await waitFor(() => {
      const title = screen.getByText('Rollback failed');
      toastRegion = title.closest('[role="status"]') as HTMLElement ?? title.parentElement!.parentElement!;
    });
    expect(toastRegion.textContent).not.toMatch(/is_error|session_id|TypeError|undefined/i);
  });

  // F-02 (audit, primary path): a null `execute_command` result used to be
  // routed into the catch block via `throw`, silently firing a SECOND RPC
  // (invoke_mcp_tool/vox_check) instead of just reporting that the primary
  // call returned nothing. The guard must produce an honest toast directly
  // and must NOT fire the fallback RPC.
  it('null execute_command result produces an honest audit toast without triggering the MCP fallback (F-02)', async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'chat_list_sessions') return Promise.resolve([]);
      if (cmd === 'get_memory_status') return Promise.resolve({ corpus_counts: {} });
      if (cmd === 'execute_command') return Promise.resolve(null);
      return Promise.resolve(null);
    });
    window.location.hash = '#view=chat';
    renderApp();

    const composer = await screen.findByPlaceholderText(/describe a task/i);
    const user = userEvent.setup();
    await user.click(composer);
    await user.type(composer, '/audit');
    await user.keyboard('{Enter}');

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith('execute_command', expect.anything()));
    await waitFor(() => {
      expect(screen.getByText('Audit unavailable')).toBeInTheDocument();
      expect(screen.getByText('No response from the backend.')).toBeInTheDocument();
    });
    expect(invokeMock).not.toHaveBeenCalledWith('invoke_mcp_tool', { tool: 'vox_check', args: {} });
  });

  // F-02 (audit, MCP-fallback path): when execute_command throws and the
  // invoke_mcp_tool fallback's result contains leak-pattern text, the raw
  // res.result used to render directly into the toast body with no
  // sanitization (unlike the rollback handler's equivalent branch).
  it('a leak-pattern MCP fallback result is sanitized in the audit failure toast (F-02)', async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'chat_list_sessions') return Promise.resolve([]);
      if (cmd === 'get_memory_status') return Promise.resolve({ corpus_counts: {} });
      if (cmd === 'execute_command') return Promise.reject(new Error('execute_command unavailable'));
      if (cmd === 'invoke_mcp_tool') {
        return Promise.resolve({
          tool: 'vox_check',
          is_error: true,
          result: 'failed to invoke __TAURI_INTERNALS__ command',
        });
      }
      return Promise.resolve(null);
    });
    window.location.hash = '#view=chat';
    renderApp();

    const composer = await screen.findByPlaceholderText(/describe a task/i);
    const user = userEvent.setup();
    await user.click(composer);
    await user.type(composer, '/audit');
    await user.keyboard('{Enter}');

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith('invoke_mcp_tool', { tool: 'vox_check', args: {} }));
    let toastRegion!: HTMLElement;
    await waitFor(() => {
      const title = screen.getByText('Audit failed');
      toastRegion = title.closest('[role="status"]') as HTMLElement ?? title.parentElement!.parentElement!;
    });
    expect(toastRegion.textContent).not.toMatch(/__TAURI_INTERNALS__|\binvoke\b/);
  });

  // Fix Task 2 (chat-harness audit): /spawn dispatches via chat_turn with execution: 'background'
  // with no explicit session_id, which used to default to activeSessionId — making
  // it look like part of the ongoing chat session in the GUI transcript while the
  // orchestrator's own chat_history:{session_id} context store (only written to by
  // vox_chat_message) never learns this happened. /spawn must use its own,
  // clearly-separate session id instead of borrowing the active chat session's.
  it('/spawn does not reuse the active chat session id', async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'chat_list_sessions') return Promise.resolve([]);
      if (cmd === 'get_memory_status') return Promise.resolve({ corpus_counts: {} });
      if (cmd === 'chat_create_session') return Promise.resolve({ session_id: 'chat-session-abc' });
      if (cmd === 'chat_turn') return Promise.resolve({ task_id: '1', duplicate_of: null });
      return Promise.resolve(null);
    });
    window.location.hash = '#view=chat';
    renderApp();

    const composer = await screen.findByPlaceholderText(/describe a task/i);
    const user = userEvent.setup();
    await user.click(composer);
    await user.type(composer, '/spawn');
    await user.keyboard('{Enter}');

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('chat_turn', expect.anything()),
    );
    const submitCall = invokeMock.mock.calls.find(([cmd]) => cmd === 'chat_turn');
    expect(submitCall![1].input.session_id).not.toBe('chat-session-abc');
  });

  // Phase D Task D3: before this fix, `/spawn <anything>` always sent the same
  // hardcoded generic description, discarding whatever the user actually
  // typed — a delegated agent (and anyone reading its task later) had no way
  // to recover the real ask. Bare `/spawn` (no trailing text) still falls
  // back to the generic description, covered by the test above.
  it('/spawn carries the user\'s actual typed text as the task description', async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'chat_list_sessions') return Promise.resolve([]);
      if (cmd === 'get_memory_status') return Promise.resolve({ corpus_counts: {} });
      if (cmd === 'chat_create_session') return Promise.resolve({ session_id: 'chat-session-abc' });
      if (cmd === 'chat_turn') return Promise.resolve({ task_id: '1', duplicate_of: null });
      return Promise.resolve(null);
    });
    window.location.hash = '#view=chat';
    renderApp();

    const composer = await screen.findByPlaceholderText(/describe a task/i);
    const user = userEvent.setup();
    await user.click(composer);
    await user.type(composer, '/spawn fix the login bug');
    await user.keyboard('{Enter}');

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('chat_turn', expect.anything()),
    );
    const submitCall = invokeMock.mock.calls.find(([cmd]) => cmd === 'chat_turn');
    expect(submitCall![1].input.content).toBe('fix the login bug');
  });

  // Fix Task 2 (chat-harness audit): same bug, second call site — deploying an
  // installed skill from the Omnibar dispatches via chat_turn too,
  // and must not silently borrow the active chat session's identity either.
  it('Deploy skill (Omnibar) does not reuse the active chat session id', async () => {
    invokeMock.mockImplementation((cmd: string, args: any) => {
      if (cmd === 'chat_list_sessions') return Promise.resolve([]);
      if (cmd === 'get_memory_status') return Promise.resolve({ corpus_counts: {} });
      if (cmd === 'chat_create_session') return Promise.resolve({ session_id: 'chat-session-abc' });
      if (cmd === 'invoke_mcp_tool' && args?.tool === 'vox_skill_list') {
        return Promise.resolve({
          result: [{ id: 'skill-1', name: 'my-test-skill', description: 'A test skill' }],
        });
      }
      if (cmd === 'chat_turn') return Promise.resolve({ task_id: '1', duplicate_of: null });
      return Promise.resolve(null);
    });
    window.location.hash = '#view=chat';
    renderApp();

    // Open the Omnibar (Mod+K) and search for the installed skill.
    const ev = new KeyboardEvent('keydown', { key: 'k', metaKey: true, cancelable: true, bubbles: true });
    window.dispatchEvent(ev);
    const searchInput = await screen.findByPlaceholderText(/search/i);
    const user = userEvent.setup();
    // '/' is the skills+docs prefix mode (see Omnibar.tsx's federatedKindsForMode:
    // the default/no-prefix mode does NOT include the 'skill' kind).
    await user.type(searchInput, '/my-test-skill');
    // The row's label text may be split across highlight-match spans, so match
    // on any element whose combined text content includes the skill name, and
    // click the smallest (most specific) such element directly — clicking the
    // outermost match (e.g. the modal backdrop) would instead close the Omnibar.
    let skillRow: HTMLElement | undefined;
    await waitFor(() => {
      const matches = screen.getAllByText((_, el) => !!el?.textContent?.includes('my-test-skill'));
      expect(matches.length).toBeGreaterThan(0);
      skillRow = matches.reduce((smallest, el) =>
        el.textContent!.length < smallest.textContent!.length ? el : smallest,
      );
    });
    await user.click(skillRow!);

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('chat_turn', expect.anything()),
    );
    const submitCall = invokeMock.mock.calls.find(([cmd]) => cmd === 'chat_turn');
    expect(submitCall![1].input.session_id).not.toBe('chat-session-abc');
  });

  // Task A3: the old command-name fork is gone. Both send modes now reach the
  // SAME `chat_turn` command; the distinction is the payload's `execution`
  // field plus which store lifecycle App runs. A plain chat send is
  // `execution: 'sync'` — a terminal request/response whose reply text
  // settles the pending bubble (chatPending -> chatReplySettled).
  it("a plain chat send dispatches chat_turn with execution: 'sync' and settles the reply bubble", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'chat_list_sessions') return Promise.resolve([]);
      if (cmd === 'get_memory_status') return Promise.resolve({ corpus_counts: {} });
      if (cmd === 'chat_create_session') return Promise.resolve({ session_id: 'gui-test-session' });
      if (cmd === 'chat_turn') {
        return Promise.resolve({
          id: 42,
          role: 'assistant',
          content: 'hello back',
          created_at: '2026-07-31T00:00:00Z',
          task_id: null,
          model_id: 'openrouter/auto',
        });
      }
      return Promise.resolve(null);
    });
    window.location.hash = '#view=chat';
    renderApp();

    const composer = await screen.findByPlaceholderText(/describe a task/i);
    const user = userEvent.setup();
    await user.click(composer);
    await user.type(composer, 'hello there');
    await user.keyboard('{Enter}');

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('chat_turn', expect.anything()),
    );
    const turnCall = invokeMock.mock.calls.find(([cmd]) => cmd === 'chat_turn');
    expect(turnCall![1].input.execution).toBe('sync');
    // The deleted fork: no second dispatch command exists any more.
    expect(invokeMock).not.toHaveBeenCalledWith('submit_orchestrator_task', expect.anything());
    expect(invokeMock).not.toHaveBeenCalledWith('chat_send_message', expect.anything());
    // Sync lifecycle: the reply text lands in the transcript.
    await waitFor(() => {
      expect(screen.getByText('hello back')).toBeInTheDocument();
    });
  });

  // GroundingCheckToggle fix: the composer's opt-in grounding-check toggle
  // (default off) must actually reach `chat_turn`'s
  // `grounding_check_enabled` arg for a plain chat send — before this fix the
  // toggle's state was never threaded into the synchronous send path at all.
  it('enabling the grounding check toggle forwards grounding_check_enabled=true to chat_turn', async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'chat_list_sessions') return Promise.resolve([]);
      if (cmd === 'get_memory_status') return Promise.resolve({ corpus_counts: {} });
      if (cmd === 'chat_create_session') return Promise.resolve({ session_id: 'gui-test-session' });
      if (cmd === 'chat_turn') {
        return Promise.resolve({
          id: 43,
          role: 'assistant',
          content: 'grounded reply',
          created_at: '2026-08-02T00:00:00Z',
          task_id: null,
          model_id: 'openrouter/auto',
        });
      }
      return Promise.resolve(null);
    });
    window.location.hash = '#view=chat';
    renderApp();

    const composer = await screen.findByPlaceholderText(/describe a task/i);
    const user = userEvent.setup();

    // Accessible name comes from the button's `aria-label` ("Grounding check
    // on/off"), not its visible text ("grounding: on/off") — RTL's `name`
    // matcher matches the accessible name, so the pattern must include "check".
    const groundingToggle = await screen.findByRole('button', { name: /grounding check (on|off)/i });
    expect(groundingToggle).toHaveAttribute('aria-pressed', 'false');
    await user.click(groundingToggle);
    expect(groundingToggle).toHaveAttribute('aria-pressed', 'true');

    await user.click(composer);
    await user.type(composer, 'check this please');
    await user.keyboard('{Enter}');

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('chat_turn', expect.anything()),
    );
    const sendCall = invokeMock.mock.calls.find(([cmd]) => cmd === 'chat_turn');
    expect(sendCall![1].input.grounding_check_enabled).toBe(true);
  });

  // Task A3 counterpart of the test above: the composer's "Background task"
  // toggle position reaches the SAME `chat_turn` command, differing only in
  // `execution: 'background'`. Its store lifecycle is submit -> submitResolved
  // (the sole writer of taskToSession), NOT chatPending/chatReplySettled — the
  // background response carries a task_id and no answer text, so settling it
  // 'done' would strand an empty bubble the pending watchdog cannot rescue.
  it("background-task mode dispatches the same chat_turn command with execution: 'background'", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'chat_list_sessions') return Promise.resolve([]);
      if (cmd === 'get_memory_status') return Promise.resolve({ corpus_counts: {} });
      if (cmd === 'chat_create_session') return Promise.resolve({ session_id: 'gui-test-session' });
      if (cmd === 'chat_append_message') return Promise.resolve(1);
      if (cmd === 'chat_turn') {
        // What run_background actually returns: a task id, no assistant row.
        return Promise.resolve({ id: 0, role: 'assistant', content: '', created_at: '', task_id: '1', duplicate_of: null });
      }
      return Promise.resolve(null);
    });
    window.location.hash = '#view=chat';
    renderApp();

    const composer = await screen.findByPlaceholderText(/describe a task/i);
    const user = userEvent.setup();

    const modeButton = await screen.findByRole('button', { name: /choose send mode/i });
    await user.click(modeButton);
    const backgroundOption = await screen.findByRole('button', { name: /set send mode: background task/i });
    await user.click(backgroundOption);

    await user.click(composer);
    await user.type(composer, 'do this in the background');
    await user.keyboard('{Enter}');

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('chat_turn', expect.anything()),
    );
    const turnCall = invokeMock.mock.calls.find(([cmd]) => cmd === 'chat_turn');
    expect(turnCall![1].input.execution).toBe('background');
    expect(invokeMock).not.toHaveBeenCalledWith('submit_orchestrator_task', expect.anything());
    expect(invokeMock).not.toHaveBeenCalledWith('chat_send_message', expect.anything());
    // Background lifecycle proof: the sync branch is the only one that marks
    // the session in-flight, so a second Enter here must dispatch again rather
    // than being refused with the "Please wait" toast.
    await user.type(composer, 'and another one');
    await user.keyboard('{Enter}');
    await waitFor(() =>
      expect(invokeMock.mock.calls.filter(([cmd]) => cmd === 'chat_turn').length).toBe(2),
    );
    expect(screen.queryByText('Please wait')).toBeNull();
  });

  // Code-review fix: the composer's "Background task" toggle went through a
  // DIFFERENT onSubmit wrapper than /spawn and Deploy-skill above, and that
  // wrapper wasn't updated when those two were fixed to stop reusing
  // activeSessionId -- reintroducing the exact bug this same branch's Fix
  // Task 2 patched, just via a third entry point.
  it('background-task mode does not reuse the active chat session id', async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'chat_list_sessions') return Promise.resolve([]);
      if (cmd === 'get_memory_status') return Promise.resolve({ corpus_counts: {} });
      if (cmd === 'chat_create_session') return Promise.resolve({ session_id: 'chat-session-abc' });
      if (cmd === 'chat_append_message') return Promise.resolve(1);
      if (cmd === 'chat_turn') {
        return Promise.resolve({ task_id: '1', duplicate_of: null });
      }
      return Promise.resolve(null);
    });
    window.location.hash = '#view=chat';
    renderApp();

    const composer = await screen.findByPlaceholderText(/describe a task/i);
    const user = userEvent.setup();

    const modeButton = await screen.findByRole('button', { name: /choose send mode/i });
    await user.click(modeButton);
    const backgroundOption = await screen.findByRole('button', { name: /set send mode: background task/i });
    await user.click(backgroundOption);

    await user.click(composer);
    await user.type(composer, 'do this in the background too');
    await user.keyboard('{Enter}');

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('chat_turn', expect.anything()),
    );
    const submitCall = invokeMock.mock.calls.find(([cmd]) => cmd === 'chat_turn');
    expect(submitCall![1].input.session_id).not.toBe('chat-session-abc');
  });

  // Task A3: the toggle default must still be the sync lifecycle. `execution`
  // is emitted explicitly by Loquela for BOTH positions — it is never derived
  // from the absence of the retired `task_category: 'chat'` sentinel.
  it("quick-chat mode (the toggle default) sends execution: 'sync'", async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'chat_list_sessions') return Promise.resolve([]);
      if (cmd === 'get_memory_status') return Promise.resolve({ corpus_counts: {} });
      if (cmd === 'chat_create_session') return Promise.resolve({ session_id: 'gui-test-session' });
      if (cmd === 'chat_turn') {
        return Promise.resolve({
          id: 43,
          role: 'assistant',
          content: 'default mode reply',
          created_at: '2026-08-01T00:00:00Z',
          task_id: null,
          model_id: 'openrouter/auto',
        });
      }
      return Promise.resolve(null);
    });
    window.location.hash = '#view=chat';
    renderApp();

    const composer = await screen.findByPlaceholderText(/describe a task/i);
    const user = userEvent.setup();
    // Do NOT touch the mode toggle -- default should remain 'chat'.
    await user.click(composer);
    await user.type(composer, 'hello again');
    await user.keyboard('{Enter}');

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('chat_turn', expect.anything()),
    );
    const turnCall = invokeMock.mock.calls.find(([cmd]) => cmd === 'chat_turn');
    expect(turnCall![1].input.execution).toBe('sync');
    expect(invokeMock).not.toHaveBeenCalledWith('submit_orchestrator_task', expect.anything());
    expect(invokeMock).not.toHaveBeenCalledWith('chat_send_message', expect.anything());
    await waitFor(() => {
      expect(screen.getByText('default mode reply')).toBeInTheDocument();
    });
  });

  // Code-review follow-up (8448c477a1): chat_turn already persists
  // the assistant reply server-side. The pre-existing "persist assistant
  // transcript rows" effect sweeps chatStore.sessions for any 'done'/'failed'
  // assistant message not yet in persistedAssistantIdsRef and calls
  // chat_append_message for it — with no awareness of the synchronous chat
  // path, it would re-persist the same reply a second time. The fix
  // pre-seeds persistedAssistantIdsRef with the settled message's id before
  // dispatching chatReplySettled, so the sweep effect treats it as
  // already-persisted.
  it('a plain chat reply is not re-persisted by the assistant-transcript sweep effect', async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'chat_list_sessions') return Promise.resolve([]);
      if (cmd === 'get_memory_status') return Promise.resolve({ corpus_counts: {} });
      if (cmd === 'chat_create_session') return Promise.resolve({ session_id: 'gui-test-session' });
      if (cmd === 'chat_turn') {
        return Promise.resolve({
          id: 99,
          role: 'assistant',
          content: 'no duplicates please',
          created_at: '2026-07-31T00:00:00Z',
          task_id: null,
          model_id: 'openrouter/auto',
        });
      }
      return Promise.resolve(null);
    });
    window.location.hash = '#view=chat';
    renderApp();

    const composer = await screen.findByPlaceholderText(/describe a task/i);
    const user = userEvent.setup();
    await user.click(composer);
    await user.type(composer, 'dedupe check');
    await user.keyboard('{Enter}');

    await waitFor(() => {
      expect(screen.getByText('no duplicates please')).toBeInTheDocument();
    });
    // Give the persistence effect (which runs on the next chatStore-driven
    // render) a chance to fire before asserting it did NOT double-persist.
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(
      invokeMock.mock.calls.filter(
        ([cmd, args]: [string, any]) =>
          cmd === 'chat_append_message' && args?.input?.content === 'no duplicates please',
      ),
    ).toHaveLength(0);
  });

  // Fix Task 1 (gui-chat-harness-fixes): nothing prevented a user from
  // pressing Enter a second time while a prior chat_turn() call was
  // still pending — two independent tempIds each got their own
  // chatPending/chatReplySettled lifecycle, settling out of order with no
  // user-visible indication. A send-lock keyed by sessionId must block the
  // second send while the first is still in flight.
  it('does not send a second chat message while the first is still pending', async () => {
    let resolveFirst!: (value: unknown) => void;
    const pending = new Promise((resolve) => { resolveFirst = resolve; });
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'chat_list_sessions') return Promise.resolve([]);
      if (cmd === 'get_memory_status') return Promise.resolve({ corpus_counts: {} });
      if (cmd === 'chat_create_session') return Promise.resolve({ session_id: 'gui-test-session' });
      if (cmd === 'chat_append_message') return Promise.resolve(1);
      if (cmd === 'chat_turn') return pending;
      return Promise.resolve(null);
    });
    window.location.hash = '#view=chat';
    renderApp();

    const composer = await screen.findByPlaceholderText(/describe a task/i);
    const user = userEvent.setup();

    await user.click(composer);
    await user.type(composer, 'first message');
    await user.keyboard('{Enter}');

    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('chat_turn', expect.anything()),
    );

    await user.click(composer);
    await user.type(composer, 'second message');
    await user.keyboard('{Enter}');

    const chatSendCalls = invokeMock.mock.calls.filter(([cmd]: [string]) => cmd === 'chat_turn');
    expect(chatSendCalls.length).toBe(1);
    // Code-review fix: the send-lock guard must be checked BEFORE
    // chat_append_message persists anything -- otherwise the rejected second
    // send still writes an orphaned user-message row with no reply ever
    // generated for it (only caught by inspecting chat_append_message's own
    // call args, not just chat_turn's count).
    expect(
      invokeMock.mock.calls.filter(
        ([cmd, args]: [string, any]) =>
          cmd === 'chat_append_message' && args?.input?.content === 'second message',
      ),
    ).toHaveLength(0);

    resolveFirst({ id: 1, role: 'assistant', content: 'reply', created_at: '2026-07-31T00:00:00Z', task_id: null, model_id: 'openrouter/auto' });
  });

  // Regression (Task 5 nav-shell redesign, ee7903cf4b): openParentNav used to
  // alias directly to useActiveView().navigateToParent, which only calls the
  // hook's internal setActiveView — it never calls syncViewToLocation, so the
  // URL hash went stale on every sidebar parent-group click (e.g. "Agents").
  // A reload right after would silently revert the navigation. openParentNav
  // must route through the local navigateTo wrapper so the hash stays in sync.
  it('clicking a sidebar parent group syncs the URL hash (Task 5 regression)', async () => {
    window.location.hash = '#view=chat';
    renderApp();

    const user = userEvent.setup();
    let agentsNav: HTMLElement | undefined;
    await waitFor(() => {
      agentsNav = screen.getAllByRole('button').find(b => (b.textContent ?? '').startsWith('Agents'));
      expect(agentsNav).toBeTruthy();
    });
    await user.click(agentsNav!);

    await waitFor(() => expect(window.location.hash).toBe('#view=dashboard'));
  });

  // Second Task 5 regression, found by review of the fix above: openParentNav
  // was changed to `navigateTo(parentKey)`, which passes the bare key straight
  // into resolveNavigation. PARENT_CHILD_MAP has a self-referential entry for
  // 'runs' (`runs: { parent: 'runs', child: 'runs' }`) that resolveNavigation
  // matches BEFORE falling back to DEFAULT_CHILD_BY_PARENT, so clicking the
  // "Runs" sidebar parent group opened the "Runs" child tab instead of the
  // intended default "Approvals" (human's review queue first). openParentNav
  // must pre-resolve via DEFAULT_CHILD_BY_PARENT before calling navigateTo.
  it('clicking the "Runs" sidebar parent group (labelled "Review") opens Approvals, not Runs (Task 5 regression)', async () => {
    window.location.hash = '#view=chat';
    renderApp();

    const user = userEvent.setup();
    let runsNav: HTMLElement | undefined;
    await waitFor(() => {
      runsNav = screen.getAllByRole('button').find(b => (b.textContent ?? '').startsWith('Review'));
      expect(runsNav).toBeTruthy();
    });
    await user.click(runsNav!);

    await waitFor(() => expect(window.location.hash).toBe('#view=approvals'));
  });

  // Harness-issue polling (App.tsx's 8s-interval effect): the first poll
  // establishes the pending baseline without toasting (so restarting the app
  // doesn't re-toast an existing backlog); only issues that appear on a LATER
  // poll are genuinely new and get toasted.
  describe('harness issue polling', () => {
    beforeEach(() => {
      vi.useFakeTimers({ shouldAdvanceTime: true });
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    it('suppresses toasts for the first-poll backlog, then toasts only genuinely-new issues on a later poll', async () => {
      invokeMock.mockImplementation((cmd: string) => {
        if (cmd === 'chat_list_sessions') return Promise.resolve([]);
        if (cmd === 'get_memory_status') return Promise.resolve({ corpus_counts: {} });
        return Promise.resolve(null);
      });

      const issue1 = { id: 1, source: 'chat_session', session_key: 's1', target_path: null, detected_at_ms: 0, category: 'x', severity: 'low', summary: 'x', evidence_json: '{}', status: 'pending' };
      const issue2 = { id: 2, source: 'chat_session', session_key: 's2', target_path: null, detected_at_ms: 0, category: 'y', severity: 'low', summary: 'y', evidence_json: '{}', status: 'pending' };
      const issue3 = { id: 3, source: 'chat_session', session_key: 's3', target_path: null, detected_at_ms: 0, category: 'z', severity: 'low', summary: 'z', evidence_json: '{}', status: 'pending' };

      listHarnessIssuesMock
        .mockResolvedValueOnce([issue1, issue2])
        .mockResolvedValueOnce([issue1, issue2, issue3]);

      renderApp();

      // First poll fires on mount (poll() is invoked immediately, not just on
      // the interval tick).
      await waitFor(() => expect(listHarnessIssuesMock).toHaveBeenCalledTimes(1));
      expect(listHarnessIssuesMock).toHaveBeenCalledWith('pending', 'chat_session');

      // Give the first poll's promise chain (async listHarnessIssues +
      // dynamic import) a tick to resolve and update state.
      await vi.waitFor(() => {
        expect(screen.queryByText('Harness issue detected')).toBeNull();
      });
      // No toast for either backlog issue's summary.
      expect(screen.queryByText('x')).toBeNull();
      expect(screen.queryByText('y')).toBeNull();

      // Advance the 8s interval to trigger the second poll.
      await vi.advanceTimersByTimeAsync(8_000);

      await waitFor(() => expect(listHarnessIssuesMock).toHaveBeenCalledTimes(2));

      // Exactly one new toast, for issue 3 only.
      await vi.waitFor(() => {
        expect(screen.getByText('Harness issue detected')).toBeInTheDocument();
      });
      expect(screen.getAllByText('Harness issue detected')).toHaveLength(1);
      expect(screen.getByText('z')).toBeInTheDocument();
      expect(screen.queryByText('x')).toBeNull();
      expect(screen.queryByText('y')).toBeNull();

      // Badge propagation (pendingHarnessIssueSessionIds -> session-issue
      // badges) is not asserted here: this test file's renderApp() does not
      // reach an expanded chat-session sidebar section (no sessions rendered
      // given the mocked chat_list_sessions=[] and default view), so a
      // `session-issue-badge-*` testid would not be present regardless of
      // the polling logic's correctness. Covered instead at the
      // SessionSidebarSection component level (see its own badge test).
    });
  });
});
