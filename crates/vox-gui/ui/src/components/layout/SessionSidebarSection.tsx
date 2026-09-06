import { useState } from 'react';
import type { ChatSession } from '../../lib/useChatSessions';

const VISIBLE_PER_GROUP = 5;

interface Props {
  sessions: ChatSession[];
  activeSessionId: string | null;
  taskCounts: Record<string, number>;
  archivedSessions: ChatSession[];
  showArchived: boolean;
  /** session_ids with at least one PENDING scientia_harness_issues row —
   * intentionally pending-only, not "any issue ever," so a dismissed/
   * confirmed issue doesn't leave a stale attention dot. */
  pendingIssueSessionIds?: Set<string>;
  onSessionChange: (sessionId: string) => void;
  onCreateSession: () => void;
  onRenameSession: (sessionId: string, title: string) => void;
  onArchiveSession: (sessionId: string) => void;
  onUnarchiveSession: (sessionId: string) => void;
  onToggleArchivedView: () => void;
  onTaskBadgeClick: (sessionId: string) => void;
}

function groupByRepo(sessions: ChatSession[]): Map<string, ChatSession[]> {
  const groups = new Map<string, ChatSession[]>();
  for (const s of sessions) {
    const key = s.repository_id ?? 'Other';
    const list = groups.get(key) ?? [];
    list.push(s);
    groups.set(key, list);
  }
  for (const list of groups.values()) {
    list.sort((a, b) => (a.updated_at < b.updated_at ? 1 : a.updated_at > b.updated_at ? -1 : 0));
  }
  return groups;
}

