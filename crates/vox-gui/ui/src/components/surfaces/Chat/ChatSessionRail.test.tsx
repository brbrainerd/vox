// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import React from 'react';
import { ChatSessionRail } from './ChatSessionRail';
import { LanguageProvider } from '../../../hooks/useLanguage';

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
      <LanguageProvider>
        <ChatSessionRail
          sessions={sessions}
          activeSessionId="s1"
          onSessionChange={vi.fn()}
          onCreateSession={vi.fn()}
        />
      </LanguageProvider>,
    );
    const active = screen.getByRole('tab', { name: /First/ });
    expect(active.getAttribute('aria-pressed')).toBe('true');
  });

  it('labels the aside landmark (axe landmark-unique)', () => {
    render(
      <LanguageProvider>
        <ChatSessionRail
          sessions={sessions}
          activeSessionId="s1"
          onSessionChange={vi.fn()}
          onCreateSession={vi.fn()}
        />
      </LanguageProvider>,
    );
    expect(screen.getByRole('complementary')).toHaveAttribute('aria-label', 'Chat sessions');
  });

  it('can collapse and expand the sessions rail', async () => {
    const user = userEvent.setup();
    render(
      <LanguageProvider>
        <ChatSessionRail
          sessions={sessions}
          activeSessionId="s1"
          onSessionChange={vi.fn()}
          onCreateSession={vi.fn()}
        />
      </LanguageProvider>,
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
      <LanguageProvider>
        <ChatSessionRail
          sessions={sessions}
          activeSessionId="s1"
          onSessionChange={vi.fn()}
          onCreateSession={vi.fn()}
        />
      </LanguageProvider>,
    );

    await user.click(screen.getByRole('button', { name: /collapse sessions rail/i }));
    expect(localStorage.getItem('gui.chat.sessions_collapsed.v1')).toBe('true');
  });

  it('renames a session through the row menu', async () => {
    const user = userEvent.setup();
    const onRename = vi.fn();
    render(
      <LanguageProvider>
        <ChatSessionRail
          sessions={sessions}
          activeSessionId="s1"
          onSessionChange={vi.fn()}
          onCreateSession={vi.fn()}
          onRenameSession={onRename}
          onArchiveSession={vi.fn()}
        />
      </LanguageProvider>,
    );
    await user.click(screen.getByRole('button', { name: /session actions for First/i }));
    await user.click(screen.getByRole('menuitem', { name: /rename/i }));
    const input = screen.getByRole('textbox', { name: /new session title/i });
    await user.clear(input);
    await user.type(input, 'Renamed{Enter}');
    expect(onRename).toHaveBeenCalledWith('s1', 'Renamed');
  });

  it('archives a session through the row menu', async () => {
    const user = userEvent.setup();
    const onArchive = vi.fn();
    render(
      <LanguageProvider>
        <ChatSessionRail
          sessions={sessions}
          activeSessionId="s1"
          onSessionChange={vi.fn()}
          onCreateSession={vi.fn()}
          onRenameSession={vi.fn()}
          onArchiveSession={onArchive}
        />
      </LanguageProvider>,
    );
    await user.click(screen.getByRole('button', { name: /session actions for Second/i }));
    await user.click(screen.getByRole('menuitem', { name: /archive/i }));
    expect(onArchive).toHaveBeenCalledWith('s2');
  });
});
