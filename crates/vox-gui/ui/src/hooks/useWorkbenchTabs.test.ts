// @vitest-environment jsdom
import { describe, it, expect, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useWorkbenchTabs } from './useWorkbenchTabs';

const STORAGE_KEY = 'vox_workbench_tabs.v1';

describe('useWorkbenchTabs', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('openTab adds a leaf tab and focuses it', () => {
    const { result } = renderHook(() => useWorkbenchTabs());
    act(() => result.current.openTab('console'));
    expect(result.current.openTabs).toContain('console');
    expect(result.current.activeTab).toBe('console');
  });

  it('openTab focuses existing tab without duplicate', () => {
    const { result } = renderHook(() => useWorkbenchTabs());
    act(() => {
      result.current.openTab('chat');
      result.current.openTab('console');
      result.current.openTab('chat');
    });
    expect(result.current.openTabs.filter((t) => t === 'chat').length).toBe(1);
    expect(result.current.openTabs).toContain('console');
    expect(result.current.activeTab).toBe('chat');
  });

  it('closeTab removes tab and focuses neighbor', () => {
    localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({ openTabs: ['console', 'chat'], activeTab: 'console' }),
    );
    const { result } = renderHook(() => useWorkbenchTabs());
    act(() => result.current.closeTab('console'));
    expect(result.current.openTabs).toEqual(['chat']);
    expect(result.current.activeTab).toBe('chat');
  });

  it('closeTab on pinned chat is a no-op', () => {
    localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({ openTabs: ['chat', 'dashboard'], activeTab: 'dashboard' }),
    );
    const { result } = renderHook(() => useWorkbenchTabs());
    act(() => result.current.closeTab('chat'));
    expect(result.current.openTabs).toEqual(['chat', 'dashboard']);
    expect(result.current.activeTab).toBe('dashboard');
  });

  it('closeTab on last closable tab restores pinned defaults', () => {
    localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({ openTabs: ['console'], activeTab: 'console' }),
    );
    const { result } = renderHook(() => useWorkbenchTabs());
    act(() => result.current.closeTab('console'));
    expect(result.current.activeTab).toBe('dashboard');
    expect(result.current.openTabs).toEqual(['chat', 'dashboard']);
  });

  it('doc tab ids are stable and titles persist', () => {
    const { result } = renderHook(() => useWorkbenchTabs());
    act(() => result.current.openDocTab('docs/src/reference/cli.md', 'CLI Reference'));
    expect(result.current.activeTab).toBe('doc:docs/src/reference/cli.md');
    expect(result.current.docLabels['doc:docs/src/reference/cli.md']).toBe('CLI Reference');
  });

  it('migrates legacy vox_active_view on first load', () => {
    localStorage.setItem('vox_active_view', JSON.stringify('flow'));
    const { result } = renderHook(() => useWorkbenchTabs());
    expect(result.current.openTabs).toContain('flow');
    expect(result.current.activeTab).toBe('flow');
  });
});
