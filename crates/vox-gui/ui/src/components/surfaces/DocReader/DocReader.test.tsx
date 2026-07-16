// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import React from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('../../../transport', () => ({
  voxTransport: {
    openLocator: vi.fn().mockResolvedValue({ action: 'opened' }),
    readDocMarkdown: vi.fn(),
  },
}));

import { voxTransport } from '../../../transport';
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
    vi.mocked(voxTransport.readDocMarkdown).mockReset();
    vi.mocked(voxTransport.readDocMarkdown).mockResolvedValue('# Title\n\nBody text');
  });

  it('loads and renders markdown for a doc tab id', async () => {
    renderDocReader('doc:docs/src/reference/cli.md');
    await waitFor(() => {
      expect(screen.getByTestId('doc-reader')).toBeDefined();
      expect(screen.getByText(/Body text/)).toBeDefined();
    });
    expect(voxTransport.readDocMarkdown).toHaveBeenCalledWith('docs/src/reference/cli.md');
  });
});
