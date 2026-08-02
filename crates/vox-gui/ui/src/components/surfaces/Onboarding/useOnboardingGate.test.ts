// @vitest-environment jsdom
import { describe, expect, it, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useOnboardingGate } from './useOnboardingGate';

describe('useOnboardingGate', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('shows the wizard when zero secrets, zero local models, and not dismissed', () => {
    const { result } = renderHook(() => useOnboardingGate({ secretCount: 0, localModelCount: 0 }));
    expect(result.current.shouldShow).toBe(true);
  });

  it('hides the wizard when at least one secret is configured', () => {
    const { result } = renderHook(() => useOnboardingGate({ secretCount: 1, localModelCount: 0 }));
    expect(result.current.shouldShow).toBe(false);
  });

  it('hides the wizard when at least one local model is available', () => {
    const { result } = renderHook(() => useOnboardingGate({ secretCount: 0, localModelCount: 1 }));
    expect(result.current.shouldShow).toBe(false);
  });

  it('hides the wizard after dismiss() is called, and persists across remounts', () => {
    const { result, rerender } = renderHook(() => useOnboardingGate({ secretCount: 0, localModelCount: 0 }));
    expect(result.current.shouldShow).toBe(true);
    act(() => result.current.dismiss());
    rerender();
    expect(result.current.shouldShow).toBe(false);
  });

  it('replay() re-shows the wizard even with zero secrets/models afterward', () => {
    const { result, rerender } = renderHook(() => useOnboardingGate({ secretCount: 0, localModelCount: 0 }));
    act(() => result.current.dismiss());
    rerender();
    expect(result.current.shouldShow).toBe(false);
    act(() => result.current.replay());
    rerender();
    expect(result.current.shouldShow).toBe(true);
  });
});
