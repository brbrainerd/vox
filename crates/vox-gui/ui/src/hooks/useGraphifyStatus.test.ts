// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import React from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

// VG-1 G9: this file now guards the DEPRECATED one-release back-compat
// re-export. The real behavior is tested in useVoxGraphStatus.test.ts; here we
// only assert the deprecated `useGraphifyStatus` / `GRAPHIFY_STATUS_QUERY_KEY`
// aliases still resolve to the renamed hook (T8 MCP-dispatch path preserved).
const mockInvokeMcpTool = vi.fn();
vi.mock('../transport', () => ({
  voxTransport: { invokeMcpTool: (...a: unknown[]) => mockInvokeMcpTool(...a) },
}));

import { useGraphifyStatus, GRAPHIFY_STATUS_QUERY_KEY } from './useGraphifyStatus';

function wrapper({ children }: { children: React.ReactNode }) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return React.createElement(QueryClientProvider, { client }, children);
}

describe('useGraphifyStatus (deprecated re-export)', () => {
  beforeEach(() => vi.clearAllMocks());

  it('re-exports the renamed query key', () => {
    expect(GRAPHIFY_STATUS_QUERY_KEY).toEqual(['vox-graph', 'status']);
  });

  it('still fetches through the vox_search_status MCP dispatch via the alias', async () => {
    mockInvokeMcpTool.mockResolvedValue({
      tool: 'vox_search_status',
      is_error: false,
      result: { success: true, data: { default_corpus_id: 'repo-code-graph', corpora: [] } },
    });

    const { result } = renderHook(() => useGraphifyStatus(), { wrapper });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));

    expect(mockInvokeMcpTool).toHaveBeenCalledWith('vox_search_status', {});
    expect(result.current.data?.default_corpus_id).toBe('repo-code-graph');
  });
});
