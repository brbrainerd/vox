// crates/vox-gui/ui/src/components/gamify/urbs/layout.test.ts
import { describe, it, expect } from 'vitest';
import { layoutTown, assignNewFile } from './layout';
import type { TownScan } from './types';

function scanFixture(): TownScan {
  const mk = (name: string, n: number): { name: string; root: string; files: { path: string; lines: number }[] } => ({
    name,
    root: `crates/${name}`,
    files: Array.from({ length: n }, (_, i) => ({
      path: `crates/${name}/src/f${i}.rs`,
      lines: (i + 1) * 40,
    })),
  });
  return {
    crates: [mk('alpha', 12), mk('beta', 5), mk('gamma', 30)],
    root: '/ws',
    scanned_at_ms: 0,
    truncated: false,
  };
}

describe('layoutTown', () => {
  it('is deterministic: same input, identical output', () => {
    const a = layoutTown(scanFixture(), new Set(['gamma']));
    const b = layoutTown(scanFixture(), new Set(['gamma']));
    expect(JSON.stringify(a)).toEqual(JSON.stringify(b));
  });

  it('places every file inside its own district rect, no overlaps', () => {
    const layout = layoutTown(scanFixture(), new Set());
    const seen = new Set<string>();
    for (const d of layout.districts) {
      for (const b of d.buildings) {
        expect(b.x).toBeGreaterThanOrEqual(d.x0);
        expect(b.x).toBeLessThan(d.x1);
        expect(b.y).toBeGreaterThanOrEqual(d.y0);
        expect(b.y).toBeLessThan(d.y1);
        const key = `${b.x},${b.y}`;
        expect(seen.has(key)).toBe(false);
        seen.add(key);
      }
    }
    expect(Object.keys(layout.byPath)).toHaveLength(12 + 5 + 30);
  });

  it('district rects do not overlap each other', () => {
    const { districts } = layoutTown(scanFixture(), new Set());
    for (let i = 0; i < districts.length; i++) {
      for (let j = i + 1; j < districts.length; j++) {
        const a = districts[i]; const b = districts[j];
        const disjoint = a.x1 <= b.x0 || b.x1 <= a.x0 || a.y1 <= b.y0 || b.y1 <= a.y0;
        expect(disjoint).toBe(true);
      }
    }
  });

  it('assigns tiers by line-count quartile within the district', () => {
    const layout = layoutTown(scanFixture(), new Set());
    const gamma = layout.districts.find((d) => d.name === 'gamma')!;
    const tiers = gamma.buildings.map((b) => b.tier);
    expect(Math.min(...tiers)).toBe(0);
    expect(Math.max(...tiers)).toBe(3);
    // Highest line count gets the highest tier.
    const biggest = gamma.buildings.find((b) => b.path.endsWith('f29.rs'))!;
    expect(biggest.tier).toBe(3);
  });

  it('marks god-node crates as landmark districts', () => {
    const layout = layoutTown(scanFixture(), new Set(['gamma']));
    expect(layout.districts.find((d) => d.name === 'gamma')!.landmark).toBe(true);
    expect(layout.districts.find((d) => d.name === 'alpha')!.landmark).toBe(false);
  });

  it('roads cover district border tiles and are within the grid', () => {
    const layout = layoutTown(scanFixture(), new Set());
    const { w, h } = layout.grid;
    expect(layout.roads).toHaveLength(w * h);
    const d = layout.districts[0];
    // The tile just outside the district's left edge is road (or grid edge).
    if (d.x0 > 0) expect(layout.roads[d.y0 * w + (d.x0 - 1)]).toBe(true);
  });

  it('never drops files under skewed crate sizes (sliver regression guard)', () => {
    // One 500-file crate beside ten 1-file crates — the shape that broke
    // recursive treemaps with 0/1/2-tile sliver districts.
    const skewed: TownScan = {
      crates: [
        { name: 'giant', root: 'crates/giant', files: Array.from({ length: 500 }, (_, i) => ({ path: `crates/giant/f${i}.rs`, lines: i })) },
        ...Array.from({ length: 10 }, (_, c) => ({
          name: `tiny${c}`, root: `crates/tiny${c}`,
          files: [{ path: `crates/tiny${c}/lib.rs`, lines: 1 }],
        })),
      ],
      root: '/ws', scanned_at_ms: 0, truncated: false,
    };
    const layout = layoutTown(skewed, new Set());
    expect(Object.keys(layout.byPath)).toHaveLength(510);
    for (const d of layout.districts) {
      expect(d.x1 - d.x0).toBeGreaterThanOrEqual(3);
      expect(d.y1 - d.y0).toBeGreaterThanOrEqual(3);
    }
  });

  it('assignNewFile parks a mid-session file on a free tile in its district', () => {
    const layout = layoutTown(scanFixture(), new Set());
    const plot = assignNewFile(layout, 'crates/beta/src/brand_new.rs');
    expect(plot).not.toBeNull();
    const beta = layout.districts.find((d) => d.name === 'beta')!;
    expect(plot!.x).toBeGreaterThanOrEqual(beta.x0);
    expect(plot!.x).toBeLessThan(beta.x1);
    expect(layout.byPath['crates/beta/src/brand_new.rs']).toEqual(plot);
    // Unknown crate → null (renderer ignores it; honesty over invention).
    expect(assignNewFile(layout, 'elsewhere/x.rs')).toBeNull();
  });

  it('lays out 5,000 files in under 50ms', () => {
    const big: TownScan = {
      crates: Array.from({ length: 100 }, (_, c) => ({
        name: `crate${c}`, root: `crates/crate${c}`,
        files: Array.from({ length: 50 }, (_, i) => ({ path: `crates/crate${c}/f${i}.rs`, lines: i })),
      })),
      root: '/ws', scanned_at_ms: 0, truncated: false,
    };
    const t0 = performance.now();
    layoutTown(big, new Set());
    expect(performance.now() - t0).toBeLessThan(50);
  });
});
