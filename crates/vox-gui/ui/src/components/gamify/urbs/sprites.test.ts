// crates/vox-gui/ui/src/components/gamify/urbs/sprites.test.ts
import { describe, it, expect } from 'vitest';
import { planAtlas, drawSprite, SPRITE_KEYS, type SpriteKey } from './sprites';

function stubCtx() {
  const calls: string[] = [];
  const rec = (name: string) => (..._a: unknown[]) => { calls.push(name); };
  return {
    calls,
    ctx: new Proxy({}, {
      get: (_t, prop: string) => {
        if (prop === 'canvas') return { width: 4096, height: 4096 };
        return rec(prop);
      },
      set: () => true,
    }) as unknown as CanvasRenderingContext2D,
  };
}

describe('sprite atlas', () => {
  it('plans a rect for every sprite key with no overlaps', () => {
    const plan = planAtlas(2);
    const keys = Object.keys(plan.rects) as SpriteKey[];
    expect(keys.sort()).toEqual([...SPRITE_KEYS].sort());
    const rs = keys.map((k) => plan.rects[k]);
    for (let i = 0; i < rs.length; i++) {
      for (let j = i + 1; j < rs.length; j++) {
        const a = rs[i]; const b = rs[j];
        const disjoint = a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y;
        expect(disjoint).toBe(true);
      }
    }
    expect(plan.width).toBeGreaterThan(0);
    expect(plan.height).toBeGreaterThan(0);
  });

  it('anchor lies inside each sprite rect', () => {
    const plan = planAtlas(1);
    for (const k of SPRITE_KEYS) {
      const r = plan.rects[k];
      expect(r.ax).toBeGreaterThanOrEqual(0);
      expect(r.ax).toBeLessThanOrEqual(r.w);
      expect(r.ay).toBeGreaterThanOrEqual(0);
      expect(r.ay).toBeLessThanOrEqual(r.h);
    }
  });

  it('every sprite drawer issues at least one path/fill call', () => {
    const plan = planAtlas(1);
    for (const k of SPRITE_KEYS) {
      const { calls, ctx } = stubCtx();
      drawSprite(ctx, k, plan.rects[k]);
      expect(calls.length, `sprite ${k} drew nothing`).toBeGreaterThan(0);
    }
  });
});
