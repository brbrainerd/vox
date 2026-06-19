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
});
