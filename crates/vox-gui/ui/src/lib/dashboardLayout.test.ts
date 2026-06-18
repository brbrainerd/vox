import { describe, it, expect } from 'vitest';
import {
  validateDashboardLayout,
  defaultDashboardLayout,
  addWidgetToLayout,
  resetDashboardLayout,
  DASHBOARD_WIDGET_KINDS,
} from './dashboardLayout';

describe('dashboardLayout', () => {
  it('default layout has stream, agents, and alerts widgets', () => {
    const layout = defaultDashboardLayout();
    expect(layout.widgets.map((w) => w.kind)).toEqual(
      expect.arrayContaining(['stream', 'agents', 'alerts']),
    );
  });

  it('rejects unknown widget kind', () => {
    expect(() =>
      validateDashboardLayout({
        version: 1,
        columns: 12,
        widgets: [{ id: 'x', kind: 'not-real', grid: { col: 1, row: 1, w: 4, h: 2 } }],
      }),
    ).toThrow(/unknown widget kind/i);
  });

  it('rejects widgets that overflow the grid', () => {
    expect(() =>
      validateDashboardLayout({
        version: 1,
        columns: 12,
        widgets: [{ id: 'x', kind: 'stream', grid: { col: 10, row: 1, w: 4, h: 2 } }],
      }),
    ).toThrow(/overflow/i);
  });

  it('addWidgetToLayout appends a widget of the given kind', () => {
    const layout = defaultDashboardLayout();
    const next = addWidgetToLayout(layout, 'line_chart');
    expect(next.widgets.some((w) => w.kind === 'line_chart')).toBe(true);
    expect(next.widgets.length).toBe(layout.widgets.length + 1);
  });

  it('addWidgetToLayout assigns a unique widget id', () => {
    const layout = defaultDashboardLayout();
    const once = addWidgetToLayout(layout, 'line_chart');
    const twice = addWidgetToLayout(once, 'line_chart');
    const ids = twice.widgets.map((w) => w.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it('resetDashboardLayout returns the default profile', () => {
    const mutated = addWidgetToLayout(defaultDashboardLayout(), 'bar_chart');
    expect(resetDashboardLayout()).toEqual(defaultDashboardLayout());
    expect(mutated.widgets.length).toBeGreaterThan(defaultDashboardLayout().widgets.length);
  });
});

describe('DASHBOARD_WIDGET_KINDS', () => {
  it('matches the dashboard-layout contract catalog', () => {
    expect(DASHBOARD_WIDGET_KINDS).toEqual(
      expect.arrayContaining(['stream', 'agents', 'alerts', 'line_chart', 'bar_chart']),
    );
    expect(DASHBOARD_WIDGET_KINDS.length).toBeGreaterThanOrEqual(14);
  });
});
