import { describe, it, expect } from 'vitest';
import { clampSidebarWidth, snapToPreset, SIDEBAR_MIN, SIDEBAR_MAX } from './sidebarWidth';

describe('sidebar width', () => {
  it('clamps to [min,max]', () => {
    expect(clampSidebarWidth(10)).toBe(SIDEBAR_MIN);
    expect(clampSidebarWidth(9999)).toBe(SIDEBAR_MAX);
    expect(clampSidebarWidth(240)).toBe(240);
  });

  it('snaps to a preset within tolerance, else keeps exact width', () => {
    expect(snapToPreset(210)).toBe(212);   // near "default" (212) → snap
    expect(snapToPreset(250)).toBe(250);   // outside tolerance → exact
    expect(snapToPreset(66)).toBe(64);     // near "rail" (64) → snap
  });
});
