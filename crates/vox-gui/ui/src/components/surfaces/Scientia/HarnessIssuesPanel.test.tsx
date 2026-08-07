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

const PENDING_PROPOSAL = {
  id: 7,
  issue_id: 42,
  target_path: 'examples/golden/foo.vox',
  proposed_content: 'new content',
  proposed_diff: '--- a/examples/golden/foo.vox\n+++ b/examples/golden/foo.vox\n@@ -1 +1 @@\n-old\n+new',
  status: 'pending_approval',
  proposed_at_ms: Date.now() - 1_000,
  resolved_at_ms: null,
};

// A confirmed issue with a target_path but no matching pending proposal —
// the "confirm succeeded, propose-fix failed/pending" stuck state the retry
// affordance exists to unstick.
const CONFIRMED_STUCK_ROW = {
  ...PENDING_ROW,
  id: 99,
  status: 'confirmed',
  summary: 'Confirmed but the fix proposal never landed',
};

let fixProposals: unknown[] = [];

const invokeMock = vi.fn((cmd: string, args?: unknown) => {
  if (cmd === 'list_harness_issues') {
    const status = (args as { status?: string } | undefined)?.status;
    if (status === 'confirmed') return Promise.resolve([CONFIRMED_STUCK_ROW]);
    return Promise.resolve([PENDING_ROW]);
  }
  if (cmd === 'list_harness_fix_proposals') return Promise.resolve(fixProposals);
  if (cmd === 'record_harness_issue_decision') return Promise.resolve();
  if (cmd === 'propose_harness_issue_fix') return Promise.resolve(1);
  if (cmd === 'resolve_harness_fix_proposal') return Promise.resolve();
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

  it('approving a fix proposal invokes resolve_harness_fix_proposal with approve=true', async () => {
    fixProposals = [PENDING_PROPOSAL];
    render(<HarnessIssuesPanel pushToast={vi.fn()} />);

    await screen.findByText('examples/golden/foo.vox');
    expect(await screen.findByText(/-old/)).toBeTruthy();

    fireEvent.click(screen.getByText('Approve & apply'));

    await waitFor(() => {
      expect(
        invokeMock.mock.calls.some(
          ([cmd, args]) =>
            cmd === 'resolve_harness_fix_proposal' &&
            (args as { proposalId: number; approve: boolean }).proposalId === 7 &&
            (args as { proposalId: number; approve: boolean }).approve === true,
        ),
      ).toBe(true);
    });

    fixProposals = [];
  });

  it('offers a retry-propose-fix action for a confirmed issue with no matching pending proposal', async () => {
    render(<HarnessIssuesPanel pushToast={vi.fn()} />);
    await screen.findByText('Golden example foo.vox references a retired API');

    fireEvent.change(screen.getByLabelText('Filter by status'), { target: { value: 'confirmed' } });
    await screen.findByText('Confirmed but the fix proposal never landed');

    fireEvent.click(screen.getByText('Retry propose fix'));

    await waitFor(() => {
      expect(
        invokeMock.mock.calls.some(
          ([cmd, args]) =>
            cmd === 'propose_harness_issue_fix' &&
            (args as { issueId: number; targetPath: string }).issueId === 99 &&
            (args as { issueId: number; targetPath: string }).targetPath === 'examples/golden/foo.vox',
        ),
      ).toBe(true);
    });
  });
});
