// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import { OperatorConsole } from './OperatorConsole';

describe('OperatorConsole', () => {
  it('renders the KPI strip, Resources and Agents sections', () => {
    render(<OperatorConsole />);
    expect(screen.getByText('Active Agents')).toBeInTheDocument();
    expect(screen.getByText('Resources')).toBeInTheDocument();
    expect(screen.getByText('Agents')).toBeInTheDocument();
    // hero agent + a row agent
    expect(screen.getByText('Atlas')).toBeInTheDocument();
    expect(screen.getByText('Castellum')).toBeInTheDocument();
  });

  it('shows Approve/Reject only for agents with pending approvals', () => {
    render(<OperatorConsole />);
    // Atlas (pending) and Groma (pending) → 2 Approve buttons
    expect(screen.getAllByRole('button', { name: 'Approve' })).toHaveLength(2);
    expect(screen.getAllByRole('button', { name: 'Reject' })).toHaveLength(2);
  });

  it('introduces each section with an underlined heading (border-b, not border-t)', () => {
    render(<OperatorConsole />);
    const resources = screen.getByText('Resources');
    expect(resources.className).toMatch(/border-b/);
    expect(resources.className).not.toMatch(/border-t/);
  });
});
