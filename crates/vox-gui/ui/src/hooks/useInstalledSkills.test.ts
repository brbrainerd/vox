// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import { useInstalledSkills } from './useInstalledSkills';

const mockInvokeMcpTool = vi.fn();

vi.mock('../transport', () => ({
  voxTransport: {
    invokeMcpTool: (...args: unknown[]) => mockInvokeMcpTool(...args),
  },
}));

describe('useInstalledSkills', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('maps vox_skill_list rows to slash-ready skill records', async () => {
    mockInvokeMcpTool.mockResolvedValue({
      is_error: false,
      result: {
        success: true,
        data: [
          { id: 'vox.tdd', name: 'test-driven-development', description: 'RED-GREEN-REFACTOR' },
        ],
      },
    });

    const { result } = renderHook(() => useInstalledSkills(true));
    await waitFor(() => expect(result.current.length).toBe(1));
    expect(result.current[0]).toMatchObject({
      id: 'vox.tdd',
      name: 'test-driven-development',
      description: 'RED-GREEN-REFACTOR',
    });
    expect(mockInvokeMcpTool).toHaveBeenCalledWith('vox_skill_list', {});
  });

  it('returns empty list when MCP call fails', async () => {
    mockInvokeMcpTool.mockRejectedValue(new Error('daemon offline'));
    const { result } = renderHook(() => useInstalledSkills(true));
    await waitFor(() => expect(mockInvokeMcpTool).toHaveBeenCalled());
    expect(result.current).toEqual([]);
  });
});
