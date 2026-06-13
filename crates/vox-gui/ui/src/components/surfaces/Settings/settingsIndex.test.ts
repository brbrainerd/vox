import { describe, it, expect } from 'vitest';
import { SETTINGS_INDEX, searchSettings } from './settingsIndex';

describe('SETTINGS_INDEX', () => {
  it('has unique ids and a section for every entry', () => {
    const ids = SETTINGS_INDEX.map(s => s.id);
    expect(new Set(ids).size).toBe(ids.length);
    expect(SETTINGS_INDEX.every(s => s.section.length > 0)).toBe(true);
  });
});

describe('searchSettings', () => {
  it('matches on label, hint, and keywords case-insensitively', () => {
    expect(searchSettings('OPENROUTER').length).toBeGreaterThan(0);
    expect(searchSettings('parallel').some(s => s.section === 'llm')).toBe(true);
    expect(searchSettings('zzzznothing')).toHaveLength(0);
  });

  it('returns nothing for an empty query', () => {
    expect(searchSettings('')).toHaveLength(0);
    expect(searchSettings('   ')).toHaveLength(0);
  });
});
