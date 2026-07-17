import React, { useState } from 'react';
import { Glass } from '../../ui/Glass';
import { Button } from '../../ui/Button';
import { Icon } from '../../ui/Icons';
import { useLocalStorage } from '../../../hooks/useLocalStorage';
import { useLabel } from '../../../hooks/useLanguage';

export interface ChatSessionItem {
  session_id: string;
  title: string;
  message_count: number;
}

export interface ChatSessionRailProps {
  sessions: ChatSessionItem[];
  activeSessionId: string;
  onSessionChange: (sessionId: string) => void;
  onCreateSession: () => void;
  onRenameSession?: (sessionId: string, title: string) => void;
  onArchiveSession?: (sessionId: string) => void;
}

const SESSIONS_COLLAPSED_KEY = 'gui.chat.sessions_collapsed.v1';

export function ChatSessionRail({
  sessions,
  activeSessionId,
  onSessionChange,
  onCreateSession,
  onRenameSession,
  onArchiveSession,
}: ChatSessionRailProps) {
  const [collapsed, setCollapsed] = useLocalStorage<boolean>(SESSIONS_COLLAPSED_KEY, false);
  const [menuFor, setMenuFor] = useState<string | null>(null);
  const [renaming, setRenaming] = useState<string | null>(null);

  if (collapsed) {
    return (
      <aside className="shrink-0" data-testid="chat-session-rail">
        <Glass className="flex flex-col items-center gap-2 p-2">
          <button
            type="button"
            aria-label="Expand sessions rail"
            aria-expanded={false}
            onClick={() => setCollapsed(false)}
            className="rounded-lg border border-border-subtle p-2 text-text-muted transition hover:border-brass/40 hover:text-brass"
          >
            <span className="font-mono text-sm" aria-hidden="true">
              »
            </span>
          </button>
        </Glass>
      </aside>
    );
  }

  return (
    <aside className="w-44 shrink-0" data-testid="chat-session-rail">
      <Glass className="flex h-full max-h-[70vh] flex-col gap-2 p-3">
        <div className="flex items-center justify-between gap-2">
          <h2 className="text-[10px] uppercase tracking-[0.18em] text-brass">{useLabel('chat-sessions')}</h2>
          <button
            type="button"
            aria-label="Collapse sessions rail"
            aria-expanded={true}
            onClick={() => setCollapsed(true)}
            className="rounded p-1 text-text-muted transition hover:bg-overlay-subtle hover:text-text-secondary"
          >
            <span className="font-mono text-xs" aria-hidden="true">
              «
            </span>
          </button>
        </div>

        <div
          role="tablist"
          aria-label="Chat sessions"
          className="flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto custom-scrollbar"
        >
          {sessions.map(s => {
            const isActive = s.session_id === activeSessionId;
            if (renaming === s.session_id) {
              return (
                <input
                  key={s.session_id}
                  aria-label="New session title"
                  defaultValue={s.title}
                  autoFocus
                  onKeyDown={e => {
                    if (e.key === 'Enter') {
                      const title = (e.target as HTMLInputElement).value.trim();
                      if (title) onRenameSession?.(s.session_id, title);
                      setRenaming(null);
                    }
                    if (e.key === 'Escape') setRenaming(null);
                  }}
                  className="w-full rounded-lg border border-brass/40 bg-bg-base px-2.5 py-2 text-xs text-text-primary outline-none"
                />
              );
            }
            return (
              <div key={s.session_id} className="relative flex items-stretch gap-1">
                <Button
                  role="tab"
                  aria-pressed={isActive}
                  aria-selected={isActive}
                  onClick={() => onSessionChange(s.session_id)}
                  className={`min-w-0 flex-1 justify-start rounded-lg border px-2.5 py-2 text-left text-xs ${
                    isActive
                      ? 'border-brass/40 bg-brass/10 text-brass'
                      : 'border-border-subtle text-text-muted hover:text-text-secondary'
                  }`}
                >
                  <span className="block truncate">{s.title}</span>
                  {s.message_count > 0 ? (
                    <span className="mt-0.5 block font-mono text-[10px] text-text-muted">
                      {s.message_count} msg{s.message_count === 1 ? '' : 's'}
                    </span>
                  ) : null}
                </Button>
                {(onRenameSession || onArchiveSession) && (
                  <button
                    type="button"
                    aria-label={`Session actions for ${s.title}`}
                    aria-haspopup="menu"
                    aria-expanded={menuFor === s.session_id}
                    onClick={() => setMenuFor(m => (m === s.session_id ? null : s.session_id))}
                    className="shrink-0 rounded px-1 text-text-muted hover:bg-overlay-subtle hover:text-text-secondary"
                  >
                    ⋯
                  </button>
                )}
                {menuFor === s.session_id && (
                  <div
                    role="menu"
                    className="absolute right-0 top-full z-50 mt-1 w-28 rounded-lg border border-border-subtle bg-bg-base p-1"
                  >
                    {onRenameSession && (
                      <button
                        type="button"
                        role="menuitem"
                        onClick={() => { setMenuFor(null); setRenaming(s.session_id); }}
                        className="w-full rounded px-2 py-1 text-left text-xs text-text-secondary hover:bg-overlay-subtle"
                      >
                        Rename
                      </button>
                    )}
                    {onArchiveSession && (
                      <button
                        type="button"
                        role="menuitem"
                        onClick={() => { setMenuFor(null); onArchiveSession(s.session_id); }}
                        className="w-full rounded px-2 py-1 text-left text-xs text-rose-300 hover:bg-overlay-subtle"
                      >
                        Archive
                      </button>
                    )}
                  </div>
                )}
              </div>
            );
          })}
        </div>

        <Button
          type="button"
          onClick={onCreateSession}
          aria-label="New chat session"
          className="shrink-0 justify-center rounded-lg border border-border-subtle px-2 py-1.5 text-xs text-text-muted hover:text-brass"
        >
          <Icon.plus className="size-3.5" aria-hidden="true" /> New
        </Button>
      </Glass>
    </aside>
  );
}
