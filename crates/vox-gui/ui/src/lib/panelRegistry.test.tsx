// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { DOCKABLE_VIEW_KEYS, panelTitle, resolvePanelView } from './panelRegistry';

describe('panelRegistry', () => {
  it('lists the primary surfaces as dockable', () => {
    // A representative spread across parents must be dockable.
    for (const key of ['dashboard', 'chat', 'memory', 'models', 'settings', 'activity']) {
      expect(DOCKABLE_VIEW_KEYS).toContain(key);
    }
  });

  it('gives every dockable view a non-empty human title', () => {
    for (const key of DOCKABLE_VIEW_KEYS) {
      expect(panelTitle(key).length).toBeGreaterThan(0);
    }
  });

  it('resolves a top-level parent key to a renderable child', () => {
    // `knowledge` is a parent with no childRenderer case — must resolve to a child.
    expect(resolvePanelView('knowledge')).not.toBe('knowledge');
    // a key that is already a child resolves to itself (idempotent).
    expect(resolvePanelView('memory')).toBe('memory');
  });
});
