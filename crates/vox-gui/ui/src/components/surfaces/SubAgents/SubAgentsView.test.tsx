// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import React from 'react';
const fetchTreeMock = vi.fn();
const listenActivityMock = vi.fn();
vi.mock('./subAgentClient', () => ({
  fetchTree: (...a: unknown[]) => fetchTreeMock(...a),
  listenActivity: (...a: unknown[]) => listenActivityMock(...a),
}));
import { SubAgentsView } from './SubAgentsView';

describe('SubAgentsView', () => {
  beforeEach(() => {
    fetchTreeMock.mockReset();
    fetchTreeMock.mockResolvedValue([{ windowId: 'w1', parentWindowId: null, title: 'planner', skill: 'plan',
      model: { id: 'sonnet', maxTokens: 200000, toolCapable: true }, status: 'running', usedTokens: 1000, depth: 0, children: [] }]);
    listenActivityMock.mockReset();
    listenActivityMock.mockRejectedValue(new Error('not tauri'));
  });
  it('loads the tree and selecting a node shows its title and activity stream only', async () => {
    render(<SubAgentsView pushToast={() => {}} />);
    await waitFor(() => expect(screen.getAllByText('planner').length).toBeGreaterThanOrEqual(1));
    fireEvent.click(screen.getAllByText('planner')[0]);
    await waitFor(() => expect(screen.getByLabelText('activity for w1')).toBeDefined());
    expect(screen.getAllByText('planner').length).toBeGreaterThanOrEqual(2);
    expect(screen.queryByLabelText('committed set for w1')).toBeNull();
  });
  it('does not render the dead overrule control on selection', async () => {
    render(<SubAgentsView pushToast={() => {}} />);
    await waitFor(() => expect(screen.getAllByText('planner').length).toBeGreaterThanOrEqual(1));
    fireEvent.click(screen.getAllByText('planner')[0]);
    await waitFor(() => expect(screen.getByLabelText('activity for w1')).toBeDefined());
    expect(screen.queryByLabelText('overrule note')).toBeNull();
  });

  it('unlistens immediately when unmounted before listenActivity resolves (leak guard)', async () => {
    const unlisten = vi.fn();
    let resolveListen!: (u: () => void) => void;
    listenActivityMock.mockReset();
    listenActivityMock.mockImplementation(
      () => new Promise<() => void>((res) => { resolveListen = res; }),
    );
    const { unmount } = render(<SubAgentsView pushToast={() => {}} />);
    unmount();
    // Subscription resolves only AFTER unmount — the disposed flag must fire it.
    resolveListen(unlisten);
    await waitFor(() => expect(unlisten).toHaveBeenCalledTimes(1));
  });
});
