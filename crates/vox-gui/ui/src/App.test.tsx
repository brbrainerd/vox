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
});
