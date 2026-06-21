// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';

const mockGetMemoryStatus = vi.fn();
vi.mock('../transport', () => ({
  voxTransport: {
    getMemoryStatus: () => mockGetMemoryStatus(),
  },
}));

import { useMemoryStatus } from './useMemoryStatus';

beforeEach(() => {
  vi.clearAllMocks();
});

describe('useMemoryStatus', () => {
  it('exposes the vector-store (proj) corpus count', async () => {
    mockGetMemoryStatus.mockResolvedValue({ corpus_counts: { proj: 12400, docs: 30 }, shards: [], recent_recalls: [], embedding_dim: 1024 });
    const { result } = renderHook(() => useMemoryStatus());
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.vectorCount).toBe(12400);
    expect(result.current.error).toBeNull();
  });
  it('reports error and null count when the command rejects', async () => {
    mockGetMemoryStatus.mockRejectedValue(new Error('No workspace db found'));
    const { result } = renderHook(() => useMemoryStatus());
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.vectorCount).toBeNull();
    expect(result.current.error).toBe('No workspace db found');
  });
});
