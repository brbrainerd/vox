// @vitest-environment jsdom
import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import { ActivityTimeline } from './ActivitySurface';
import React from 'react';

describe('ActivityTimeline', () => {
  it('renders rows newest-first with kind + summary', () => {
    render(<ActivityTimeline rows={[
      { id: 2, ts_ms: 2000, agent_id: 'A1', kind: 'TaskCompleted', summary: 'done', detail_json: '{}' },
      { id: 1, ts_ms: 1000, agent_id: 'A1', kind: 'AgentSpawned', summary: 'spawned', detail_json: '{}' },
    ]} />);
    const items = screen.getAllByTestId('activity-row');
    expect(items[0]).toHaveTextContent('TaskCompleted');
    expect(items[1]).toHaveTextContent('AgentSpawned');
  });

  it('folds 3 or more consecutive CostIncurred rows for the same agent', () => {
    render(<ActivityTimeline rows={[
      { id: 4, ts_ms: 4000, agent_id: 'A1', kind: 'TaskCompleted', summary: 'done', detail_json: '{}' },
      { id: 3, ts_ms: 3000, agent_id: 'A1', kind: 'CostIncurred', summary: 'Cost incurred: $0.0100 via anthropic/claude-opus', detail_json: '{"CostIncurred": {"cost_usd": 0.01}}' },
      { id: 2, ts_ms: 2000, agent_id: 'A1', kind: 'CostIncurred', summary: 'Cost incurred: $0.0200 via anthropic/claude-opus', detail_json: '{"CostIncurred": {"cost_usd": 0.02}}' },
      { id: 1, ts_ms: 1000, agent_id: 'A1', kind: 'CostIncurred', summary: 'Cost incurred: $0.0300 via anthropic/claude-opus', detail_json: '{"CostIncurred": {"cost_usd": 0.03}}' },
    ]} />);
    expect(screen.getByText(/spent \$0\.0600/)).toBeDefined();
    expect(screen.getByText(/\(3 calls\)/)).toBeDefined();
    const items = screen.getAllByTestId('activity-row');
    expect(items.length).toBe(2); // 1 TaskCompleted + 1 Folded CostIncurred
  });
});
