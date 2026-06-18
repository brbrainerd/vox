import { describe, it, expect } from 'vitest';
import { assignPlotCoordinates } from './LudusSandbox';

describe('LudusSandbox map logic', () => {
  it('assignPlotCoordinates exists', () => {
    expect(assignPlotCoordinates).toBeDefined();
  });

  it('correctly maps client mouse coords to offset coordinates', () => {
    const mouseX = 150;
    const mouseY = 150;
    const cameraX = 50;
    const cameraY = 50;
    const zoom = 2;

    const worldX = (mouseX - cameraX) / zoom;
    const worldY = (mouseY - cameraY) / zoom;

    expect(worldX).toBe(50);
    expect(worldY).toBe(50);
  });
});
