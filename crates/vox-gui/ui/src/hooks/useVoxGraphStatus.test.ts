// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import React from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

// T8: the status read must flow through the shared MCP dispatch
// (`voxTransport.invokeMcpTool('vox_search_status', …)`), NOT a separate
// `vox_graphify_status` Tauri command. Mock the transport seam and assert
// both the tool name and that the unwrapped payload reaches the hook.
const mockInvokeMcpTool = vi.fn();
vi.mock('../transport', () => ({
  voxTransport: { invokeMcpTool: (...a: unknown[]) => mockInvokeMcpTool(...a) },
}));

import { useVoxGraphStatus, VOX_GRAPH_STATUS_QUERY_KEY } from './useVoxGraphStatus';

function wrapper({ children }: { children: React.ReactNode }) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return React.createElement(QueryClientProvider, { client }, children);
}

describe('useVoxGraphStatus', () => {
  beforeEach(() => vi.clearAllMocks());

  it('exports the renamed query key', () => {
    expect(VOX_GRAPH_STATUS_QUERY_KEY).toEqual(['vox-graph', 'status']);
  });

  it('fetches status through the vox_search_status MCP dispatch', async () => {
    // Daemon envelope: { success, data: { default_corpus_id, corpora } } under `.result`.
    mockInvokeMcpTool.mockResolvedValue({
      tool: 'vox_search_status',
      is_error: false,
      result: { success: true, data: { default_corpus_id: 'repo-code-graph', corpora: [] } },
    });

    const { result } = renderHook(() => useVoxGraphStatus(), { wrapper });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));

    expect(mockInvokeMcpTool).toHaveBeenCalledWith('vox_search_status', {});
    expect(result.current.data?.default_corpus_id).toBe('repo-code-graph');
  });

  // F-02: invokeMcpTool's Promise<{...}> signature lies about non-nullability
  // (it's a thin passthrough to Tauri invoke(), which can resolve null). The
  // hook must surface a clean error, not a raw TypeError from a null deref.
  it('surfaces a clean error when invokeMcpTool resolves null', async () => {
    mockInvokeMcpTool.mockResolvedValue(null);

    const { result } = renderHook(() => useVoxGraphStatus(), { wrapper });
    await waitFor(() => expect(result.current.isError).toBe(true));

    expect(result.current.error?.message).toBe('vox_search_status: no response from backend');
  });
});
