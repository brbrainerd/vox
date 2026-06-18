import { describe, it, expect } from 'vitest';
import { findPath, Coord } from './pathfinder';

describe('A* Grid Pathfinder', () => {
  it('finds a direct path between start and end with no obstacles', () => {
    const start: Coord = { x: 0, y: 0 };
    const end: Coord = { x: 2, y: 2 };
    const obstacles = new Set<string>();

    const path = findPath(start, end, obstacles);
    expect(path).toBeDefined();
    expect(path[0]).toEqual(start);
    expect(path[path.length - 1]).toEqual(end);
    expect(path.length).toBe(5); // Manhattan path step count
  });

  it('navigates around obstacles', () => {
    const start: Coord = { x: 0, y: 1 };
    const end: Coord = { x: 2, y: 1 };
    // Block the direct path at x: 1, y: 1
    const obstacles = new Set<string>(['1,1']);

    const path = findPath(start, end, obstacles);
    expect(path).toBeDefined();
    const hasObstacle = path.some(c => c.x === 1 && c.y === 1);
    expect(hasObstacle).toBe(false);
  });
});
