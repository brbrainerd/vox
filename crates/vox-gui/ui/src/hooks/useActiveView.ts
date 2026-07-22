import { useCallback } from 'react';
import { useLocalStorage } from './useLocalStorage';
import { LEGACY_VIEW_ALIASES, resolveNavigation } from '../lib/navigation';

const STORAGE_KEY = 'vox_active_view.v2';
const LEGACY_TABS_KEY = 'vox_workbench_tabs.v1';
const FALLBACK_VIEW = 'dashboard';

function normalizeViewKey(key: string): string {
  return LEGACY_VIEW_ALIASES[key] ?? key;
}

/**
 * One-time, one-directional migration from the old open-tabs storage shape.
 * Read only as the initial value for useLocalStorage — once the new key has
 * been written even once (including this migration's own first write), the
 * old key is never consulted again.
 */
function migrateFromLegacyTabs(): string {
  try {
    const raw = localStorage.getItem(LEGACY_TABS_KEY);
    if (!raw) return FALLBACK_VIEW;
    const parsed = JSON.parse(raw) as { activeTab?: string | null };
    if (parsed.activeTab && !parsed.activeTab.startsWith('doc:')) {
      return normalizeViewKey(parsed.activeTab);
    }
  } catch {
    // fall through to default
  }
  return FALLBACK_VIEW;
}

/**
 * Single-view navigation state: exactly one active view key, persisted and
 * URL-hash-syncable. Replaces the "open tabs" half of the old useWorkbenchTabs
 * hook — there is no list of open views, just the current one. Navigating to a
 * parent key resolves to that parent's default child (same behavior the old
 * hook's openParent provided); navigating to a child/leaf key goes there directly.
 */
export function useActiveView() {
  const [activeView, setActiveView] = useLocalStorage<string>(STORAGE_KEY, migrateFromLegacyTabs());

  const navigateTo = useCallback(
    (viewKey: string) => {
      const key = normalizeViewKey(viewKey);
      const { child } = resolveNavigation(key);
      setActiveView(child);
    },
    [setActiveView],
  );

  return { activeView, navigateTo };
}
