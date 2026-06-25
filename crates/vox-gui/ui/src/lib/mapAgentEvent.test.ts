import { describe, it, expect } from 'vitest';
import { mapAgentEvent } from './mapAgentEvent';
import type { AgentEventFrame } from './chatCorrelation';

describe('mapAgentEvent', () => {
  it('maps task_started to agent stream item with metadata', () => {
    const frame: AgentEventFrame = {
      id: 42,
      timestamp_ms: 1_700_000_000_000,
      kind: { type: 'task_started', task_id: 7, agent_id: 'agent-1' },
    };
    const item = mapAgentEvent(frame);
    expect(item.kind).toBe('agent');
    expect(item.tag).toBe('TASK');
    expect(item.metadata?.eventType).toBe('task_started');
    expect(item.metadata?.agentId).toBe('agent-1');
    expect(item.metadata?.timestampMs).toBe(1_700_000_000_000);
  });

  it('maps token_streamed with TOKEN tag and agent body', () => {
    const frame: AgentEventFrame = {
      id: 99,
      timestamp_ms: 1_700_000_001_000,
      kind: { type: 'token_streamed', agent_id: 'agent-2', text: 'hello' },
    };
    const item = mapAgentEvent(frame);
    expect(item.tag).toBe('TOKEN');
    expect(item.body).toBe('hello');
    expect(item.metadata?.eventType).toBe('token_streamed');
    expect(item.metadata?.agentId).toBe('agent-2');
  });

  it('maps task_failed to doubted kind', () => {
    const frame: AgentEventFrame = {
      id: 3,
      timestamp_ms: 0,
      kind: { type: 'task_failed', task_id: 1, error: 'boom' },
    };
    const item = mapAgentEvent(frame);
    expect(item.kind).toBe('doubted');
    expect(item.tag).toBe('FAILED');
    expect(item.body).toContain('boom');
  });

  it('carries numeric taskId on task events', () => {
    const frame: AgentEventFrame = {
      id: 1,
      timestamp_ms: 0,
      kind: { type: 'task_started', task_id: 42, agent_id: 'a' },
    };
    const item = mapAgentEvent(frame);
    expect(item.taskId).toBe(42);
  });

  it('leaves taskId undefined for non-task events', () => {
    const frame: AgentEventFrame = {
      id: 2,
      timestamp_ms: 0,
      kind: { type: 'agent_spawned', agent_id: 'a' },
    };
    const item = mapAgentEvent(frame);
    expect(item.taskId).toBeUndefined();
  });
});
