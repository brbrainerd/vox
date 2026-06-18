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
