// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import React from 'react';
const fetchTreeMock = vi.fn();
vi.mock('./subAgentClient', () => ({
  fetchTree: (...a: unknown[]) => fetchTreeMock(...a),
  getContext: vi.fn().mockResolvedValue([]),
  setContext: vi.fn().mockResolvedValue(undefined),
  control: vi.fn().mockResolvedValue(undefined),
  listenActivity: vi.fn().mockRejectedValue(new Error('not tauri')),
}));
import { SubAgentsView } from './SubAgentsView';

describe('SubAgentsView', () => {
  beforeEach(() => {
    fetchTreeMock.mockReset();
    fetchTreeMock.mockResolvedValue([{ windowId: 'w1', parentWindowId: null, title: 'planner', skill: 'plan',
      model: { id: 'sonnet', maxTokens: 200000, toolCapable: true }, status: 'running', usedTokens: 1000, depth: 0, children: [] }]);
  });
  it('loads the tree and selecting a node shows its context editor', async () => {
    render(<SubAgentsView pushToast={() => {}} />);
    await waitFor(() => expect(screen.getByText('planner')).toBeDefined());
    fireEvent.click(screen.getByText('planner'));
    await waitFor(() => expect(screen.getByLabelText('committed set for w1')).toBeDefined());
  });
});
