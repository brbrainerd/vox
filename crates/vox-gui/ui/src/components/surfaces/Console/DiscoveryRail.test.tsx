// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, waitFor } from '@testing-library/react';
import React from 'react';

const recordMock = vi.fn().mockResolvedValue(undefined);
vi.mock('../../../transport', () => ({
  discoveryHelp: vi.fn().mockResolvedValue({
    action_id: 'vox.scientia.review',
    about: 'Review queued nanopubs',
    args: [{ name: '--limit', help: 'max items', required: false }],
    example: 'vox scientia review',
  }),
  discoveryRecord: (...a: unknown[]) => recordMock(...a),
}));

import { DiscoveryRail } from './DiscoveryRail';

describe('DiscoveryRail', () => {
  beforeEach(() => {
    cleanup();
    recordMock.mockClear();
  });

  it('renders help for the active action id', async () => {
    render(<DiscoveryRail actionId="vox.scientia.review" nowMs={1000} />);
    await waitFor(() => expect(screen.getByText('Review queued nanopubs')).toBeTruthy());
    expect(screen.getByText('vox scientia review')).toBeTruthy();
  });

  it('records a seen exposure for the displayed action', async () => {
    render(<DiscoveryRail actionId="vox.scientia.review" nowMs={1000} />);
    // The "seen" exposure fires after a 2s dwell timer, so wait past it.
    await waitFor(() => expect(recordMock).toHaveBeenCalled(), { timeout: 3000 });
    expect(recordMock.mock.calls[0][0]).toBe('vox.scientia.review');
    expect(recordMock.mock.calls[0][1]).toBe(false);
  });

  it('announces help updates via a polite live region', async () => {
    render(<DiscoveryRail actionId="vox.scientia.review" nowMs={1000} />);
    const rail = screen.getByLabelText('discovery');
    expect(rail.getAttribute('aria-live')).toBe('polite');
    await waitFor(() => expect(screen.getByText('Review queued nanopubs')).toBeTruthy());
  });
});
