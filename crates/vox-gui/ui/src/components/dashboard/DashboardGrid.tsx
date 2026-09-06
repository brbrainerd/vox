import React from 'react';
import {
  DndContext,
  closestCenter,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
} from '@dnd-kit/core';
import {
  SortableContext,
  rectSortingStrategy,
  useSortable,
} from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import type { DashboardLayout, DashboardWidget } from '../../lib/dashboardLayout';
import { reorderDashboardWidgets, resizeDashboardWidget, effectiveColumns, MIN_COL_PX } from '../../lib/dashboardGrid';
import { SHELL_PREFERENCE_KEYS } from '../../lib/shellPersistence';
import { validateDashboardLayout } from '../../lib/dashboardLayout';

export interface DashboardGridProps {
  layout: DashboardLayout;
  customizeMode: boolean;
  onLayoutChange: (layout: DashboardLayout) => void;
  renderWidget: (widget: DashboardWidget) => React.ReactNode;
}

function gridCellStyle(widget: DashboardWidget, effectiveCols: number): React.CSSProperties {
  const { col, row, w, h } = widget.grid;
  // Clamp the starting column and span to the effective column count so a
  // widget configured for a wider grid never overflows a narrower render.
  const startCol = Math.min(col, effectiveCols);
  const span = Math.max(1, Math.min(w, effectiveCols - startCol + 1));
  return {
    gridColumn: `${startCol} / span ${span}`,
    gridRow: `${row} / span ${h}`,
  };
}

function gridCellMetrics(gridEl: HTMLElement, columns: number): { colWidth: number; rowHeight: number; gap: number } {
  const rect = gridEl.getBoundingClientRect();
  const styles = getComputedStyle(gridEl);
  const gap = parseFloat(styles.gap) || 0;
  const paddingLeft = parseFloat(styles.paddingLeft) || 0;
  const paddingRight = parseFloat(styles.paddingRight) || 0;
  const innerWidth = rect.width - paddingLeft - paddingRight;
  const colWidth = (innerWidth - gap * (columns - 1)) / columns;
  return { colWidth, rowHeight: colWidth, gap };
}

function pointerDeltaToCellDelta(
  dx: number,
  dy: number,
  colWidth: number,
  rowHeight: number,
  gap: number,
): { deltaW: number; deltaH: number } {
  const stepX = colWidth + gap;
  const stepY = rowHeight + gap;
  return {
    deltaW: stepX > 0 ? Math.round(dx / stepX) : 0,
    deltaH: stepY > 0 ? Math.round(dy / stepY) : 0,
  };
}

interface SortableWidgetCellProps {
  widget: DashboardWidget;
  customizeMode: boolean;
  layout: DashboardLayout;
  effectiveCols: number;
  gridRef: React.RefObject<HTMLDivElement | null>;
  onResize: (layout: DashboardLayout) => void;
  children: React.ReactNode;
}

function SortableWidgetCell({
  widget,
  customizeMode,
  layout,
  effectiveCols,
  gridRef,
  onResize,
  children,
}: SortableWidgetCellProps) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: widget.id,
    disabled: !customizeMode,
  });

  const resizeSessionRef = React.useRef<{
    startLayout: DashboardLayout;
    startX: number;
    startY: number;
  } | null>(null);

  const style: React.CSSProperties = {
    ...gridCellStyle(widget, effectiveCols),
    transform: CSS.Transform.toString(transform),
    transition,
    opacity: isDragging ? 0.6 : undefined,
    position: 'relative',
  };

  function handleResizePointerDown(event: React.PointerEvent<HTMLButtonElement>) {
    event.preventDefault();
    event.stopPropagation();
    resizeSessionRef.current = {
      startLayout: layout,
      startX: event.clientX,
      startY: event.clientY,
    };
    event.currentTarget.setPointerCapture(event.pointerId);
  }

  function handleResizePointerMove(event: React.PointerEvent<HTMLButtonElement>) {
    const session = resizeSessionRef.current;
    const gridEl = gridRef.current;
    if (!session || !gridEl) {
      return;
    }

    const dx = event.clientX - session.startX;
    const dy = event.clientY - session.startY;
    const { colWidth, rowHeight, gap } = gridCellMetrics(gridEl, session.startLayout.columns);
    const { deltaW, deltaH } = pointerDeltaToCellDelta(dx, dy, colWidth, rowHeight, gap);
    const startWidget = session.startLayout.widgets.find((entry) => entry.id === widget.id);
    if (!startWidget) {
      return;
    }

    const next = resizeDashboardWidget(
      session.startLayout,
      widget.id,
      deltaW,
      deltaH,
    );
    const nextWidget = next.widgets.find((entry) => entry.id === widget.id);
    const currentWidget = layout.widgets.find((entry) => entry.id === widget.id);
    if (
      nextWidget &&
      currentWidget &&
      (nextWidget.grid.w !== currentWidget.grid.w || nextWidget.grid.h !== currentWidget.grid.h)
    ) {
      onResize(next);
    }
  }

  function handleResizePointerUp(event: React.PointerEvent<HTMLButtonElement>) {
    if (resizeSessionRef.current) {
      resizeSessionRef.current = null;
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
  }

  return (
    <div ref={setNodeRef} style={style} data-widget-id={widget.id}>
      {customizeMode && (
        <button
          type="button"
          className="absolute left-2 top-2 z-10 cursor-grab rounded-sm border border-border-subtle bg-bg-base/80 px-1.5 py-0.5 font-mono text-[10px] text-text-muted active:cursor-grabbing"
          aria-label={`Drag to reorder ${widget.kind}`}
          {...attributes}
          {...listeners}
        >
          ⋮⋮
        </button>
      )}
      {customizeMode && (
        <button
          type="button"
          className="absolute bottom-1 right-1 z-10 h-4 w-4 cursor-se-resize rounded-xs border border-white/20 bg-bg-base/90"
          aria-label="Resize widget"
          onPointerDown={handleResizePointerDown}
          onPointerMove={handleResizePointerMove}
          onPointerUp={handleResizePointerUp}
          onPointerCancel={handleResizePointerUp}
        />
      )}
      {children}
    </div>
  );
}

