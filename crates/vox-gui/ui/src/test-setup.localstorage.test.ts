// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';

/**
 * Regression guard for the shared test setup's Storage repair.
 *
 * Node >= 22 defines a disabled built-in `localStorage` on globalThis, which
 * shadows the working one jsdom provides. Without the repair in test-setup.ts
 * that lands as `undefined` and roughly a third of this suite dies on
 * "Cannot read properties of undefined". These assertions fail loudly and
 * locally if the repair is removed, a Node upgrade changes the shape again, or
 * vitest changes how it merges jsdom globals.
 */
describe('test environment storage', () => {
  it('exposes a working localStorage on both window and globalThis', () => {
    expect(window.localStorage).toBeDefined();
    expect(globalThis.localStorage).toBeDefined();
    expect(typeof window.localStorage.clear).toBe('function');
  });

  it('round-trips values and reports absent keys as null', () => {
    window.localStorage.clear();
    expect(window.localStorage.getItem('vox.absent')).toBeNull();
    window.localStorage.setItem('vox.k', 'v');
    expect(window.localStorage.getItem('vox.k')).toBe('v');
    expect(window.localStorage.length).toBe(1);
    window.localStorage.removeItem('vox.k');
    expect(window.localStorage.getItem('vox.k')).toBeNull();
  });

  it('clear() empties the store', () => {
    window.localStorage.setItem('a', '1');
    window.localStorage.setItem('b', '2');
    window.localStorage.clear();
    expect(window.localStorage.length).toBe(0);
  });

  it('coerces non-string keys and values the way Storage does', () => {
    window.localStorage.clear();
    window.localStorage.setItem(1 as unknown as string, 2 as unknown as number as string);
    expect(window.localStorage.getItem('1')).toBe('2');
  });

  it('provides sessionStorage too, since the same shadowing applies', () => {
    expect(typeof window.sessionStorage?.setItem).toBe('function');
  });
});
