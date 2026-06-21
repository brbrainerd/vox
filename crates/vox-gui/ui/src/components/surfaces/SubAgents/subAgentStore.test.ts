import { describe, it, expect, beforeEach } from 'vitest';
import { useSubAgentStore } from './subAgentStore';
import type { SubAgentNode } from './types';

const node = (id: string, depth = 0, children: SubAgentNode[] = []): SubAgentNode => ({
  windowId: id, parentWindowId: null, title: id, skill: null,
  model: { id: 'm', maxTokens: 100, toolCapable: true }, status: 'running', usedTokens: 0, depth, children,
});

describe('subAgentStore', () => {
  beforeEach(() => useSubAgentStore.getState().reset());
  it('setTree stores nodes and toggleExpand flips a node', () => {
    useSubAgentStore.getState().setTree([node('w1', 0, [node('w2', 1)])]);
    expect(useSubAgentStore.getState().tree.length).toBe(1);
    useSubAgentStore.getState().toggleExpand('w1');
    expect(useSubAgentStore.getState().expanded.has('w1')).toBe(true);
  });
  it('pushEvent appends to the selected window event log capped at 200', () => {
    const s = useSubAgentStore.getState();
    s.select('w1');
    for (let i = 0; i < 250; i++) s.pushEvent('w1', { id: i, timestamp_ms: i, kind: { type: 'token_streamed' } });
    expect(useSubAgentStore.getState().eventsByWindow['w1'].length).toBe(200);
  });
});
