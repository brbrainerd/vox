// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import React from 'react';

const invokeMock = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

const noopToast = () => {};

import { ChatSurface } from './ChatSurface';
import type { ChatMessage } from '../../../lib/chatCorrelation';

describe('ChatSurface', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'chat_list_sessions') {
        return Promise.resolve([
          { session_id: 's1', title: 'First', message_count: 2 },
          { session_id: 's2', title: 'Second', message_count: 0 },
        ]);
      }
      return Promise.resolve(null);
    });
  });

  it('marks the active session tab with aria-pressed', async () => {
    render(<ChatSurface pushToast={noopToast} activeSessionId="s1" />);
    const active = await screen.findByRole('tab', { name: /First/ });
    expect(active.getAttribute('aria-pressed')).toBe('true');
    const inactive = screen.getByRole('tab', { name: /Second/ });
    expect(inactive.getAttribute('aria-pressed')).toBe('false');
  });

  it('gives the New session button an explicit type', async () => {
    render(<ChatSurface pushToast={noopToast} activeSessionId="s1" />);
    const newBtn = await screen.findByRole('button', { name: /new chat session/i });
    expect(newBtn.getAttribute('type')).toBe('button');
  });

  it('renders an empty state when the session has no messages', async () => {
    render(<ChatSurface pushToast={noopToast} activeSessionId="s1" messages={[]} />);
    await waitFor(() => {
      expect(screen.getByText(/no messages yet/i)).toBeDefined();
    });
  });

  it('exposes the transcript as a polite log region when messages exist', async () => {
    const messages: ChatMessage[] = [
      { id: 'm1', role: 'user', text: 'hi', status: 'done' } as ChatMessage,
    ];
    render(<ChatSurface pushToast={noopToast} activeSessionId="s1" messages={messages} />);
    const log = await screen.findByRole('log', { name: /chat transcript/i });
    expect(log.getAttribute('aria-live')).toBe('polite');
  });
});
