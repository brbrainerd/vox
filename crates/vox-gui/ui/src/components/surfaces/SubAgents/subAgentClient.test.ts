import { describe, it, expect, vi, beforeEach } from 'vitest';
const invokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invokeMock(...a) }));
import { fetchTree, getContext, setContext, control } from './subAgentClient';

describe('subAgentClient', () => {
  beforeEach(() => invokeMock.mockReset());
  it('fetchTree calls subagent_tree and returns nodes', async () => {
    invokeMock.mockResolvedValue({ is_error: false, result: { nodes: [{ windowId: 'w1', parentWindowId: null, title: 't', skill: null, model: { id: 'm', maxTokens: 1, toolCapable: false }, status: 'idle', usedTokens: 0, depth: 0, children: [] }] } });
    const nodes = await fetchTree();
    expect(invokeMock).toHaveBeenCalledWith('subagent_tree', {});
    expect(nodes[0].windowId).toBe('w1');
  });
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
});
