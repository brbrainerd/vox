// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, fireEvent, waitFor } from '@testing-library/react';
import React from 'react';

// One pending corpus_scan issue with a target_path, so the confirm action
// should also dispatch a fix proposal.
const PENDING_ROW = {
  id: 42,
  source: 'corpus_scan',
  session_key: null,
  target_path: 'examples/golden/foo.vox',
  detected_at_ms: Date.now() - 1_000,
  category: 'staleness',
  severity: 'medium',
  summary: 'Golden example foo.vox references a retired API',
  evidence_json: '{}',
  status: 'pending',
};

const invokeMock = vi.fn((cmd: string, _args?: unknown) => {
  if (cmd === 'list_harness_issues') return Promise.resolve([PENDING_ROW]);
  if (cmd === 'list_harness_fix_proposals') return Promise.resolve([]);
  if (cmd === 'record_harness_issue_decision') return Promise.resolve();
  if (cmd === 'propose_harness_issue_fix') return Promise.resolve(1);
  return Promise.resolve(null);
});
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));

import { HarnessIssuesPanel } from './HarnessIssuesPanel';

describe('HarnessIssuesPanel', () => {
  beforeEach(() => {
    cleanup();
    invokeMock.mockClear();
  });

  it('renders a pending issue summary', async () => {
    render(<HarnessIssuesPanel pushToast={vi.fn()} />);
    expect(await screen.findByText('Golden example foo.vox references a retired API')).toBeTruthy();
  });

  it('confirming an issue with a target_path records the decision and proposes a fix', async () => {
    render(<HarnessIssuesPanel pushToast={vi.fn()} />);
    await screen.findByText('Golden example foo.vox references a retired API');

    fireEvent.click(screen.getByText('Confirm & propose fix'));

    await waitFor(() => {
      expect(
        invokeMock.mock.calls.some(
          ([cmd, args]) =>
            cmd === 'record_harness_issue_decision' &&
            (args as { issueId: number; decision: string; reason: string | null }).issueId === 42 &&
            (args as { issueId: number; decision: string }).decision === 'confirmed' &&
            (args as { reason: string | null }).reason === null,
        ),
      ).toBe(true);
    });

    await waitFor(() => {
      expect(
        invokeMock.mock.calls.some(
          ([cmd, args]) =>
            cmd === 'propose_harness_issue_fix' &&
            (args as { issueId: number; targetPath: string }).issueId === 42 &&
            (args as { issueId: number; targetPath: string }).targetPath === 'examples/golden/foo.vox',
        ),
      ).toBe(true);
    });
  });
});
