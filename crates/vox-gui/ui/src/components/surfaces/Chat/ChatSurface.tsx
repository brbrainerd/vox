import React, { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Transcript } from '../Loquela/Transcript';
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
      <div className="flex items-center gap-2 overflow-x-auto custom-scrollbar pb-1">
        {sessions.map(s => (
          <button
            key={s.session_id}
            type="button"
            onClick={() => onSessionChange?.(s.session_id)}
            className={`shrink-0 rounded-lg border px-3 py-1.5 text-xs ${
              s.session_id === activeId
                ? 'border-brass/40 bg-brass/10 text-brass'
                : 'border-white/10 text-zinc-400 hover:text-zinc-200'
            }`}
          >
            {s.title}
            {s.message_count > 0 ? ` (${s.message_count})` : ''}
          </button>
        ))}
        <button
          type="button"
          onClick={createSession}
          className="shrink-0 rounded-lg border border-white/10 px-2 py-1 text-xs text-zinc-400 hover:text-brass"
        >
          + New
        </button>
      </div>
      <Transcript messages={messages} />
      <p className="text-[11px] text-zinc-600">
        Composer is docked at the bottom — submit tasks there; this view mirrors the same session transcript.
      </p>
    </div>
  );
}
