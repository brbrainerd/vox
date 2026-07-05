// crates/vox-gui/ui/src/components/gamify/urbs/lod.ts
import { projectIso, unprojectIso } from '../../../lib/projection';
import type { WorldBounds } from './camera';
import type { Pt, TownLayout } from './types';

export const TILE_W = 64;
export const TILE_H = 32;
/** 8-tile margin keeps out-of-grid landmarks (castrum/portus/gate) in-world. */
export const MARGIN_TILES = 8;
/** Below this zoom districts render as aggregate landmarks. */
export const LOD_THRESHOLD = 0.55;

export function bandForZoom(zoom: number): 0 | 1 {
  return zoom < LOD_THRESHOLD ? 0 : 1;
}

/** Iso-project a tile coordinate into world pixel space (origin ≥ 0). */
export function worldPx(layout: TownLayout, x: number, y: number): { px: number; py: number } {
  const offsetX = (layout.grid.h + MARGIN_TILES * 2) * (TILE_W / 2);
  const offsetY = MARGIN_TILES * TILE_H;
  return projectIso(x + MARGIN_TILES, y + MARGIN_TILES, 0, TILE_W, TILE_H, offsetX, offsetY);
}

/** Inverse of worldPx: world pixel → integer tile coordinate. */
export function tileFromWorld(layout: TownLayout, wx: number, wy: number): Pt {
  const offsetX = (layout.grid.h + MARGIN_TILES * 2) * (TILE_W / 2);
  const offsetY = MARGIN_TILES * TILE_H;
  const t = unprojectIso(wx, wy, TILE_W, TILE_H, offsetX, offsetY);
  return { x: Math.round(t.x) - MARGIN_TILES, y: Math.round(t.y) - MARGIN_TILES };
}

export function worldBounds(layout: TownLayout): WorldBounds {
  const n = layout.grid.w + MARGIN_TILES * 2;
  const m = layout.grid.h + MARGIN_TILES * 2;
  return { minX: 0, minY: 0, maxX: ((n + m) * TILE_W) / 2, maxY: ((n + m) * TILE_H) / 2 + 140 };
}

// NOTE (deliberate, spec §4): there is no per-frame viewport culling. The
// full world is painted once into an offscreen buffer and pan/zoom is a
// camera-transformed blit; the buffer's size is bounded by a render-scale
// clamp in worldRenderer. Culling would only help buffer *repaints*, which
// happen on data/LOD changes, not per frame.
