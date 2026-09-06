// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { Glass } from './Glass';

describe('Glass Primitive', () => {
  it('applies padding based on size prop', () => {
    const { rerender } = render(<Glass size="sm" data-testid="g">Content</Glass>);
    expect(screen.getByTestId('g')).toHaveClass('p-3');

    rerender(<Glass size="lg" data-testid="g">Content</Glass>);
    expect(screen.getByTestId('g')).toHaveClass('p-6');
  });

  it('adds interactive hover states when interactive prop is true', () => {
    render(<Glass interactive data-testid="g">Clickable</Glass>);
    expect(screen.getByTestId('g')).toHaveClass('cursor-pointer');
  });

  it('uses an opaque background, not a low-alpha overlay tint (Firefox backdrop-blur-sm compositing bug)', () => {
    render(<Glass data-testid="g">Content</Glass>);
    const el = screen.getByTestId('g');
    expect(el).toHaveClass('bg-overlay-solid');
    expect(el).not.toHaveClass('bg-overlay-subtle');
  });

  it('interactive hover state stays opaque too (no translucent hover regression)', () => {
    render(<Glass interactive data-testid="g">Clickable</Glass>);
    const el = screen.getByTestId('g');
    expect(el).not.toHaveClass('hover:bg-overlay-subtle');
  });
});
