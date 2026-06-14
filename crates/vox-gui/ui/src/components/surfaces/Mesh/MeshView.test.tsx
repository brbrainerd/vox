// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, waitFor } from '@testing-library/react';
import React from 'react';

const NODES = {
  source: 'control_plane',
  nodes: [
    { id: 'node-1', status: 'online', host_triple: 'x86_64-linux', gpu_summary: 'rtx', trust_tier: 'trusted', advertised_models: ['a'], last_seen_unix_ms: Date.now() },
  ],
  queue_depth: 2,
};

const invokeMock = vi.fn((cmd: string, args?: any) => {
  if (cmd === 'invoke_mcp_tool') {
    if (args?.tool === 'vox_mesh_nodes') {
      return Promise.resolve({ tool: 'vox_mesh_nodes', is_error: false, result: NODES });
    }
    if (args?.tool === 'vox_mesh_queue_stats') {
      return Promise.resolve({ tool: 'vox_mesh_queue_stats', is_error: false, result: { pending_count: 2 } });
    }
  }
  return Promise.resolve(null);
});
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));

import { MeshView } from './MeshView';

describe('MeshView', () => {
  beforeEach(() => {
    cleanup();
    invokeMock.mockClear();
  });

  it('renders the mesh heading', () => {
    render(<MeshView pushToast={vi.fn()} />);
    expect(screen.getByText('Vox Populi Mesh')).toBeTruthy();
  });

  it('every button carries an explicit type="button"', async () => {
    render(<MeshView pushToast={vi.fn()} />);
    await waitFor(() => expect(screen.getAllByText('node-1').length).toBeGreaterThan(0));
    for (const b of screen.getAllByRole('button')) {
      expect(b.getAttribute('type')).toBe('button');
    }
  });

  it('marks column headers with scope=col', async () => {
    render(<MeshView pushToast={vi.fn()} />);
    await waitFor(() => expect(screen.getAllByText('node-1').length).toBeGreaterThan(0));
    const headers = screen.getAllByRole('columnheader');
    expect(headers.length).toBeGreaterThan(0);
    for (const h of headers) expect(h.getAttribute('scope')).toBe('col');
  });

  it('exposes the node table region as a polite live region', async () => {
    render(<MeshView pushToast={vi.fn()} />);
    const region = await screen.findByLabelText('Mesh nodes');
    expect(region.getAttribute('aria-live')).toBe('polite');
  });
});
