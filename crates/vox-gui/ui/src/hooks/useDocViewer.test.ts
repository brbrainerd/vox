// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useDocViewer } from './useDocViewer';

describe('useDocViewer', () => {
  it('starts closed with no active doc', () => {
    const { result } = renderHook(() => useDocViewer());
    expect(result.current.activeDoc).toBeNull();
  });

  it('openDoc opens the drawer with the given path and optional title', () => {
    const { result } = renderHook(() => useDocViewer());
    act(() => {
      result.current.openDoc('docs/foo.md', 'Foo');
    });
    expect(result.current.activeDoc).toEqual({ path: 'docs/foo.md', title: 'Foo' });
  });

  it('opening a second doc while one is open replaces it, not stacks', () => {
    const { result } = renderHook(() => useDocViewer());
    act(() => {
      result.current.openDoc('docs/foo.md', 'Foo');
      result.current.openDoc('docs/bar.md', 'Bar');
    });
    expect(result.current.activeDoc).toEqual({ path: 'docs/bar.md', title: 'Bar' });
  });

  it('closeDoc clears the active doc', () => {
    const { result } = renderHook(() => useDocViewer());
    act(() => {
      result.current.openDoc('docs/foo.md');
      result.current.closeDoc();
    });
    expect(result.current.activeDoc).toBeNull();
  });

  it('openDoc without a title falls back to the filename', () => {
    const { result } = renderHook(() => useDocViewer());
    act(() => {
      result.current.openDoc('docs/some-guide.md');
    });
    expect(result.current.activeDoc?.title).toBe('some-guide');
  });
});
