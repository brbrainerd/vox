import { describe, it, expect } from 'vitest';
import { coalesceToast, MAX_TOASTS, OVERFLOW_GROUP_KEY } from './toastQueue';
import type { Toast } from '../types/tauri';

function mk(title: string, overrides: Partial<Toast> = {}): Toast {
  return { tone: 'ok', title, cause: 'backend-ok', ...overrides };
}

describe('coalesceToast', () => {
  it('appends a new entry when no toasts are present', () => {
    const { items, touchedId } = coalesceToast([], mk('Build succeeded'), 'id-1');
    expect(items).toHaveLength(1);
    expect(items[0]).toMatchObject({ id: 'id-1', title: 'Build succeeded', count: 1 });
    expect(touchedId).toBe('id-1');
  });

  it('merges a same-title toast into the existing entry with an incremented count', () => {
    const first = coalesceToast([], mk('Task updated'), 'id-1').items;
    const { items, touchedId } = coalesceToast(first, mk('Task updated'), 'id-2');
    expect(items).toHaveLength(1);
    expect(items[0]).toMatchObject({ id: 'id-1', title: 'Task updated', count: 2 });
    expect(touchedId).toBe('id-1');
  });

  it('merges by explicit groupKey even when titles differ', () => {
    const first = coalesceToast([], mk('Task A updated', { groupKey: 'task-updates' }), 'id-1').items;
    const { items } = coalesceToast(first, mk('Task B updated', { groupKey: 'task-updates' }), 'id-2');
    expect(items).toHaveLength(1);
    expect(items[0]).toMatchObject({ id: 'id-1', title: 'Task B updated', count: 2, groupKey: 'task-updates' });
  });

  it('does not coalesce toasts with different group keys', () => {
    const first = coalesceToast([], mk('Build succeeded'), 'id-1').items;
    const { items } = coalesceToast(first, mk('Lint warning'), 'id-2');
    expect(items).toHaveLength(2);
    expect(items.map(i => i.title)).toEqual(['Build succeeded', 'Lint warning']);
    expect(items.every(i => (i.count ?? 1) === 1)).toBe(true);
  });

  it('keeps appending distinct-group toasts up to MAX_TOASTS without dropping anything', () => {
    let items = coalesceToast([], mk('One'), 'id-1').items;
    items = coalesceToast(items, mk('Two'), 'id-2').items;
    items = coalesceToast(items, mk('Three'), 'id-3').items;
    expect(items).toHaveLength(MAX_TOASTS);
    expect(items.map(i => i.title)).toEqual(['One', 'Two', 'Three']);
  });

  it('folds a distinct-group toast beyond the cap into an overflow summary instead of dropping the oldest', () => {
    let items = coalesceToast([], mk('One'), 'id-1').items;
    items = coalesceToast(items, mk('Two'), 'id-2').items;
    items = coalesceToast(items, mk('Three'), 'id-3').items;
    const result = coalesceToast(items, mk('Four'), 'id-4');
    // Nothing already visible was dropped:
    expect(result.items.map(i => i.title)).toEqual(['One', 'Two', 'Three', '2 more notifications']);
    expect(result.items.find(i => i.groupKey === OVERFLOW_GROUP_KEY)).toMatchObject({ count: 2 });
    expect(result.touchedId).toBe('id-4');
  });

  it('increments the existing overflow summary instead of creating a second one', () => {
    let items = coalesceToast([], mk('One'), 'id-1').items;
    items = coalesceToast(items, mk('Two'), 'id-2').items;
    items = coalesceToast(items, mk('Three'), 'id-3').items;
    items = coalesceToast(items, mk('Four'), 'id-4').items;
    const result = coalesceToast(items, mk('Five'), 'id-5');
    const overflowEntries = result.items.filter(i => i.groupKey === OVERFLOW_GROUP_KEY);
    expect(overflowEntries).toHaveLength(1);
    expect(overflowEntries[0]).toMatchObject({ title: '3 more notifications', count: 3 });
    expect(result.touchedId).toBe(overflowEntries[0].id);
  });

  it('an overflow-eligible toast that matches an existing group still merges normally, even at capacity', () => {
    let items = coalesceToast([], mk('One'), 'id-1').items;
    items = coalesceToast(items, mk('Two'), 'id-2').items;
    items = coalesceToast(items, mk('Three'), 'id-3').items;
    const result = coalesceToast(items, mk('One'), 'id-4');
    expect(result.items).toHaveLength(3);
    expect(result.items[0]).toMatchObject({ id: 'id-1', title: 'One', count: 2 });
  });
});
