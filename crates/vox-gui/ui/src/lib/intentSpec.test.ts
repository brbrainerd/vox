import { describe, it, expect } from 'vitest';
import { EMPTY_INTENT, hasIntent, composeDescription, effortToPriority, type IntentFields } from './intentSpec';

const intent = (over: Partial<IntentFields>): IntentFields => ({ ...EMPTY_INTENT, ...over });

describe('intentSpec', () => {
  it('plain text passes through untouched when no intent fields are set', () => {
    expect(composeDescription('fix the login bug', EMPTY_INTENT)).toBe('fix the login bug');
    expect(hasIntent(EMPTY_INTENT)).toBe(false);
  });
  it('appends markdown sections for filled fields only', () => {
    const d = composeDescription('fix login', intent({ goal: 'users stay signed in', acceptance: 'refresh keeps session' }));
    expect(d).toBe('fix login\n\n## Goal\nusers stay signed in\n\n## Acceptance criteria\nrefresh keeps session');
    expect(d).not.toContain('## Constraints');
  });
  it('goal alone can carry the task (empty free text)', () => {
    expect(composeDescription('', intent({ goal: 'ship dark mode' }))).toBe('ship dark mode');
  });
  it('maps effort onto the backend TaskPriority strings', () => {
    expect(effortToPriority('urgent')).toBe('urgent');
    expect(effortToPriority('normal')).toBe('normal');
    expect(effortToPriority('background')).toBe('background');
    expect(effortToPriority('')).toBeNull();
  });
  it('a constraints-only intent with no head text starts at the section heading, not a blank line', () => {
    // Callers are expected to gate submission on non-empty text/goal (hasIntent()),
    // but this pins the standalone behavior of the module itself.
    const d = composeDescription('', intent({ constraints: 'no breaking changes' }));
    expect(d).toBe('## Constraints\nno breaking changes');
  });
});
