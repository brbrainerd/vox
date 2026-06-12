import { describe, it, expect } from 'vitest';
import { buildSlashEntries, BUILTIN_SLASH } from './slashCommands';

describe('buildSlashEntries', () => {
  it('appends skills after builtins as /name with kind=skill', () => {
    const entries = buildSlashEntries([
      { id: 'vox.tdd', name: 'test-driven-development', description: 'RED-GREEN-REFACTOR' },
    ]);
    expect(entries.slice(0, BUILTIN_SLASH.length)).toEqual(BUILTIN_SLASH);
    const skill = entries.find((e) => e.cmd === '/test-driven-development');
    expect(skill).toMatchObject({
      kind: 'skill',
      skillId: 'vox.tdd',
      desc: 'RED-GREEN-REFACTOR',
    });
  });

  it('dedupes a skill that collides with a builtin command', () => {
    const entries = buildSlashEntries([
      { id: 'x', name: 'plan', description: 'collides with /plan' },
    ]);
    expect(entries.filter((e) => e.cmd === '/plan')).toHaveLength(1);
    expect(entries.find((e) => e.cmd === '/plan')!.kind).toBe('builtin');
  });

  it('falls back to name as skillId when id is absent', () => {
    const entries = buildSlashEntries([{ name: 'brainstorming', description: 'design' }]);
    expect(entries.find((e) => e.cmd === '/brainstorming')!.skillId).toBe('brainstorming');
  });

  it('tolerates malformed skill records', () => {
    expect(buildSlashEntries([{} as any, null as any, undefined as any])).toEqual(BUILTIN_SLASH);
  });

  it('returns builtins unchanged for empty/nullish input', () => {
    expect(buildSlashEntries([])).toEqual(BUILTIN_SLASH);
    expect(buildSlashEntries(null as any)).toEqual(BUILTIN_SLASH);
  });
});
