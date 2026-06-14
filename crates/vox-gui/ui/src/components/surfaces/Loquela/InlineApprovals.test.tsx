// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import React from 'react';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

import { InlineApprovals } from './InlineApprovals';

const envelope = (rows: unknown[]) => ({
  is_error: false,
  result: { approvals: rows },
});

const row = {
  approval_id: 'a1',
  tool: 'vox_write_file',
  summary: 'Write to disk',
  requested_at_ms: 0,
};

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue(envelope([row]));
});

describe('InlineApprovals', () => {
  it('renders pending approvals as a labeled live list', async () => {
    render(<InlineApprovals pushToast={() => {}} />);
    await waitFor(() => expect(screen.getByText('vox_write_file')).toBeDefined());
    const region = screen.getByRole('region', { name: /approval required/i });
    expect(region.getAttribute('aria-live')).toBe('polite');
    expect(screen.getByRole('list')).toBeDefined();
    expect(screen.getAllByRole('listitem').length).toBeGreaterThan(0);
  });

  it('every control carries an explicit type="button"', async () => {
    render(<InlineApprovals pushToast={() => {}} onViewAll={() => {}} />);
    await waitFor(() => expect(screen.getByText('vox_write_file')).toBeDefined());
    for (const b of screen.getAllByRole('button')) {
      expect(b.getAttribute('type')).toBe('button');
    }
  });
});
