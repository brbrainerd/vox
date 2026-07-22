// @vitest-environment jsdom
import { describe, it, expect, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useActiveView } from './useActiveView';

describe('useActiveView', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('defaults to dashboard when nothing is stored', () => {
    const { result } = renderHook(() => useActiveView());
    expect(result.current.activeView).toBe('dashboard');
  });

  it('navigateTo resolves a parent key to its default child and stores it', () => {
    const { result } = renderHook(() => useActiveView());
    act(() => {
      result.current.navigateTo('agents');
    });
    // 'agents' parent's default child per DEFAULT_CHILD_BY_PARENT in navigation.ts
    expect(result.current.activeView).not.toBe('agents');
    expect(result.current.activeView).toBe('dashboard');
  });

  it('navigateTo a leaf child view navigates directly to it', () => {
    const { result } = renderHook(() => useActiveView());
    act(() => {
      result.current.navigateTo('flow');
    });
    expect(result.current.activeView).toBe('flow');
  });

  it('persists the active view across remounts via localStorage', () => {
    const { result, unmount } = renderHook(() => useActiveView());
    act(() => {
      result.current.navigateTo('console');
    });
    unmount();
    const { result: result2 } = renderHook(() => useActiveView());
    expect(result2.current.activeView).toBe('console');
  });

  it('migrates forward from the old vox_workbench_tabs.v1 activeTab on first read', () => {
    localStorage.setItem(
      'vox_workbench_tabs.v1',
      JSON.stringify({ openTabs: ['chat', 'repository'], activeTab: 'repository' }),
    );
    const { result } = renderHook(() => useActiveView());
    expect(result.current.activeView).toBe('repository');
  });

  it('ignores the old key once its own key has ever been written', () => {
    localStorage.setItem(
      'vox_workbench_tabs.v1',
      JSON.stringify({ openTabs: ['chat', 'repository'], activeTab: 'repository' }),
    );
    const { result, unmount } = renderHook(() => useActiveView());
    act(() => {
      result.current.navigateTo('models'); // writes its own key
    });
    unmount();
    // Change the old key after migration already happened once — should have no further effect.
    localStorage.setItem(
      'vox_workbench_tabs.v1',
      JSON.stringify({ openTabs: ['chat', 'settings'], activeTab: 'settings' }),
    );
    const { result: result2 } = renderHook(() => useActiveView());
    expect(result2.current.activeView).toBe('models');
  });
});
