// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, act } from '@testing-library/react';
import React from 'react';
import {
  chatRailVisibility,
  CHAT_SESSION_RAIL_MIN_WIDTH,
  CHAT_EXECUTION_RAIL_MIN_WIDTH,
} from './chatRailVisibility';

describe('chatRailVisibility (pure helper)', () => {
  it('hides both rails at mobile width (~375)', () => {
    expect(chatRailVisibility(375)).toEqual({ sessionRail: false, executionRail: false });
  });

  it('hides both rails at tablet width (~768)', () => {
    expect(chatRailVisibility(768)).toEqual({ sessionRail: false, executionRail: false });
  });

  it('shows session rail but hides execution rail at mid width (~1000)', () => {
    expect(chatRailVisibility(1000)).toEqual({ sessionRail: true, executionRail: false });
  });

  it('shows both rails at desktop width (~1400)', () => {
    expect(chatRailVisibility(1400)).toEqual({ sessionRail: true, executionRail: true });
  });

  it('is inclusive exactly at each breakpoint', () => {
    expect(chatRailVisibility(CHAT_SESSION_RAIL_MIN_WIDTH).sessionRail).toBe(true);
    expect(chatRailVisibility(CHAT_SESSION_RAIL_MIN_WIDTH - 1).sessionRail).toBe(false);
    expect(chatRailVisibility(CHAT_EXECUTION_RAIL_MIN_WIDTH).executionRail).toBe(true);
    expect(chatRailVisibility(CHAT_EXECUTION_RAIL_MIN_WIDTH - 1).executionRail).toBe(false);
  });

  it('treats unmeasured width (0) as the wide layout to avoid first-paint flicker', () => {
    expect(chatRailVisibility(0)).toEqual({ sessionRail: true, executionRail: true });
    expect(chatRailVisibility(NaN)).toEqual({ sessionRail: true, executionRail: true });
  });
});

// ---- Component-level: narrow container collapses rails to a reveal toggle ----

const invokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));
vi.mock('../../../transport', async (importOriginal) => {
  const actual = await importOriginal<typeof import('../../../transport')>();
  return {
    ...actual,
    listenSecretaryProposed: vi.fn(() => Promise.resolve(() => {})),
  };
});

import { ChatSurface } from './ChatSurface';
import { LanguageProvider } from '../../../hooks/useLanguage';

/** Minimal ResizeObserver mock driven to a controllable width. */
let lastRoCallback: ResizeObserverCallback | null = null;
class MockResizeObserver {
  constructor(cb: ResizeObserverCallback) {
    lastRoCallback = cb;
  }
  observe() {}
  unobserve() {}
  disconnect() {}
}

function emitWidth(width: number) {
  act(() => {
    lastRoCallback?.(
      [{ contentRect: { width } } as ResizeObserverEntry],
      {} as ResizeObserver,
    );
  });
}

describe('ChatSurface responsive rails', () => {
  beforeEach(() => {
    lastRoCallback = null;
    (globalThis as any).ResizeObserver = MockResizeObserver;
    invokeMock.mockReset();
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'chat_list_sessions') {
        return Promise.resolve([{ session_id: 's1', title: 'First', message_count: 2 }]);
      }
      return Promise.resolve(null);
    });
  });

  it('shows the session toggle and reveals the rail as an overlay when narrow', async () => {
    render(<LanguageProvider><ChatSurface pushToast={() => {}} activeSessionId="s1" onNavigate={() => {}} /></LanguageProvider>);
    await screen.findByTestId('chat-surface-layout');

    emitWidth(375);

    const toggle = await screen.findByTestId('chat-session-rail-toggle');
    expect(screen.queryByTestId('chat-session-rail-overlay')).toBeNull();

    act(() => toggle.click());
    await waitFor(() => {
      expect(screen.getByTestId('chat-session-rail-overlay')).toBeInTheDocument();
    });
  });

  it('keeps both rails inline (no toggles) at desktop width', async () => {
    render(<LanguageProvider><ChatSurface pushToast={() => {}} activeSessionId="s1" onNavigate={() => {}} /></LanguageProvider>);
    await screen.findByTestId('chat-surface-layout');

    emitWidth(1400);

    await waitFor(() => {
      expect(screen.getByTestId('chat-session-rail')).toBeInTheDocument();
    });
    expect(screen.queryByTestId('chat-session-rail-toggle')).toBeNull();
    expect(screen.queryByTestId('chat-execution-rail-toggle')).toBeNull();
  });
});
