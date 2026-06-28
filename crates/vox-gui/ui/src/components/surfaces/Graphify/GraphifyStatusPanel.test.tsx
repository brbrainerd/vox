// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import React from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

const mockUse = vi.fn();
vi.mock('../../../hooks/useGraphifyStatus', () => ({
  useGraphifyStatus: () => mockUse(),
  GRAPHIFY_STATUS_QUERY_KEY: ['graphify', 'status'],
}));

const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

import { GraphifyStatusPanel } from './GraphifyStatusPanel';

function renderWithClient(ui: React.ReactElement) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(<QueryClientProvider client={client}>{ui}</QueryClientProvider>);
}

const STALE_CORPUS = {
  corpus_id: 'repo-code-graph',
  title: 'Repo',
  graph_exists: false,
  manifest_exists: false,
  node_count: null,
  edge_count: null,
  built_at: null,
  manifest_git_sha: null,
  head_git_sha: 'abc',
  stale_reasons: ['graph_missing'],
  warnings: [],
  is_fresh: false,
};

describe('GraphifyStatusPanel', () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockInvoke.mockResolvedValue(undefined);
  });

  it('renders corpus health and rebuild command for stale corpora', () => {
    mockUse.mockReturnValue({
      isLoading: false,
      isError: false,
      data: {
        default_corpus_id: 'repo-code-graph',
        corpora: [STALE_CORPUS],
      },
    });
    renderWithClient(<GraphifyStatusPanel />);
    expect(screen.getByText('Repo')).toBeDefined();
    expect(screen.getByText('Stale')).toBeDefined();
    expect(screen.getByText(/graph_missing/)).toBeDefined();
    expect(screen.getByText(/vox graphify rebuild --corpus repo-code-graph/)).toBeDefined();
  });

  it('renders a relative built time for fresh corpora', () => {
    mockUse.mockReturnValue({
      isLoading: false,
      isError: false,
      data: {
        default_corpus_id: 'repo-code-graph',
        corpora: [
          {
            ...STALE_CORPUS,
            graph_exists: true,
            manifest_exists: true,
            node_count: 1200,
            edge_count: 3400,
            built_at: new Date(Date.now() - 3 * 3600_000).toISOString(),
            stale_reasons: [],
            is_fresh: true,
          },
        ],
      },
    });
    renderWithClient(<GraphifyStatusPanel />);
    expect(screen.getByText('Fresh')).toBeDefined();
    expect(screen.getByText('3h ago')).toBeDefined();
  });

  it('invokes vox_graphify_rebuild by name when Rebuild is clicked', async () => {
    mockUse.mockReturnValue({
      isLoading: false,
      isError: false,
      data: {
        default_corpus_id: 'repo-code-graph',
        corpora: [STALE_CORPUS],
      },
    });
    renderWithClient(<GraphifyStatusPanel />);
    fireEvent.click(screen.getByRole('button', { name: 'Rebuild repo-code-graph' }));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('invoke_mcp_tool', {
        tool: 'vox_graphify_rebuild',
        args: { corpus: 'repo-code-graph' },
      });
    });
  });

  it('shows loading state', () => {
    mockUse.mockReturnValue({ isLoading: true, isError: false });
    renderWithClient(<GraphifyStatusPanel />);
    expect(screen.getByText(/Loading graphify status/i)).toBeDefined();
  });
});
