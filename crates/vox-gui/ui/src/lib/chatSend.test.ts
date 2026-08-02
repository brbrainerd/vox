import { describe, it, expect } from 'vitest';
import { parseSendReply } from './chatSend';

describe('parseSendReply', () => {
  it('extracts content and role from a successful ChatMessageDto', () => {
    const dto = { id: 7, role: 'assistant', content: 'Hello!', created_at: '2026-07-31T00:00:00Z', task_id: null, model_id: 'openrouter/auto' };
    const parsed = parseSendReply(dto);
    expect(parsed).toEqual({ id: '7', role: 'assistant', text: 'Hello!', modelId: 'openrouter/auto', createdAt: '2026-07-31T00:00:00Z' });
  });

  it('returns undefined modelId when absent', () => {
    const dto = { id: 8, role: 'assistant', content: 'Hi', created_at: '2026-07-31T00:00:01Z', task_id: null };
    const parsed = parseSendReply(dto);
    expect(parsed.modelId).toBeUndefined();
  });

  it('carries latencyMs through from dto.latency_ms', () => {
    const dto = { id: 9, role: 'assistant', content: 'Hi', created_at: '2026-07-31T00:00:02Z', task_id: null, latency_ms: 842 };
    const parsed = parseSendReply(dto);
    expect(parsed.latencyMs).toBe(842);
  });

  it('returns undefined latencyMs when absent', () => {
    const dto = { id: 10, role: 'assistant', content: 'Hi', created_at: '2026-07-31T00:00:03Z', task_id: null };
    const parsed = parseSendReply(dto);
    expect(parsed.latencyMs).toBeUndefined();
  });

  it('carries selectionReason through from dto.selection_reason', () => {
    const dto = { id: 11, role: 'assistant', content: 'Hi', created_at: '2026-07-31T00:00:04Z', task_id: null, selection_reason: 'Chosen by the model scorer' };
    const parsed = parseSendReply(dto);
    expect(parsed.selectionReason).toBe('Chosen by the model scorer');
  });

  it('returns undefined selectionReason when absent', () => {
    const dto = { id: 12, role: 'assistant', content: 'Hi', created_at: '2026-07-31T00:00:05Z', task_id: null };
    const parsed = parseSendReply(dto);
    expect(parsed.selectionReason).toBeUndefined();
  });
});
