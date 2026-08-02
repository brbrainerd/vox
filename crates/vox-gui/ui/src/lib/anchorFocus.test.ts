// @vitest-environment jsdom
import { describe, it, expect, afterEach } from 'vitest';
import { scrollAndFocusAnchor } from './anchorFocus';

describe('scrollAndFocusAnchor', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('scrolls the target into view and moves keyboard focus to it', () => {
    const el = document.createElement('div');
    el.id = 'keys-secrets-section';
    el.tabIndex = -1;
    let scrolled = false;
    el.scrollIntoView = () => {
      scrolled = true;
    };
    document.body.appendChild(el);

    scrollAndFocusAnchor('keys-secrets-section');

    expect(scrolled).toBe(true);
    expect(document.activeElement).toBe(el);
  });

  it('is a no-op when the target does not exist', () => {
    expect(() => scrollAndFocusAnchor('does-not-exist')).not.toThrow();
  });
});
