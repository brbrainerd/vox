// crates/vox-gui/ui/src/components/gamify/urbs/camera.test.ts
import { describe, it, expect } from 'vitest';
import {
  worldToScreen, screenToWorld, zoomAt, clampCamera, fitBounds,
  type Camera, type WorldBounds,
} from './camera';

const BOUNDS: WorldBounds = { minX: 0, minY: 0, maxX: 2000, maxY: 1200 };
const VP = { w: 800, h: 500 };

describe('camera math', () => {
  it('screen↔world round-trips at arbitrary zoom', () => {
    const cam: Camera = { x: -137, y: 42, zoom: 1.73 };
    const w = screenToWorld(cam, 400, 250);
    const s = worldToScreen(cam, w.wx, w.wy);
    expect(s.sx).toBeCloseTo(400, 6);
    expect(s.sy).toBeCloseTo(250, 6);
  });

  it('zoomAt keeps the world point under the cursor fixed', () => {
    const cam: Camera = { x: 0, y: 0, zoom: 1 };
    const before = screenToWorld(cam, 600, 100);
    const zoomed = zoomAt(cam, 600, 100, 1.5, 0.2, 4);
    const after = screenToWorld(zoomed, 600, 100);
    expect(after.wx).toBeCloseTo(before.wx, 6);
    expect(after.wy).toBeCloseTo(before.wy, 6);
  });

  it('zoomAt clamps zoom to [min,max]', () => {
    const cam: Camera = { x: 0, y: 0, zoom: 3.9 };
    expect(zoomAt(cam, 0, 0, 2, 0.2, 4).zoom).toBe(4);
    expect(zoomAt({ ...cam, zoom: 0.25 }, 0, 0, 0.1, 0.2, 4).zoom).toBe(0.2);
  });

  it('clampCamera keeps the world covering the viewport', () => {
    // Panned absurdly far right/down: world must still touch the viewport.
    const cam = clampCamera({ x: 99999, y: 99999, zoom: 1 }, BOUNDS, VP.w, VP.h);
    expect(cam.x).toBeLessThanOrEqual(VP.w);
    expect(cam.y).toBeLessThanOrEqual(VP.h);
    const cam2 = clampCamera({ x: -99999, y: -99999, zoom: 1 }, BOUNDS, VP.w, VP.h);
    // Left/top edge: world max corner may not scroll past viewport origin.
    expect(worldToScreen(cam2, BOUNDS.maxX, BOUNDS.maxY).sx).toBeGreaterThanOrEqual(0);
    expect(worldToScreen(cam2, BOUNDS.maxX, BOUNDS.maxY).sy).toBeGreaterThanOrEqual(0);
  });

  it('fitBounds centers the world in the viewport with padding', () => {
    const cam = fitBounds(BOUNDS, VP.w, VP.h, 20);
    const tl = worldToScreen(cam, BOUNDS.minX, BOUNDS.minY);
    const br = worldToScreen(cam, BOUNDS.maxX, BOUNDS.maxY);
    // Fully visible…
    expect(tl.sx).toBeGreaterThanOrEqual(0);
    expect(br.sx).toBeLessThanOrEqual(VP.w);
    // …and horizontally centered (world is wider than tall for this aspect).
    expect(tl.sx + br.sx).toBeCloseTo(VP.w, 0);
  });

  it('fitBounds does not produce NaN/Infinity on zero-width bounds', () => {
    const degenerate: WorldBounds = { minX: 0, minY: 0, maxX: 0, maxY: 500 };
    const cam = fitBounds(degenerate, VP.w, VP.h, 20);
    expect(Number.isFinite(cam.zoom)).toBe(true);
    expect(Number.isFinite(cam.x)).toBe(true);
    expect(Number.isFinite(cam.y)).toBe(true);
  });

  it('fitBounds does not produce NaN/Infinity on zero-height bounds', () => {
    const degenerate: WorldBounds = { minX: 0, minY: 0, maxX: 500, maxY: 0 };
    const cam = fitBounds(degenerate, VP.w, VP.h, 20);
    expect(Number.isFinite(cam.zoom)).toBe(true);
    expect(Number.isFinite(cam.x)).toBe(true);
    expect(Number.isFinite(cam.y)).toBe(true);
  });

  it('fitBounds does not produce NaN/Infinity on a single point', () => {
    const point: WorldBounds = { minX: 10, minY: 10, maxX: 10, maxY: 10 };
    const cam = fitBounds(point, VP.w, VP.h, 20);
    expect(Number.isFinite(cam.zoom)).toBe(true);
    expect(Number.isFinite(cam.x)).toBe(true);
    expect(Number.isFinite(cam.y)).toBe(true);
  });

  it('zoomAt never lets zoom reach exactly 0 even with minZoom: 0', () => {
    let cam: Camera = { x: 0, y: 0, zoom: 1 };
    for (let i = 0; i < 50; i++) {
      cam = zoomAt(cam, 100, 100, 0.5, 0, 4);
    }
    expect(cam.zoom).toBeGreaterThan(0);
    const w = screenToWorld(cam, 100, 100);
    expect(Number.isFinite(w.wx)).toBe(true);
    expect(Number.isFinite(w.wy)).toBe(true);
  });
});
