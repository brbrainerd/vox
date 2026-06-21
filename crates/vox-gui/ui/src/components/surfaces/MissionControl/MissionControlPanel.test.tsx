// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import React from 'react';

const invokeMock = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import { MissionControlPanel } from './MissionControlPanel';

describe('MissionControlPanel', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    // Default: empty responses for both commands
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'list_subagent_tree') return Promise.resolve([]);
      if (cmd === 'list_mc_approvals') return Promise.resolve([]);
      return Promise.resolve(null);
    });
  });

  it('renders the three section headings', async () => {
    render(<MissionControlPanel />);
    await waitFor(() => {
      expect(screen.getByText('Agents')).toBeDefined();
      expect(screen.getByText('Needs You')).toBeDefined();
      expect(screen.getByText('Mesh')).toBeDefined();
    });
  });

  it('shows empty state for agents when tree is empty', async () => {
    render(<MissionControlPanel />);
    await waitFor(() => {
      expect(screen.getByText(/No active subagent delegations/i)).toBeDefined();
    });
  });

  it('shows empty state for approvals when list is empty', async () => {
    render(<MissionControlPanel />);
    await waitFor(() => {
      expect(screen.getByText(/No pending approvals/i)).toBeDefined();
    });
  });

  it('renders a subagent node when tree is non-empty', async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'list_subagent_tree') {
        return Promise.resolve([
          { task_id: 42, agent_id: 7, reason: 'delegate-writes', parent_agent_id: 1 },
        ]);
      }
      return Promise.resolve([]);
    });
    render(<MissionControlPanel />);
    await waitFor(() => {
      // The reason text is rendered in its own span
      expect(screen.getByText('delegate-writes')).toBeDefined();
    });
  });

  it('renders approval rows with Approve and Reject buttons', async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'list_mc_approvals') {
        return Promise.resolve([
          {
            approval_id: 'REV-42',
            tool: 'task_review',
            summary: 'Review required for task 42',
            requested_at_ms: Date.now(),
          },
        ]);
      }
      return Promise.resolve([]);
    });
    render(<MissionControlPanel />);
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /Approve REV-42/i })).toBeDefined();
      expect(screen.getByRole('button', { name: /Reject REV-42/i })).toBeDefined();
    });
  });

  it('renders the mesh policy Apply button', async () => {
    render(<MissionControlPanel />);
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /Apply/i })).toBeDefined();
    });
  });
});
