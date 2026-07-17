// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import React from 'react';
import { invoke } from '@tauri-apps/api/core';

const MOCK_TASKS = [
  {
    item_id: 'task-1',
    intent: 'Task 1',
    priority: 1,
    state: 'assigned',
  },
  {
    item_id: 'task-2',
    intent: 'Task 2',
    priority: 2,
    state: 'inbox',
  },
];

// Mock Tauri invoke — TasksView calls list_orchestrator_tasks on mount.
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue([]),
}));

// Mock @tauri-apps/api/event (listen)
const { mockListen, mockUnlisten } = vi.hoisted(() => {
  const mockUn = vi.fn();
  const mockLi = vi.fn().mockResolvedValue(mockUn);
  return { mockListen: mockLi, mockUnlisten: mockUn };
});
vi.mock('@tauri-apps/api/event', () => ({
  listen: mockListen,
}));

vi.mock('../../../transport', () => ({
  feedbackList: vi.fn().mockResolvedValue({ needsYou: [], withheld: [] }),
  listenFeedbackChanged: vi.fn().mockResolvedValue(() => {}),
  hopperList: vi.fn().mockResolvedValue([]),
  hopperMarkDone: vi.fn().mockResolvedValue({}),
  voxTransport: { listOrchestratorTasks: vi.fn().mockResolvedValue([]) },
}));

import { feedbackList, hopperList } from '../../../transport';
import { TasksView } from './TasksView';

describe('TasksView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(hopperList).mockResolvedValue(MOCK_TASKS as any);
  });

  it('renders the Tasks heading', async () => {
    render(<TasksView />);
    expect(screen.getByText('Tasks')).toBeDefined();
  });

  it('renders In progress and Queued section headings', async () => {
    render(<TasksView />);
    await waitFor(() => {
      expect(screen.getAllByText(/In progress/i).length).toBeGreaterThan(0);
      expect(screen.getAllByText(/Queued/i).length).toBeGreaterThan(0);
    });
  });

  it('shows empty-state messages when lists are empty', async () => {
    vi.mocked(hopperList).mockResolvedValueOnce([]);
    render(<TasksView />);
    expect(screen.getByPlaceholderText('Add a task…')).toBeDefined();
    await waitFor(() => {
      expect(screen.getByText('No tasks in this workspace')).toBeDefined();
    });
  });

  it('renders the Add button', () => {
    render(<TasksView />);
    expect(screen.getByText('Add')).toBeDefined();
  });

  it('every button carries an explicit type="button"', async () => {
    render(<TasksView />);
    await waitFor(() => {
      for (const b of screen.getAllByRole('button')) {
        expect(b.getAttribute('type')).toBe('button');
      }
    });
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
    await waitFor(() => expect(mockUnlisten).toHaveBeenCalled());
  });

  it('does NOT set a polling interval', async () => {
    const setIntervalSpy = vi.spyOn(globalThis, 'setInterval');
    render(<TasksView />);
    expect(setIntervalSpy).not.toHaveBeenCalled();
    setIntervalSpy.mockRestore();
  });

  it('renders columns using DataTable key definitions', async () => {
    render(<TasksView />);
    await waitFor(() => {
      expect(screen.getByText('Priority')).toBeDefined();
      expect(screen.getByText('Task ID')).toBeDefined();
    });
  });

  it('renders Blocked section when a task matches feedback gates', async () => {
    vi.mocked(feedbackList).mockResolvedValue({
      needsYou: [
        {
          feedbackId: 'F-1',
          kind: 'clarification',
          prompt: 'Which database?',
          options: ['sqlite', 'postgres'],
          gates: [999],
          doubtedTaskId: null,
          surface: 'needs_you',
          infoGainBits: 0.8,
        },
      ],
      withheld: [],
    });
    vi.mocked(hopperList).mockResolvedValue([
      {
        item_id: 'task-blocked',
        intent: 'A blocked task',
        priority: 1,
        state: 'inbox',
        task_id: 999,
      },
    ] as any);
    render(<TasksView />);
    await waitFor(() => {
      expect(screen.getAllByText(/Blocked/i).length).toBeGreaterThan(0);
      expect(screen.getByText(/waiting on Needs You/i)).toBeDefined();
    });
  });

  it('sources hopper rows from the shared attention inbox and only self-fetches the orchestrator read', async () => {
    const attention = {
      approvals: [],
      needsYou: [],
      withheld: [],
      blockedTasksCount: 0,
      totalCount: 0,
      hopperTasks: [
        { item_id: 'task-1', intent: 'Task 1', priority: 1, state: 'assigned', task_id: 1 },
        { item_id: 'task-2', intent: 'Task 2', priority: 2, state: 'inbox', task_id: 2 },
      ],
      refresh: vi.fn(),
      resolveApproval: vi.fn(),
      resolveFeedback: vi.fn(),
    };
    render(<TasksView attention={attention as any} />);
    await waitFor(() => {
      expect(screen.getByText('Task 1')).toBeDefined();
      expect(screen.getByText('Task 2')).toBeDefined();
    });
    expect(invoke).not.toHaveBeenCalled();
    expect(hopperList).not.toHaveBeenCalled();
    expect(feedbackList).not.toHaveBeenCalled();
    // Phase 2 Task 10: attention mode still runs the orchestrator merge read,
    // subscribed to tasks-changed only (no feedback listener, no polling).
    expect(mockListen).toHaveBeenCalledWith('vox://tasks-changed', expect.any(Function));
  });

  it('derives blocked lifecycle from attention.needsYou gates, not its own fetch, when attention is provided', async () => {
    const attention = {
      approvals: [],
      withheld: [],
      blockedTasksCount: 1,
      totalCount: 1,
      needsYou: [
        {
          feedbackId: 'F-1',
          kind: 'clarification',
          prompt: 'Which database?',
          options: ['sqlite', 'postgres'],
          gates: [999],
          doubtedTaskId: null,
          surface: 'needs_you',
          infoGainBits: 0.8,
        },
      ],
      hopperTasks: [
        { item_id: 'task-blocked', intent: 'A blocked task', priority: 1, state: 'inbox', task_id: 999 },
      ],
      refresh: vi.fn(),
      resolveApproval: vi.fn(),
      resolveFeedback: vi.fn(),
    };
    render(<TasksView attention={attention as any} />);
    await waitFor(() => {
      expect(screen.getAllByText(/Blocked/i).length).toBeGreaterThan(0);
      expect(screen.getByText(/waiting on Needs You/i)).toBeDefined();
    });
  });

  it('mutations (e.g. cancel) refresh via the shared attention inbox, not the local self-fetch', async () => {
    const attention = {
      approvals: [],
      needsYou: [],
      withheld: [],
      blockedTasksCount: 0,
      totalCount: 0,
      hopperTasks: [
        { item_id: 'task-1', intent: 'Task 1', priority: 1, state: 'assigned', task_id: 1 },
      ],
      refresh: vi.fn().mockResolvedValue(undefined),
      resolveApproval: vi.fn(),
      resolveFeedback: vi.fn(),
    };
    vi.mocked(invoke).mockResolvedValue(undefined);
    render(<TasksView attention={attention as any} />);
    await waitFor(() => expect(screen.getByText('Task 1')).toBeDefined());
    fireEvent.click(screen.getByTitle('Cancel task'));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('hopper_cancel', { itemId: 'task-1' }));
    await waitFor(() => expect(attention.refresh).toHaveBeenCalled());
    expect(hopperList).not.toHaveBeenCalled();
  });
});
