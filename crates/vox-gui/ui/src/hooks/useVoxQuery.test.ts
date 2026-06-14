// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import React from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useVoxQuery, useVoxMutation } from './useVoxQuery';

function makeWrapper() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return ({ children }: { children: React.ReactNode }) =>
    React.createElement(QueryClientProvider, { client: qc }, children);
}

describe('useVoxQuery', () => {
  it('returns data when fetcher resolves', async () => {
    const { result } = renderHook(
      () => useVoxQuery(['test-key'], () => Promise.resolve('hello')),
      { wrapper: makeWrapper() }
    );
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data).toBe('hello');
  });

  it('returns error when fetcher rejects', async () => {
    const { result } = renderHook(
      () => useVoxQuery(['test-err'], () => Promise.reject(new Error('boom'))),
      { wrapper: makeWrapper() }
    );
    await waitFor(() => expect(result.current.isError).toBe(true));
    expect((result.current.error as Error).message).toBe('boom');
  });
});

describe('useVoxMutation', () => {
  it('calls mutator and returns data', async () => {
    const mutator = vi.fn().mockResolvedValue('done');
    const { result } = renderHook(
      () => useVoxMutation(mutator),
      { wrapper: makeWrapper() }
    );
    result.current.mutate('input');
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(mutator).toHaveBeenCalledWith('input');
    expect(result.current.data).toBe('done');
  });
});
