// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useAchievementToasts } from './useAchievementToasts';
import type { GuiEventResultDto } from '../lib/gamifyGuiEvents';

describe('useAchievementToasts', () => {
  beforeEach(() => {
    vi.useRealTimers();
  });

  it('queues one toast when a GUI event grants XP', () => {
    const { result } = renderHook(() => useAchievementToasts(true, 'balanced'));

    act(() => {
      result.current.handleGuiEventResult({
        xpGranted: 5,
        lumensGranted: 0,
        achievementTitle: 'XP',
      } satisfies GuiEventResultDto);
    });

    expect(result.current.toasts).toHaveLength(1);
    expect(result.current.toasts[0]?.title).toBe('XP');
    expect(result.current.toasts[0]?.body).toMatch(/\+5 XP/);
  });

  it('does not queue toasts when gamifyMode is serious', () => {
    const { result } = renderHook(() => useAchievementToasts(true, 'serious'));

    act(() => {
      result.current.handleGuiEventResult({
        xpGranted: 5,
        lumensGranted: 0,
        achievementTitle: 'XP',
      });
    });

    expect(result.current.toasts).toHaveLength(0);
  });

  it('does not queue toasts when gamify is disabled', () => {
    const { result } = renderHook(() => useAchievementToasts(false, 'balanced'));

    act(() => {
      result.current.handleGuiEventResult({
        xpGranted: 5,
        lumensGranted: 0,
        achievementTitle: 'XP',
      });
    });

    expect(result.current.toasts).toHaveLength(0);
  });

  it('dismissToast removes a toast by id', () => {
    const { result } = renderHook(() => useAchievementToasts(true, 'balanced'));

    act(() => {
      result.current.handleGuiEventResult({
        xpGranted: 2,
        lumensGranted: 0,
        achievementTitle: 'XP',
      });
    });

    const id = result.current.toasts[0]?.id;
    expect(id).toBeTruthy();

    act(() => {
      result.current.dismissToast(id!);
    });

    expect(result.current.toasts).toHaveLength(0);
  });
});
