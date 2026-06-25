import { describe, it, expect } from 'vitest';
import { defaultDashboardLayout, type DashboardLayout } from './dashboardLayout';
import {
  reorderDashboardWidgets,
  resizeDashboardWidget,
  effectiveColumns,
  MIN_COL_PX,
} from './dashboardGrid';

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

describe('effectiveColumns', () => {
  it('returns 1 column at a narrow 300px width', () => {
    expect(effectiveColumns(300, 12)).toBe(1);
  });

  it('returns 2 columns at ~520px width', () => {
    expect(effectiveColumns(520, 12)).toBe(2);
  });

  it('returns the max column count at a wide width', () => {
    expect(effectiveColumns(12 * MIN_COL_PX, 12)).toBe(12);
    expect(effectiveColumns(99999, 12)).toBe(12);
  });

  it('never exceeds the configured max even when many columns fit', () => {
    expect(effectiveColumns(99999, 4)).toBe(4);
  });

  it('clamps to at least 1 column', () => {
    expect(effectiveColumns(10, 12)).toBe(1);
    expect(effectiveColumns(0, 12)).toBe(12);
  });

  it('keeps each column at least MIN_COL_PX wide', () => {
    expect(effectiveColumns(3 * MIN_COL_PX + 10, 12)).toBe(3);
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
