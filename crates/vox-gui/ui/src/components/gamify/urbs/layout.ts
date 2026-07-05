// crates/vox-gui/ui/src/components/gamify/urbs/layout.ts
import type { BuildingPlot, District, Pt, Tier, TownLayout, TownScan } from './types';

/** Deterministic 32-bit string hash (FNV-1a) for stable tie-breaking. */
export function hash32(s: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  return h >>> 0;
}

interface Rect { x0: number; y0: number; x1: number; y1: number }

/** Side of the square interior a crate needs at ~50% building density.
 *  interiorSide² ≥ 2·files guarantees the checkerboard pass alone can hold
 *  every file — capacity is a construction invariant, not a hope. */
function interiorSide(fileCount: number): number {
  return Math.max(2, Math.ceil(Math.sqrt(fileCount * 2)));
}

/** Deterministic shelf-packing (same idea as the sprite atlas): each district
 *  is a square sized from its OWN file count (+2 for the road border), packed
 *  into rows of ~square overall width. No recursive splitting → no sliver
 *  rects, no crate can ever lose its interior, and a 1-file crate beside a
 *  3000-file district just gets a small square next to a big one. */
function packDistricts(
  crates: { name: string; files: number }[],
): { rects: Map<string, Rect>; side: number } {
  const sized = crates.map((c) => ({ name: c.name, s: interiorSide(c.files) + 2 }));
  const totalArea = sized.reduce((a, d) => a + d.s * d.s, 0);
  const targetW = Math.ceil(Math.sqrt(totalArea * 1.1));
  // Height-desc order (name-hash tiebreak) keeps rows tight; deterministic.
  const order = [...sized].sort((a, b) => b.s - a.s || hash32(a.name) - hash32(b.name));
  const rects = new Map<string, Rect>();
  let x = 0; let y = 0; let rowH = 0; let maxX = 0;
  for (const d of order) {
    if (x > 0 && x + d.s > Math.max(targetW, d.s)) { x = 0; y += rowH; rowH = 0; }
    rects.set(d.name, { x0: x, y0: y, x1: x + d.s, y1: y + d.s });
    x += d.s;
    rowH = Math.max(rowH, d.s);
    maxX = Math.max(maxX, x);
  }
  return { rects, side: Math.max(maxX, y + rowH) };
}

function tierFor(lines: number, sorted: number[]): Tier {
  const idx = sorted.findIndex((v) => v >= lines);
  const q = (idx < 0 ? sorted.length - 1 : idx) / Math.max(1, sorted.length - 1);
  if (q < 0.25) return 0;
  if (q < 0.5) return 1;
  if (q < 0.75) return 2;
  return 3;
}

export function layoutTown(scan: TownScan, godNodes: Set<string>): TownLayout {
  // Deterministic order for placement: size desc, then name-hash.
  const crates = [...scan.crates].sort(
    (a, b) => b.files.length - a.files.length || hash32(a.name) - hash32(b.name),
  );
  const { rects, side } = packDistricts(
    crates.map((c) => ({ name: c.name, files: c.files.length })),
  );

  const districts: District[] = [];
  const byPath: Record<string, BuildingPlot> = {};
  // Everything that is not a district INTERIOR is road: borders, alleys, and
  // the ragged gaps between packed rows. One connected walkable network by
  // construction — A* never strands a citizen between districts.
  const roads = new Array<boolean>(side * side).fill(true);

  for (const crate of crates) {
    const r = rects.get(crate.name)!;
    for (let y = r.y0 + 1; y < r.y1 - 1; y++) {
      for (let x = r.x0 + 1; x < r.x1 - 1; x++) {
        roads[y * side + x] = false; // interior = buildable, not walkable
      }
    }
    const files = [...crate.files].sort((a, b) => hash32(a.path) - hash32(b.path));
    const sortedLines = crate.files.map((f) => f.lines).sort((a, b) => a - b);
    const buildings: BuildingPlot[] = [];
    let i = 0;
    // Checkerboard first (alleys between buildings), other parity as overflow.
    for (const parity of [0, 1]) {
      for (let y = r.y0 + 1; y < r.y1 - 1 && i < files.length; y++) {
        for (let x = r.x0 + 1; x < r.x1 - 1 && i < files.length; x++) {
          if ((x + y) % 2 !== parity) continue;
          const f = files[i++];
          const plot: BuildingPlot = { path: f.path, x, y, tier: tierFor(f.lines, sortedLines) };
          buildings.push(plot);
          byPath[f.path] = plot;
        }
      }
    }
    // interiorSide² ≥ 2·files makes leftovers impossible; assert the invariant
    // so a future sizing change fails loudly instead of dropping files.
    if (i < files.length) {
      throw new Error(`layoutTown: district ${crate.name} under-fit (${i}/${files.length})`);
    }
    districts.push({
      name: crate.name, x0: r.x0, y0: r.y0, x1: r.x1, y1: r.y1,
      landmark: godNodes.has(crate.name), buildings,
    });
  }

  const landmarks: { castrum: Pt; portus: Pt; aqvae: Pt; gate: Pt } = {
    castrum: { x: side + 4, y: 2 },
    portus: { x: -6, y: Math.floor(side / 2) },
    aqvae: { x: -3, y: -3 },
    gate: { x: Math.floor(side / 2), y: side + 3 },
  };
  return { districts, byPath, grid: { w: side, h: side }, roads, landmarks };
}

/** Mid-session file creation: nearest free interior tile in its crate district. */
export function assignNewFile(layout: TownLayout, path: string): BuildingPlot | null {
  const m = path.match(/^crates\/([^/]+)\//);
  const name = m ? m[1] : '(workspace)';
  const d = layout.districts.find((x) => x.name === name);
  if (!d) return null;
  const occupied = new Set(d.buildings.map((b) => `${b.x},${b.y}`));
  for (let y = d.y0 + 1; y < d.y1 - 1; y++) {
    for (let x = d.x0 + 1; x < d.x1 - 1; x++) {
      if (!occupied.has(`${x},${y}`)) {
        const plot: BuildingPlot = { path, x, y, tier: 0 };
        d.buildings.push(plot);
        layout.byPath[path] = plot;
        return plot;
      }
    }
  }
  return null;
}
