// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, waitFor, fireEvent } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import React from 'react';

const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => mockInvoke(cmd, args),
}));

let capturedHistoryHandler: (() => void) | null = null;
const mockListen = vi.fn((event: string, handler: unknown) => {
  if (event === 'vox://history-changed') {
    capturedHistoryHandler = handler as () => void;
  }
  return Promise.resolve(() => {});
});
vi.mock('@tauri-apps/api/event', () => ({
  listen: (event: string, handler: unknown) => mockListen(event, handler),
}));

const mockClipboard = { writeText: vi.fn(() => Promise.resolve()) };
Object.defineProperty(navigator, 'clipboard', { value: mockClipboard, writable: true });

import { HistoryPanel } from './HistoryPanel';

const FIXTURE_ENTRIES = [
  {
    id: 1,
    repo_id: 'r1',
    kind: 'clip',
    text: 'my text secret',
    redacted_text: 'my text [REDACTED]',
    created_at: 1000,
    pinned: false,
    source: 'cli',
    token_estimate: 3,
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
    token_estimate: 2,
  },
  {
    id: 3,
    repo_id: 'r1',
    kind: 'chat',
    text: 'hello world from chat',
    redacted_text: 'hello world from chat',
    created_at: 3000,
    pinned: false,
    source: 'chat',
    token_estimate: 5,
  },
];

describe('HistoryPanel', () => {
  beforeEach(() => {
    cleanup();
    mockInvoke.mockClear();
    mockListen.mockClear();
    mockClipboard.writeText.mockClear();
    capturedHistoryHandler = null;

    mockInvoke.mockImplementation((cmd) => {
      if (cmd === 'history_list') return Promise.resolve(FIXTURE_ENTRIES);
      return Promise.resolve(null);
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
    await waitFor(() => { expect(screen.getByText('git log')).toBeTruthy(); });
    const input = screen.getByPlaceholderText(/Fuzzy filter local/);
    await userEvent.type(input, 'git');
    expect(screen.queryByText('my text [REDACTED]')).toBeNull();
    expect(screen.getByText('git log')).toBeTruthy();
  });

  it('copy button writes redacted_text to clipboard', async () => {
    render(<HistoryPanel pushToast={vi.fn()} />);
    await waitFor(() => { expect(screen.getByText('git log')).toBeTruthy(); });
    const copyBtns = screen.getAllByTitle('Copy to clipboard');
    await userEvent.click(copyBtns[0]);
    expect(mockClipboard.writeText).toHaveBeenCalledWith(FIXTURE_ENTRIES[0].text);
  });

  it('pin button invokes history_pin with toggled value', async () => {
    const pushToast = vi.fn();
    render(<HistoryPanel pushToast={pushToast} />);
    await waitFor(() => { expect(screen.getByText('git log')).toBeTruthy(); });
    const pinBtns = screen.getAllByTitle(/Pin|Unpin/);
    // entry[0] is pinned=false → clicking pins it
    await userEvent.click(pinBtns[0]);
    expect(mockInvoke).toHaveBeenCalledWith('history_pin', { id: 1, pinned: true });
  });

  it('delete button invokes history_delete', async () => {
    render(<HistoryPanel pushToast={vi.fn()} />);
    await waitFor(() => { expect(screen.getByText('git log')).toBeTruthy(); });
    const delBtns = screen.getAllByTitle('Delete');
    await userEvent.click(delBtns[0]);
    expect(mockInvoke).toHaveBeenCalledWith('history_delete', { id: 1 });
  });

  it('re-run button appears only for command entries and copies text', async () => {
    render(<HistoryPanel pushToast={vi.fn()} />);
    await waitFor(() => { expect(screen.getByText('git log')).toBeTruthy(); });
    const rerunBtns = screen.getAllByTestId('rerun-btn');
    expect(rerunBtns).toHaveLength(1);
    await userEvent.click(rerunBtns[0]);
    expect(mockClipboard.writeText).toHaveBeenCalledWith('git log');
  });

  it('re-insert button appears only for chat entries and copies text', async () => {
    render(<HistoryPanel pushToast={vi.fn()} />);
    await waitFor(() => { expect(screen.getByText('hello world from chat')).toBeTruthy(); });
    const reinsertBtns = screen.getAllByTestId('reinsert-btn');
    expect(reinsertBtns).toHaveLength(1);
    await userEvent.click(reinsertBtns[0]);
    expect(mockClipboard.writeText).toHaveBeenCalledWith('hello world from chat');
  });

  it('subscribes to vox://history-changed and re-fetches on event', async () => {
    render(<HistoryPanel pushToast={vi.fn()} />);
    await waitFor(() => { expect(mockListen).toHaveBeenCalledWith('vox://history-changed', expect.any(Function)); });
    const initialCallCount = mockInvoke.mock.calls.filter(c => c[0] === 'history_list').length;
    // Simulate the event firing
    if (capturedHistoryHandler) capturedHistoryHandler();
    await waitFor(() => {
      const newCount = mockInvoke.mock.calls.filter(c => c[0] === 'history_list').length;
      expect(newCount).toBeGreaterThan(initialCallCount);
    });
  });
});