function StaticWidgetCell({
  widget,
  effectiveCols,
  children,
}: {
  widget: DashboardWidget;
  effectiveCols: number;
  children: React.ReactNode;
}) {
  return (
    <div style={gridCellStyle(widget, effectiveCols)} data-widget-id={widget.id}>
      {children}
    </div>
  );
}

export function persistDashboardLayout(layout: DashboardLayout): void {
  try {
    window.localStorage.setItem(
      SHELL_PREFERENCE_KEYS.dashboardLayout,
      JSON.stringify(layout),
    );
  } catch {
    // ignore quota / private mode
  }
}

export function loadDashboardLayout(fallback: DashboardLayout): DashboardLayout {
  try {
    const raw = window.localStorage.getItem(SHELL_PREFERENCE_KEYS.dashboardLayout);
    if (!raw) {
      return fallback;
    }
    return validateDashboardLayout(JSON.parse(raw));
  } catch {
    return fallback;
  }
}

export function DashboardGrid({
  layout,
  customizeMode,
  onLayoutChange,
  renderWidget,
}: DashboardGridProps) {
  const gridRef = React.useRef<HTMLDivElement>(null);
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 6 } }),
  );

  // Measure the grid container and derive the render-time effective column
  // count so widgets never get narrower than MIN_COL_PX. layout.columns stays
  // the persisted maximum; this is purely a render derivation.
  const [containerWidth, setContainerWidth] = React.useState(0);
  React.useEffect(() => {
    const el = gridRef.current;
    if (!el || typeof ResizeObserver === 'undefined') {
      return;
    }
    setContainerWidth(el.getBoundingClientRect().width);
    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) {
        setContainerWidth(entry.contentRect.width);
      }
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  const effectiveCols = effectiveColumns(containerWidth, layout.columns);

  const widgetIds = layout.widgets.map((w) => w.id);

  function handleDragEnd(event: DragEndEvent) {
    const { active, over } = event;
    if (!over || active.id === over.id) {
      return;
    }
    const next = reorderDashboardWidgets(layout, String(active.id), String(over.id));
    onLayoutChange(next);
    persistDashboardLayout(next);
  }

  function handleResize(next: DashboardLayout) {
    onLayoutChange(next);
    persistDashboardLayout(next);
  }

  const cells = layout.widgets.map((widget) => {
    const content = renderWidget(widget);
    if (customizeMode) {
      return (
        <SortableWidgetCell
          key={widget.id}
          widget={widget}
          customizeMode
          layout={layout}
          effectiveCols={effectiveCols}
          gridRef={gridRef}
          onResize={handleResize}
        >
          {content}
        </SortableWidgetCell>
      );
    }
    return (
      <StaticWidgetCell key={widget.id} widget={widget} effectiveCols={effectiveCols}>
        {content}
      </StaticWidgetCell>
    );
  });

  const grid = (
    <div
      ref={gridRef}
      className="grid gap-5 p-5"
      style={{
        gridTemplateColumns: `repeat(${effectiveCols}, minmax(${MIN_COL_PX}px, 1fr))`,
      }}
      data-customize-mode={customizeMode ? 'true' : 'false'}
    >
      {cells}
    </div>
  );

  if (!customizeMode) {
    return grid;
  }

  return (
    <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
      <SortableContext items={widgetIds} strategy={rectSortingStrategy}>
        {grid}
      </SortableContext>
    </DndContext>
  );
}
