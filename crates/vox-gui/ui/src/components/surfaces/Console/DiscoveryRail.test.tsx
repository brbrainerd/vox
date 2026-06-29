// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import React from 'react';

const recordMock = vi.fn().mockResolvedValue(undefined);
const recordGamifyMock = vi.fn().mockResolvedValue(null);
vi.mock('../../../transport', () => ({
  discoveryHelp: vi.fn().mockResolvedValue({
    action_id: 'vox.scientia.review',
    about: 'Review queued nanopubs',
    args: [{ name: '--limit', help: 'max items', required: false }],
    example: 'vox scientia review',
  }),
  discoveryRecord: (...a: unknown[]) => recordMock(...a),
}));
vi.mock('../../../lib/gamifyGuiEvents', () => ({
  recordGamifyGuiEvent: (...args: unknown[]) => recordGamifyMock(...args),
}));

import { LanguageProvider } from '../../../hooks/useLanguage';
import { DiscoveryRail } from './DiscoveryRail';

describe('DiscoveryRail', () => {
  beforeEach(() => {
    cleanup();
    localStorage.clear();
    recordMock.mockClear();
    recordGamifyMock.mockClear();
  });

  it('renders help for the active action id', async () => {
    render(<LanguageProvider><DiscoveryRail actionId="vox.scientia.review" nowMs={1000} /></LanguageProvider>);
    await waitFor(() => expect(screen.getByText('Review queued nanopubs')).toBeTruthy());
    expect(screen.getByText('vox scientia review')).toBeTruthy();
  });

  it('records a seen exposure for the displayed action', async () => {
    render(<LanguageProvider><DiscoveryRail actionId="vox.scientia.review" nowMs={1000} /></LanguageProvider>);
    // The "seen" exposure fires after a 2s dwell timer, so wait past it.
    await waitFor(() => expect(recordMock).toHaveBeenCalled(), { timeout: 3000 });
    expect(recordMock.mock.calls[0][0]).toBe('vox.scientia.review');
    expect(recordMock.mock.calls[0][1]).toBe(false);
  });

  it('fires discovery_action_used when Use is clicked', async () => {
    const onUse = vi.fn();
    render(
      <LanguageProvider>
        <DiscoveryRail
          actionId="vox.scientia.review"
          nowMs={1000}
          gamifyEnabled
          onUseAction={onUse}
        />
      </LanguageProvider>,
    );
    await waitFor(() => expect(screen.getByRole('button', { name: /Use suggested action/i })).toBeTruthy());
    screen.getByRole('button', { name: /Use suggested action/i }).click();
    expect(recordGamifyMock).toHaveBeenCalledWith(
      'discovery_action_used',
      { action_id: 'vox.scientia.review' },
      { enabled: true },
    );
    expect(onUse).toHaveBeenCalledWith('vox scientia review', 'vox.scientia.review');
  });

  it('announces help updates via a polite live region', async () => {
    render(<LanguageProvider><DiscoveryRail actionId="vox.scientia.review" nowMs={1000} /></LanguageProvider>);
    const rail = screen.getByLabelText('discovery');
    expect(rail.getAttribute('aria-live')).toBe('polite');
    await waitFor(() => expect(screen.getByText('Review queued nanopubs')).toBeTruthy());
  });

  it('can collapse and expand the discovery rail with aria-expanded', async () => {
    const user = userEvent.setup();
    render(<LanguageProvider><DiscoveryRail actionId="vox.scientia.review" nowMs={1000} /></LanguageProvider>);
    await waitFor(() => expect(screen.getByText('Review queued nanopubs')).toBeTruthy());

    const collapse = screen.getByRole('button', { name: /collapse discovery rail/i });
    expect(collapse.getAttribute('aria-expanded')).toBe('true');
    await user.click(collapse);
    expect(screen.queryByText('Review queued nanopubs')).toBeNull();

    const expand = screen.getByRole('button', { name: /expand discovery rail/i });
    expect(expand.getAttribute('aria-expanded')).toBe('false');
    await user.click(expand);
    await waitFor(() => expect(screen.getByText('Review queued nanopubs')).toBeTruthy());
  });

  it('persists collapsed state in localStorage', async () => {
    const user = userEvent.setup();
    render(<LanguageProvider><DiscoveryRail actionId="vox.scientia.review" nowMs={1000} /></LanguageProvider>);
    await waitFor(() => expect(screen.getByText('Review queued nanopubs')).toBeTruthy());

    await user.click(screen.getByRole('button', { name: /collapse discovery rail/i }));
    expect(localStorage.getItem('gui.console.discovery_rail_collapsed.v1')).toBe('true');
  });
});
