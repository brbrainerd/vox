import { describe, it, expect } from 'vitest';
import { contrastRatio } from './contrast';
import { tokens } from '../styles/tokens.generated';

const bg = tokens.color.bg.base;       // #09090b
const surface = tokens.color.bg.surface;

describe('contrastRatio', () => {
  it('computes ~21:1 for black on white', () => {
    expect(contrastRatio('#000000', '#ffffff')).toBeCloseTo(21, 0);
  });
  it('computes 1:1 for identical colors', () => {
    expect(contrastRatio('#445566', '#445566')).toBeCloseTo(1, 5);
  });
});

describe('token pairs meet WCAG AA', () => {
  it('text.primary on bg.base >= 4.5', () => {
    expect(contrastRatio(tokens.color.text.primary, bg)).toBeGreaterThanOrEqual(4.5);
  });
  it('text.secondary on bg.base >= 4.5', () => {
    expect(contrastRatio(tokens.color.text.secondary, bg)).toBeGreaterThanOrEqual(4.5);
  });
  it('text.muted on bg.base >= 4.5', () => {
    expect(contrastRatio(tokens.color.text.muted, bg)).toBeGreaterThanOrEqual(4.5);
  });
  it('accent.default on bg.surface >= 3 (UI component)', () => {
    expect(contrastRatio(tokens.color.accent.default, surface)).toBeGreaterThanOrEqual(3);
  });
  it('every status color on bg.surface >= 3', () => {
    for (const c of Object.values(tokens.color.status)) {
      expect(contrastRatio(c, surface)).toBeGreaterThanOrEqual(3);
    }
  });
});
