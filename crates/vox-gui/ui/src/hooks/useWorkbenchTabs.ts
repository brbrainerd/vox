import { useCallback, useMemo } from 'react';
import { useLocalStorage } from './useLocalStorage';
import { DEFAULT_CHILD_BY_PARENT, LEGACY_VIEW_ALIASES } from '../lib/navigation';

export type TabId = string;

const STORAGE_KEY = 'vox_workbench_tabs.v1';
const LEGACY_VIEW_KEY = 'vox_active_view';
const FALLBACK_TAB = 'dashboard';
export const PINNED_TABS: TabId[] = ['chat'];

export function isPinnedTab(id: TabId): boolean {
  return PINNED_TABS.includes(id);
}

export interface StoredWorkbenchState {
  openTabs: TabId[];
  activeTab: TabId | null;
}

function normalizeViewKey(key: string): string {
  return LEGACY_VIEW_ALIASES[key] ?? key;
}

function docTabId(path: string): TabId {
  return `doc:${path.replace(/\\/g, '/')}`;
}

export function isDocTab(id: TabId): boolean {
  return id.startsWith('doc:');
}

export function docPathFromTab(id: TabId): string {
  return id.slice('doc:'.length);
}

function migrateLegacyView(): StoredWorkbenchState | null {
  try {
    const legacy = localStorage.getItem(LEGACY_VIEW_KEY);
    if (!legacy) return null;
    const parsed = JSON.parse(legacy) as string;
    const view = normalizeViewKey(parsed);
    return { openTabs: [view], activeTab: view };
  } catch {
    return null;
  }
}

function loadStoredState(): StoredWorkbenchState {
  try {
    const item = localStorage.getItem(STORAGE_KEY);
    if (item) return JSON.parse(item) as StoredWorkbenchState;
  } catch {
    // fall through to migration / defaults
  }
  const migrated = migrateLegacyView();
  if (migrated) return migrated;
  return { openTabs: [...PINNED_TABS, FALLBACK_TAB], activeTab: FALLBACK_TAB };
}

export function useWorkbenchTabs() {
  const [stored, setStored] = useLocalStorage<StoredWorkbenchState>(
    STORAGE_KEY,
    loadStoredState(),
  );

  const openTabs = stored.openTabs;
  const activeTab = stored.activeTab;

  const openTab = useCallback(
    (viewKey: string) => {
      const key = normalizeViewKey(viewKey);
      setStored((prev) => ({
        openTabs: prev.openTabs.includes(key) ? prev.openTabs : [...prev.openTabs, key],
        activeTab: key,
      }));
    },
    [setStored],
  );

  const openParent = useCallback(
    (parentKey: string) => {
      const child = DEFAULT_CHILD_BY_PARENT[parentKey] ?? parentKey;
      openTab(child);
    },
    [openTab],
  );

  const openDocTab = useCallback(
    (path: string, _title?: string) => {
      const id = docTabId(path);
      setStored((prev) => ({
        openTabs: prev.openTabs.includes(id) ? prev.openTabs : [...prev.openTabs, id],
        activeTab: id,
      }));
    },
    [setStored],
  );

  const closeTab = useCallback(
    (id: TabId) => {
      if (isPinnedTab(id)) return;
      setStored((prev) => {
        const idx = prev.openTabs.indexOf(id);
        if (idx === -1) return prev;
        const nextTabs = prev.openTabs.filter((t) => t !== id);
        if (nextTabs.length === 0) {
          return { openTabs: [...PINNED_TABS, FALLBACK_TAB], activeTab: FALLBACK_TAB };
        }
        const neighbor = nextTabs[Math.min(idx, nextTabs.length - 1)] ?? nextTabs[0];
        return { openTabs: nextTabs, activeTab: neighbor };
      });
    },
    [setStored],
  );

  const activeViewKey = useMemo(
    () => (activeTab && !isDocTab(activeTab) ? activeTab : null),
    [activeTab],
  );

  return { openTabs, activeTab, activeViewKey, openTab, openParent, openDocTab, closeTab };
}
