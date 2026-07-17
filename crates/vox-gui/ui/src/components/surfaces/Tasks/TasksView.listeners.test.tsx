// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import React from 'react';

// Simulate the bare-browser case: the Tauri event bridge is unavailable and
// every listen() rejects. Vitest fails the run on unhandled rejections, so
// this test is red until the .catch guards exist.
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockRejectedValue(new Error('event bridge unavailable')),
}));
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue(null),
}));

import * as transport from '../../../transport';
import { TasksView } from './TasksView';

describe('TasksView listener guards', () => {
  it('mounts and unmounts without unhandled rejections when listen() rejects', async () => {
    vi.spyOn(transport, 'hopperList').mockResolvedValue([]);
    vi.spyOn(transport, 'feedbackList').mockResolvedValue({ needsYou: [], withheld: [] });
    vi.spyOn(transport, 'listenFeedbackChanged').mockRejectedValue(
      new Error('event bridge unavailable'),
    );
    const { unmount } = render(<TasksView />);
    await waitFor(() => expect(screen.getByText('Tasks')).toBeTruthy());
    unmount();
    // Flush microtasks so any dangling rejection surfaces and fails the run.
    await new Promise((r) => setTimeout(r, 0));
  });
});
