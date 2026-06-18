// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import React from 'react';
import { ChatSessionRail } from './ChatSessionRail';

const sessions = [
  { session_id: 's1', title: 'First', message_count: 2 },
  { session_id: 's2', title: 'Second', message_count: 0 },
];

describe('ChatSessionRail', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('marks the active session tab with aria-pressed', () => {
    render(
      <ChatSessionRail
        sessions={sessions}
        activeSessionId="s1"
        onSessionChange={vi.fn()}
        onCreateSession={vi.fn()}
      />,
    );
    const active = screen.getByRole('tab', { name: /First/ });
    expect(active.getAttribute('aria-pressed')).toBe('true');
  });

  it('can collapse and expand the sessions rail', async () => {
    const user = userEvent.setup();
    render(
      <ChatSessionRail
        sessions={sessions}
        activeSessionId="s1"
        onSessionChange={vi.fn()}
        onCreateSession={vi.fn()}
      />,
    );

    expect(screen.getByRole('tablist', { name: /chat sessions/i })).toBeInTheDocument();
    const collapse = screen.getByRole('button', { name: /collapse sessions rail/i });
    expect(collapse.getAttribute('aria-expanded')).toBe('true');
    await user.click(collapse);
    expect(screen.queryByRole('tablist', { name: /chat sessions/i })).toBeNull();

    const expand = screen.getByRole('button', { name: /expand sessions rail/i });
    expect(expand.getAttribute('aria-expanded')).toBe('false');
    await user.click(expand);
    expect(screen.getByRole('tablist', { name: /chat sessions/i })).toBeInTheDocument();
  });

  it('persists collapsed state in localStorage', async () => {
    const user = userEvent.setup();
    render(
      <ChatSessionRail
        sessions={sessions}
        activeSessionId="s1"
        onSessionChange={vi.fn()}
        onCreateSession={vi.fn()}
      />,
    );

    await user.click(screen.getByRole('button', { name: /collapse sessions rail/i }));
    expect(localStorage.getItem('gui.chat.sessions_collapsed.v1')).toBe('true');
  });
});
