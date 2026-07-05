// crates/vox-gui/ui/src/components/gamify/urbs/camera.ts
/** Screen = world * zoom + offset. All pure; the component owns the state. */
export interface Camera { x: number; y: number; zoom: number }
export interface WorldBounds { minX: number; minY: number; maxX: number; maxY: number }

export function worldToScreen(cam: Camera, wx: number, wy: number): { sx: number; sy: number } {
  return { sx: wx * cam.zoom + cam.x, sy: wy * cam.zoom + cam.y };
}

export function screenToWorld(cam: Camera, sx: number, sy: number): { wx: number; wy: number } {
  return { wx: (sx - cam.x) / cam.zoom, wy: (sy - cam.y) / cam.zoom };
}

/** Zoom by `factor` keeping the world point under screen (sx, sy) stationary. */
export function zoomAt(
  cam: Camera, sx: number, sy: number, factor: number, minZoom: number, maxZoom: number,
): Camera {
  const floor = Math.max(minZoom, 1e-6);
  const zoom = Math.min(maxZoom, Math.max(floor, cam.zoom * factor));
  const { wx, wy } = screenToWorld(cam, sx, sy);
  return { zoom, x: sx - wx * zoom, y: sy - wy * zoom };
}

/** Keep at least part of the world on screen (never lose it off an edge). */
export function clampCamera(cam: Camera, b: WorldBounds, vw: number, vh: number): Camera {
  const x = Math.min(vw - b.minX * cam.zoom, Math.max(-b.maxX * cam.zoom, cam.x));
  const y = Math.min(vh - b.minY * cam.zoom, Math.max(-b.maxY * cam.zoom, cam.y));
  return { ...cam, x, y };
}

/** Camera that fits (and centers) the whole bounds in the viewport. */
export function fitBounds(b: WorldBounds, vw: number, vh: number, pad: number): Camera {
  const w = Math.max(1, b.maxX - b.minX);
  const h = Math.max(1, b.maxY - b.minY);
  const zoom = Math.min((vw - 2 * pad) / w, (vh - 2 * pad) / h);
  return {
    zoom,
    x: (vw - w * zoom) / 2 - b.minX * zoom,
    y: (vh - h * zoom) / 2 - b.minY * zoom,
  };
}
