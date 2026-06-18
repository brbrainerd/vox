import React from 'react';
import { Glass } from '../../ui/Glass';
import { Button } from '../../ui/Button';
import { Icon } from '../../ui/Icons';
import { useLocalStorage } from '../../../hooks/useLocalStorage';

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
}

const SESSIONS_COLLAPSED_KEY = 'gui.chat.sessions_collapsed.v1';

export function ChatSessionRail({
  sessions,
  activeSessionId,
  onSessionChange,
  onCreateSession,
}: ChatSessionRailProps) {
  const [collapsed, setCollapsed] = useLocalStorage<boolean>(SESSIONS_COLLAPSED_KEY, false);

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
          <h2 className="text-[10px] uppercase tracking-[0.18em] text-brass">Sessions</h2>
          <button
            type="button"
            aria-label="Collapse sessions rail"
            aria-expanded={true}
            onClick={() => setCollapsed(true)}
            className="rounded p-1 text-zinc-500 transition hover:bg-white/[0.04] hover:text-zinc-300"
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
            return (
              <Button
                key={s.session_id}
                role="tab"
                aria-pressed={isActive}
                aria-selected={isActive}
                onClick={() => onSessionChange(s.session_id)}
                className={`w-full justify-start rounded-lg border px-2.5 py-2 text-left text-xs ${
                  isActive
                    ? 'border-brass/40 bg-brass/10 text-brass'
                    : 'border-border-subtle text-text-muted hover:text-zinc-200'
                }`}
              >
                <span className="block truncate">{s.title}</span>
                {s.message_count > 0 ? (
                  <span className="mt-0.5 block font-mono text-[10px] text-zinc-500">
                    {s.message_count} msg{s.message_count === 1 ? '' : 's'}
                  </span>
                ) : null}
              </Button>
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
