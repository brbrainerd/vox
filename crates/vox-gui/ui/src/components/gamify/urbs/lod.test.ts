// crates/vox-gui/ui/src/components/gamify/urbs/lod.test.ts
import { describe, it, expect } from 'vitest';
import { bandForZoom, worldPx, worldBounds, tileFromWorld, LOD_THRESHOLD } from './lod';
import { layoutTown } from './layout';
import type { TownScan } from './types';

const scan: TownScan = {
  crates: [
    { name: 'a', root: 'crates/a', files: Array.from({ length: 8 }, (_, i) => ({ path: `crates/a/${i}.rs`, lines: i })) },
    { name: 'b', root: 'crates/b', files: Array.from({ length: 8 }, (_, i) => ({ path: `crates/b/${i}.rs`, lines: i })) },
  ],
  root: '/ws', scanned_at_ms: 0, truncated: false,
};

describe('lod', () => {
  it('band 0 (aggregate) below threshold, band 1 (buildings) at/above', () => {
    expect(bandForZoom(LOD_THRESHOLD - 0.01)).toBe(0);
    expect(bandForZoom(LOD_THRESHOLD)).toBe(1);
    expect(bandForZoom(3)).toBe(1);
  });

  it('worldPx projects tile → world pixels with non-negative margin origin', () => {
    const layout = layoutTown(scan, new Set());
    const b = worldBounds(layout);
    expect(b.minX).toBe(0);
    expect(b.minY).toBe(0);
    const p = worldPx(layout, 0, 0);
    expect(p.px).toBeGreaterThan(0);
    expect(p.py).toBeGreaterThan(0);
    const q = worldPx(layout, layout.grid.w - 1, layout.grid.h - 1);
    expect(q.px).toBeLessThan(b.maxX);
    expect(q.py).toBeLessThan(b.maxY);
  });

  it('tileFromWorld inverts worldPx for every grid tile', () => {
    const layout = layoutTown(scan, new Set());
    for (const tile of [{ x: 0, y: 0 }, { x: 3, y: 1 }, { x: layout.grid.w - 1, y: layout.grid.h - 1 }]) {
      const { px, py } = worldPx(layout, tile.x, tile.y);
      expect(tileFromWorld(layout, px, py)).toEqual(tile);
    }
  });

});
