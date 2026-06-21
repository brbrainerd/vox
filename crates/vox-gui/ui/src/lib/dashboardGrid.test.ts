import { describe, it, expect } from 'vitest';
import { defaultDashboardLayout, type DashboardLayout } from './dashboardLayout';
import { reorderDashboardWidgets, resizeDashboardWidget } from './dashboardGrid';

// Inline fixture so these math tests are independent of the product default layout.
const fixture = (): DashboardLayout => ({ version: 1, columns: 12, widgets: [
  { id: 'a', kind: 'stream', grid: { col: 1, row: 1, w: 8, h: 4 } },
  { id: 'b', kind: 'alerts', grid: { col: 9, row: 1, w: 4, h: 2 } },
  { id: 'c', kind: 'agents', grid: { col: 9, row: 3, w: 4, h: 2 } },
] });

describe('reorderDashboardWidgets', () => {
  it('swaps widget order when active and over differ', () => {
    const layout = fixture();
    const ids = layout.widgets.map((w) => w.id);
    expect(ids).toEqual(['a', 'b', 'c']);

    const next = reorderDashboardWidgets(layout, 'a', 'c');
    expect(next.widgets.map((w) => w.id)).toEqual(['c', 'b', 'a']);
  });

  it('returns the same layout when active and over are identical', () => {
    const layout = defaultDashboardLayout();
    const next = reorderDashboardWidgets(layout, 'stream', 'stream');
    expect(next).toBe(layout);
  });

  it('returns the same layout when ids are unknown', () => {
    const layout = defaultDashboardLayout();
    const next = reorderDashboardWidgets(layout, 'missing', 'stream');
    expect(next).toBe(layout);
  });
});

describe('resizeDashboardWidget', () => {
  it('increases w and h when deltas are positive', () => {
    const layout = fixture();
    const next = resizeDashboardWidget(layout, 'a', 1, 1);
    const stream = next.widgets.find((w) => w.id === 'a');
    expect(stream?.grid.w).toBe(9);
    expect(stream?.grid.h).toBe(5);
  });

  it('decreases w and h when deltas are negative', () => {
    const layout = defaultDashboardLayout();
    const next = resizeDashboardWidget(layout, 'stream', -6, -2);
    const stream = next.widgets.find((w) => w.id === 'stream');
    expect(stream?.grid.w).toBe(2);
    expect(stream?.grid.h).toBe(2);
  });

  it('clamps width to the column boundary', () => {
    const layout = defaultDashboardLayout();
    const next = resizeDashboardWidget(layout, 'alerts', 10, 0);
    const alerts = next.widgets.find((w) => w.id === 'alerts');
    expect(alerts?.grid.w).toBe(4);
    expect(alerts?.grid.col).toBe(9);
  });

  it('clamps height to minimum span of 2', () => {
    const layout = defaultDashboardLayout();
    const next = resizeDashboardWidget(layout, 'alerts', 0, -5);
    const alerts = next.widgets.find((w) => w.id === 'alerts');
    expect(alerts?.grid.h).toBe(2);
  });

  it('clamps width to minimum span of 2', () => {
    const layout = defaultDashboardLayout();
    const next = resizeDashboardWidget(layout, 'stream', -10, 0);
    const stream = next.widgets.find((w) => w.id === 'stream');
    expect(stream?.grid.w).toBe(2);
  });

  it('returns the same layout when widget id is unknown', () => {
    const layout = defaultDashboardLayout();
    const next = resizeDashboardWidget(layout, 'missing', 1, 1);
    expect(next).toBe(layout);
  });

  it('does not change other widgets', () => {
    const layout = defaultDashboardLayout();
    const next = resizeDashboardWidget(layout, 'alerts', 1, 0);
    const stream = next.widgets.find((w) => w.id === 'stream');
    expect(stream?.grid).toEqual(layout.widgets.find((w) => w.id === 'stream')?.grid);
  });
});
