// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue(null),
}));

import { TasksView } from './TasksView';
import type { AttentionInbox } from '../../../hooks/useAttentionInbox';

// Shared-inbox mode: no self-fetching, so no transport spies needed.
const attention: AttentionInbox = {
  approvals: [],
  needsYou: [],
  withheld: [],
  blockedTasksCount: 0,
  hopperTasks: [],
  totalCount: 0,
  refresh: vi.fn().mockResolvedValue(undefined),
  resolveApproval: vi.fn().mockResolvedValue(undefined),
  resolveFeedback: vi.fn().mockResolvedValue(undefined),
};

describe('TasksView copy honesty (B1 interim)', () => {
  it('does not claim chat submissions land here, and says where rows really come from', () => {
    render(<TasksView attention={attention} />);
    // The old lie: chat submissions go to the orchestrator task graph, not the hopper.
    expect(screen.queryByText(/chat submissions land here/i)).toBeNull();
    // The honest replacement names both stores.
    expect(screen.getByText(/hopper/i)).toBeTruthy();
    expect(screen.getByText(/orchestrator task graph/i)).toBeTruthy();
  });
});
