// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { Dashboard } from './Dashboard';
import type { DashboardData, KPI } from '../../../types/dashboard';

const mockKPI: KPI = { label: 'Test', value: 0, cap: 1, spark: [] };
const emptyDash: DashboardData = {
  peers: [],
  kpis: { budgetBurn: mockKPI, mesh: mockKPI },
  agents: [],
  stream: [],
  alerts: [],
  contextChips: [],
  skills: [],
};

describe('Dashboard', () => {
  it('renders "The Stream" heading', () => {
    render(
      <Dashboard
        data={emptyDash}
        onPause={vi.fn()}
        onResume={vi.fn()}
        onDoubt={vi.fn()}
        onOverrule={vi.fn()}
        onAckLudus={vi.fn()}
        filterKind="all"
        setFilterKind={vi.fn()}
      />
    );
    expect(screen.getByText('The Stream')).toBeDefined();
  });

  it('renders "Active Agents" heading', () => {
    render(
      <Dashboard
        data={emptyDash}
        onPause={vi.fn()}
        onResume={vi.fn()}
        onDoubt={vi.fn()}
        onOverrule={vi.fn()}
        onAckLudus={vi.fn()}
        filterKind="all"
        setFilterKind={vi.fn()}
      />
    );
    expect(screen.getByText('Active Agents')).toBeDefined();
  });

  it('shows empty state for The Stream when stream is empty', () => {
    render(
      <Dashboard
        data={emptyDash}
        onPause={vi.fn()}
        onResume={vi.fn()}
        onDoubt={vi.fn()}
        onOverrule={vi.fn()}
        onAckLudus={vi.fn()}
        filterKind="all"
        setFilterKind={vi.fn()}
      />
    );
    expect(screen.getByText(/No events yet/i)).toBeDefined();
  });

  it('filter buttons have type="button"', () => {
    render(
      <Dashboard
        data={emptyDash}
        onPause={vi.fn()}
        onResume={vi.fn()}
        onDoubt={vi.fn()}
        onOverrule={vi.fn()}
        onAckLudus={vi.fn()}
        filterKind="all"
        setFilterKind={vi.fn()}
      />
    );
    const filterBtns = screen.getAllByRole('button');
    filterBtns.forEach(btn => {
      expect(btn.getAttribute('type')).toBe('button');
    });
  });
});
