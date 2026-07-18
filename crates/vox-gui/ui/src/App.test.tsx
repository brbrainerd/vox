// @vitest-environment jsdom
import { describe, it, expect, vi, beforeAll } from 'vitest';
import React from 'react';

// ── Tauri APIs ────────────────────────────────────────────────────────────────
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue(null),
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

import { render } from '@testing-library/react';
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
});
