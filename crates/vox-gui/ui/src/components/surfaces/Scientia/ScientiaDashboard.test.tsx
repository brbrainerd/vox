// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, waitFor } from '@testing-library/react';
import React from 'react';

// The dashboard fetches a queue snapshot via execute_command and a cost rollup.
// Resolve both so the component renders its populated state.
const SNAP = {
  candidates: { total: 3, by_class: { perf: 2 }, top_5_by_confidence: [] },
  claims_pending: { verifiable: 1, abstained: 0, extraction_running: 0 },
  manifests_in_reply_window: [],
  retraction_queue: [],
  stalls: [],
};
const invokeMock = vi.fn((cmd: string, args?: { path?: string[] }) => {
  if (cmd === 'execute_command' && args?.path?.[1] === 'dashboard') {
    return Promise.resolve({ exit_code: 0, stdout: JSON.stringify(SNAP), stderr: '' });
  }
  if (cmd === 'execute_command' && args?.path?.[1] === 'cost') {
    return Promise.resolve({ exit_code: 0, stdout: JSON.stringify({
      this_quarter: { total_usd: 0 }, by_provider: [], per_finding_average_usd: 0,
    }), stderr: '' });
  }
  return Promise.resolve({ exit_code: 0, stdout: '{}', stderr: '' });
});
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));
vi.mock('../../../transport', () => ({
  listenScientiaQueue: vi.fn().mockRejectedValue(new Error('not in tauri')),
}));

import { ScientiaDashboard } from './ScientiaDashboard';

describe('ScientiaDashboard', () => {
  beforeEach(() => {
    cleanup();
    invokeMock.mockClear();
  });

  it('Refresh control is an explicit type="button"', async () => {
    render(<ScientiaDashboard pushToast={vi.fn()} />);
    const btn = await screen.findByRole('button', { name: /refresh/i });
    expect(btn.getAttribute('type')).toBe('button');
  });

  it('marks the snapshot region as an aria-live polite region', async () => {
    const { container } = render(<ScientiaDashboard pushToast={vi.fn()} />);
    await waitFor(() => {
      expect(container.querySelector('[aria-live="polite"]')).toBeTruthy();
    });
  });
});
