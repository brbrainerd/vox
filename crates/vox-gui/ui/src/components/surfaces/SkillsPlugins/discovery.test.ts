import { describe, it, expect } from 'vitest';
import { mapDiscoveredSkills } from './discovery';

describe('mapDiscoveredSkills', () => {
  it('maps well-formed rows', () => {
    const rows = mapDiscoveredSkills([
      { id: 'tdd', name: 'tdd', description: 'RED-GREEN', path: '/r/.agents/skills/tdd', installed: true },
      { id: 'brainstorming', name: 'brainstorming', description: 'design', path: '/r/.claude/skills/brainstorming', installed: false },
    ]);
    expect(rows).toHaveLength(2);
    expect(rows[0]).toMatchObject({ id: 'tdd', installed: true });
    expect(rows[1].installed).toBe(false);
  });

  it('falls back to id for a missing name and drops rows with no id', () => {
    const rows = mapDiscoveredSkills([
      { id: 'x', description: 'no name' },
      { name: 'orphan', description: 'no id' },
    ]);
    expect(rows).toHaveLength(1);
    expect(rows[0]).toMatchObject({ id: 'x', name: 'x' });
  });

  it('treats non-boolean installed as false and tolerates non-array input', () => {
    expect(mapDiscoveredSkills([{ id: 'a', installed: 'yes' as any }])[0].installed).toBe(false);
    expect(mapDiscoveredSkills(null)).toEqual([]);
    expect(mapDiscoveredSkills(undefined)).toEqual([]);
    expect(mapDiscoveredSkills({} as any)).toEqual([]);
  });
});
