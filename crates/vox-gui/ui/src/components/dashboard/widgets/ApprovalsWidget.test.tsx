// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import React from 'react';

const invokeMcpTool = vi.fn();
vi.mock('../../../transport', () => ({
  voxTransport: { invokeMcpTool: (...a: unknown[]) => invokeMcpTool(...a) },
}));

import { ApprovalsWidget } from './ApprovalsWidget';

beforeEach(() => {
  invokeMcpTool.mockReset();
});

describe('ApprovalsWidget', () => {
  it('sources the count from the real vox_pending_approvals feed, not status.alerts', async () => {
    invokeMcpTool.mockResolvedValue({
      is_error: false,
      result: {
        approvals: [
          { approval_id: 'ap1', tool: 'shell', summary: 'run x', requested_at_ms: 0 },
          { approval_id: 'ap2', tool: 'shell', summary: 'run y', requested_at_ms: 0 },
        ],
      },
    });
    render(<ApprovalsWidget />);
    await waitFor(() => expect(invokeMcpTool).toHaveBeenCalledWith('vox_pending_approvals', {}));
    await waitFor(() => expect(screen.getByText('2')).toBeDefined());
    expect(screen.getByText('awaiting you')).toBeDefined();
  });

  it('shows "all clear" when there are no pending approvals', async () => {
    invokeMcpTool.mockResolvedValue({ is_error: false, result: { approvals: [] } });
    render(<ApprovalsWidget />);
    await waitFor(() => expect(invokeMcpTool).toHaveBeenCalled());
    await waitFor(() => expect(screen.getByText('all clear')).toBeDefined());
    expect(screen.getByText('0')).toBeDefined();
  });
});
