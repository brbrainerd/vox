// @vitest-environment jsdom
import { describe, it, expect, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import React from 'react';
import { SubAgentTree } from './SubAgentTree';
import { useSubAgentStore } from './subAgentStore';
import type { SubAgentNode } from './types';

const tree: SubAgentNode[] = [{
  windowId: 'w1', parentWindowId: null, title: 'planner', skill: 'plan',
  model: { id: 'sonnet', maxTokens: 200000, toolCapable: true }, status: 'running', usedTokens: 1000, depth: 0,
  children: [{ windowId: 'w2', parentWindowId: 'w1', title: 'searcher', skill: 'search',
    model: { id: 'haiku', maxTokens: 8000, toolCapable: true }, status: 'idle', usedTokens: 7600, depth: 1, children: [] }],
}];

describe('SubAgentTree', () => {
  beforeEach(() => { useSubAgentStore.getState().reset(); useSubAgentStore.getState().setTree(tree); });
  it('shows the root with its skill badge', () => {
    render(<SubAgentTree />);
    expect(screen.getByText('planner')).toBeDefined();
    expect(screen.getByText('plan')).toBeDefined();
  });
  it('reveals a nested child after expanding', () => {
    render(<SubAgentTree />);
    expect(screen.queryByText('searcher')).toBeNull();
    fireEvent.click(screen.getByLabelText('expand planner'));
    expect(screen.getByText('searcher')).toBeDefined();
  });
  it('marks an over-budget node', () => {
    useSubAgentStore.getState().toggleExpand('w1');
    render(<SubAgentTree />);
    expect(screen.getByTestId('budget-w2').getAttribute('data-fate')).toBe('warn');
  });
  it('never renders a fabricated 0/0 budget when maxTokens is unknown', () => {
    useSubAgentStore.getState().setTree([{
      windowId: 'w3', parentWindowId: null, title: 'orchestrator edge', skill: null,
      model: { id: 'orchestrator', maxTokens: 0, toolCapable: false }, status: 'running', usedTokens: 0, depth: 0, children: [],
    }]);
    render(<SubAgentTree />);
    expect(screen.queryByText('0/0')).toBeNull();
  });
});
