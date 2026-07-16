// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import React from 'react';

// Simulate the bare-browser case (F1): both Activity listener chains reject
// when the Tauri event bridge is unavailable. Vitest fails the run on
// unhandled rejections, so this test is red until the .catch guards exist.
vi.mock('../../../transport', () => ({
  activityQuery: vi.fn().mockResolvedValue([]),
  listenActivityAppended: vi.fn().mockRejectedValue(new Error('event bridge unavailable')),
  listenAgentEvents: vi.fn().mockRejectedValue(new Error('event bridge unavailable')),
}));

import { ActivitySurface } from './ActivitySurface';

describe('ActivitySurface listener guards', () => {
  it('mounts and unmounts without unhandled rejections when listen() rejects', async () => {
    const { unmount } = render(<ActivitySurface pushToast={() => {}} />);
    await waitFor(() => expect(screen.getByText(/agent activity timeline/i)).toBeTruthy());
    unmount();
    // Flush microtasks so any dangling rejection surfaces and fails the run.
    await new Promise((r) => setTimeout(r, 0));
  });
});
