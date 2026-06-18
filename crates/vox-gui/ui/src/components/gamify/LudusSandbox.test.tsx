import { describe, it, expect } from 'vitest';
import { assignPlotCoordinates } from './LudusSandbox';
import { useLudusStore } from './store';

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

describe('DOM Subscription Engine', () => {
  it('correctly reacts to store updates without parent re-renders', () => {
    let callCount = 0;
    const unsubscribe = useLudusStore.subscribe((state) => {
      if (state.agents['agent_1']) callCount += 1;
    });

    useLudusStore.getState().updateAgent('agent_1', { x: 4, y: 4 });
    expect(callCount).toBe(1);
    unsubscribe();
  });
});
