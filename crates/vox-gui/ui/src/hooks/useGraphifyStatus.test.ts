// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import React from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

const mockGet = vi.fn();
vi.mock('../transport', () => ({ getGraphifyStatus: () => mockGet() }));

import { useGraphifyStatus } from './useGraphifyStatus';

function wrapper({ children }: { children: React.ReactNode }) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return React.createElement(QueryClientProvider, { client }, children);
}

describe('useGraphifyStatus', () => {
  beforeEach(() => vi.clearAllMocks());
  it('fetches graphify status via transport', async () => {
    mockGet.mockResolvedValue({ default_corpus_id: 'repo-code-graph', corpora: [] });
    const { result } = renderHook(() => useGraphifyStatus(), { wrapper });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data?.default_corpus_id).toBe('repo-code-graph');
  });
});
