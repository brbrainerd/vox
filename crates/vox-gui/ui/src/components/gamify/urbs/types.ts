// crates/vox-gui/ui/src/components/gamify/urbs/types.ts
/** Mirrors the Rust TownScanDto (crates/vox-gui/src/commands/workspace_town.rs). */
export interface TownFile { path: string; lines: number }
export interface TownCrate { name: string; root: string; files: TownFile[] }
export interface TownScan {
  crates: TownCrate[];
  /** Absolute workspace root (forward slashes) for open_locator path joins. */
  root: string;
  scanned_at_ms: number;
  truncated: boolean;
}

/** Building tier by line-count quartile within its district. */
export type Tier = 0 | 1 | 2 | 3; // hut | villa | insula | temple

export interface BuildingPlot { path: string; x: number; y: number; tier: Tier }

export interface District {
  name: string;
  /** Tile rect, half-open: [x0, x1) × [y0, y1). */
  x0: number; y0: number; x1: number; y1: number;
  landmark: boolean;
  buildings: BuildingPlot[];
}

export interface TownLayout {
  districts: District[];
  byPath: Record<string, BuildingPlot>;
  /** Interior grid size in tiles (landmarks live outside, see margins). */
  grid: { w: number; h: number };
  /** Row-major w×h road mask (district border tiles). */
  roads: boolean[];
  /** Fixed harness landmark anchors in tile coords (may be outside the grid). */
  landmarks: { castrum: Pt; portus: Pt; aqvae: Pt; gate: Pt };
}
export interface Pt { x: number; y: number }
