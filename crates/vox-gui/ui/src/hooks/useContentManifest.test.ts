// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';

const mockVoxContentManifest = vi.fn();
vi.mock('../transport', () => ({
  voxTransport: { voxContentManifest: (...a: unknown[]) => mockVoxContentManifest(...a) },
}));

import { useContentManifest } from './useContentManifest';
import type { ContentManifestEntry } from './useContentManifest';

const ROW: ContentManifestEntry = {
  viewKey: 'approvals',
  label: 'Approvals',
  route: '#view=approvals',
  headings: ['Pending', 'Resolved'],
  copy: ['Resolve or reject pending approvals'],
  commands: ['vox_resolve_approval'],
  docs: [],
};

describe('useContentManifest', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('returns the manifest rows on success', async () => {
    mockVoxContentManifest.mockResolvedValue([ROW]);
    const { result } = renderHook(() => useContentManifest());
    await waitFor(() => expect(result.current).toHaveLength(1));
    expect(result.current[0].label).toBe('Approvals');
  });

  it('defaults to [] when the command rejects (VG-1 not landed)', async () => {
    mockVoxContentManifest.mockRejectedValue(new Error('unknown command'));
    const { result } = renderHook(() => useContentManifest());
    await waitFor(() => expect(mockVoxContentManifest).toHaveBeenCalled());
    expect(result.current).toEqual([]);
  });

  it('defaults to [] when mockVoxContentManifest is undefined', async () => {
    // Simulate the transport method not existing yet.
    mockVoxContentManifest.mockImplementation(() => {
      throw new TypeError('mockVoxContentManifest is not a function');
    });
    const { result } = renderHook(() => useContentManifest());
    await waitFor(() => expect(result.current).toEqual([]));
  });
});
