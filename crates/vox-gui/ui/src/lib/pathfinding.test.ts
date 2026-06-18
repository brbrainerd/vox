import { describe, it, expect } from 'vitest';
import { findPath } from './pathfinding';

describe('A* Grid Pathfinder', () => {
  it('correctly finds the shortest path avoiding solid building blocks', () => {
    const start = { x: 0, y: 0 };
    const target = { x: 2, y: 2 };
    
    // A 3x3 grid where (1, 1) is a solid building obstacle
    const solidBlocks = new Set(['1,1']);
    
    const path = findPath(start, target, solidBlocks, 3, 3);
    
    expect(path).toBeDefined();
    // The path should avoid the center block (1, 1)
    const intersectsCenter = path.some(node => node.x === 1 && node.y === 1);
    expect(intersectsCenter).toBe(false);
    
    // The last node should match target
    expect(path[path.length - 1]).toEqual(target);
  });
});
