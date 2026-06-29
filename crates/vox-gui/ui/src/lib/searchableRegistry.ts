/**
 * In-memory runtime registry of searchable dynamic text, keyed by surface id.
 * Ships as a no-op: a surface opts in via `useSearchable` only when it has
 * watch-worthy dynamic strings. The Omnibar's ON-SCREEN facet reads this
 * synchronously alongside the build-time content manifest.
 *
 * See docs/superpowers/specs/2026-06-27-vox-graph-omnibar-dashboard-design.md §2.2.
 */
import { useEffect } from 'react';

export interface SearchableEntry {
  /** The on-screen text to match (e.g. "3 pending approvals"). */
  label: string;
  /** Optional context shown after the label (e.g. surface name). */
  detail?: string;
  /** View key to navigate to when activated. */
  viewKey: string;
  /** Optional DOM id to scrollIntoView after navigating. */
  anchorId?: string;
}

export interface SearchableHit extends SearchableEntry {
  surfaceId: string;
}

const REGISTRY = new Map<string, SearchableEntry[]>();

export function registerSearchable(surfaceId: string, entries: SearchableEntry[]): void {
  REGISTRY.set(surfaceId, entries);
}

export function unregisterSearchable(surfaceId: string): void {
  REGISTRY.delete(surfaceId);
}

export function clearSearchableRegistry(): void {
  REGISTRY.clear();
}

export function querySearchableRegistry(query: string): SearchableHit[] {
  const q = query.trim().toLowerCase();
  if (!q) return [];
  const out: SearchableHit[] = [];
  for (const [surfaceId, entries] of REGISTRY) {
    for (const e of entries) {
      if (
        e.label.toLowerCase().includes(q) ||
        (e.detail ?? '').toLowerCase().includes(q)
      ) {
        out.push({ ...e, surfaceId });
      }
    }
  }
  return out;
}

/**
 * Opt-in hook: a surface registers its dynamic searchable text while mounted.
 * No-op-friendly — surfaces with nothing dynamic never call this.
 */
export function useSearchable(surfaceId: string, entries: SearchableEntry[]): void {
  useEffect(() => {
    registerSearchable(surfaceId, entries);
    return () => unregisterSearchable(surfaceId);
  }, [surfaceId, entries]);
}
