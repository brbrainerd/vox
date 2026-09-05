import { describe, expect, it } from 'vitest';
import {
  getSessionMessages,
  initialSessionChatStore,
  resolveSessionForEvent,
  sessionChatReducer,
} from './sessionChatStore';

const evt = (kind: Record<string, unknown>) => ({
  id: Math.floor(Math.random() * 100000),
  timestamp_ms: 0,
  kind: kind as { type: string; [k: string]: unknown },
});

describe('sessionChatStore', () => {
  it('keeps Loquela and chat sessions isolated', () => {
    let store = sessionChatReducer(initialSessionChatStore, {
      type: 'submit',
      sessionId: 'sess-a',
      runId: 'R1',
      prompt: 'hello a',
    });
    store = sessionChatReducer(store, {
      type: 'submit',
      sessionId: 'sess-b',
      runId: 'R2',
      prompt: 'hello b',
    });
    expect(getSessionMessages(store, 'sess-a').length).toBe(2);
    expect(getSessionMessages(store, 'sess-b').length).toBe(2);
    expect(getSessionMessages(store, 'sess-a')[0].text).toBe('hello a');
    expect(getSessionMessages(store, 'sess-b')[0].text).toBe('hello b');
  });

  it('routes token_streamed to the session that owns the task mapping', () => {
    let store = sessionChatReducer(initialSessionChatStore, {
      type: 'submit',
      sessionId: 'sess-a',
      runId: 'R1',
      prompt: 'q',
    });
    store = sessionChatReducer(store, {
      type: 'submitResolved',
      sessionId: 'sess-a',
      runId: 'R1',
      taskId: '7',
    });
    store = sessionChatReducer(store, {
      type: 'agentEvent',
      event: evt({ type: 'task_started', task_id: 7, agent_id: 3, session_id: 'sess-a' }),
    });
    store = sessionChatReducer(store, {
      type: 'agentEvent',
      event: evt({ type: 'token_streamed', agent_id: 3, text: 'Hi' }),
    });
    const assistant = getSessionMessages(store, 'sess-a').find(m => m.role === 'assistant');
    expect(assistant?.text).toBe('Hi');
    expect(getSessionMessages(store, 'sess-b').length).toBe(0);
  });

  it('hydrates DB rows without overwriting live messages', () => {
    let store = sessionChatReducer(initialSessionChatStore, {
      type: 'submit',
      sessionId: 'sess-a',
      runId: 'R1',
      prompt: 'live',
    });
    store = sessionChatReducer(store, {
      type: 'hydrate',
      sessionId: 'sess-a',
      messages: [
        {
          id: 'db-1',
          role: 'user',
          text: 'old',
          status: 'done',
          runId: 'db',
        },
      ],
    });
    expect(getSessionMessages(store, 'sess-a').some(m => m.text === 'live')).toBe(true);
    expect(getSessionMessages(store, 'sess-a').some(m => m.text === 'old')).toBe(false);
  });

  it('failRun marks the optimistic assistant bubble as failed (duplicate-skip retraction)', () => {
    let store = sessionChatReducer(initialSessionChatStore, {
      type: 'submit',
      sessionId: 'sess-a',
      runId: 'R1',
      prompt: 'build the thing',
    });
    // Daemon refused as a near-duplicate -> retract the pending bubble.
    store = sessionChatReducer(store, {
      type: 'failRun',
      sessionId: 'sess-a',
      runId: 'R1',
      error: 'duplicate of task #7',
    });
    const assistant = getSessionMessages(store, 'sess-a').find(m => m.role === 'assistant');
    expect(assistant?.status).toBe('failed');
    expect(assistant?.error).toBe('duplicate of task #7');
    // Other sessions untouched.
    expect(getSessionMessages(store, 'sess-b').length).toBe(0);
  });

  it('resolveSessionForEvent prefers taskToSession', () => {
    const store = {
      sessions: {},
      taskToSession: { '42': 'sess-x' },
      pending: [],
    };
    expect(
      resolveSessionForEvent(store, evt({ type: 'task_completed', task_id: 42, agent_id: 1 })),
    ).toBe('sess-x');
  });

  // Task G1: the sync chat path (`chat_turn`'s `run_sync`) has no
  // submit/submitResolved/task_started correlation at all -- it drives the
  // store via `chatPending`/`chatReplySettled` keyed by a client-minted
  // `tempId`, with no task_id or agentToTask entry ever populated. Before
  // this, `token_streamed` could ONLY resolve a session via the
  // agentToTask scan, so tokens streamed during a sync turn had nowhere to
  // route. `resolveSessionForEvent` must route a `token_streamed` frame
  // that carries `session_id` DIRECTLY, with no task/agent bookkeeping
  // required at all.
  it('resolveSessionForEvent routes token_streamed directly via session_id, with no task/agent mapping', () => {
    const store = { sessions: {}, taskToSession: {}, pending: [] };
    expect(
      resolveSessionForEvent(
        store,
        evt({ type: 'token_streamed', agent_id: 0, text: 'Hi', session_id: 'sess-sync' }),
      ),
    ).toBe('sess-sync');
  });

  it('sync quick-chat: token_streamed with session_id appends to the chatPending bubble with no task/agent correlation', () => {
    let store = sessionChatReducer(initialSessionChatStore, {
      type: 'chatPending',
      sessionId: 'sess-a',
      tempId: 'temp-1',
      userText: 'hello',
    });
    store = sessionChatReducer(store, {
      type: 'agentEvent',
      event: evt({ type: 'token_streamed', agent_id: 0, text: 'Hi ', session_id: 'sess-a' }),
    });
    store = sessionChatReducer(store, {
      type: 'agentEvent',
      event: evt({ type: 'token_streamed', agent_id: 0, text: 'there', session_id: 'sess-a' }),
    });
    const assistant = getSessionMessages(store, 'sess-a').find((m) => m.role === 'assistant');
    expect(assistant?.text).toBe('Hi there');
    expect(assistant?.status).toBe('streaming');
    // Never touches the background-task correlation maps.
    expect(store.taskToSession).toEqual({});
  });

  it('routes activity_changed to the session that owns the agent mapping', () => {
    let store = sessionChatReducer(initialSessionChatStore, {
      type: 'submit',
      sessionId: 'sess-a',
      runId: 'R1',
      prompt: 'q',
    });
    store = sessionChatReducer(store, {
      type: 'submitResolved',
      sessionId: 'sess-a',
      runId: 'R1',
      taskId: '7',
    });
    store = sessionChatReducer(store, {
      type: 'agentEvent',
      event: evt({ type: 'task_started', task_id: 7, agent_id: 3, session_id: 'sess-a' }),
    });
    store = sessionChatReducer(store, {
      type: 'agentEvent',
      event: evt({
        type: 'activity_changed',
        agent_id: 3,
        activity: 'executing',
        active_skill: 'vox_git_diff',
      }),
    });
    expect(
      getSessionMessages(store, 'sess-a').some((m) => m.role === 'system' && m.text.includes('vox_git_diff')),
    ).toBe(true);
  });

  it('routes snapshot_captured via session_id on the event frame', () => {
    let store = sessionChatReducer(initialSessionChatStore, {
      type: 'submit',
      sessionId: 'sess-b',
      runId: 'R2',
      prompt: 'checkpoint test',
    });
    store = sessionChatReducer(store, {
      type: 'agentEvent',
      event: evt({
        type: 'snapshot_captured',
        agent_id: 9,
        session_id: 'sess-b',
        snapshot_id: 'snap-1',
        file_count: 2,
        description: 'pre-edit',
      }),
    });
    expect(
      getSessionMessages(store, 'sess-b').some((m) => m.text.includes('snap-1')),
    ).toBe(true);
  });

  it('buffers frames that race ahead of submitResolved and replays them (token-loss fix)', () => {
    let store = sessionChatReducer(initialSessionChatStore, {
      type: 'submit',
      sessionId: 'sess-a',
      runId: 'R1',
      prompt: 'q',
    });
    // task_started arrives BEFORE submitResolved and carries no session_id —
    // unroutable at this point.
    store = sessionChatReducer(store, {
      type: 'agentEvent',
      event: evt({ type: 'task_started', task_id: 7, agent_id: 3 }),
    });
    store = sessionChatReducer(store, {
      type: 'agentEvent',
      event: evt({ type: 'token_streamed', agent_id: 3, text: 'Hi' }),
    });
    // Nothing routed yet; both frames are held, not dropped.
    expect(getSessionMessages(store, 'sess-a').find(m => m.role === 'assistant')?.text).toBe('');
    expect(store.pending.length).toBe(2);
    // The submit resolves — buffered frames replay in order.
    store = sessionChatReducer(store, {
      type: 'submitResolved',
      sessionId: 'sess-a',
      runId: 'R1',
      taskId: '7',
    });
    const assistant = getSessionMessages(store, 'sess-a').find(m => m.role === 'assistant');
    expect(assistant?.text).toBe('Hi');
    expect(assistant?.status).toBe('streaming');
    expect(store.pending.length).toBe(0);
  });

  it('evicts buffered frames older than the replay window', () => {
    let store = sessionChatReducer(initialSessionChatStore, {
      type: 'agentEvent',
      event: { id: 1, timestamp_ms: 1_000, kind: { type: 'token_streamed', agent_id: 9, text: 'stale' } },
    });
    // 60s later — the stale frame is outside the 30s window and gets evicted.
    store = sessionChatReducer(store, {
      type: 'agentEvent',
      event: { id: 2, timestamp_ms: 61_000, kind: { type: 'token_streamed', agent_id: 9, text: 'fresh' } },
    });
    expect(store.pending.map(f => f.id)).toEqual([2]);
  });

  it('does not buffer unroutable frame types outside the race (task_completed etc.)', () => {
    const store = sessionChatReducer(initialSessionChatStore, {
      type: 'agentEvent',
      event: evt({ type: 'task_completed', task_id: 999 }),
    });
    expect(store.pending.length).toBe(0);
  });

  it('routes cost_incurred through the agent map and stamps modelId end-to-end', () => {
    let store = sessionChatReducer(initialSessionChatStore, {
      type: 'submit',
      sessionId: 'sess-a',
      runId: 'R1',
      prompt: 'q',
    });
    store = sessionChatReducer(store, {
      type: 'submitResolved',
      sessionId: 'sess-a',
      runId: 'R1',
      taskId: '7',
    });
    store = sessionChatReducer(store, {
      type: 'agentEvent',
      event: evt({ type: 'task_started', task_id: 7, agent_id: 3, session_id: 'sess-a' }),
    });
    store = sessionChatReducer(store, {
      type: 'agentEvent',
      event: evt({ type: 'cost_incurred', agent_id: 3, provider: 'openrouter', model: 'anthropic/claude-opus-4.7' }),
    });
    const assistant = getSessionMessages(store, 'sess-a').find(m => m.role === 'assistant');
    expect(assistant?.modelId).toBe('anthropic/claude-opus-4.7');
  });
});
