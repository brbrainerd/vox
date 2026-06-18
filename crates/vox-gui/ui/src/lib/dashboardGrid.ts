import type { DashboardLayout } from './dashboardLayout';

const MIN_WIDGET_SPAN = 2;

function clampSpan(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

/**
 * Reorder dashboard widgets by swapping the active widget with the drop target
 * in the layout's widget array (DOM order).
 */
export function reorderDashboardWidgets(
  layout: DashboardLayout,
  activeId: string,
  overId: string,
): DashboardLayout {
  if (activeId === overId) {
    return layout;
  }

  const fromIndex = layout.widgets.findIndex((w) => w.id === activeId);
  const toIndex = layout.widgets.findIndex((w) => w.id === overId);
  if (fromIndex === -1 || toIndex === -1) {
    return layout;
  }

  const widgets = [...layout.widgets];
  [widgets[fromIndex], widgets[toIndex]] = [widgets[toIndex], widgets[fromIndex]];
  return { ...layout, widgets };
}

/**
 * Resize a dashboard widget by grid-cell deltas. Width is clamped to min 2 and
 * the layout column boundary; height is clamped to min 2.
 */
export function resizeDashboardWidget(
  layout: DashboardLayout,
  widgetId: string,
  deltaW: number,
  deltaH: number,
): DashboardLayout {
  const index = layout.widgets.findIndex((w) => w.id === widgetId);
  if (index === -1) {
    return layout;
  }

  const widget = layout.widgets[index];
  const maxW = layout.columns - widget.grid.col + 1;
  const nextW = clampSpan(widget.grid.w + deltaW, MIN_WIDGET_SPAN, maxW);
  const nextH = clampSpan(widget.grid.h + deltaH, MIN_WIDGET_SPAN, Number.MAX_SAFE_INTEGER);

  if (nextW === widget.grid.w && nextH === widget.grid.h) {
    return layout;
  }

  const widgets = layout.widgets.map((entry, i) =>
    i === index
      ? {
          ...entry,
          grid: {
            ...entry.grid,
            w: nextW,
            h: nextH,
          },
        }
      : entry,
  );

  return { ...layout, widgets };
}
