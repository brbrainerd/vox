// crates/vox-gui/ui/src/components/gamify/urbs/pathfind.test.ts
import { describe, it, expect } from 'vitest';
import { findPath, nearestRoad } from './pathfind';

// 5×5 grid, road ring around the border, buildings inside.
const W = 5, H = 5;
const roads = Array.from({ length: W * H }, (_, i) => {
  const x = i % W, y = Math.floor(i / W);
  return x === 0 || y === 0 || x === W - 1 || y === H - 1;
});

describe('pathfind', () => {
  it('nearestRoad snaps an interior tile to an adjacent road tile', () => {
    const r = nearestRoad(roads, W, H, { x: 2, y: 2 });
    expect(r).not.toBeNull();
    expect(roads[r!.y * W + r!.x]).toBe(true);
  });

  it('finds a path along roads between two corners', () => {
    const path = findPath(roads, W, H, { x: 0, y: 0 }, { x: 4, y: 4 });
    expect(path).not.toBeNull();
    expect(path![0]).toEqual({ x: 0, y: 0 });
    expect(path![path!.length - 1]).toEqual({ x: 4, y: 4 });
    // Every step is a road tile and adjacent to the previous step.
    for (let i = 0; i < path!.length; i++) {
      expect(roads[path![i].y * W + path![i].x]).toBe(true);
      if (i > 0) {
        const d = Math.abs(path![i].x - path![i - 1].x) + Math.abs(path![i].y - path![i - 1].y);
        expect(d).toBe(1);
      }
    }
  });

  it('returns null when no road route exists', () => {
    const blocked = [...roads];
    for (let y = 0; y < H; y++) blocked[y * W + 2] = false; // cut the ring? column 2 only crosses at y=0 and y=4
    blocked[0 * W + 2] = false;
    blocked[4 * W + 2] = false;
    expect(findPath(blocked, W, H, { x: 0, y: 0 }, { x: 4, y: 4 })).toBeNull();
  });
});
