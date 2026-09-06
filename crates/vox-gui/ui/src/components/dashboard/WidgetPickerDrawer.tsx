import React from 'react';
import type { DashboardLayout, DashboardWidgetKind } from '../../lib/dashboardLayout';
import { availableWidgetKinds, widgetKindLabel } from '../../lib/dashboardLayout';
import { DASHBOARD_SECTIONS, surfacesForSection, type DashboardSection } from '../../lib/dashboardSections';
import { labelFor, currentLang } from '../../lib/lexicon';

export interface WidgetPickerDrawerProps {
  layout: DashboardLayout;
  open: boolean;
  onClose: () => void;
  onAdd: (kind: DashboardWidgetKind) => void;
  /** Add a surface-backed widget (surface_widget slot with this surfaceKey). */
  onAddSurface?: (surfaceKey: string) => void;
}

const SECTION_LABELS: Record<DashboardSection, string> = {
  operations: 'Operations',
  cost: 'Cost',
  knowledge: 'Knowledge',
  surfaces: 'Surfaces',
};

/** Surfaces offered per section, with the synthetic Cost monitorable injected. */
function surfaceOfferings(section: DashboardSection): Array<{ key: string; label: string }> {
  if (section === 'cost') return [{ key: 'cost', label: 'OpenRouter Spend' }];
  return surfacesForSection(section).map((r) => ({ key: r.viewKey as string, label: labelFor(r.viewKey as string, currentLang()) }));
}

export function WidgetPickerDrawer({ layout, open, onClose, onAdd, onAddSurface }: WidgetPickerDrawerProps) {
  if (!open) {
    return null;
  }

  const kinds = availableWidgetKinds(layout);

  return (
    <div
      role="dialog"
      aria-label="Add dashboard widget"
      className="absolute right-5 top-10 z-30 w-72 rounded-lg border border-border-subtle bg-bg-base/95 p-3 shadow-xl backdrop-blur-sm"
    >
      <div className="mb-2 flex items-center justify-between">
        <h3 className="font-display text-[12px] font-semibold tracking-wide text-text-secondary">
          Add widget
        </h3>
        <button
          type="button"
          aria-label="Close widget picker"
          onClick={onClose}
          className="rounded-sm px-1.5 py-0.5 text-[11px] text-text-muted hover:bg-overlay-subtle hover:text-text-secondary"
        >
          ✕
        </button>
      </div>

      <div className="max-h-80 overflow-y-auto">
        {onAddSurface &&
          DASHBOARD_SECTIONS.map((section) => {
            const offerings = surfaceOfferings(section);
            if (offerings.length === 0) return null;
            return (
              <div key={section} data-testid={`picker-section-${section}`} className="mb-3">
                <div className="mb-1 border-b border-border-subtle pb-1 font-display text-[9px] uppercase tracking-[0.24em] text-text-muted">
                  {SECTION_LABELS[section]}
                </div>
                <ul className="flex flex-col gap-1">
                  {offerings.map((o) => (
                    <li key={o.key}>
                      <button
                        type="button"
                        data-testid={`picker-surface-${o.key}`}
                        onClick={() => onAddSurface(o.key)}
                        className="w-full rounded-md border border-border-subtle px-2.5 py-1.5 text-left text-[11px] text-text-secondary hover:bg-overlay-subtle"
                      >
                        {o.label}
                      </button>
                    </li>
                  ))}
                </ul>
              </div>
            );
          })}

        {kinds.length > 0 ? (
          <div className="mb-1">
            <div className="mb-1 border-b border-border-subtle pb-1 font-display text-[9px] uppercase tracking-[0.24em] text-text-muted">
              Charts &amp; legacy
            </div>
            <ul className="flex flex-col gap-1">
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
          </div>
        ) : !onAddSurface ? (
          <p className="py-2 text-[11px] text-text-muted">All widget types are already on the dashboard.</p>
        ) : null}
      </div>
    </div>
  );
}
