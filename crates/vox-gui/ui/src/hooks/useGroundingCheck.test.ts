// @vitest-environment jsdom
import { describe, it, expect, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useGroundingCheck, groundingCheckKey } from './useGroundingCheck';

describe('useGroundingCheck', () => {
  beforeEach(() => localStorage.clear());

  it('defaults to false for a session with no stored preference', () => {
    const { result } = renderHook(() => useGroundingCheck('session-a'));
    expect(result.current[0]).toBe(false);
  });

  it('persists an enabled toggle to localStorage scoped to the session id', () => {
    const { result } = renderHook(() => useGroundingCheck('session-a'));
    act(() => result.current[1](true));
    expect(result.current[0]).toBe(true);
    expect(localStorage.getItem(groundingCheckKey('session-a'))).toBe('true');
  });

  it('scopes the preference independently per session id', () => {
    const a = renderHook(() => useGroundingCheck('session-a'));
    const b = renderHook(() => useGroundingCheck('session-b'));
    act(() => a.result.current[1](true));
    expect(b.result.current[0]).toBe(false);
  });
});
