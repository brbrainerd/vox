import { describe, it, expect } from 'vitest';
import { projectIso, getZIndex } from './projection';

describe('Isometric Projection', () => {
  it('correctly projects 3D coordinates to 2D pixels', () => {
    const tileWidth = 64;
    const tileHeight = 32;
    const offsetX = 300;
    const offsetY = 150;

    // Center tile (0, 0, 0)
    const p1 = projectIso(0, 0, 0, tileWidth, tileHeight, offsetX, offsetY);
    expect(p1.px).toBe(300);
    expect(p1.py).toBe(150);

    // Coordinate with depth and elevation (2, 3, 1)
    const p2 = projectIso(2, 3, 1, tileWidth, tileHeight, offsetX, offsetY);
    expect(p2.px).toBe(268);
    expect(p2.py).toBe(210);
  });

  it('correctly computes depth zIndex based on tile distance', () => {
    expect(getZIndex(0, 0)).toBe(0);
    expect(getZIndex(2.5, 3.1)).toBe(5);
  });
});
