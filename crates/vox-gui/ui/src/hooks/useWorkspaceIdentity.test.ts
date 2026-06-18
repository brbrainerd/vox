// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import { useWorkspaceIdentity } from './useWorkspaceIdentity';

const mockGetIdentitySummary = vi.fn();

vi.mock('../transport', () => ({
  voxTransport: {
    getIdentitySummary: () => mockGetIdentitySummary(),
  },
}));

describe('useWorkspaceIdentity', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('defaults workspaceTitle to Operator before identity loads', () => {
    mockGetIdentitySummary.mockReturnValue(new Promise(() => {}));
    const { result } = renderHook(() => useWorkspaceIdentity());
    expect(result.current.workspaceTitle).toBe('Operator');
  });

  it('uses display_name from get_identity_summary when available', async () => {
    mockGetIdentitySummary.mockResolvedValue({ display_name: 'Alice' });
    const { result } = renderHook(() => useWorkspaceIdentity());
    await waitFor(() => expect(result.current.workspaceTitle).toBe('Alice'));
  });
});
