// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';
import { useChatSessions } from './useChatSessions';
import { invoke } from '@tauri-apps/api/core';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

function session(overrides: Partial<import('./useChatSessions').ChatSession>) {
  return {
    session_id: 's1', title: 'Untitled', updated_at: '', message_count: 0,
    conversation_id: 1, repository_id: 'repo-a', ...overrides,
  };
}

describe('useChatSessions', () => {
  beforeEach(() => vi.mocked(invoke).mockReset());

  it('loads sessions on mount', async () => {
    vi.mocked(invoke).mockResolvedValueOnce([session({ session_id: 's1' })]);
    const { result } = renderHook(() => useChatSessions());
    await waitFor(() => expect(result.current.sessions).toHaveLength(1));
    expect(invoke).toHaveBeenCalledWith('chat_list_sessions', expect.anything());
  });

  it('creates a session and prepends it to the list', async () => {
    vi.mocked(invoke).mockResolvedValueOnce([]); // initial load
    const { result } = renderHook(() => useChatSessions());
    await waitFor(() => expect(result.current.sessions).toHaveLength(0));

    vi.mocked(invoke).mockResolvedValueOnce(session({ session_id: 's2', title: 'New chat' }));
    await act(async () => { await result.current.createSession(); });

    expect(result.current.sessions[0].session_id).toBe('s2');
    expect(invoke).toHaveBeenCalledWith('chat_create_session', expect.anything());
  });

  it('archiving removes the session from the default (non-archived) list', async () => {
    vi.mocked(invoke).mockResolvedValueOnce([session({ session_id: 's1' })]);
    const { result } = renderHook(() => useChatSessions());
    await waitFor(() => expect(result.current.sessions).toHaveLength(1));

    vi.mocked(invoke).mockResolvedValueOnce(undefined); // chat_archive_session
    await act(async () => { await result.current.archiveSession('s1'); });

    expect(result.current.sessions).toHaveLength(0);
    expect(invoke).toHaveBeenCalledWith('chat_archive_session', { sessionId: 's1' });
  });

  it('archiving the active session reassigns activeSessionId to the next remaining one', async () => {
    vi.mocked(invoke).mockResolvedValueOnce([session({ session_id: 's1' }), session({ session_id: 's2' })]);
    const { result } = renderHook(() => useChatSessions());
    await waitFor(() => expect(result.current.sessions).toHaveLength(2));

    vi.mocked(invoke).mockResolvedValueOnce(undefined); // chat_archive_session
    const onActiveSessionArchived = vi.fn();
    await act(async () => { await result.current.archiveSession('s1', { wasActive: true, onReassign: onActiveSessionArchived }); });

    expect(onActiveSessionArchived).toHaveBeenCalledWith('s2');
  });

  it('surfaces a create failure via the returned error rather than throwing unhandled', async () => {
    vi.mocked(invoke).mockResolvedValueOnce([]); // initial load
    const { result } = renderHook(() => useChatSessions());
    await waitFor(() => expect(result.current.sessions).toHaveLength(0));

    vi.mocked(invoke).mockRejectedValueOnce(new Error('backend unreachable'));
    await expect(result.current.createSession()).rejects.toThrow('backend unreachable');
    expect(result.current.sessions).toHaveLength(0); // no optimistic entry left behind
  });
});
