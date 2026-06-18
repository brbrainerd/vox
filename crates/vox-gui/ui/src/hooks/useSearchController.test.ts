// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useSearchController } from './useSearchController';

vi.mock('../transport', () => ({
  voxTransport: {
    voxSearchQuery: vi.fn().mockResolvedValue({ hits: [{ title: 'hit-1' }] }),
  },
}));

import { voxTransport } from '../transport';

describe('useSearchController', () => {
  beforeEach(() => {
    vi.mocked(voxTransport.voxSearchQuery).mockClear();
  });

  it('debounces search and returns hits', async () => {
    vi.useFakeTimers();
    const { result } = renderHook(() => useSearchController({ debounceMs: 100 }));

    act(() => {
      result.current.setQuery('vox check');
    });

    await act(async () => {
      await vi.advanceTimersByTimeAsync(100);
    });

    expect(voxTransport.voxSearchQuery).toHaveBeenCalledWith('vox check', 30, expect.any(Array));
    expect(result.current.state.hits.length).toBe(1);
    vi.useRealTimers();
  });

  it('clears hits when query empty', async () => {
    vi.useFakeTimers();
    const { result } = renderHook(() => useSearchController({ debounceMs: 50 }));

    act(() => {
      result.current.setQuery('   ');
    });

    await act(async () => {
      await vi.advanceTimersByTimeAsync(50);
    });

    expect(result.current.state.hits).toEqual([]);
    expect(voxTransport.voxSearchQuery).not.toHaveBeenCalled();
    vi.useRealTimers();
  });
});
