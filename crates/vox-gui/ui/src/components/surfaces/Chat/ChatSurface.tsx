import React, { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Transcript } from '../Loquela/Transcript';
import { Button } from '../../ui/Button';
import { EmptyState } from '../../ui/EmptyState';
import { Icon } from '../../ui/Icons';
import type { ChatMessage } from '../../../lib/chatCorrelation';

interface ChatSession {
  session_id: string;
  title: string;
  message_count: number;
}

interface ChatSurfaceProps {
  pushToast: (t: any) => void;
  onNavigate?: (viewKey: string) => void;
  messages?: ChatMessage[];
  activeSessionId?: string;
  onSessionChange?: (sessionId: string) => void;
  onHydrateSession?: (sessionId: string) => void;
}

export function ChatSurface({
  pushToast,
  messages = [],
  activeSessionId,
  onSessionChange,
  onHydrateSession,
}: ChatSurfaceProps) {
  const [sessions, setSessions] = useState<ChatSession[]>([]);
  const activeId = activeSessionId ?? '';

  const loadSessions = useCallback(async () => {
    try {
      const list = await invoke<ChatSession[]>('chat_list_sessions', { limit: 40 });
      setSessions(list);
      if (!activeId && list.length > 0) {
        onSessionChange?.(list[0].session_id);
      }
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Chat sessions', body: String(err) });
    }
  }, [activeId, onSessionChange, pushToast]);

  useEffect(() => {
    loadSessions();
  }, [loadSessions]);

  useEffect(() => {
    if (activeId && onHydrateSession) onHydrateSession(activeId);
  }, [activeId, onHydrateSession]);

  const createSession = async () => {
    try {
      const s = await invoke<ChatSession & { conversation_id: number }>('chat_create_session', {
        title: 'New chat',
      });
      setSessions(prev => [s, ...prev]);
      onSessionChange?.(s.session_id);
    } catch (err) {
      pushToast({ tone: 'warn', title: 'New session failed', body: String(err) });
    }
  };

  return (
    <div className="flex flex-col gap-4 min-h-[60vh]">
      <div
        role="tablist"
        aria-label="Chat sessions"
        className="flex items-center gap-2 overflow-x-auto custom-scrollbar pb-1"
      >
        {sessions.map(s => {
          const isActive = s.session_id === activeId;
          return (
            <Button
              key={s.session_id}
              role="tab"
              aria-pressed={isActive}
              aria-selected={isActive}
              onClick={() => onSessionChange?.(s.session_id)}
              className={`shrink-0 rounded-lg border px-3 py-1.5 text-xs ${
                isActive
                  ? 'border-brass/40 bg-brass/10 text-brass'
                  : 'border-border-subtle text-text-muted hover:text-zinc-200'
              }`}
            >
              {s.title}
              {s.message_count > 0 ? ` (${s.message_count})` : ''}
            </Button>
          );
        })}
        <Button
          onClick={createSession}
          aria-label="New chat session"
          className="shrink-0 rounded-lg border border-border-subtle px-2 py-1 text-xs text-text-muted hover:text-brass"
        >
          <Icon.plus className="size-3.5" aria-hidden="true" /> New
        </Button>
      </div>
      {messages.length === 0 ? (
        <EmptyState
          icon={<Icon.spark className="size-8 text-brass" aria-hidden="true" />}
          title="No messages yet"
          description="Submit a task from the composer docked below — the transcript mirrors this session here."
        />
      ) : (
        <Transcript messages={messages} />
      )}
      <p className="text-[11px] text-text-muted">
        Composer is docked at the bottom — submit tasks there; this view mirrors the same session transcript.
      </p>
    </div>
  );
}
