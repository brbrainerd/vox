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

  it('resolveSessionForEvent prefers taskToSession', () => {
    const store = {
      sessions: {},
      taskToSession: { '42': 'sess-x' },
    };
    expect(
      resolveSessionForEvent(store, evt({ type: 'task_completed', task_id: 42, agent_id: 1 })),
    ).toBe('sess-x');
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
});
