import React from 'react';
import type { DashboardLayout, DashboardWidgetKind } from '../../lib/dashboardLayout';
import { availableWidgetKinds, widgetKindLabel } from '../../lib/dashboardLayout';

export interface WidgetPickerDrawerProps {
  layout: DashboardLayout;
  open: boolean;
  onClose: () => void;
  onAdd: (kind: DashboardWidgetKind) => void;
}

export function WidgetPickerDrawer({ layout, open, onClose, onAdd }: WidgetPickerDrawerProps) {
  if (!open) {
    return null;
  }

  const kinds = availableWidgetKinds(layout);

  return (
    <div
      role="dialog"
      aria-label="Add dashboard widget"
      className="absolute right-5 top-10 z-30 w-64 rounded-lg border border-border-subtle bg-bg-base/95 p-3 shadow-xl backdrop-blur"
    >
      <div className="mb-2 flex items-center justify-between">
        <h3 className="font-display text-[12px] font-semibold tracking-wide text-text-secondary">
          Add widget
        </h3>
        <button
          type="button"
          aria-label="Close widget picker"
          onClick={onClose}
          className="rounded px-1.5 py-0.5 text-[11px] text-text-muted hover:bg-overlay-subtle hover:text-text-secondary"
        >
          ✕
        </button>
      </div>
      {kinds.length === 0 ? (
        <p className="py-2 text-[11px] text-text-muted">All widget types are already on the dashboard.</p>
      ) : (
        <ul className="flex max-h-72 flex-col gap-1 overflow-y-auto">
          {kinds.map((kind) => (
            <li key={kind}>
              <button
                type="button"
                onClick={() => onAdd(kind)}
                className="w-full rounded-md border border-border-subtle px-2.5 py-1.5 text-left text-[11px] text-text-secondary hover:border-border-subtle hover:bg-overlay-subtle"
              >
                {widgetKindLabel(kind)}
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
