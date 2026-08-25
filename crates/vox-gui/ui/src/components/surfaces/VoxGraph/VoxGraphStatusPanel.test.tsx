// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import React from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { LanguageProvider } from '../../../hooks/useLanguage';

const mockUse = vi.fn();
vi.mock('../../../hooks/useVoxGraphStatus', () => ({
  useVoxGraphStatus: () => mockUse(),
  VOX_GRAPH_STATUS_QUERY_KEY: ['vox-graph', 'status'],
}));

const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

import { VoxGraphStatusPanel } from './VoxGraphStatusPanel';

function renderWithClient(ui: React.ReactElement) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(<LanguageProvider><QueryClientProvider client={client}>{ui}</QueryClientProvider></LanguageProvider>);
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

describe('VoxGraphStatusPanel', () => {
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
    renderWithClient(<VoxGraphStatusPanel />);
    expect(screen.getByText('Repo')).toBeDefined();
    expect(screen.getByText('Stale')).toBeDefined();
    expect(screen.getByText(/graph_missing/)).toBeDefined();
    expect(screen.getByText(/vox graphify rebuild --corpus repo-code-graph/)).toBeDefined();
  });

  it('condensed prop renders only a fresh/total corpora summary, not the per-corpus cards', () => {
    mockUse.mockReturnValue({
      isLoading: false,
      isError: false,
      data: {
        default_corpus_id: 'repo-code-graph',
        corpora: [STALE_CORPUS, { ...STALE_CORPUS, corpus_id: 'other', is_fresh: true }],
      },
    });
    renderWithClient(<VoxGraphStatusPanel condensed />);
    // Same is_fresh flag each corpus card already renders (as its Fresh/Stale
    // pill), rolled up into one "N/M fresh" line.
    expect(screen.getByText(/1\/2 fresh/i)).toBeDefined();
    expect(screen.queryByText('Stale')).toBeNull();
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
    renderWithClient(<VoxGraphStatusPanel />);
    expect(screen.getByText('Fresh')).toBeDefined();
    expect(screen.getByText('3h ago')).toBeDefined();
  });

  it('invokes vox_search_rebuild by name when Rebuild is clicked', async () => {
    mockUse.mockReturnValue({
      isLoading: false,
      isError: false,
      data: {
        default_corpus_id: 'repo-code-graph',
        corpora: [STALE_CORPUS],
      },
    });
    renderWithClient(<VoxGraphStatusPanel />);
    fireEvent.click(screen.getByRole('button', { name: 'Rebuild repo-code-graph' }));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('invoke_mcp_tool', {
        tool: 'vox_search_rebuild',
        args: { corpus: 'repo-code-graph' },
        permissionMode: null,
      });
    });
  });

  it('shows the effective TTL and saves an edited value', async () => {
    mockUse.mockReturnValue({
      isLoading: false,
      isError: false,
      data: {
        default_corpus_id: 'repo-code-graph',
        ttl_days: 30,
        ttl_days_env_forced: false,
        corpora: [STALE_CORPUS],
      },
    });
    renderWithClient(<VoxGraphStatusPanel />);

    const input = screen.getByLabelText('Staleness TTL in days') as HTMLInputElement;
    expect(input.value).toBe('30');

    fireEvent.change(input, { target: { value: '7' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save TTL' }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('invoke_mcp_tool', {
        tool: 'vox_search_set_ttl',
        args: { ttl_days: 7 },
        permissionMode: null,
      });
    });
  });

  it('prefills the contract TTL, not the env-resolved effective one', () => {
    // Save writes the contract, so the control must show the contract value.
    // Prefilling the effective 7 would let one click rewrite ttl_days_default
    // from 30 to 7 in a tracked file the user never chose to change.
    mockUse.mockReturnValue({
      isLoading: false,
      isError: false,
      data: {
        default_corpus_id: 'repo-code-graph',
        ttl_days: 7,
        ttl_days_contract: 30,
        ttl_days_env_forced: true,
        corpora: [STALE_CORPUS],
      },
    });
    renderWithClient(<VoxGraphStatusPanel />);

    expect((screen.getByLabelText('Staleness TTL in days') as HTMLInputElement).value).toBe('30');
    // The effective value stays visible so the user can see what is in force.
    expect(screen.getByText(/Currently in force: 7 days/)).toBeInTheDocument();
  });

  it('tells the user the save wrote a tracked file that needs committing', async () => {
    mockUse.mockReturnValue({
      isLoading: false,
      isError: false,
      data: {
        default_corpus_id: 'repo-code-graph',
        ttl_days: 30,
        ttl_days_env_forced: false,
        corpora: [STALE_CORPUS],
      },
    });
    // Real shape: `invoke_mcp_tool` (crates/vox-gui/src/commands/mcp.rs) wraps
    // the daemon's own `{ success, data }` envelope under `result` — `data` is
    // graphify_set_ttl's payload from graph_tools.rs.
    mockInvoke.mockResolvedValue({
      tool: 'vox_search_set_ttl',
      is_error: false,
      result: {
        success: true,
        data: {
          ttl_days_written: 7,
          ttl_days_effective: 7,
          env_override_active: false,
          contract_path: 'contracts/retrieval/vox-graph-corpora.v1.yaml',
          requires_commit: true,
        },
      },
    });
    renderWithClient(<VoxGraphStatusPanel />);

    fireEvent.change(screen.getByLabelText('Staleness TTL in days'), { target: { value: '7' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save TTL' }));

    // The TTL lives in a TRACKED contract. A user who is not told will not commit,
    // and CI will keep enforcing the old value.
    expect(
      await screen.findByText(/contracts\/retrieval\/vox-graph-corpora\.v1\.yaml/),
    ).toBeInTheDocument();
  });

  it('reports a failed save instead of looking like it worked', async () => {
    mockUse.mockReturnValue({
      isLoading: false,
      isError: false,
      data: {
        default_corpus_id: 'repo-code-graph',
        ttl_days: 30,
        ttl_days_env_forced: false,
        corpora: [STALE_CORPUS],
      },
    });
    // The tool reports failure in-band (`success: false`), it does not throw.
    mockInvoke.mockResolvedValue({
      tool: 'vox_search_set_ttl',
      is_error: true,
      result: { success: false, error: 'write ttl: permission denied' },
    });
    renderWithClient(<VoxGraphStatusPanel />);

    fireEvent.change(screen.getByLabelText('Staleness TTL in days'), { target: { value: '7' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save TTL' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(/permission denied/);
  });

  it('rejects an out-of-range TTL without calling the backend', async () => {
    mockUse.mockReturnValue({
      isLoading: false,
      isError: false,
      data: {
        default_corpus_id: 'repo-code-graph',
        ttl_days: 30,
        ttl_days_env_forced: false,
        corpora: [STALE_CORPUS],
      },
    });
    renderWithClient(<VoxGraphStatusPanel />);

    fireEvent.change(screen.getByLabelText('Staleness TTL in days'), { target: { value: '0' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save TTL' }));

    expect(await screen.findByRole('alert')).toHaveTextContent(/between 1 and 3650/);
    expect(mockInvoke).not.toHaveBeenCalled();
  });

  it('tells the user when an env var overrides the stored TTL', () => {
    mockUse.mockReturnValue({
      isLoading: false,
      isError: false,
      data: {
        default_corpus_id: 'repo-code-graph',
        ttl_days: 5,
        ttl_days_env_forced: true,
        corpora: [STALE_CORPUS],
      },
    });
    renderWithClient(<VoxGraphStatusPanel />);
    expect(screen.getByText(/VOX_GRAPHIFY_TTL_DAYS/)).toBeInTheDocument();
  });

  it('omits the TTL editor entirely when the backend sends no ttl_days', () => {
    mockUse.mockReturnValue({
      isLoading: false,
      isError: false,
      data: { default_corpus_id: 'repo-code-graph', corpora: [STALE_CORPUS] },
    });
    renderWithClient(<VoxGraphStatusPanel />);
    expect(screen.queryByLabelText('Staleness TTL in days')).toBeNull();
  });

  it('shows loading state', () => {
    mockUse.mockReturnValue({ isLoading: true, isError: false });
    renderWithClient(<VoxGraphStatusPanel />);
    expect(screen.getByText(/Loading graphify status/i)).toBeDefined();
  });
});
