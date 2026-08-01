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

  // Fix Task 2 (chat-harness audit): /spawn dispatches via submit_orchestrator_task
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
      if (cmd === 'submit_orchestrator_task') return Promise.resolve({ task_id: '1', duplicate_of: null });
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
      expect(invokeMock).toHaveBeenCalledWith('submit_orchestrator_task', expect.anything()),
    );
    const submitCall = invokeMock.mock.calls.find(([cmd]) => cmd === 'submit_orchestrator_task');
    expect(submitCall![1].input.session_id).not.toBe('chat-session-abc');
  });

  // Plan Task 2 (gui-chat-agent-loop-wiring): a plain chat send (Loquela's
  // normal Enter-to-send path, which tags task_category: 'chat') must go
  // through the synchronous chat_send_message command, not the background
  // submit_orchestrator_task dispatch loop used by every other category.
  it('a plain chat send calls chat_send_message and not submit_orchestrator_task', async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'chat_list_sessions') return Promise.resolve([]);
      if (cmd === 'get_memory_status') return Promise.resolve({ corpus_counts: {} });
      if (cmd === 'chat_create_session') return Promise.resolve({ session_id: 'gui-test-session' });
      if (cmd === 'chat_send_message') {
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
      expect(invokeMock).toHaveBeenCalledWith('chat_send_message', expect.anything()),
    );
    expect(invokeMock).not.toHaveBeenCalledWith('submit_orchestrator_task', expect.anything());
    await waitFor(() => {
      expect(screen.getByText('hello back')).toBeInTheDocument();
    });
  });

  // Code-review follow-up (8448c477a1): chat_send_message already persists
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
      if (cmd === 'chat_send_message') {
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
  // pressing Enter a second time while a prior chat_send_message() call was
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
      if (cmd === 'chat_send_message') return pending;
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
      expect(invokeMock).toHaveBeenCalledWith('chat_send_message', expect.anything()),
    );

    await user.click(composer);
    await user.type(composer, 'second message');
    await user.keyboard('{Enter}');

    const chatSendCalls = invokeMock.mock.calls.filter(([cmd]: [string]) => cmd === 'chat_send_message');
    expect(chatSendCalls.length).toBe(1);

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
});
