// @vitest-environment jsdom
import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import React from 'react';
import { DueNudge } from './DueNudge';

describe('DueNudge', () => {
  it('renders active due actions nudge count', () => {
    const onOpen = vi.fn();
    render(<DueNudge count={3} onOpen={onOpen} />);
    expect(screen.getByText(/3 actions due/i)).toBeInTheDocument();
    
    fireEvent.click(screen.getByRole('button'));
    expect(onOpen).toHaveBeenCalled();
  });

  it('renders empty encouraging state when count is 0', () => {
    render(<DueNudge count={0} onOpen={vi.fn()} />);
    expect(screen.queryByText(/actions due/i)).not.toBeInTheDocument();
    expect(screen.getByText(/all caught up/i)).toBeInTheDocument();
  });
});
