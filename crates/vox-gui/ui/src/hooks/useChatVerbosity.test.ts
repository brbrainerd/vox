// @vitest-environment jsdom
import { describe, it, expect, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useChatVerbosity, CHAT_VERBOSITY_KEY } from './useChatVerbosity';

describe('useChatVerbosity', () => {
  beforeEach(() => localStorage.clear());

  it('defaults to normal', () => {
    const { result } = renderHook(() => useChatVerbosity());
    expect(result.current[0]).toBe('normal');
  });

  it('persists a changed level to localStorage', () => {
    const { result } = renderHook(() => useChatVerbosity());
    act(() => result.current[1]('verbose'));
    expect(result.current[0]).toBe('verbose');
    expect(localStorage.getItem(CHAT_VERBOSITY_KEY)).toBe('"verbose"');
  });
});
