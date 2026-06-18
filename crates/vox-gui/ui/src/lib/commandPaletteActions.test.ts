import { describe, expect, it, vi } from 'vitest';
import { handleSubmitTaskAction } from './commandPaletteActions';

describe('handleSubmitTaskAction', () => {
  it('navigates to chat then schedules composer focus', () => {
    const navigateTo = vi.fn();
    const focusComposer = vi.fn();
    const scheduleFocus = vi.fn((fn: () => void) => fn());

    handleSubmitTaskAction(navigateTo, focusComposer, scheduleFocus);

    expect(navigateTo).toHaveBeenCalledWith('chat');
    expect(scheduleFocus).toHaveBeenCalledOnce();
    expect(focusComposer).toHaveBeenCalledOnce();
  });

  it('focuses composer only after navigation', () => {
    const order: string[] = [];
    const navigateTo = vi.fn(() => order.push('navigate'));
    const focusComposer = vi.fn(() => order.push('focus'));
    const scheduleFocus = (fn: () => void) => fn();

    handleSubmitTaskAction(navigateTo, focusComposer, scheduleFocus);

    expect(order).toEqual(['navigate', 'focus']);
  });
});
