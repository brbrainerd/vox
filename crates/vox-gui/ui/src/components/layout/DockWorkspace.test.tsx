// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { panelIdForView, planOpen } from './DockWorkspace';

// Pure layout-intent helpers are unit-tested without mounting dockview.
describe('DockWorkspace helpers', () => {
  it('derives a stable panel id from a viewKey', () => {
    expect(panelIdForView('memory')).toBe('surface:memory');
    expect(panelIdForView('memory')).toBe(panelIdForView('memory'));
  });

  it('planOpen focuses an existing panel instead of duplicating', () => {
    const existing = new Set(['surface:memory']);
    expect(planOpen('memory', existing)).toEqual({ action: 'focus', id: 'surface:memory' });
  });

  it('planOpen adds a new panel when not present', () => {
    const existing = new Set(['surface:memory']);
    expect(planOpen('models', existing)).toEqual({ action: 'add', id: 'surface:models', viewKey: 'models' });
  });
});
