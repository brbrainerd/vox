import { describe, it, expect, vi, beforeEach } from 'vitest';
const invokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invokeMock(...a) }));
import { fetchTree, buildSubAgentTree, getContext, setContext, control } from './subAgentClient';

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

describe('buildSubAgentTree', () => {
  it('reports unknown token budgets as 0/0, never fabricated numbers', () => {
    const [root] = buildSubAgentTree([{ task_id: 1, agent_id: 7, parent_agent_id: null, reason: 'x' }]);
    expect(root.model.maxTokens).toBe(0);
    expect(root.usedTokens).toBe(0);
    expect(root.title).toContain('#1');
  });
});

describe('subAgentClient (unrelated, untouched in Task 2)', () => {
  it('setContext sends ordered item ids to context_set', async () => {
    invokeMock.mockResolvedValue({ is_error: false, result: {} });
    await setContext('w2', ['i1', 'i2']);
    expect(invokeMock).toHaveBeenCalledWith('context_set', { windowId: 'w2', orderedItemIds: ['i1', 'i2'] });
  });
  it('control forwards a typed action to subagent_control', async () => {
    invokeMock.mockResolvedValue({ is_error: false, result: {} });
    await control('w2', { kind: 'pause' });
    expect(invokeMock).toHaveBeenCalledWith('subagent_control', { windowId: 'w2', action: { kind: 'pause' } });
  });
  it('getContext calls context_get and returns items', async () => {
    invokeMock.mockResolvedValue({ is_error: false, result: { items: [] } });
    const items = await getContext('w1');
    expect(invokeMock).toHaveBeenCalledWith('context_get', { windowId: 'w1' });
    expect(items).toEqual([]);
  });
});
