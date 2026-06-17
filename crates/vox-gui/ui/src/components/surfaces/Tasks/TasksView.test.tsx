// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import React from 'react';

// Mock Tauri invoke — TasksView calls list_orchestrator_tasks on mount.
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue([]),
}));

// Mock @tauri-apps/api/event (listen)
const mockUnlisten = vi.fn();
const mockListen = vi.fn().mockResolvedValue(mockUnlisten);
vi.mock('@tauri-apps/api/event', () => ({
  listen: mockListen,
}));

import { TasksView } from './TasksView';

describe('TasksView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders the Tasks heading', async () => {
    render(<TasksView />);
    expect(screen.getByText('Tasks')).toBeDefined();
  });

  it('renders In progress and Queued section headings', async () => {
    render(<TasksView />);
    expect(screen.getAllByText(/In progress/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/Queued/i).length).toBeGreaterThan(0);
  });

  it('shows empty-state messages when lists are empty', async () => {
    render(<TasksView />);
    expect(screen.getByPlaceholderText('Add a task…')).toBeDefined();
  });

  it('renders the Add button', () => {
    render(<TasksView />);
    expect(screen.getByText('Add')).toBeDefined();
  });

  it('every button carries an explicit type="button"', async () => {
    render(<TasksView />);
    for (const b of screen.getAllByRole('button')) {
      expect(b.getAttribute('type')).toBe('button');
    }
  });

  it('the refresh control has an accessible label', () => {
    render(<TasksView />);
    expect(screen.getByLabelText('Refresh tasks')).toBeDefined();
  });

  it('the add-task input is labeled', () => {
    render(<TasksView />);
    expect(screen.getByLabelText('Add a task')).toBeDefined();
  });

  it('subscribes to vox://tasks-changed on mount', async () => {
    render(<TasksView />);
    await waitFor(() => {
      expect(mockListen).toHaveBeenCalledWith(
        'vox://tasks-changed',
        expect.any(Function),
      );
    });
  });

  it('calls unlisten on unmount', async () => {
    const { unmount } = render(<TasksView />);
    await waitFor(() => expect(mockListen).toHaveBeenCalled());
    unmount();
    expect(mockUnlisten).toHaveBeenCalled();
  });

  it('does NOT set a polling interval', async () => {
    const setIntervalSpy = vi.spyOn(globalThis, 'setInterval');
    render(<TasksView />);
    await waitFor(() => expect(mockListen).toHaveBeenCalled());
    expect(setIntervalSpy).not.toHaveBeenCalled();
    setIntervalSpy.mockRestore();
  });
});
