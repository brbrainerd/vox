import { describe, it, expect } from 'vitest';
import {
  validateDashboardLayout,
  defaultDashboardLayout,
  addWidgetToLayout,
  resetDashboardLayout,
  DASHBOARD_WIDGET_KINDS,
  surfaceKeyOf,
  type DashboardLayout,
} from './dashboardLayout';

describe('dashboardLayout', () => {
  it('default layout is the operator-console composition (resources + agents lead)', () => {
    const l = defaultDashboardLayout();
    expect(validateDashboardLayout(l)).toEqual(l); // stays schema-valid
    expect(l.widgets[0].kind).toBe('resources');
    expect(l.widgets.some((w) => w.kind === 'agents')).toBe(true);
    expect(l.widgets.some((w) => w.kind === 'stream')).toBe(true); // no feature loss
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

describe('surface_widget kind', () => {
  it('includes surface_widget in the kind SSOT', () => {
    expect(DASHBOARD_WIDGET_KINDS).toContain('surface_widget');
  });

  it('validates a surface_widget slot carrying a surfaceKey in config', () => {
    const raw = {
      version: 1,
      columns: 12,
      widgets: [
        { id: 'mesh-mini', kind: 'surface_widget', grid: { col: 1, row: 1, w: 4, h: 2 }, config: { surfaceKey: 'mesh' } },
      ],
    };
    const layout: DashboardLayout = validateDashboardLayout(raw);
    expect(layout.widgets[0].kind).toBe('surface_widget');
    expect(surfaceKeyOf(layout.widgets[0])).toBe('mesh');
  });

  it('surfaceKeyOf returns null when config has no string surfaceKey', () => {
    expect(surfaceKeyOf({ id: 'x', kind: 'agents', grid: { col: 1, row: 1, w: 4, h: 2 } })).toBeNull();
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
