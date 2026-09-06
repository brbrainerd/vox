import { describe, expect, it } from 'vitest';
import { cn } from './cn';

describe('cn', () => {
  it('joins truthy class names', () => {
    expect(cn('a', 'b')).toBe('a b');
  });
  it('drops falsy values', () => {
    expect(cn('a', false, null, undefined, 'b')).toBe('a b');
  });
  it('merges conflicting tailwind classes (last wins)', () => {
    expect(cn('px-2', 'px-4')).toBe('px-4');
  });
});

// PR #495: tailwind-merge v3 on Tailwind v3 silently drops `focus-visible:outline`,
// leaving outline-style: none and removing the keyboard focus ring from every
// button. This asserts the merged output still carries BOTH an outline-style
// source and a width. On Tailwind v3 that means the bare `outline` class must
// survive; on v4 `outline-2` alone implies solid, so either shape is acceptable.
describe('cn() preserves the focus ring', () => {
  const BUTTON_BASE =
    'inline-flex items-center justify-center font-medium tracking-wide transition-all ' +
    'focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 ' +
    'focus-visible:outline-brass';

  it('keeps a visible focus outline width', () => {
    expect(cn(BUTTON_BASE)).toContain('focus-visible:outline-2');
  });

  it('keeps a source for outline-style', () => {
    const out = cn(BUTTON_BASE);
    // v3: the bare `outline` class. v4: `outline-2` implies solid on its own.
    const hasBareOutline = /(^|\s)focus-visible:outline(\s|$)/.test(out);
    const isV4 = Number(
      require('tailwindcss/package.json').version.split('.')[0],
    ) >= 4;
    expect(hasBareOutline || isV4).toBe(true);
  });
});
