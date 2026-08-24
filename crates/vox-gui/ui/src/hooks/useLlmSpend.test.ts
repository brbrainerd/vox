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

describe('useLlmSpend surfaces the whole DTO', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useRealTimers();
  });

  it('returns the daily spend and the daily cap the budget guard enforces', async () => {
    // Regression: the hook projected the payload to { totalUsd } only, so the
    // status bar rendered lifetime spend with no cap — while the cap that
    // actually blocks dispatch is daily_budget_usd vs day_usd.
    mockGetLlmSpend.mockResolvedValue(sampleSpend);
    const { result } = renderHook(() => useLlmSpend());
    await waitFor(() => expect(result.current.dayUsd).toBe(0.5));
    expect(result.current.dailyBudgetUsd).toBe(10);
    expect(result.current.sessionUsd).toBe(0.1);
    expect(result.current.perSessionBudgetUsd).toBe(2);
    expect(result.current.error).toBeNull();
  });

  it('distinguishes a failed fetch from having no data', async () => {
    mockGetLlmSpend.mockRejectedValue(new Error('store unavailable'));
    const { result } = renderHook(() => useLlmSpend());
    // Text comes from sanitizeErrorForToast (the app-wide scrubber), so assert
    // it carries the cause rather than pinning its exact formatting.
    await waitFor(() => expect(result.current.error).toMatch(/store unavailable/));
    expect(result.current.dayUsd).toBeNull();
  });

  it('honours a configured poll cadence instead of the fixed 60s', async () => {
    vi.useFakeTimers();
    mockGetLlmSpend.mockResolvedValue(sampleSpend);
    renderHook(() => useLlmSpend(10_000));
    await act(async () => { await Promise.resolve(); });
    expect(mockGetLlmSpend).toHaveBeenCalledTimes(1);
    await act(async () => { await vi.advanceTimersByTimeAsync(10_000); });
    expect(mockGetLlmSpend).toHaveBeenCalledTimes(2);
  });
});
