/**
 * Dashboard layout validator — SSOT kinds from contracts/gui/dashboard-layout.v1.yaml
 */

export const DASHBOARD_WIDGET_KINDS = [
  'stream',
  'agents',
  'alerts',
  'kpi_spark',
  'line_chart',
  'bar_chart',
  'area_chart',
  'queue_depth',
  'budget_burn',
  'mesh_peers',
  'model_active',
  'openrouter_spend',
  'task_summary',
  'custom_text',
  'resources',
  'surface_widget',
] as const;

export type DashboardWidgetKind = (typeof DASHBOARD_WIDGET_KINDS)[number];

export interface DashboardGridCell {
  col: number;
  row: number;
  w: number;
  h: number;
}

export interface DashboardWidget {
  id: string;
  kind: DashboardWidgetKind;
  grid: DashboardGridCell;
  config?: Record<string, unknown>;
}

export interface DashboardLayout {
  version: 1;
  columns: number;
  widgets: DashboardWidget[];
}

const KIND_SET = new Set<string>(DASHBOARD_WIDGET_KINDS);

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

function parseGrid(raw: unknown, path: string): DashboardGridCell {
  if (!isRecord(raw)) {
    throw new Error(`${path}: grid must be an object`);
  }
  const col = raw.col;
  const row = raw.row;
  const w = raw.w;
  const h = raw.h;
  if (
    typeof col !== 'number' ||
    typeof row !== 'number' ||
    typeof w !== 'number' ||
    typeof h !== 'number' ||
    !Number.isInteger(col) ||
    !Number.isInteger(row) ||
    !Number.isInteger(w) ||
    !Number.isInteger(h) ||
    col < 1 ||
    row < 1 ||
    w < 1 ||
    h < 1
  ) {
    throw new Error(`${path}: grid col/row/w/h must be positive integers`);
  }
  return { col, row, w, h };
}

export function defaultDashboardLayout(): DashboardLayout {
  return {
    version: 1,
    columns: 12,
    widgets: [
      { id: 'resources', kind: 'resources', grid: { col: 1, row: 1, w: 12, h: 2 } },
      { id: 'agents', kind: 'agents', grid: { col: 1, row: 3, w: 8, h: 4 } },
      { id: 'alerts', kind: 'alerts', grid: { col: 9, row: 3, w: 4, h: 2 } },
      { id: 'stream', kind: 'stream', grid: { col: 9, row: 5, w: 4, h: 2 } },
    ],
  };
}

export function resetDashboardLayout(): DashboardLayout {
  return defaultDashboardLayout();
}

function nextWidgetId(layout: DashboardLayout, kind: DashboardWidgetKind): string {
  if (!layout.widgets.some((w) => w.id === kind)) {
    return kind;
  }
  let n = 2;
  while (layout.widgets.some((w) => w.id === `${kind}-${n}`)) {
    n += 1;
  }
  return `${kind}-${n}`;
}

function nextGridSlot(layout: DashboardLayout): DashboardGridCell {
  let maxBottom = 0;
  for (const widget of layout.widgets) {
    maxBottom = Math.max(maxBottom, widget.grid.row + widget.grid.h - 1);
  }
  return { col: 1, row: maxBottom + 1, w: 4, h: 2 };
}

export function addWidgetToLayout(
  layout: DashboardLayout,
  kind: DashboardWidgetKind,
): DashboardLayout {
  const widget: DashboardWidget = {
    id: nextWidgetId(layout, kind),
    kind,
    grid: nextGridSlot(layout),
  };
  return {
    ...layout,
    widgets: [...layout.widgets, widget],
  };
}

/** The surface key a `surface_widget` slot is backed by, or null. */
export function surfaceKeyOf(widget: DashboardWidget): string | null {
  const key = widget.config?.surfaceKey;
  return typeof key === 'string' && key.length > 0 ? key : null;
}

export function widgetKindLabel(kind: DashboardWidgetKind): string {
  return kind
    .split('_')
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(' ');
}

export function availableWidgetKinds(layout: DashboardLayout): DashboardWidgetKind[] {
  const present = new Set(layout.widgets.map((w) => w.kind));
  return DASHBOARD_WIDGET_KINDS.filter((kind) => !present.has(kind));
}

export function validateDashboardLayout(raw: unknown): DashboardLayout {
  if (!isRecord(raw)) {
    throw new Error('layout must be an object');
  }
  if (raw.version !== 1) {
    throw new Error('layout version must be 1');
  }
  const columns = raw.columns;
  if (typeof columns !== 'number' || !Number.isInteger(columns) || columns < 1) {
    throw new Error('columns must be a positive integer');
  }
  if (!Array.isArray(raw.widgets)) {
    throw new Error('widgets must be an array');
  }

  const widgets: DashboardWidget[] = [];
  const seenIds = new Set<string>();

  for (let i = 0; i < raw.widgets.length; i++) {
    const w = raw.widgets[i];
    const path = `widgets[${i}]`;
    if (!isRecord(w)) {
      throw new Error(`${path}: widget must be an object`);
    }
    if (typeof w.id !== 'string' || w.id.trim() === '') {
      throw new Error(`${path}: id must be a non-empty string`);
    }
    if (seenIds.has(w.id)) {
      throw new Error(`${path}: duplicate widget id "${w.id}"`);
    }
    seenIds.add(w.id);

    if (typeof w.kind !== 'string' || !KIND_SET.has(w.kind)) {
      throw new Error(`${path}: unknown widget kind "${String(w.kind)}"`);
    }

    const grid = parseGrid(w.grid, path);
    if (grid.col + grid.w - 1 > columns) {
      throw new Error(`${path}: widget grid overflow (col ${grid.col} + w ${grid.w} > ${columns} columns)`);
    }

    const widget: DashboardWidget = {
      id: w.id,
      kind: w.kind as DashboardWidgetKind,
      grid,
    };
    if (w.config !== undefined) {
      if (!isRecord(w.config)) {
        throw new Error(`${path}: config must be an object`);
      }
      widget.config = w.config;
    }
    widgets.push(widget);
  }

  return { version: 1, columns, widgets };
}
