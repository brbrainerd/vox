export interface ScreenCoords {
  px: number;
  py: number;
}

/**
 * Projects a 3D grid coordinate (x, y, z) into 2D screen pixels.
 */
export function projectIso(
  x: number,
  y: number,
  z: number,
  tileWidth: number,
  tileHeight: number,
  offsetX: number,
  offsetY: number,
  heightScale: number = 20
): ScreenCoords {
  const tileWidthHalf = tileWidth / 2;
  const tileHeightHalf = tileHeight / 2;

  const px = (x - y) * tileWidthHalf + offsetX;
  const py = (x + y) * tileHeightHalf - z * heightScale + offsetY;

  return { px, py };
}

/**
 * Calculates depth zIndex ordering for DOM overlays.
 */
export function getZIndex(x: number, y: number): number {
  return Math.floor(x + y);
}

/**
 * Reverses a 2D screen coordinate (px, py) back to 3D grid coordinate (x, y) assuming z = 0.
 */
export function unprojectIso(
  px: number,
  py: number,
  tileWidth: number,
  tileHeight: number,
  offsetX: number,
  offsetY: number
): { x: number; y: number } {
  const tileWidthHalf = tileWidth / 2;
  const tileHeightHalf = tileHeight / 2;

  const dx = px - offsetX;
  const dy = py - offsetY;

  const A = dx / tileWidthHalf;
  const B = dy / tileHeightHalf;

  const x = (A + B) / 2;
  const y = (B - A) / 2;

  return { x, y };
}
