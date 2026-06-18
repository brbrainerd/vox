import { describe, it, expect } from 'vitest';
import {
  installedSkillToCatalogEntry,
  parseInstalledSkills,
  unwrapMcpToolData,
} from './installedSkills';

describe('installedSkills', () => {
  it('unwrapMcpToolData pulls nested data envelope', () => {
    expect(unwrapMcpToolData({ success: true, data: [{ id: 'a' }] })).toEqual([{ id: 'a' }]);
  });

  it('parseInstalledSkills keeps spec-shaped names only', () => {
    expect(
      parseInstalledSkills([
        { id: 'vox.tdd', name: 'test-driven-development', description: 'RED-GREEN-REFACTOR' },
        { id: 'x', name: 'Bad Name', description: 'skip' },
      ]),
    ).toEqual([
      { id: 'vox.tdd', name: 'test-driven-development', description: 'RED-GREEN-REFACTOR' },
    ]);
  });

  it('installedSkillToCatalogEntry maps to command palette row shape', () => {
    const entry = installedSkillToCatalogEntry({
      id: 'vox.tdd',
      name: 'test-driven-development',
      description: 'RED-GREEN-REFACTOR',
    });
    expect(entry.command).toBe('test-driven-development');
    expect(entry.about).toBe('RED-GREEN-REFACTOR');
    expect(entry.capability_id).toBe('vox.tdd');
  });
});
