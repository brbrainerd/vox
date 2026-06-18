import { describe, it, expect } from 'vitest';
import {
  buildTranscriptTimeline,
  isTokenStreamEvent,
} from './chatTranscriptTimeline';
import type { ChatMessage } from './chatCorrelation';
import type { StreamItem } from '../types/dashboard';

function agentItem(
  id: string,
  eventType: string,
  timestampMs: number,
  extra: Partial<StreamItem> = {},
): StreamItem {
  return {
    id,
    kind: 'agent',
    tag: eventType === 'token_streamed' ? 'TOKEN' : 'TASK',
    title: eventType,
    body: '',
    ts: '12:00',
    metadata: { eventType, timestampMs, agentId: 'agent-1' },
    ...extra,
  };
}

describe('isTokenStreamEvent', () => {
  it('returns true for token_streamed metadata', () => {
    expect(isTokenStreamEvent(agentItem('1', 'token_streamed', 100))).toBe(true);
  });

  it('returns false for task_started', () => {
    expect(isTokenStreamEvent(agentItem('2', 'task_started', 200))).toBe(false);
  });
});

describe('buildTranscriptTimeline', () => {
  const messages: ChatMessage[] = [
    {
      id: 'm1',
      role: 'user',
      text: 'hello',
      status: 'done',
      runId: 'r1',
    },
    {
      id: 'm2',
      role: 'assistant',
      text: 'hi there',
      status: 'done',
      runId: 'r1',
    },
  ];

  it('merges chat messages and agent items sorted by timestamp', () => {
    const agentItems: StreamItem[] = [
      agentItem('a1', 'task_started', 1500),
      agentItem('a2', 'task_completed', 2500),
    ];

    const rows = buildTranscriptTimeline(messages, agentItems, { messageStepMs: 1000 });

    expect(rows.map((r) => r.kind)).toEqual([
      'message',
      'message',
      'agent',
      'agent',
    ]);
    expect(rows.map((r) => r.atMs)).toEqual([0, 1000, 1500, 2500]);
  });

  it('groups consecutive token_streamed events collapsed by default', () => {
    const agentItems: StreamItem[] = [
      agentItem('t1', 'token_streamed', 500, { body: 'hel' }),
      agentItem('t2', 'token_streamed', 600, { body: 'lo' }),
      agentItem('t3', 'task_started', 700),
    ];

    const rows = buildTranscriptTimeline([], agentItems);

    const tokenRow = rows.find((r) => r.kind === 'token_group');
    expect(tokenRow).toBeDefined();
    expect(tokenRow?.kind).toBe('token_group');
    if (tokenRow?.kind === 'token_group') {
      expect(tokenRow.items).toHaveLength(2);
      expect(tokenRow.collapsed).toBe(true);
    }

    const taskRow = rows.find((r) => r.kind === 'agent');
    expect(taskRow?.kind).toBe('agent');
    if (taskRow?.kind === 'agent') {
      expect(taskRow.collapsed).toBe(false);
    }
  });
});
