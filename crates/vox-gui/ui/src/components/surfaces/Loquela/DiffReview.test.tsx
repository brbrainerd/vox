// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import React from 'react';
import { DiffReview } from './DiffReview';

describe('DiffReview', () => {
  it('renders a loading state', () => {
    render(<DiffReview diff="" loading onClose={() => {}} />);
    expect(screen.getByText(/loading worktree diff/i)).toBeDefined();
  });

  it('renders an empty state when the diff is blank', () => {
    render(<DiffReview diff="   " onClose={() => {}} />);
    expect(screen.getByText(/no unstaged changes/i)).toBeDefined();
  });

  it('close control is labeled, typed, and fires onClose', () => {
    const onClose = vi.fn();
    render(<DiffReview diff="diff --git a b" onClose={onClose} />);
    const btn = screen.getByRole('button', { name: /close diff/i });
    expect(btn.getAttribute('type')).toBe('button');
    fireEvent.click(btn);
    expect(onClose).toHaveBeenCalled();
  });

  it('decorative icons inside labeled buttons are aria-hidden', () => {
    render(<DiffReview diff="diff --git a b" onClose={() => {}} />);
    const btn = screen.getByRole('button', { name: /close diff/i });
    expect(btn.querySelector('svg')?.getAttribute('aria-hidden')).toBe('true');
  });
});
