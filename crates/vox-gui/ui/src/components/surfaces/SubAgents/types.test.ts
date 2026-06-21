import { describe, it, expect } from 'vitest';
import { flattenTree, tokenFate, type SubAgentNode } from './types';

const tree: SubAgentNode[] = [
  { windowId: 'w1', parentWindowId: null, title: 'root', skill: 'plan', model: { id: 'sonnet', maxTokens: 200000, toolCapable: true }, status: 'running', usedTokens: 1000, depth: 0,
    children: [
      { windowId: 'w2', parentWindowId: 'w1', title: 'child', skill: 'search', model: { id: 'haiku', maxTokens: 8000, toolCapable: true }, status: 'idle', usedTokens: 7000, depth: 1, children: [] },
    ] },
];

describe('SubAgents types', () => {
  it('flattenTree yields depth-ordered rows for virtualization', () => {
    const rows = flattenTree(tree, new Set(['w1']));
    expect(rows.map((r) => r.windowId)).toEqual(['w1', 'w2']);
    expect(rows[1].depth).toBe(1);
  });
  it('flattenTree hides children of collapsed nodes', () => {
    expect(flattenTree(tree, new Set()).map((r) => r.windowId)).toEqual(['w1']);
  });
  it('tokenFate flags a node over its model budget', () => {
    expect(tokenFate(7000, 8000)).toBe('warn');
    expect(tokenFate(8200, 8000)).toBe('over');
    expect(tokenFate(100, 8000)).toBe('ok');
  });
});
