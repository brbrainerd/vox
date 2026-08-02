import type { Toast } from '../types/tauri';
import type { ToastItem } from '../components/ui/Toasts';

/** Reserved group key for the "N more notifications" overflow summary toast. */
export const OVERFLOW_GROUP_KEY = '__toast_overflow__';

/** Max distinct toast entries shown at once (the overflow summary counts as one). */
export const MAX_TOASTS = 3;

/** A toast's coalescing identity: caller-supplied `groupKey`, or its title. */
export function toastGroupKey(t: Toast): string {
  return t.groupKey ?? t.title;
}

export interface CoalesceResult {
  /** The next toast list to render. */
  items: ToastItem[];
  /** id of the entry that changed (new, merged, or overflow) — restart its expiry timer. */
  touchedId: string;
}

/**
 * Adds a new toast to the current list, coalescing where possible instead of
 * silently dropping the oldest entry.
 *
 * - If an existing (still-visible) toast shares the same group key — same
 *   caller-supplied `groupKey`, or same `title` by default — the new toast
 *   is merged into it (count++, freshest tone/body/cmd win) rather than
 *   appended as a separate entry.
 * - If the list is already at MAX_TOASTS and the new toast doesn't match any
 *   existing group, it is folded into (or starts) an "N more notifications"
 *   overflow toast instead of bumping an unrelated toast the user may not
 *   have seen yet.
 */
export function coalesceToast(curr: ToastItem[], t: Toast, id: string): CoalesceResult {
  const groupKey = toastGroupKey(t);

  const existingIdx = curr.findIndex(x => x.groupKey === groupKey);
  if (existingIdx !== -1) {
    const existing = curr[existingIdx];
    const next = [...curr];
    next[existingIdx] = {
      ...existing,
      tone: t.tone,
      title: t.title,
      body: t.body,
      cmd: t.cmd,
      cause: t.cause,
      count: (existing.count ?? 1) + 1,
    };
    return { items: next, touchedId: existing.id };
  }

  if (curr.length < MAX_TOASTS) {
    return { items: [...curr, { ...t, id, groupKey, count: 1 }], touchedId: id };
  }

  // At capacity with no matching group — coalesce into the overflow summary
  // rather than dropping an unrelated, unseen toast.
  const overflowIdx = curr.findIndex(x => x.groupKey === OVERFLOW_GROUP_KEY);
  if (overflowIdx !== -1) {
    const overflow = curr[overflowIdx];
    const count = (overflow.count ?? 1) + 1;
    const next = [...curr];
    next[overflowIdx] = { ...overflow, title: `${count} more notifications`, count };
    return { items: next, touchedId: overflow.id };
  }

  const overflowItem: ToastItem = {
    id,
    tone: 'info',
    title: '2 more notifications',
    cause: t.cause,
    groupKey: OVERFLOW_GROUP_KEY,
    count: 2,
  };
  return { items: [...curr, overflowItem], touchedId: id };
}
