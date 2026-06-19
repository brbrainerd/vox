// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import React from 'react';

const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => mockInvoke(cmd, args),
}));

const mockListen = vi.fn(() => Promise.resolve(() => {}));
vi.mock('@tauri-apps/api/event', () => ({
  listen: (event: string, handler: unknown) => mockListen(event, handler),
}));

import { HistoryPanel } from './HistoryPanel';

describe('HistoryPanel', () => {
  beforeEach(() => {
    cleanup();
    mockInvoke.mockClear();
    mockListen.mockClear();

    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'history_list') {
        return Promise.resolve([
          {
            id: 1,
            repo_id: 'r1',
            kind: 'clip',
            text: 'my text secret',
            redacted_text: 'my text [REDACTED]',
            created_at: 1000,
            pinned: false,
            source: 'cli',
            token_estimate: 0,
          },
          {
            id: 2,
            repo_id: 'r1',
            kind: 'command',
            text: 'git log',
            redacted_text: 'git log',
            created_at: 2000,
            pinned: true,
            source: 'osc633',
            token_estimate: 0,
          },
        ]);
      }
      return Promise.resolve([]);
    });
  });

  it('renders history panel title and items', async () => {
    render(<HistoryPanel pushToast={vi.fn()} />);

    expect(screen.getByText('History & Clips')).toBeTruthy();

    await waitFor(() => {
      expect(screen.getByText('my text [REDACTED]')).toBeTruthy();
      expect(screen.getByText('git log')).toBeTruthy();
    });
  });

  it('filters items locally on search query change', async () => {
    render(<HistoryPanel pushToast={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText('git log')).toBeTruthy();
    });

    const input = screen.getByPlaceholderText(/Fuzzy filter local/);
    await userEvent.type(input, 'git');

    expect(screen.queryByText('my text [REDACTED]')).toBeNull();
    expect(screen.getByText('git log')).toBeTruthy();
  });
});
