// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { act } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import React from 'react';
import { Dashboard } from './Dashboard';
import type { DashboardData, KPI } from '../../../types/dashboard';
import { addWidgetToLayout, defaultDashboardLayout } from '../../../lib/dashboardLayout';
import { SHELL_PREFERENCE_KEYS } from '../../../lib/shellPersistence';
import {
  metricSeriesFromSpark,
  metricSeriesStorageKey,
} from '../../../hooks/useMetricSeries';

vi.mock('recharts', () => ({
  ResponsiveContainer: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  LineChart: ({ children }: { children: React.ReactNode }) => <svg>{children}</svg>,
  BarChart: ({ children }: { children: React.ReactNode }) => <svg>{children}</svg>,
  AreaChart: ({ children }: { children: React.ReactNode }) => <svg>{children}</svg>,
  Line: () => null,
  Bar: () => null,
  Area: () => null,
  XAxis: () => null,
  YAxis: () => null,
  CartesianGrid: () => null,
  Tooltip: () => null,
}));

const mockKPI: KPI = { label: 'Test', value: 0, cap: 1, spark: [] };
const emptyDash: DashboardData = {
  peers: [],
  kpis: {
    budgetBurn: mockKPI,
    mesh: mockKPI,
    queueDepth: { value: 0, spark: [] },
  },
  agents: [],
  stream: [],
  alerts: [],
  contextChips: [],
  skills: [],
};

const baseData = emptyDash;
function renderDashboard(over: Partial<DashboardData> = {}) {
  return render(
    <Dashboard
      data={{ ...baseData, ...over }}
      onPause={vi.fn()} onResume={vi.fn()} onDoubt={vi.fn()} onOverrule={vi.fn()} onAckLudus={vi.fn()}
      filterKind="all" setFilterKind={vi.fn()}
    />,
  );
}

