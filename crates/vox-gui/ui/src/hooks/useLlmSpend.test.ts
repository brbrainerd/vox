// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';
import { useLlmSpend } from './useLlmSpend';

const mockGetLlmSpend = vi.fn();

vi.mock('../transport', () => ({
  voxTransport: {
    getLlmSpend: () => mockGetLlmSpend(),
  },
}));

const sampleSpend = {
  sessionUsd: 0.1,
  dayUsd: 0.5,
  totalUsd: 3.25,
  dailyBudgetUsd: 10,
  perSessionBudgetUsd: 2,
};

describe('useLlmSpend', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useRealTimers();
  });

  it('returns totalUsd from get_llm_spend', async () => {
    mockGetLlmSpend.mockResolvedValue(sampleSpend);

    const { result } = renderHook(() => useLlmSpend());
    await waitFor(() => expect(result.current.totalUsd).toBe(3.25));
    expect(mockGetLlmSpend).toHaveBeenCalledTimes(1);
  });

  it('polls get_llm_spend every 60 seconds', async () => {
    vi.useFakeTimers();
    mockGetLlmSpend.mockResolvedValue({ ...sampleSpend, totalUsd: 1 });

    renderHook(() => useLlmSpend());
    await act(async () => {
      await Promise.resolve();
    });
    expect(mockGetLlmSpend).toHaveBeenCalledTimes(1);

    mockGetLlmSpend.mockResolvedValue({ ...sampleSpend, totalUsd: 2 });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(60_000);
    });
    expect(mockGetLlmSpend).toHaveBeenCalledTimes(2);
  });

  it('returns null totalUsd when get_llm_spend fails', async () => {
    mockGetLlmSpend.mockRejectedValue(new Error('store unavailable'));
    const { result } = renderHook(() => useLlmSpend());
    await waitFor(() => expect(result.current.totalUsd).toBeNull());
  });
});
