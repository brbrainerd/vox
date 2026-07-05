// crates/vox-gui/ui/src/components/gamify/urbs/citizens.test.ts
import { describe, it, expect } from 'vitest';
import { stepCitizen, spawnCitizen, type Citizen } from './citizens';

const path = [{ x: 0, y: 0 }, { x: 1, y: 0 }, { x: 2, y: 0 }];

describe('citizen state machine', () => {
  it('spawns Idle at the given tile', () => {
    const c = spawnCitizen('agent-1', { x: 3, y: 3 });
    expect(c.state).toBe('idle');
    expect(c.pos).toEqual({ x: 3, y: 3 });
  });

  it('commutes along its path at walk speed, then works', () => {
    let c: Citizen = { ...spawnCitizen('a', { x: 0, y: 0 }), state: 'commuting', path, pathIdx: 0 };
    // Walk speed = 2 tiles/s → 1500ms covers 3 tiles.
    for (let t = 0; t < 15; t++) c = stepCitizen(c, 100, 1);
    expect(c.state).toBe('working');
    expect(c.pos.x).toBeCloseTo(2, 1);
  });

  it('speed multiplier scales movement; 0 (pause) freezes it', () => {
    let a: Citizen = { ...spawnCitizen('a', { x: 0, y: 0 }), state: 'commuting', path, pathIdx: 0 };
    let b: Citizen = { ...a };
    a = stepCitizen(a, 250, 1);
    b = stepCitizen(b, 250, 0);
    expect(a.pos.x).toBeGreaterThan(0);
    expect(b.pos.x).toBe(0);
  });

  it('exhausted citizens do not move', () => {
    let c: Citizen = { ...spawnCitizen('a', { x: 0, y: 0 }), state: 'exhausted', path, pathIdx: 0 };
    c = stepCitizen(c, 1000, 1);
    expect(c.pos).toEqual({ x: 0, y: 0 });
  });
});