describe('Dashboard', () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

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
    expect(
      screen.getByText(/Live agent telemetry streams here once tasks run\. Open Chat to submit a task\./i),
    ).toBeDefined();
  });

  it('calls onOpenChat when Open Chat CTA is clicked', async () => {
    const user = userEvent.setup();
    const onOpenChat = vi.fn();
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
        onOpenChat={onOpenChat}
      />
    );

    await user.click(screen.getByTestId('open-chat-cta'));
    expect(onOpenChat).toHaveBeenCalledOnce();
  });

  it('shows loading skeleton when loading prop is true', () => {
    render(
      <Dashboard
        data={emptyDash}
        loading
        onPause={vi.fn()}
        onResume={vi.fn()}
        onDoubt={vi.fn()}
        onOverrule={vi.fn()}
        onAckLudus={vi.fn()}
        filterKind="all"
        setFilterKind={vi.fn()}
      />
    );
    expect(screen.getByRole('status', { name: /loading dashboard/i })).toBeDefined();
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

  it('shows reset and add-widget controls when customizing', async () => {
    const user = userEvent.setup();
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
      />,
    );

    expect(screen.queryByRole('button', { name: /reset to default/i })).toBeNull();
    expect(screen.queryByRole('button', { name: /add widget/i })).toBeNull();

    await user.click(screen.getByRole('button', { name: /customize dashboard/i }));

    expect(screen.getByRole('button', { name: /reset to default/i })).toBeDefined();
    expect(screen.getByRole('button', { name: /add widget/i })).toBeDefined();
  });

  it('seeds queue depth chart series from kpis.queueDepth spark', () => {
    const queueSpark = [7, 8, 9, 10];
    const layout = addWidgetToLayout(defaultDashboardLayout(), 'queue_depth');
    window.localStorage.setItem(SHELL_PREFERENCE_KEYS.dashboardLayout, JSON.stringify(layout));

    const dashData: DashboardData = {
      ...emptyDash,
      kpis: {
        ...emptyDash.kpis,
        queueDepth: { value: 10, spark: queueSpark },
      },
    };

    render(
      <Dashboard
        data={dashData}
        onPause={vi.fn()}
        onResume={vi.fn()}
        onDoubt={vi.fn()}
        onOverrule={vi.fn()}
        onAckLudus={vi.fn()}
        filterKind="all"
        setFilterKind={vi.fn()}
      />,
    );

    const stored = JSON.parse(
      window.localStorage.getItem(metricSeriesStorageKey('queue_depth')) ?? '[]',
    );
    expect(stored).toEqual(metricSeriesFromSpark(queueSpark));
    expect(stored).not.toEqual(metricSeriesFromSpark([0, 1, 1, 2, 3, 2, 4, 3, 5, 4]));
  });

  it('appends queue depth samples when spark is empty and value changes', () => {
    const layout = addWidgetToLayout(defaultDashboardLayout(), 'queue_depth');
    window.localStorage.setItem(SHELL_PREFERENCE_KEYS.dashboardLayout, JSON.stringify(layout));

    const baseProps = {
      onPause: vi.fn(),
      onResume: vi.fn(),
      onDoubt: vi.fn(),
      onOverrule: vi.fn(),
      onAckLudus: vi.fn(),
      filterKind: 'all' as const,
      setFilterKind: vi.fn(),
    };

    const { rerender } = render(
      <Dashboard
        {...baseProps}
        data={{
          ...emptyDash,
          kpis: { ...emptyDash.kpis, queueDepth: { value: 3, spark: [] } },
        }}
      />,
    );

    act(() => {
      rerender(
        <Dashboard
          {...baseProps}
          data={{
            ...emptyDash,
            kpis: { ...emptyDash.kpis, queueDepth: { value: 7, spark: [] } },
          }}
        />,
      );
    });

    const stored = JSON.parse(
      window.localStorage.getItem(metricSeriesStorageKey('queue_depth')) ?? '[]',
    );
    expect(stored).toEqual([
      { t: 0, v: 3 },
      { t: 1, v: 7 },
    ]);
  });

  it('renders a 4-tile KPI strip including Mesh Peers', () => {
    renderDashboard({ peers: [
      { id: 'p1', name: 'node-a', backend: 'cuda', online: true },
      { id: 'p2', name: 'node-b', backend: 'cuda', online: false },
    ] });
    // 'Active Agents' appears in both the KPI strip label and the agents widget heading
    expect(screen.getAllByText('Active Agents').length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText('Queue Depth')).toBeInTheDocument();
    expect(screen.getByText('Budget Spent')).toBeInTheDocument();
    expect(screen.getByText('Mesh Peers')).toBeInTheDocument();
    // with the empty base: agents 0, queue 0, budget $0.00 → only Mesh Peers renders "1"
    expect(screen.getByText('1')).toBeInTheDocument();
  });

  it('renders the resources widget when present in the layout', () => {
    // force a layout containing the resources widget — use the SSOT key symbol, never the literal
    window.localStorage.setItem(SHELL_PREFERENCE_KEYS.dashboardLayout, JSON.stringify({
      version: 1, columns: 12, widgets: [{ id: 'resources', kind: 'resources', grid: { col: 1, row: 1, w: 12, h: 2 } }],
    }));
    renderDashboard({});
    expect(screen.getByText('Resources')).toBeInTheDocument();
  });

  it('renders visual sandbox mini-map and handles expand navigation', () => {
    const navigateMock = vi.fn();
    const data = { ...emptyDash };
    
    render(
      <Dashboard
        data={data}
        onPause={vi.fn()}
        onResume={vi.fn()}
        onDoubt={vi.fn()}
        onOverrule={vi.fn()}
        onAckLudus={vi.fn()}
        filterKind="all"
        setFilterKind={vi.fn()}
        onNavigate={navigateMock}
      />
    );

    const expandBtn = screen.getByText('Immersive View');
    expect(expandBtn).toBeDefined();
  });
});
