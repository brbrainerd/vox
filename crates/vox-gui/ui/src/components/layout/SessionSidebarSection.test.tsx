// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SessionSidebarSection } from './SessionSidebarSection';
import type { ChatSession } from '../../lib/useChatSessions';

function session(overrides: Partial<ChatSession>): ChatSession {
  return {
    session_id: 's1', title: 'Untitled', updated_at: '2026-01-01T00:00:00Z', message_count: 0,
    conversation_id: 1, repository_id: 'vox', ...overrides,
  };
}

const noop = () => {};
const baseProps = {
  activeSessionId: null as string | null,
  taskCounts: {} as Record<string, number>,
  archivedSessions: [] as ChatSession[],
  showArchived: false,
  onSessionChange: noop,
  onCreateSession: noop,
  onRenameSession: noop as (id: string, title: string) => void,
  onArchiveSession: noop as (id: string) => void,
  onUnarchiveSession: noop as (id: string) => void,
  onToggleArchivedView: noop,
  onTaskBadgeClick: noop as (id: string) => void,
};

describe('SessionSidebarSection', () => {
  it('groups sessions by repository_id under separate headers', () => {
    const sessions = [session({ session_id: 'a', repository_id: 'vox' }), session({ session_id: 'b', repository_id: 'vox-server' })];
    render(<SessionSidebarSection {...baseProps} sessions={sessions} />);
    expect(screen.getByText('vox')).toBeInTheDocument();
    expect(screen.getByText('vox-server')).toBeInTheDocument();
  });

  it('sessions without a repository_id fall under "Other"', () => {
    render(<SessionSidebarSection {...baseProps} sessions={[session({ repository_id: null })]} />);
    expect(screen.getByText('Other')).toBeInTheDocument();
  });

  it('sorts each repo group by updated_at descending before truncating', () => {
    const sessions = [
      session({ session_id: 'old', title: 'Old', repository_id: 'vox', updated_at: '2026-01-01T00:00:00Z' }),
      session({ session_id: 'new', title: 'New', repository_id: 'vox', updated_at: '2026-06-01T00:00:00Z' }),
    ];
    render(<SessionSidebarSection {...baseProps} sessions={sessions} />);
    const tabs = screen.getAllByRole('tab');
    expect(tabs[0]).toHaveTextContent('New');
    expect(tabs[1]).toHaveTextContent('Old');
  });

  it('truncates each repo group independently at 5, with its own Show more', () => {
    const sessions = Array.from({ length: 7 }, (_, i) => session({ session_id: `v${i}`, title: `Session ${i}`, repository_id: 'vox', updated_at: `2026-01-0${i + 1}T00:00:00Z` }));
    render(<SessionSidebarSection {...baseProps} sessions={sessions} />);
    expect(screen.getAllByRole('tab')).toHaveLength(5);
    fireEvent.click(screen.getByText('Show 2 more'));
    expect(screen.getAllByRole('tab')).toHaveLength(7);
  });

  it('expanding one repo group does not affect another', () => {
    const sessions = [
      ...Array.from({ length: 7 }, (_, i) => session({ session_id: `v${i}`, title: `V${i}`, repository_id: 'vox', updated_at: `2026-01-0${i + 1}T00:00:00Z` })),
      session({ session_id: 'w0', title: 'W0', repository_id: 'vox-server' }),
    ];
    render(<SessionSidebarSection {...baseProps} sessions={sessions} />);
    fireEvent.click(screen.getByText('Show 2 more'));
    expect(screen.getAllByRole('tab')).toHaveLength(8);
  });

  it('clicking + New session calls onCreateSession', () => {
    const onCreateSession = vi.fn();
    render(<SessionSidebarSection {...baseProps} sessions={[]} onCreateSession={onCreateSession} />);
    fireEvent.click(screen.getByText('+ New session'));
    expect(onCreateSession).toHaveBeenCalled();
  });

  it('clicking a task badge calls onTaskBadgeClick with the session, not onSessionChange', () => {
    const onSessionChange = vi.fn();
    const onTaskBadgeClick = vi.fn();
    render(<SessionSidebarSection {...baseProps} sessions={[session({ session_id: 'a' })]} taskCounts={{ a: 3 }} onSessionChange={onSessionChange} onTaskBadgeClick={onTaskBadgeClick} />);
    fireEvent.click(screen.getByText('3'));
    expect(onTaskBadgeClick).toHaveBeenCalledWith('a');
    expect(onSessionChange).not.toHaveBeenCalled();
  });

  it('renames a session via inline edit, matching the row not the whole list', () => {
    const onRenameSession = vi.fn();
    render(<SessionSidebarSection {...baseProps} sessions={[session({ session_id: 'a', title: 'Old title' })]} onRenameSession={onRenameSession} />);
    fireEvent.doubleClick(screen.getByText('Old title'));
    const input = screen.getByDisplayValue('Old title');
    fireEvent.change(input, { target: { value: 'New title' } });
    fireEvent.blur(input);
    expect(onRenameSession).toHaveBeenCalledWith('a', 'New title');
  });

  it('"Show archived" toggles onToggleArchivedView and, once open, renders archivedSessions with a working Unarchive action', () => {
    const onToggleArchivedView = vi.fn();
    const onUnarchiveSession = vi.fn();
    const archived = [session({ session_id: 'arch-1', title: 'Archived one', repository_id: 'vox' })];

    const { rerender } = render(<SessionSidebarSection {...baseProps} sessions={[]} onToggleArchivedView={onToggleArchivedView} />);
    fireEvent.click(screen.getByText('Show archived'));
    expect(onToggleArchivedView).toHaveBeenCalled();

    rerender(<SessionSidebarSection {...baseProps} sessions={[]} showArchived archivedSessions={archived} onUnarchiveSession={onUnarchiveSession} />);
    expect(screen.getByText('Archived one')).toBeInTheDocument();
    fireEvent.click(screen.getByText('Unarchive'));
    expect(onUnarchiveSession).toHaveBeenCalledWith('arch-1');
  });
});
