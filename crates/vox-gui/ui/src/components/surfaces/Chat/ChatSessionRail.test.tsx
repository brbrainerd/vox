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

  it('keeps the session rail narrow but still surfaces the full title via tooltip (F-05)', () => {
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
    const rail = screen.getByRole('complementary');
    expect(rail).not.toHaveClass('w-44');
    // Rows use the compact left-gutter stack (2026-07-21 live-test redesign),
    // but titles wrap to 2 lines rather than truncating to 1: a CDP
    // measurement in the real w-64 rail (see
    // .remediation-notes/task4-truncate-verdict.md) found realistic
    // 30-40 char titles genuinely clip at 1 line but fit at 2, reproducing
    // the original F-05 bug. line-clamp-2 restores the F-05 fix while
    // keeping the compact gutter/badge/button styling from Task 4.
    const title = screen.getByText('First');
    expect(title).toHaveClass('line-clamp-2');
    expect(title).not.toHaveClass('truncate');
    // Full text is always discoverable via native tooltip, even when elided.
    expect(screen.getByRole('tab', { name: /First/ })).toHaveAttribute('title', 'First');
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

  it('gives the root aside a real height so it can fill and scroll within its dock panel (regression guard for cf677dce9a)', () => {
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
    expect(screen.getByRole('complementary').className).toContain('h-full');
  });

  it('has no leftover per-panel collapse/expand chevron UI (panel visibility is controlled entirely by the dock Panels menu now)', () => {
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
    expect(screen.queryByRole('button', { name: /collapse sessions rail/i })).toBeNull();
    expect(screen.queryByRole('button', { name: /expand sessions rail/i })).toBeNull();
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

  it('renders a realistic 10-session list as a compact stacked left-gutter list, not tall cards (Task 4 live-test fix)', () => {
    const manySessions = [
      { session_id: 'r1', title: 'Fix auth middleware', message_count: 42 },
      { session_id: 'r2', title: 'Debug CI runner freshness guard', message_count: 187 },
      { session_id: 'r3', title: 'Panels menu redesign', message_count: 9 },
      { session_id: 'r4', title: 'Sessions rail density pass', message_count: 3 },
      { session_id: 'r5', title: 'Investigate flaky dockview drag test', message_count: 61 },
      { session_id: 'r6', title: 'Review CodeRabbit sweep PR #433', message_count: 14 },
      { session_id: 'r7', title: 'Storage tiering orchestrator spec', message_count: 0 },
      { session_id: 'r8', title: 'Build broker L1 fair-FIFO shim', message_count: 27 },
      { session_id: 'r9', title: 'New chat', message_count: 0 },
      { session_id: 'r10', title: 'Vox Terminal ratatui TUI phase 2', message_count: 5 },
    ];
    render(
      <LanguageProvider>
        <ChatSessionRail
          sessions={manySessions}
          activeSessionId="r1"
          onSessionChange={vi.fn()}
          onCreateSession={vi.fn()}
        />
      </LanguageProvider>,
    );
    const rows = manySessions.map(s => screen.getByTestId(`session-row-${s.session_id}`));
    // Compact: rows are a stacked list, not oversized cards. Each row carries
    // a left-gutter accent border (same convention as CommandCatalogForm.tsx
    // catalog list: border-l-2, transparent when inactive, accent-colored
    // when active) rather than a full rounded-card outline.
    for (const row of rows) {
      expect(row.className).toMatch(/border-l-2/);
    }
    const activeRow = screen.getByTestId('session-row-r1');
    expect(activeRow.className).toMatch(/border-brass/);
    const inactiveRow = screen.getByTestId('session-row-r2');
    expect(inactiveRow.className).toMatch(/border-transparent/);
  });

  it('shows a pending-issue badge only for sessions in pendingIssueSessionIds', () => {
    render(
      <LanguageProvider>
        <ChatSessionRail
          sessions={sessions}
          activeSessionId="s1"
          onSessionChange={vi.fn()}
          onCreateSession={vi.fn()}
          pendingIssueSessionIds={new Set(['s1'])}
        />
      </LanguageProvider>,
    );
    expect(screen.getByTestId('session-issue-badge-s1')).toBeInTheDocument();
    expect(screen.queryByTestId('session-issue-badge-s2')).toBeNull();
  });
});
