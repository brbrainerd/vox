// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import React from 'react';
import { clearSearchableRegistry, registerSearchable } from '../../lib/searchableRegistry';

const invokeMcpTool = vi.fn();
vi.mock('../../transport', () => ({
  voxTransport: {
    voxDocsIndex: vi.fn().mockResolvedValue([]),
    voxContentManifest: vi.fn().mockResolvedValue([]),
    listPolicies: vi.fn().mockResolvedValue([]),
    openLocator: vi.fn().mockResolvedValue(undefined),
    invokeMcpTool: (...a: unknown[]) => invokeMcpTool(...a),
  },
}));

vi.mock('../../hooks/useSearchController', () => ({
  useSearchController: vi.fn(() => ({
    state: { query: 'pending', hits: [], loading: false, scopes: ['code'], requestToken: 1 },
    setQuery: vi.fn(),
    setScopes: vi.fn(),
  })),
}));

// finding #8: the mock must HONOR the query and the { kinds } filter, mirroring
// the real searchFederatedIndex(entries, query, { kinds }) — otherwise the test
// proves nothing about query filtering or prefix-mode kind restriction (#4).
vi.mock('../../hooks/useFederatedSearchIndex', () => ({
  useFederatedSearchIndex: () => ({
    entries: [],
    search: (query: string, options?: { kinds?: string[] }) => {
      const q = (query ?? '').toLowerCase();
      const all = [
        {
          kind: 'surface',
          id: 'surface:approvals',
          label: 'Approvals',
          detail: 'Runs',
          payload: { type: 'surface', viewKey: 'approvals' },
        },
      ];
      const byQuery = q ? all.filter((e) => e.label.toLowerCase().includes(q)) : [];
      const kinds = options?.kinds;
      return kinds && kinds.length > 0
        ? byQuery.filter((e) => kinds.includes(e.kind))
        : byQuery;
    },
  }),
}));

vi.mock('../../hooks/useContentManifest', () => ({
  useContentManifest: () => [
    {
      viewKey: 'activity',
      label: 'Activity',
      route: '#view=activity',
      headings: ['3 pending approvals'],
      copy: ['3 pending approvals'],
      commands: [],
      docs: [],
    },
  ],
}));

import { Omnibar } from './Omnibar';

const noop = () => {};

function renderOmnibar(overrides: Partial<React.ComponentProps<typeof Omnibar>> = {}) {
  return render(
    <Omnibar
      open
      onClose={noop}
      onNavigate={overrides.onNavigate ?? vi.fn()}
      onRunCommand={overrides.onRunCommand ?? vi.fn()}
      onSendToChat={overrides.onSendToChat ?? vi.fn()}
      onOpenDoc={overrides.onOpenDoc ?? vi.fn()}
      agents={[]}
      skills={[]}
      {...overrides}
    />,
  );
}

describe('Omnibar', () => {
  beforeEach(() => {
    clearSearchableRegistry();
    invokeMcpTool.mockReset();
    // master-spec discover shape: { result: { results: [...] } } (NOT `neighbors`).
    invokeMcpTool.mockResolvedValue({ result: { results: [] } });
  });

  it('renders SURFACES and ON-SCREEN facets from federated + manifest', async () => {
    // Reconciled to real behavior: the federated mock filters by query, and the
    // manifest heading "3 pending approvals" contains "approvals". Querying
    // "approvals" surfaces the federated "Approvals" row AND the manifest
    // on-screen copy. (The plan's draft used "pending", which the federated mock
    // would not match against the "Approvals" label.)
    renderOmnibar();
    fireEvent.change(screen.getByPlaceholderText(/search/i), { target: { value: 'approvals' } });
    await waitFor(() => expect(screen.getByText('Approvals')).toBeTruthy());
    expect(screen.getByText('3 pending approvals')).toBeTruthy();
    expect(screen.getByText(/On Screen/i)).toBeTruthy();
  });

  it('Enter activates the top hit via onNavigate', async () => {
    const onNavigate = vi.fn();
    renderOmnibar({ onNavigate });
    fireEvent.change(screen.getByPlaceholderText(/search/i), { target: { value: 'approvals' } });
    await waitFor(() => expect(screen.getByText('Approvals')).toBeTruthy());
    fireEvent.keyDown(window, { key: 'Enter' });
    expect(onNavigate).toHaveBeenCalledWith('approvals', undefined);
  });

  it('Shift+Enter sends the raw query to chat', async () => {
    const onSendToChat = vi.fn();
    renderOmnibar({ onSendToChat });
    fireEvent.change(screen.getByPlaceholderText(/search/i), { target: { value: 'why is queue stuck' } });
    fireEvent.keyDown(window, { key: 'Enter', shiftKey: true });
    expect(onSendToChat).toHaveBeenCalledWith('why is queue stuck');
  });

  it('runtime registry feeds the ON-SCREEN facet', async () => {
    registerSearchable('mesh', [{ label: 'mesh: 4 peers online', detail: 'Mesh', viewKey: 'mesh' }]);
    renderOmnibar();
    fireEvent.change(screen.getByPlaceholderText(/search/i), { target: { value: 'peers' } });
    await waitFor(() => expect(screen.getByText('mesh: 4 peers online')).toBeTruthy());
  });

  // finding #4: a `/` (skills) prefix must restrict federated kinds so the
  // surface row is NOT returned — proves prefix modes aren't decorative.
  it('skills prefix (/) restricts kinds — surface row is filtered out', async () => {
    renderOmnibar();
    fireEvent.change(screen.getByPlaceholderText(/search/i), { target: { value: '/approvals' } });
    // With kinds=['doc','skill'], the surface:approvals entry is excluded.
    await waitFor(() => expect(screen.queryByText('Approvals')).toBeNull());
  });
});
