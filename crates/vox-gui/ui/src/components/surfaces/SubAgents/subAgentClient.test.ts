import { describe, it, expect, vi, beforeEach } from 'vitest';
const invokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invokeMock(...a) }));
import { fetchTree, fetchEdges, buildSubAgentTree } from './subAgentClient';

beforeEach(() => invokeMock.mockReset());

describe('fetchTree', () => {
  it('invokes the real list_subagent_tree command (flat edge list, no envelope)', async () => {
    invokeMock.mockResolvedValue([
      { task_id: 10, agent_id: 1, parent_agent_id: null, reason: 'root plan' },
      { task_id: 11, agent_id: 2, parent_agent_id: 1, reason: 'search docs' },
    ]);
    const tree = await fetchTree();
    expect(invokeMock).toHaveBeenCalledWith('list_subagent_tree');
    expect(tree).toHaveLength(1);
    expect(tree[0].windowId).toBe('agent-1');
    expect(tree[0].children[0].windowId).toBe('agent-2');
    expect(tree[0].children[0].depth).toBe(1);
  });
  it('returns [] on a non-array payload', async () => {
    invokeMock.mockResolvedValue(null);
    expect(await fetchTree()).toEqual([]);
  });
});

describe('fetchEdges', () => {
  // Phase D Task D3: `fetchTree()` discards `chat_session_id`/`origin_turn_id`
  // while flattening edges into the windowed `SubAgentNode[]` tree — a
  // correlation caller (e.g. a future `agentsForSession`) needs the raw edges,
  // not the tree. This is the sibling accessor for that; `fetchTree()` must
  // still work unchanged (built on top of it below).
  it('returns the raw edges, including chat_session_id/origin_turn_id', async () => {
    invokeMock.mockResolvedValue([
      {
        task_id: 10,
        agent_id: 1,
        parent_agent_id: null,
        reason: 'root plan',
        chat_session_id: 'chat-session-abc',
        origin_turn_id: 'call_1',
      },
    ]);
    const edges = await fetchEdges();
    expect(invokeMock).toHaveBeenCalledWith('list_subagent_tree');
    expect(edges).toEqual([
      {
        task_id: 10,
        agent_id: 1,
        parent_agent_id: null,
        reason: 'root plan',
        chat_session_id: 'chat-session-abc',
        origin_turn_id: 'call_1',
      },
    ]);
  });

  it('returns [] on a non-array payload, same as fetchTree', async () => {
    invokeMock.mockResolvedValue(null);
    expect(await fetchEdges()).toEqual([]);
  });
});

describe('buildSubAgentTree', () => {
  it('reports unknown token budgets as 0/0, never fabricated numbers', () => {
    const [root] = buildSubAgentTree([{ task_id: 1, agent_id: 7, parent_agent_id: null, reason: 'x' }]);
    expect(root.model.maxTokens).toBe(0);
    expect(root.usedTokens).toBe(0);
    expect(root.title).toContain('#1');
  });
});
