import { describe, it, expect } from 'vitest';
import { normalizeTheme } from './theme';

describe('normalizeTheme', () => {
  it('keeps known accent themes', () => {
    expect(normalizeTheme('void')).toBe('void');
    expect(normalizeTheme('glacier')).toBe('glacier');
  });
  it('accepts high-contrast', () => {
    expect(normalizeTheme('high-contrast')).toBe('high-contrast');
  });
  it('defaults unknown/empty to arcane', () => {
    expect(normalizeTheme('nope')).toBe('arcane');
    expect(normalizeTheme(null)).toBe('arcane');
    expect(normalizeTheme(undefined)).toBe('arcane');
  });
});
