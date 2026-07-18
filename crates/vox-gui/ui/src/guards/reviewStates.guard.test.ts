import { describe, it, expect } from 'vitest';
import { SURFACE_REGISTRY } from '../generated/surfaceRegistry.generated';
import { SURFACE_STATES, VIEWPORTS } from '../../e2e/review/states';

describe('review state registry completeness', () => {
  const known = SURFACE_REGISTRY.filter((e) => e.viewKey != null).map((e) => e.viewKey as string);

  it('every registry surface has an EXPLICIT states entry (even just [DEFAULT])', () => {
    const missing = known.filter((k) => !(k in SURFACE_STATES));
    expect(missing, `add states (or [DEFAULT]) for: ${missing}`).toEqual([]);
  });
  it('declared states only reference registered surfaces (no typo rot)', () => {
    const unknown = Object.keys(SURFACE_STATES).filter((k) => !known.includes(k));
    expect(unknown).toEqual([]);
  });
  it('viewports are the spec trio', () => {
    expect(VIEWPORTS.map((v) => v.name)).toEqual(['wide', 'laptop', 'compact']);
  });
  it('viewport constraints reference real viewport names', () => {
    const names = new Set(VIEWPORTS.map((v) => v.name));
    for (const states of Object.values(SURFACE_STATES)) {
      for (const s of states) for (const v of s.viewports ?? []) expect(names.has(v)).toBe(true);
    }
  });
});
