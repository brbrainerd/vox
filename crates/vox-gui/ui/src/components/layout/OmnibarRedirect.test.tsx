// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { redirectSearchViewToOmnibar } from './omnibarRedirect';

describe('redirectSearchViewToOmnibar', () => {
  it('opens the Omnibar and clears #view=search instead of navigating to a dead surface', () => {
    const openOmnibar = vi.fn();
    const navigateTo = vi.fn();
    const handled = redirectSearchViewToOmnibar('search', { openOmnibar, navigateTo, fallbackChild: 'memory' });
    expect(handled).toBe(true);
    expect(openOmnibar).toHaveBeenCalledTimes(1);
    expect(navigateTo).toHaveBeenCalledWith('memory'); // park on a real child, not the dead 'search' shell
  });

  it('passes through non-search views untouched', () => {
    const openOmnibar = vi.fn();
    const navigateTo = vi.fn();
    const handled = redirectSearchViewToOmnibar('approvals', { openOmnibar, navigateTo, fallbackChild: 'memory' });
    expect(handled).toBe(false);
    expect(openOmnibar).not.toHaveBeenCalled();
    expect(navigateTo).not.toHaveBeenCalled();
  });
});
