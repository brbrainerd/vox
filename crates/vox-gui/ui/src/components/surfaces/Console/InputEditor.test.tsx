// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, cleanup, waitFor } from '@testing-library/react';
import React from 'react';

vi.mock('../../../transport', () => ({
  discoverySuggest: vi.fn().mockResolvedValue([
    { action_id: 'vox.config.show', completion: 'config show', about: 'show config' },
  ]),
}));

import { InputEditor } from './InputEditor';

describe('InputEditor', () => {
  beforeEach(() => cleanup());

  it('shows ghost text for the top suggestion as you type', async () => {
    render(<InputEditor onSubmit={vi.fn()} onActiveSuggestion={vi.fn()} />);
    const input = screen.getByRole('textbox');
    fireEvent.change(input, { target: { value: 'vox config' } });
    // Ghost shows only the completion remainder after what's typed ("config").
    await waitFor(() => expect(screen.getByTestId('ghost').textContent).toContain('show'));
  });

  it('accepts ghost text on Tab and submits on Enter', async () => {
    const onSubmit = vi.fn();
    render(<InputEditor onSubmit={onSubmit} onActiveSuggestion={vi.fn()} />);
    const input = screen.getByRole('textbox') as HTMLInputElement;
    fireEvent.change(input, { target: { value: 'vox config' } });
    await waitFor(() => expect(screen.getByTestId('ghost').textContent).toBeTruthy());
    fireEvent.keyDown(input, { key: 'Tab' });
    expect(input.value).toBe('vox config show');
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(onSubmit).toHaveBeenCalledWith('vox config show');
  });
});
