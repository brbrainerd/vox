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

  it('debounces the GRAPH discover MCP call — one fire per burst, not per keystroke', async () => {
    renderOmnibar();
    const input = screen.getByPlaceholderText(/search/i);
    const discoverCalls = () =>
      invokeMcpTool.mock.calls.filter((c) => c[0] === 'vox_graphify_query');
    // Type a 4-char burst fast (well within the 200ms debounce window).
    for (const v of ['z', 'zn', 'zno', 'znod']) {
      fireEvent.change(input, { target: { value: v } });
    }
    // Wait for the debounce to settle and a discover call to land.
    await waitFor(() => expect(discoverCalls().length).toBeGreaterThan(0));
    // Debounced: the burst collapses to a SINGLE discover call for the final
    // query — NOT one call per keystroke (which would be 4).
    expect(discoverCalls().length).toBe(1);
    expect(discoverCalls()[0][1]).toEqual({ query: 'znod', limit: 6 });
  });

  it('Alt+ArrowRight expands graph neighbors of the selected node', async () => {
    // First call (discover, query lane) seeds the graph facet; second call
    // (neighbors lane) returns the expansion — both in master-spec `results` shape.
    invokeMcpTool
      .mockResolvedValueOnce({ result: { results: [{ node_id: 'surface:chat' }] } })
      .mockResolvedValueOnce({ result: { results: [{ node_id: 'surface:approvals' }] } });
    renderOmnibar();
    // Use a token that matches ONLY the graph-discover lane (federated/manifest
    // do not match), so the GRAPH row is the top row — deterministic selection.
    fireEvent.change(screen.getByPlaceholderText(/search/i), { target: { value: 'znode' } });
    await waitFor(() => expect(screen.getByText('chat')).toBeTruthy()); // label derived from surface:<vk>
    // Top row is the graph node; Alt+ArrowRight (idx -1 → list[0]) expands it.
    fireEvent.keyDown(window, { key: 'ArrowRight', altKey: true });
    await waitFor(() => expect(screen.getByText('approvals')).toBeTruthy());
    expect(screen.getByText('chat')).toBeTruthy(); // original neighbor retained
  });
});
