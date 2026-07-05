// crates/vox-gui/ui/src/components/gamify/urbs/citizens.ts
import type { Pt } from './types';

export type CitizenState = 'idle' | 'commuting' | 'working' | 'exhausted';

export interface Citizen {
  id: string;
  pos: { x: number; y: number }; // tile coords, fractional while walking
  state: CitizenState;
  path: Pt[];
  pathIdx: number;
  /** Set when working — the building the citizen stands at. */
  targetPath?: string;
}

const WALK_TILES_PER_SEC = 2;

export function spawnCitizen(id: string, at: Pt): Citizen {
  return { id, pos: { ...at }, state: 'idle', path: [], pathIdx: 0 };
}

/** Advance one citizen by dtMs · speed. Pure — the shell owns the rAF loop. */
export function stepCitizen(c: Citizen, dtMs: number, speed: number): Citizen {
  if (c.state !== 'commuting' || speed <= 0) return c;
  let remaining = (dtMs / 1000) * WALK_TILES_PER_SEC * speed;
  const pos = { ...c.pos };
  let idx = c.pathIdx;
  while (remaining > 0 && idx < c.path.length - 1) {
    const next = c.path[idx + 1];
    const dx = next.x - pos.x;
    const dy = next.y - pos.y;
    const dist = Math.hypot(dx, dy);
    if (dist <= remaining) {
      pos.x = next.x; pos.y = next.y;
      remaining -= dist;
      idx++;
    } else {
      pos.x += (dx / dist) * remaining;
      pos.y += (dy / dist) * remaining;
      remaining = 0;
    }
  }
  const arrived = idx >= c.path.length - 1;
  return { ...c, pos, pathIdx: idx, state: arrived ? 'working' : 'commuting' };
}
