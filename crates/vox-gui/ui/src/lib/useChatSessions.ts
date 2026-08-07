import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

export interface ChatSession {
  session_id: string;
  title: string;
  updated_at: string;
  message_count: number;
  conversation_id: number;
  repository_id: string | null;
}

interface ArchiveOptions {
  wasActive?: boolean;
  onReassign?: (nextActiveSessionId: string) => void;
}

export function useChatSessions() {
  const [sessions, setSessions] = useState<ChatSession[]>([]);
  const [includeArchived, setIncludeArchived] = useState(false);

  const load = useCallback(async (opts?: { includeArchived?: boolean }) => {
    const list = await invoke<ChatSession[]>('chat_list_sessions', {
      limit: 200,
      includeArchived: opts?.includeArchived ?? includeArchived,
    });
    setSessions(list);
  }, [includeArchived]);

  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // No optimistic update: on failure, invoke() rejects and this function rethrows without
  // touching `sessions` state, matching ChatSurface.tsx's existing await-then-update-on-success
  // pattern (there is no rollback anywhere in the current codebase to replicate).
  const createSession = useCallback(async (title?: string) => {
    const created = await invoke<ChatSession>('chat_create_session', { title });
    setSessions(prev => [created, ...prev]);
    return created;
  }, []);

  const renameSession = useCallback(async (sessionId: string, title: string) => {
    await invoke('chat_rename_session', { sessionId, title });
    setSessions(prev => prev.map(s => (s.session_id === sessionId ? { ...s, title } : s)));
  }, []);

  const archiveSession = useCallback(async (sessionId: string, opts?: ArchiveOptions) => {
    await invoke('chat_archive_session', { sessionId });
    setSessions(prev => {
      const remaining = prev.filter(s => s.session_id !== sessionId);
      if (opts?.wasActive && remaining.length > 0) {
        opts.onReassign?.(remaining[0].session_id);
      }
      return remaining;
    });
  }, []);

  const unarchiveSession = useCallback(async (sessionId: string) => {
    await invoke('chat_unarchive_session', { sessionId });
    await load();
  }, [load]);

  const toggleArchivedView = useCallback(async () => {
    const next = !includeArchived;
    setIncludeArchived(next);
    await load({ includeArchived: next });
  }, [includeArchived, load]);

  return {
    sessions,
    includeArchived,
    createSession,
    renameSession,
    archiveSession,
    unarchiveSession,
    toggleArchivedView,
    reload: load,
  };
}
