// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, waitFor } from '@testing-library/react';
import React from 'react';

const invokeMock = vi.fn((cmd: string) => {
  if (cmd === 'list_publication_manifests') {
    return Promise.resolve([
      { publication_id: 'pub-a', state: 'draft' },
      { publication_id: 'pub-b', state: 'published' },
    ]);
  }
  if (cmd === 'get_archive_status') {
    return Promise.resolve({
      swhid: null,
      swh_task_status: null,
      zenodo_doi: '10.5281/zenodo.1',
      zenodo_state: 'published',
    });
  }
  return Promise.resolve(null);
});
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));

import { ArchiveStatusSummary } from './ArchiveStatusSummary';

describe('ArchiveStatusSummary', () => {
  beforeEach(() => {
    cleanup();
    invokeMock.mockClear();
  });

  it('renders archive deposit rollup counts after loading manifests', async () => {
    render(<ArchiveStatusSummary />);
    await waitFor(() => {
      expect(screen.getByText('Archive deposit status')).toBeTruthy();
    });
    await waitFor(() => {
      expect(screen.getByText('Zenodo DOI')).toBeTruthy();
      expect(screen.getByText('SWHID')).toBeTruthy();
      expect(screen.getByText('Pending deposit (sample)')).toBeTruthy();
    });
  });
});
