// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, fireEvent, waitFor } from '@testing-library/react';
import React from 'react';

const transport = vi.hoisted(() => ({
  getSelectionPolicy: vi.fn(() =>
    Promise.resolve(JSON.stringify({ steps: [{ emphasize_axis: { axis: 'intelligence', weight: 70 } }] })),
  ),
  setSelectionPolicy: vi.fn(() => Promise.resolve()),
  listModels: vi.fn(() => Promise.resolve([{ id: 'm-1' }])),
}));
vi.mock('../../../transport', () => ({ voxTransport: transport }));

import { PriorityChainEditor } from './PriorityChainEditor';

describe('PriorityChainEditor', () => {
  beforeEach(() => {
    cleanup();
    transport.getSelectionPolicy.mockClear();
    transport.setSelectionPolicy.mockClear();
  });

  async function ready() {
    render(<PriorityChainEditor pushToast={vi.fn()} />);
    await waitFor(() => expect(screen.getByText('Model priority chain')).toBeTruthy());
  }

  it('renders the editor heading', async () => {
    await ready();
    expect(screen.getByText('Model priority chain')).toBeTruthy();
  });

  it('every button carries an explicit type="button"', async () => {
    await ready();
    for (const b of screen.getAllByRole('button')) {
      expect(b.getAttribute('type')).toBe('button');
    }
  });

  it('gives the move/remove icon buttons aria-labels', async () => {
    await ready();
    expect(screen.getByRole('button', { name: /remove step 1/i })).toBeTruthy();
  });

  it('labels the add-step weight range input', async () => {
    await ready();
    fireEvent.click(screen.getByText('+ add step'));
    expect(screen.getByLabelText('Axis weight')).toBeTruthy();
  });
});
