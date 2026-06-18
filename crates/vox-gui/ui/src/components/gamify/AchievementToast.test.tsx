// @vitest-environment jsdom
import { describe, it, expect, vi, afterEach } from 'vitest';
import React from 'react';
import { render, screen, act } from '@testing-library/react';
import { AchievementToast } from './AchievementToast';

describe('AchievementToast', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it('renders toast item with title', () => {
    render(<AchievementToast title="Quest complete" />);
    expect(screen.getByText('Quest complete')).toBeInTheDocument();
  });

  it('calls onDismiss after autoDismissMs when provided', () => {
    vi.useFakeTimers();
    const onDismiss = vi.fn();
    render(
      <AchievementToast title="Level up" autoDismissMs={3000} onDismiss={onDismiss} />,
    );
    expect(onDismiss).not.toHaveBeenCalled();
    act(() => {
      vi.advanceTimersByTime(3000);
    });
    expect(onDismiss).toHaveBeenCalledTimes(1);
  });
});