function SessionRow({
  s, isActive, taskCount, hasPendingIssue, onSessionChange, onRenameSession, onArchiveSession, onTaskBadgeClick, showArchive,
}: {
  s: ChatSession;
  isActive: boolean;
  taskCount: number;
  hasPendingIssue: boolean;
  onSessionChange: (id: string) => void;
  onRenameSession: (id: string, title: string) => void;
  onArchiveSession: (id: string) => void;
  onTaskBadgeClick: (id: string) => void;
  showArchive: boolean;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(s.title);

  if (editing) {
    return (
      <input
        autoFocus
        value={draft}
        onChange={e => setDraft(e.target.value)}
        onBlur={() => { setEditing(false); if (draft.trim() && draft !== s.title) onRenameSession(s.session_id, draft.trim()); }}
        onKeyDown={e => { if (e.key === 'Enter') (e.target as HTMLInputElement).blur(); if (e.key === 'Escape') { setDraft(s.title); setEditing(false); } }}
        className="w-full rounded-sm px-2 py-1 text-[12px] bg-overlay-subtle"
      />
    );
  }

  return (
    <div role="tab" aria-selected={isActive}
         onClick={() => onSessionChange(s.session_id)}
         onDoubleClick={() => setEditing(true)}
         className="flex items-center justify-between rounded-sm px-2 py-1 text-[12px] cursor-pointer hover:bg-overlay-hover group">
      <span title={s.title} className="line-clamp-2 wrap-break-word">{s.title}</span>
      <span className="flex items-center gap-1 shrink-0">
        {hasPendingIssue && (
          <span
            data-testid={`session-issue-badge-${s.session_id}`}
            role="img"
            aria-label="Harness issue detected"
            title="Harness issue detected"
            className="size-1.5 shrink-0 rounded-full bg-amber-400"
          />
        )}
        {taskCount > 0 && (
          <span
            onClick={e => { e.stopPropagation(); onTaskBadgeClick(s.session_id); }}
            className="rounded-full bg-overlay-subtle px-1.5 text-[10px] text-text-muted"
          >
            {taskCount}
          </span>
        )}
        {showArchive && (
          <button
            type="button"
            onClick={e => { e.stopPropagation(); onArchiveSession(s.session_id); }}
            className="opacity-0 group-hover:opacity-100 focus:opacity-100 text-[10px] text-text-muted hover:text-text-primary"
          >
            Archive
          </button>
        )}
      </span>
    </div>
  );
}

function RepoGroup({
  repo, sessions, activeSessionId, taskCounts, pendingIssueSessionIds, onSessionChange, onRenameSession, onArchiveSession, onTaskBadgeClick,
}: {
  repo: string;
  sessions: ChatSession[];
  activeSessionId: string | null;
  taskCounts: Record<string, number>;
  pendingIssueSessionIds?: Set<string>;
  onSessionChange: (id: string) => void;
  onRenameSession: (id: string, title: string) => void;
  onArchiveSession: (id: string) => void;
  onTaskBadgeClick: (id: string) => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const visible = expanded ? sessions : sessions.slice(0, VISIBLE_PER_GROUP);
  const remaining = sessions.length - visible.length;

  return (
    <div>
      <div className="px-2 pt-1 pb-0.5 text-[10px] uppercase tracking-wide text-text-muted">{repo}</div>
      <div role="tablist" className="flex flex-col gap-0.5">
        {visible.map(s => (
          <SessionRow
            key={s.session_id}
            s={s}
            isActive={s.session_id === activeSessionId}
            taskCount={taskCounts[s.session_id] ?? 0}
            hasPendingIssue={pendingIssueSessionIds?.has(s.session_id) ?? false}
            onSessionChange={onSessionChange}
            onRenameSession={onRenameSession}
            onArchiveSession={onArchiveSession}
            onTaskBadgeClick={onTaskBadgeClick}
            showArchive
          />
        ))}
      </div>
      {remaining > 0 && !expanded && (
        <button type="button" onClick={() => setExpanded(true)} className="px-2 py-1 text-[11px] text-accent-secondary">
          Show {remaining} more
        </button>
      )}
    </div>
  );
}

export function SessionSidebarSection({
  sessions, activeSessionId, taskCounts, archivedSessions, showArchived, pendingIssueSessionIds,
  onSessionChange, onCreateSession, onRenameSession, onArchiveSession, onUnarchiveSession,
  onToggleArchivedView, onTaskBadgeClick,
}: Props) {
  const groups = groupByRepo(sessions);
  const archivedGroups = groupByRepo(archivedSessions);

  return (
    <div className="flex flex-col gap-1">
      <button type="button" onClick={onCreateSession} className="px-2 py-1 text-left text-[11px] text-text-muted hover:text-text-primary">
        + New session
      </button>
      {[...groups.entries()].map(([repo, groupSessions]) => (
        <RepoGroup
          key={repo}
          repo={repo}
          sessions={groupSessions}
          activeSessionId={activeSessionId}
          taskCounts={taskCounts}
          pendingIssueSessionIds={pendingIssueSessionIds}
          onSessionChange={onSessionChange}
          onRenameSession={onRenameSession}
          onArchiveSession={onArchiveSession}
          onTaskBadgeClick={onTaskBadgeClick}
        />
      ))}
      <button type="button" onClick={onToggleArchivedView} className="px-2 py-1 text-left text-[10px] text-text-muted">
        {showArchived ? 'Hide archived' : 'Show archived'}
      </button>
      {showArchived && [...archivedGroups.entries()].map(([repo, groupSessions]) => (
        <div key={`archived-${repo}`} className="opacity-60">
          <div className="px-2 pt-1 pb-0.5 text-[10px] uppercase tracking-wide text-text-muted">{repo} (archived)</div>
          {groupSessions.map(s => (
            <div key={s.session_id} className="flex items-center justify-between rounded-sm px-2 py-1 text-[12px]">
              <span className="truncate">{s.title}</span>
              <button type="button" onClick={() => onUnarchiveSession(s.session_id)} className="text-[10px] text-accent-secondary">
                Unarchive
              </button>
            </div>
          ))}
        </div>
      ))}
    </div>
  );
}
