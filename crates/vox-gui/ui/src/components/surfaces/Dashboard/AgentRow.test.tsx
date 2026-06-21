// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { AgentRow } from './AgentRow';
import type { Agent } from '../../../types/dashboard';

const agent: Agent = { id: 'A-1', codename: 'Atlas', phase: 'Executing', progress: 0.6, task: 'refactor', cost: 1, budget: null, eta: '2m' };

const mockAgent: Agent = {
  id: 'A-1',
  codename: 'alpha',
  phase: 'Executing',
  progress: 0.5,
  task: 'Build something',
  cost: 0.05,
  budget: 1.0,
  eta: '2m',
};

const pausedAgent: Agent = { ...mockAgent, phase: 'Paused' };

it('shows Approve/Reject only when a pending approval is supplied, and calls onApprove', async () => {
  const onApprove = vi.fn();
  render(<AgentRow a={agent} onPause={vi.fn()} onResume={vi.fn()} pendingApprovalId="ap1" onApprove={onApprove} onReject={vi.fn()} />);
  await userEvent.click(screen.getByRole('button', { name: 'Approve' }));
  expect(onApprove).toHaveBeenCalledWith('ap1');
});

it('renders no Approve/Reject without a pending approval', () => {
  render(<AgentRow a={agent} onPause={vi.fn()} onResume={vi.fn()} />);
  expect(screen.queryByRole('button', { name: 'Approve' })).toBeNull();
});

describe('AgentRow', () => {
  it('renders agent codename', () => {
    render(<AgentRow a={mockAgent} onPause={vi.fn()} onResume={vi.fn()} />);
    expect(screen.getByText('alpha')).toBeDefined();
  });

  it('pause button has aria-label when agent is running', () => {
    render(<AgentRow a={mockAgent} onPause={vi.fn()} onResume={vi.fn()} />);
    const btn = screen.queryByLabelText(/pause/i);
    expect(btn).not.toBeNull();
  });

  it('resume button has aria-label when agent is paused', () => {
    render(<AgentRow a={pausedAgent} onPause={vi.fn()} onResume={vi.fn()} />);
    const btn = screen.queryByLabelText(/resume/i);
    expect(btn).not.toBeNull();
  });

  it('pause/resume button has type="button"', () => {
    render(<AgentRow a={mockAgent} onPause={vi.fn()} onResume={vi.fn()} />);
    const buttons = screen.getAllByRole('button');
    buttons.forEach(btn => {
      expect(btn.getAttribute('type')).toBe('button');
    });
  });

  it('console button has aria-label when onOpenInConsole is provided', () => {
    render(<AgentRow a={mockAgent} onPause={vi.fn()} onResume={vi.fn()} onOpenInConsole={vi.fn()} />);
    const btn = screen.queryByLabelText(/console/i);
    expect(btn).not.toBeNull();
  });
});
