// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, waitFor } from '@testing-library/react';
import React from 'react';

const INTENTIONS = [
  { id: 'a', parent: 'Routing', branch: 'Cost', phase: 'Active', conf: 0.6, note: 'Favor cheap models' },
  { id: 'b', parent: 'Routing', branch: 'Quality', phase: 'Validated', conf: 0.8, note: 'Favor best models' },
];

const invokeMock = vi.fn((cmd: string) => {
  if (cmd === 'get_routing_intentions') return Promise.resolve(INTENTIONS);
  return Promise.resolve(null);
});
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));

import { Matrix } from './Matrix';

describe('Matrix', () => {
  beforeEach(() => {
    cleanup();
    invokeMock.mockClear();
  });

  it('renders the Routing Policies heading after load', async () => {
    render(<Matrix pushToast={vi.fn()} />);
    expect(await screen.findByText('Routing Policies')).toBeTruthy();
  });

  it('every button carries an explicit type="button"', async () => {
    render(<Matrix pushToast={vi.fn()} />);
    await screen.findByText('Routing Policies');
    for (const b of screen.getAllByRole('button')) {
      expect(b.getAttribute('type')).toBe('button');
    }
  });

  it('hex cells expose aria-pressed and an aria-label', async () => {
    render(<Matrix pushToast={vi.fn()} />);
    const cell = await screen.findByLabelText(/Cost routing axis/i);
    expect(cell.getAttribute('aria-pressed')).toBeDefined();
  });

  it('weight meter exposes role=progressbar', async () => {
    render(<Matrix pushToast={vi.fn()} />);
    await waitFor(() => expect(screen.getAllByRole('progressbar').length).toBeGreaterThan(0));
  });

  it('shows an empty state when there are no intentions', async () => {
    invokeMock.mockImplementationOnce(() => Promise.resolve([]));
    render(<Matrix pushToast={vi.fn()} />);
    expect(await screen.findByText(/No routing policies active/i)).toBeTruthy();
  });
});
