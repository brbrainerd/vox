// @vitest-environment jsdom
import { describe, it, expect, vi, afterEach } from 'vitest';
import { act, renderHook } from '@testing-library/react';
import { useLocalStorage } from './useLocalStorage';

describe('useLocalStorage error reporting', () => {
  afterEach(() => vi.restoreAllMocks());

  it('warns (not console.log) and falls back when reading throws', () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const log = vi.spyOn(console, 'log').mockImplementation(() => {});
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('storage disabled');
    });
    const { result } = renderHook(() => useLocalStorage('lk-read', 'fallback'));
    expect(result.current[0]).toBe('fallback');
    expect(warn).toHaveBeenCalled();
    expect(log).not.toHaveBeenCalled();
  });

  it('warns (not console.log) when writing throws', () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const log = vi.spyOn(console, 'log').mockImplementation(() => {});
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('quota exceeded');
    });
    const { result } = renderHook(() => useLocalStorage('lk-write', 'v'));
    act(() => { result.current[1]('next'); });
    expect(warn).toHaveBeenCalled();
    expect(log).not.toHaveBeenCalled();
  });
});
