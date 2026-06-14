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
});
