import { describe, it, expect } from 'vitest';
import { ACTION_REGISTRY, chordFromEvent, matchAction, DEFAULT_BINDINGS } from './keybinds';
describe('keybinds', () => {
  it('registry lists only real, dispatchable actions', () => {
    expect(ACTION_REGISTRY.map(a => a.id)).toContain('open-palette');
    expect(ACTION_REGISTRY.map(a => a.id)).toContain('pause-resume-agent');
    for (const a of ACTION_REGISTRY) expect(DEFAULT_BINDINGS[a.id]).toBeTruthy();
  });
  it('chordFromEvent normalizes modifiers', () => {
    expect(chordFromEvent({ key: 'k', metaKey: true, ctrlKey: false, shiftKey: false, altKey: false } as any)).toBe('Mod+K');
    expect(chordFromEvent({ key: 'B', metaKey: false, ctrlKey: true, shiftKey: false, altKey: false } as any)).toBe('Mod+B');
  });
  it('matchAction resolves a chord to an action id via bindings', () => {
    expect(matchAction('Mod+K', DEFAULT_BINDINGS)).toBe('open-palette');
    expect(matchAction('Mod+J', DEFAULT_BINDINGS)).toBeNull();
  });
});
