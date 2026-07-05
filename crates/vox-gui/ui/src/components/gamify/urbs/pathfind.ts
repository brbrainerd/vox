// crates/vox-gui/ui/src/components/gamify/urbs/pathfind.ts
import type { Pt } from './types';

/** BFS ring-search for the closest road tile to `p` (Chebyshev radius ≤ 6). */
export function nearestRoad(roads: boolean[], w: number, h: number, p: Pt): Pt | null {
  for (let r = 0; r <= 6; r++) {
    for (let dy = -r; dy <= r; dy++) {
      for (let dx = -r; dx <= r; dx++) {
        if (Math.max(Math.abs(dx), Math.abs(dy)) !== r) continue;
        const x = p.x + dx, y = p.y + dy;
        if (x >= 0 && y >= 0 && x < w && y < h && roads[y * w + x]) return { x, y };
      }
    }
  }
  return null;
}

/** A* (Manhattan heuristic, 4-neighbour) over the road mask. */
export function findPath(roads: boolean[], w: number, h: number, from: Pt, to: Pt): Pt[] | null {
  const idx = (p: Pt) => p.y * w + p.x;
  if (!roads[idx(from)] || !roads[idx(to)]) return null;
  const open: { p: Pt; g: number; f: number }[] = [{ p: from, g: 0, f: 0 }];
  const came = new Map<number, number>();
  const gScore = new Map<number, number>([[idx(from), 0]]);
  const hcost = (p: Pt) => Math.abs(p.x - to.x) + Math.abs(p.y - to.y);
  while (open.length) {
    open.sort((a, b) => a.f - b.f);
    const cur = open.shift()!;
    if (cur.p.x === to.x && cur.p.y === to.y) {
      const path: Pt[] = [cur.p];
      let k = idx(cur.p);
      while (came.has(k)) {
        k = came.get(k)!;
        path.unshift({ x: k % w, y: Math.floor(k / w) });
      }
      return path;
    }
    for (const [dx, dy] of [[1, 0], [-1, 0], [0, 1], [0, -1]] as const) {
      const n = { x: cur.p.x + dx, y: cur.p.y + dy };
      if (n.x < 0 || n.y < 0 || n.x >= w || n.y >= h || !roads[idx(n)]) continue;
      const g = cur.g + 1;
      if (g < (gScore.get(idx(n)) ?? Infinity)) {
        gScore.set(idx(n), g);
        came.set(idx(n), idx(cur.p));
        open.push({ p: n, g, f: g + hcost(n) });
      }
    }
  }
  return null;
}
