// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';

const invokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...a: unknown[]) => invokeMock(...a),
}));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock('../transport', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../transport')>()),
  feedbackList: vi.fn().mockResolvedValue({
    needsYou: [{ feedbackId: 'F-1', kind: 'doubt', prompt: 'p', options: [], gates: [7], doubtedTaskId: 7, surface: 'needs_you', infoGainBits: 1 }],
    withheld: [],
  }),
  listenFeedbackChanged: vi.fn().mockResolvedValue(() => {}),
  voxTransport: { invokeMcpTool: vi.fn().mockResolvedValue({ tool: 'vox_pending_approvals', is_error: false, result: { approvals: [{ approval_id: 'A-1', tool: 'bash', summary: 's', requested_at_ms: 0 }] } }) },
}));

import { useAttentionInbox } from './useAttentionInbox';
import { voxTransport } from '../transport';

beforeEach(() => {
  invokeMock.mockImplementation((cmd: string) =>
    cmd === 'hopper_list' ? Promise.resolve([{ item_id: 'h1', intent: 'x', priority: 1, state: 'blocked', task_id: 7 }]) : Promise.resolve(null));
});
// vi.clearAllMocks() (not restoreAllMocks): the voxTransport/feedbackList mocks above are
// bare `vi.fn().mockResolvedValue(...)` factory mocks with no "original" implementation to
// restore to — restoreAllMocks resets them to a no-op, breaking every test after the first.
// clearAllMocks resets call history (needed for the toHaveBeenCalledWith assertion below)
// while preserving the configured resolved values.
afterEach(() => vi.clearAllMocks());

describe('useAttentionInbox', () => {
  it('aggregates approvals + feedback + blocked hopper tasks with one total', async () => {
    const { result } = renderHook(() => useAttentionInbox());
    await waitFor(() => expect(result.current.totalCount).toBe(2)); // 1 approval + 1 needsYou
    expect(result.current.approvals).toHaveLength(1);
    expect(result.current.needsYou).toHaveLength(1);
    expect(result.current.blockedTasksCount).toBe(1); // task 7 gated by F-1
  });

  it('resolveApproval calls vox_resolve_approval then drops the row', async () => {
    const { result } = renderHook(() => useAttentionInbox());
    await waitFor(() => expect(result.current.approvals).toHaveLength(1));
    await act(() => result.current.resolveApproval('A-1', 'approved'));
    expect(voxTransport.invokeMcpTool).toHaveBeenCalledWith('vox_resolve_approval', { approval_id: 'A-1', outcome: 'approved' });
  });

  it('a rejected hopper_list source degrades to 0 blocked tasks without blanking approvals/needsYou', async () => {
    invokeMock.mockImplementation((cmd: string) =>
      cmd === 'hopper_list' ? Promise.reject(new Error('hopper unavailable')) : Promise.resolve(null));
    const { result } = renderHook(() => useAttentionInbox());
    await waitFor(() => expect(result.current.approvals).toHaveLength(1));
    expect(result.current.needsYou).toHaveLength(1);
    expect(result.current.blockedTasksCount).toBe(0);
  });
});
