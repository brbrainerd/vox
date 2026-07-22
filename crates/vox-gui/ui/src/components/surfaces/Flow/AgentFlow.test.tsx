// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import React from 'react';
import { AgentFlow } from './AgentFlow';
import type { Agent } from '../../../types/dashboard';

const agents: Agent[] = [
  {
    id: 'a1',
    codename: 'Falcon',
    phase: 'Executing',
    task: 'compile',
    progress: 0.5,
    cost: 1,
    budget: 5,
    eta: '2m',
    skill: 'compiler',
  } as Agent,
];

describe('AgentFlow', () => {
  it('renders nodes as keyboard-operable buttons', () => {
    render(<AgentFlow agents={agents} />);
    const nodes = screen.getAllByRole('button');
    expect(nodes.length).toBeGreaterThan(0);
    for (const n of nodes) {
      expect(n.getAttribute('tabindex')).toBe('0');
    }
  });

  it('selects a node via keyboard (Enter)', () => {
    const onSelect = vi.fn();
    render(<AgentFlow agents={agents} onSelect={onSelect} />);
    const node = screen.getByRole('button', { name: /Falcon/i });
    fireEvent.keyDown(node, { key: 'Enter' });
    expect(onSelect).toHaveBeenCalledWith('a1');
  });

  it('exposes the inspector progress as a progressbar', () => {
    render(<AgentFlow agents={agents} selectedId="a1" />);
    const bar = screen.getByRole('progressbar', { name: /progress/i });
    expect(bar.getAttribute('aria-valuenow')).toBe('50');
  });

  it('renders the inspector without crashing when budget is null (chat-category tasks have no budget)', () => {
    const noBudgetAgents: Agent[] = [
      { ...agents[0], budget: null },
    ];
    render(<AgentFlow agents={noBudgetAgents} selectedId="a1" />);
    expect(screen.getByText('—')).toBeInTheDocument();
  });

  it('allows the header title to shrink and the legend row to wrap on narrow panels', () => {
    render(<AgentFlow agents={agents} />);
    const heading = screen.getByText('Mind-Map · Agent Shards');
    const titleContainer = heading.parentElement;
    expect(titleContainer?.className).toContain('min-w-0');

    const headerRow = titleContainer?.parentElement;
    expect(headerRow?.className).toContain('flex-wrap');

    const legend = screen.getByText('Planning').closest('div')?.parentElement;
    expect(legend?.className).toContain('flex-wrap');
  });
});
