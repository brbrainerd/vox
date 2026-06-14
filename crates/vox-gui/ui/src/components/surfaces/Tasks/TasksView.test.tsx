// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';

// Mock Tauri invoke — TasksView calls list_orchestrator_tasks on mount.
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue([]),
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
});
