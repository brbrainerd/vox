import { describe, it, expect } from 'vitest';
import { xpBarPct, LudusProfile } from './ludus';

describe('xpBarPct', () => {
  it('clamps to 0..100 and renders a percent string', () => {
    expect(xpBarPct(0)).toBe('0%');
    expect(xpBarPct(0.42)).toBe('42%');
    expect(xpBarPct(1)).toBe('100%');
    expect(xpBarPct(1.5)).toBe('100%');
    expect(xpBarPct(-0.2)).toBe('0%');
  });
});
