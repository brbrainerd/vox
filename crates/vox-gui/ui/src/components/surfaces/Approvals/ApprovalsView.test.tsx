// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import React from 'react';

const invokeMock = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

const noopToast = () => {};

import { ApprovalsView } from './ApprovalsView';

function envelope(approvals: unknown[]) {
  return {
    is_error: false,
    result: { approvals },
  };
}

describe('ApprovalsView', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it('shows the empty state when there are no pending approvals', async () => {
    invokeMock.mockResolvedValue(envelope([]));
    render(<ApprovalsView pushToast={noopToast} />);
    await waitFor(() => {
      expect(screen.getByText(/No pending approvals/i)).toBeDefined();
    });
  });

  it('renders the approval queue as a polite live region', async () => {
    invokeMock.mockResolvedValue(
      envelope([
        { approval_id: 'ap-1', tool: 'shell', summary: 'rm -rf', requested_at_ms: Date.now() },
      ]),
    );
    render(<ApprovalsView pushToast={noopToast} />);
    await waitFor(() => {
      expect(screen.getByText('rm -rf')).toBeDefined();
    });
    const region = screen.getByRole('list', { name: /pending approvals/i });
    expect(region.getAttribute('aria-live')).toBe('polite');
  });

  it('gives the approve/reject buttons accessible labels and explicit type', async () => {
    invokeMock.mockResolvedValue(
      envelope([
        { approval_id: 'ap-1', tool: 'shell', summary: 'do thing', requested_at_ms: Date.now() },
      ]),
    );
    render(<ApprovalsView pushToast={noopToast} />);
    const approve = await screen.findByRole('button', { name: /approve do thing|approve ap-1/i });
    const reject = await screen.findByRole('button', { name: /reject do thing|reject ap-1/i });
    expect(approve.getAttribute('type')).toBe('button');
    expect(reject.getAttribute('type')).toBe('button');
  });
});
