// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('../../../transport', () => ({
  voxTransport: {
    harnessEvalHistory: vi.fn(),
    harnessEvalRegressions: vi.fn(),
  },
}));

import { voxTransport } from '../../../transport';
import { HarnessHealthView } from './HarnessHealthView';

function renderWithClient(ui: React.ReactElement) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(<QueryClientProvider client={client}>{ui}</QueryClientProvider>);
}

describe('HarnessHealthView', () => {
  beforeEach(() => {
    vi.mocked(voxTransport.harnessEvalHistory).mockReset();
    vi.mocked(voxTransport.harnessEvalRegressions).mockReset();
    vi.mocked(voxTransport.harnessEvalRegressions).mockResolvedValue([]);
  });

  it('renders recent runs from harness_eval_history', async () => {
    vi.mocked(voxTransport.harnessEvalHistory).mockResolvedValue([
      {
        run_id: 'abc1234-1000',
        git_sha: 'abc1234',
        triggered_by: 'ci-nightly',
        pass_count: 8,
        fail_count: 1,
        skip_count: 0,
        total_cost_usd: 0.05,
        started_at_ms: 1700000000000,
        category_breakdown: [
          { category: 'chat', pass_count: 5, fail_count: 0 },
          { category: 'privacy', pass_count: 2, fail_count: 0 },
          { category: 'tool-calling', pass_count: 1, fail_count: 1 },
        ],
      },
    ]);

    renderWithClient(<HarnessHealthView />);

    await waitFor(() => {
      expect(screen.getByText('abc1234-1000')).toBeInTheDocument();
    });
    expect(screen.getByText(/8/)).toBeInTheDocument();
  });

  it('shows an empty state when no runs exist yet', async () => {
    vi.mocked(voxTransport.harnessEvalHistory).mockResolvedValue([]);
    vi.mocked(voxTransport.harnessEvalRegressions).mockResolvedValue([]);

    renderWithClient(<HarnessHealthView />);

    await waitFor(() => {
      expect(screen.getByText(/no harness eval runs/i)).toBeInTheDocument();
    });
  });

  it('shows a regression banner when harness_eval_regressions returns a flag', async () => {
    vi.mocked(voxTransport.harnessEvalHistory).mockResolvedValue([
      {
        run_id: 'def5678-2000', git_sha: 'def5678', triggered_by: 'ci-nightly',
        pass_count: 5, fail_count: 5, skip_count: 0, total_cost_usd: 0.05, started_at_ms: 1700000001000,
        category_breakdown: [{ category: 'chat', pass_count: 5, fail_count: 5 }],
      },
    ]);
    vi.mocked(voxTransport.harnessEvalRegressions).mockResolvedValue([
      {
        kind: 'PassRateDrop',
        previous_run_id: 'abc1234-1000',
        current_run_id: 'def5678-2000',
        previous_git_sha: 'abc1234',
        current_git_sha: 'def5678',
        changed_files: ['crates/vox-orchestrator/src/runtime.rs'],
        flipped_task_ids: [],
        detail: 'pass rate dropped from 100.0% to 50.0%',
      },
    ]);

    renderWithClient(<HarnessHealthView />);

    await waitFor(() => {
      expect(screen.getByText(/pass rate dropped from 100.0% to 50.0%/i)).toBeInTheDocument();
    });
    expect(screen.getByText('crates/vox-orchestrator/src/runtime.rs')).toBeInTheDocument();
  });
});
