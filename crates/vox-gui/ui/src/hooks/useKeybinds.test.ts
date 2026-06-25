// @vitest-environment jsdom
import { renderHook } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import { useKeybinds } from './useKeybinds';
import { DEFAULT_BINDINGS } from '../lib/keybinds';
describe('useKeybinds', () => {
  it('fires the bound action on matching keydown', () => {
    const onPalette = vi.fn();
    renderHook(() => useKeybinds({ 'open-palette': onPalette }, DEFAULT_BINDINGS));
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'k', metaKey: true, cancelable: true }));
    expect(onPalette).toHaveBeenCalledTimes(1);
  });
});
