// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import React from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('../../../transport', () => ({
  voxTransport: { openLocator: vi.fn().mockResolvedValue({ action: 'opened' }) },
}));

import { invoke } from '@tauri-apps/api/core';
import { DocReader } from './DocReader';

function renderDocReader(tabId: string) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={qc}>
      <DocReader tabId={tabId} />
    </QueryClientProvider>,
  );
}

describe('DocReader', () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue('# Title\n\nBody text');
  });

  it('loads and renders markdown for a doc tab id', async () => {
    renderDocReader('doc:docs/src/reference/cli.md');
    await waitFor(() => {
      expect(screen.getByTestId('doc-reader')).toBeDefined();
      expect(screen.getByText(/Body text/)).toBeDefined();
    });
    expect(invoke).toHaveBeenCalledWith('read_doc_markdown', {
      path: 'docs/src/reference/cli.md',
    });
  });
});
