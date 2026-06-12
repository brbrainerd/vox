import { describe, it, expect } from 'vitest';
import { createSession, closeSession, renameSession } from './sessions';

describe('createSession', () => {
  it('mints unique gui- ids and numbered default titles', () => {
    const a = createSession([]);
    const b = createSession([a]);
    expect(a.id).toMatch(/^gui-/);
    expect(b.id).not.toBe(a.id);
    expect(a.title).toBe('Chat 1');
    expect(b.title).toBe('Chat 2');
  });

  it('attaches optional scope paths', () => {
    const s = createSession([], { scopePaths: ['crates/vox-gui'] });
    expect(s.scopePaths).toEqual(['crates/vox-gui']);
  });
});

describe('closeSession', () => {
  it('removes the session and nominates a neighbor as next active', () => {
    const a = createSession([]);
    const b = createSession([a]);
    const { sessions, nextActiveId } = closeSession([a, b], a.id);
    expect(sessions).toHaveLength(1);
    expect(nextActiveId).toBe(b.id);
  });

  it('never closes the last session — returns it unchanged', () => {
    const a = createSession([]);
    const { sessions, nextActiveId } = closeSession([a], a.id);
    expect(sessions).toHaveLength(1);
    expect(nextActiveId).toBe(a.id);
  });
});

describe('renameSession', () => {
  it('renames by id and ignores unknown ids and blank titles', () => {
    const a = createSession([]);
    expect(renameSession([a], a.id, 'Mesh work')[0].title).toBe('Mesh work');
    expect(renameSession([a], 'nope', 'x')[0].title).toBe(a.title);
    expect(renameSession([a], a.id, '   ')[0].title).toBe(a.title);
  });
});
