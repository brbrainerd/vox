import { describe, it, expect } from 'vitest';
import {
  chatReducer,
  initialChatState,
  messagesForSession,
  type ChatState,
} from './chatCorrelation';

// Build an agent-event frame as delivered over `vox://agent-events`.
const evt = (kind: Record<string, unknown>) => ({
  type: 'agentEvent' as const,
  event: { id: 1, timestamp_ms: 0, kind: kind as { type: string; [k: string]: unknown } },
});

const assistant = (s: ChatState, runId: string) =>
  s.messages.find((m) => m.role === 'assistant' && m.runId === runId);

describe('messagesForSession', () => {
  it('stamps sessionId on both bubbles and filters by session (legacy nulls visible)', () => {
    let s = chatReducer(initialChatState, { type: 'submit', runId: 'r1', prompt: 'p', sessionId: 'gui-a' });
    s = chatReducer(s, { type: 'submit', runId: 'r2', prompt: 'q', sessionId: 'gui-b' });
    const a = messagesForSession(s, 'gui-a');
    expect(a).toHaveLength(2); // user + pending assistant
    expect(a.every((m) => m.sessionId === 'gui-a')).toBe(true);
    expect(messagesForSession(s, 'gui-b')).toHaveLength(2);
  });
});

describe('chatReducer', () => {
  it('submit adds a user message and a pending assistant bubble for the run', () => {
    const s = chatReducer(initialChatState, { type: 'submit', runId: 'R1', prompt: 'hi there' });
    const user = s.messages.find((m) => m.role === 'user' && m.runId === 'R1');
    expect(user?.text).toBe('hi there');
    expect(assistant(s, 'R1')).toMatchObject({ text: '', status: 'pending' });
  });

  it('routes streamed tokens to the assistant bubble via TaskStarted + submit binding', () => {
    let s = chatReducer(initialChatState, { type: 'submit', runId: 'R1', prompt: 'hi' });
    s = chatReducer(s, { type: 'submitResolved', runId: 'R1', taskId: '7' });
    // task_started establishes agent 3 <-> task 7; tokens carry only agent_id.
    s = chatReducer(s, evt({ type: 'task_started', task_id: 7, agent_id: 3 }));
    s = chatReducer(s, evt({ type: 'token_streamed', agent_id: 3, text: 'Hel' }));
    s = chatReducer(s, evt({ type: 'token_streamed', agent_id: 3, text: 'lo' }));
    expect(assistant(s, 'R1')).toMatchObject({ text: 'Hello', status: 'streaming', taskId: '7' });
  });

  it('normalizes numeric event task_id against the string submit task_id', () => {
    let s = chatReducer(initialChatState, { type: 'submit', runId: 'R1', prompt: 'q' });
    s = chatReducer(s, { type: 'submitResolved', runId: 'R1', taskId: '42' });
    s = chatReducer(s, evt({ type: 'task_completed', task_id: 42, agent_id: 9 }));
    expect(assistant(s, 'R1')?.status).toBe('done');
  });

  it('task_failed marks the bubble failed with the error text', () => {
    let s = chatReducer(initialChatState, { type: 'submit', runId: 'R1', prompt: 'q' });
    s = chatReducer(s, { type: 'submitResolved', runId: 'R1', taskId: '5' });
    s = chatReducer(s, evt({ type: 'task_failed', task_id: 5, agent_id: 1, error: 'boom' }));
    expect(assistant(s, 'R1')).toMatchObject({ status: 'failed', error: 'boom' });
  });

  it('ignores a token whose agent has no known task mapping', () => {
    const s = chatReducer(initialChatState, evt({ type: 'token_streamed', agent_id: 99, text: 'x' }));
    expect(s.messages).toHaveLength(0);
  });
});
