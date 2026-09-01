// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import React from 'react';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(() => Promise.resolve()) }));

import { PlanPanel } from './PlanPanel';
import { invoke } from '@tauri-apps/api/core';

const nodes = [
  { node_id: 'n1', description: 'first step', status: 'completed' as const },
  { node_id: 'n2', description: 'second step', status: 'in_progress' as const },
  { node_id: 'n3', description: 'third step', status: 'pending' as const },
];

describe('PlanPanel', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders each node with a status-appropriate visual state', () => {
    render(<PlanPanel planSessionId="ps1" planVersion={1} nodes={nodes} />);
    expect(screen.getByText('first step')).toBeInTheDocument();
    expect(screen.getByText('second step')).toBeInTheDocument();
    expect(screen.getByDisplayValue('third step')).toBeInTheDocument();
    expect(screen.getByTestId('plan-node-n1').textContent).toMatch(/●/);
    expect(screen.getByTestId('plan-node-n2').textContent).toMatch(/◑/);
    expect(screen.getByTestId('plan-node-n3').textContent).toMatch(/○/);
  });

  it('editing a pending node description calls update_plan_node with the right input shape', async () => {
    render(<PlanPanel planSessionId="ps1" planVersion={1} nodes={nodes} />);
    const input = screen.getByDisplayValue('third step');
    fireEvent.change(input, { target: { value: 'edited third step' } });
    fireEvent.blur(input);
    expect(invoke).toHaveBeenCalledWith('update_plan_node', {
      input: { plan_session_id: 'ps1', plan_version: 1, node_id: 'n3', description: 'edited third step' },
    });
  });

  it('a completed or in-progress node is not rendered as an editable input', () => {
    render(<PlanPanel planSessionId="ps1" planVersion={1} nodes={nodes} />);
    expect(screen.getByText('first step').tagName).not.toBe('INPUT');
    expect(screen.getByText('second step').tagName).not.toBe('INPUT');
    expect(screen.queryByDisplayValue('first step')).not.toBeInTheDocument();
    expect(screen.queryByDisplayValue('second step')).not.toBeInTheDocument();
  });

  it('adding a new step calls insert_plan_node with a fresh node_id', async () => {
    render(<PlanPanel planSessionId="ps1" planVersion={1} nodes={nodes} />);
    fireEvent.click(screen.getByRole('button', { name: /add step/i }));
    const newInput = screen.getByPlaceholderText(/new step/i);
    fireEvent.change(newInput, { target: { value: 'a fourth step' } });
    fireEvent.keyDown(newInput, { key: 'Enter' });
    expect(invoke).toHaveBeenCalledWith(
      'insert_plan_node',
      expect.objectContaining({
        input: expect.objectContaining({
          plan_session_id: 'ps1',
          plan_version: 1,
          description: 'a fourth step',
          depends_on: [],
        }),
      }),
    );
  });

  it('renders an honest empty state when there is no active plan', () => {
    render(<PlanPanel planSessionId={null} planVersion={null} nodes={[]} />);
    expect(screen.getByText(/no to-dos yet/i)).toBeInTheDocument();
  });

  it('shows to-do-list-labeled zero-step copy, not "plan" language', () => {
    render(<PlanPanel planSessionId="s1" planVersion={1} nodes={[]} />);
    expect(screen.getByText('Nothing to do yet.')).toBeInTheDocument();
  });

  it('renders an approval footer and approves via approve_plan_nodes when steps are blocked', async () => {
    const blockedNodes = [
      { node_id: 'n1', description: 'first step', status: 'blocked_on_approval' as const },
      { node_id: 'n2', description: 'second step', status: 'blocked_on_approval' as const },
    ];
    render(<PlanPanel planSessionId="ps1" planVersion={1} nodes={blockedNodes} />);
    expect(screen.getByText('2 steps awaiting approval')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /approve/i }));
    expect(invoke).toHaveBeenCalledWith('approve_plan_nodes', { planSessionId: 'ps1' });
  });

  it('does not render the approval footer once nothing is blocked', () => {
    render(<PlanPanel planSessionId="ps1" planVersion={1} nodes={nodes} />);
    expect(screen.queryByTestId('plan-approval-footer')).not.toBeInTheDocument();
  });

  it('discard calls the onDiscard callback without touching the backend', () => {
    const onDiscard = vi.fn();
    const blockedNodes = [
      { node_id: 'n1', description: 'first step', status: 'blocked_on_approval' as const },
    ];
    render(<PlanPanel planSessionId="ps1" planVersion={1} nodes={blockedNodes} onDiscard={onDiscard} />);
    fireEvent.click(screen.getByRole('button', { name: /discard/i }));
    expect(onDiscard).toHaveBeenCalledTimes(1);
    expect(invoke).not.toHaveBeenCalled();
  });
});
