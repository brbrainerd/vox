// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { toFlow } from './SubAgentGraph';
import type { SubAgentNode } from './types';

const tree: SubAgentNode[] = [{
  windowId: 'w1', parentWindowId: null, title: 'root', skill: null,
  model: { id: 'm', maxTokens: 1, toolCapable: true }, status: 'running', usedTokens: 0, depth: 0,
  children: [{ windowId: 'w2', parentWindowId: 'w1', title: 'child', skill: null,
    model: { id: 'm', maxTokens: 1, toolCapable: true }, status: 'idle', usedTokens: 0, depth: 1, children: [] }],
}];

describe('SubAgentGraph.toFlow', () => {
  it('produces a node per window and an edge per parent link', () => {
    const { nodes, edges } = toFlow(tree);
    expect(nodes.map((n) => n.id).sort()).toEqual(['w1', 'w2']);
    expect(edges).toEqual([{ id: 'w1-w2', source: 'w1', target: 'w2' }]);
  });
});
