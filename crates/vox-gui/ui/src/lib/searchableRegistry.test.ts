// @vitest-environment jsdom
import { describe, it, expect, beforeEach } from 'vitest';
import { renderHook } from '@testing-library/react';
import {
  registerSearchable,
  unregisterSearchable,
  querySearchableRegistry,
  clearSearchableRegistry,
  useSearchable,
  type SearchableEntry,
} from './searchableRegistry';

const ENTRY = (label: string): SearchableEntry => ({
  label,
  detail: '',
  viewKey: 'activity',
});

describe('searchableRegistry', () => {
  beforeEach(() => clearSearchableRegistry());

  it('starts empty (no-op default)', () => {
    expect(querySearchableRegistry('pending')).toEqual([]);
  });

  it('register → query (case-insensitive substring) → unregister', () => {
    registerSearchable('activity', [ENTRY('3 pending approvals'), ENTRY('queue idle')]);
    const hits = querySearchableRegistry('PENDING');
    expect(hits).toHaveLength(1);
    expect(hits[0].label).toBe('3 pending approvals');
    expect(hits[0].surfaceId).toBe('activity');
    unregisterSearchable('activity');
    expect(querySearchableRegistry('pending')).toEqual([]);
  });

  it('blank query returns nothing', () => {
    registerSearchable('activity', [ENTRY('3 pending approvals')]);
    expect(querySearchableRegistry('   ')).toEqual([]);
  });

  it('useSearchable registers on mount and clears on unmount', () => {
    const { unmount } = renderHook(() => useSearchable('mesh', [ENTRY('4 peers online')]));
    expect(querySearchableRegistry('peers')).toHaveLength(1);
    unmount();
    expect(querySearchableRegistry('peers')).toEqual([]);
  });
});
