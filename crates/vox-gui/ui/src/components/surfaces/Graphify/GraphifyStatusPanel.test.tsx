// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';

const mockUse = vi.fn();
vi.mock('../../../hooks/useGraphifyStatus', () => ({
  useGraphifyStatus: () => mockUse(),
  GRAPHIFY_STATUS_QUERY_KEY: ['graphify', 'status'],
}));

import { GraphifyStatusPanel } from './GraphifyStatusPanel';

describe('GraphifyStatusPanel', () => {
  it('renders corpus health and rebuild command for stale corpora', () => {
    mockUse.mockReturnValue({
      isLoading: false,
      isError: false,
      data: {
        default_corpus_id: 'repo-code-graph',
        corpora: [
          {
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
          },
        ],
      },
    });
    render(<GraphifyStatusPanel />);
    expect(screen.getByText('Repo')).toBeDefined();
    expect(screen.getByText('Stale')).toBeDefined();
    expect(screen.getByText(/graph_missing/)).toBeDefined();
    expect(screen.getByText(/vox graphify rebuild --corpus repo-code-graph/)).toBeDefined();
  });

  it('shows loading state', () => {
    mockUse.mockReturnValue({ isLoading: true, isError: false });
    render(<GraphifyStatusPanel />);
    expect(screen.getByText(/Loading graphify status/i)).toBeDefined();
  });
});
