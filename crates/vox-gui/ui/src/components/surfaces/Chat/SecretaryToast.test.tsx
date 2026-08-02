// @vitest-environment jsdom
import React from 'react';
import { render, screen, act, fireEvent } from '@testing-library/react';
import { vi, describe, it, expect, beforeEach, afterEach } from 'vitest';
import { SecretaryToast } from './SecretaryToast';

describe('SecretaryToast', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('renders the intent text', () => {
    render(
      <SecretaryToast
        intent="Fix the authentication bug in the login flow"
        itemId="item-1"
        onDismiss={vi.fn()}
        onConfirm={vi.fn()}
      />
    );
    expect(screen.getByText(/Fix the authentication bug/)).toBeInTheDocument();
  });

  it('calls onDismiss when ✕ button is clicked', () => {
    const onDismiss = vi.fn();
    render(
      <SecretaryToast
        intent="Fix something important in the codebase today"
        itemId="item-2"
        onDismiss={onDismiss}
        onConfirm={vi.fn()}
      />
    );
    fireEvent.click(screen.getByRole('button', { name: /dismiss/i }));
    expect(onDismiss).toHaveBeenCalledOnce();
  });

  it('does NOT submit a task until "Add task" is explicitly clicked', () => {
    // Task 0.2: rendering the proposal alone must never call onConfirm.
    const onConfirm = vi.fn();
    render(
      <SecretaryToast
        intent="Implement the new retry logic for HTTP client failures"
        itemId="item-3"
        onDismiss={vi.fn()}
        onConfirm={onConfirm}
      />
    );
    expect(onConfirm).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: /confirm and add task/i }));
    expect(onConfirm).toHaveBeenCalledOnce();
  });

  it('auto-dismisses after 5 seconds without confirming', () => {
    const onDismiss = vi.fn();
    const onConfirm = vi.fn();
    render(
      <SecretaryToast
        intent="Fix the memory leak in the websocket handler today"
        itemId="item-4"
        onDismiss={onDismiss}
        onConfirm={onConfirm}
      />
    );
    act(() => {
      vi.advanceTimersByTime(5001);
    });
    expect(onDismiss).toHaveBeenCalledOnce();
    expect(onConfirm).not.toHaveBeenCalled();
  });

  it('truncates long intent text to 80 characters', () => {
    const long = 'Fix the ' + 'very '.repeat(30) + 'long description';
    render(
      <SecretaryToast
        intent={long}
        itemId="item-5"
        onDismiss={vi.fn()}
        onConfirm={vi.fn()}
      />
    );
    // The rendered text should not exceed 80 chars + ellipsis
    const displayed = screen.getByTestId('secretary-toast-intent').textContent ?? '';
    expect(displayed.length).toBeLessThanOrEqual(83); // 80 + '...'
  });
});
