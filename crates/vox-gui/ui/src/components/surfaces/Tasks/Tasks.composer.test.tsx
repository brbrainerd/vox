// @vitest-environment jsdom
import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import React from 'react';
import { TaskComposer } from './TaskComposer';

describe('TaskComposer', () => {
  it('submits typed intent via onSubmit', () => {
    const onSubmit = vi.fn();
    render(<TaskComposer onSubmit={onSubmit} />);
    fireEvent.change(screen.getByPlaceholderText(/add a task/i), { target: { value: 'ship it' } });
    fireEvent.click(screen.getByRole('button', { name: /add/i }));
    expect(onSubmit).toHaveBeenCalledWith('ship it');
  });
});
