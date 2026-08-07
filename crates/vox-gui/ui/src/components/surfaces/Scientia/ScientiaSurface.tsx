import React, { useState } from 'react';
import { ScientiaDashboard } from './ScientiaDashboard';
import { ClaimsView } from './ClaimsView';
import { HarnessIssuesPanel } from './HarnessIssuesPanel';
import type { SurfaceDecoratorProps } from '../decoratorRegistry';

const TABS = [
  { id: 'dashboard', label: 'Dashboard' },
  { id: 'claims', label: 'Claims' },
  { id: 'harness', label: 'Harness Issues' },
] as const;

type ScientiaTab = typeof TABS[number]['id'];

/**
 * Findings surface (view key `scientia`). Absorbs the former `claims` surface
 * as a tab — both shared the identical 12-command set and Scientia component
 * dir (gui-ia-blueprint §4 MERGE: claims + knowledge-surface → scientia).
 */
export function ScientiaSurface(props: SurfaceDecoratorProps) {
  const [tab, setTab] = useState<ScientiaTab>('dashboard');
  return (
    <div className="flex min-h-0 flex-col">
      <div role="tablist" aria-label="Findings sections" className="flex gap-1 px-4 pt-3">
        {TABS.map(t => (
          <button
            key={t.id}
            type="button"
            role="tab"
            aria-selected={tab === t.id}
            onClick={() => setTab(t.id)}
            className={`rounded-md px-3 py-1.5 font-display text-[11px] uppercase tracking-[0.16em] transition ${
              tab === t.id
                ? 'bg-overlay-subtle text-brass'
                : 'text-text-muted hover:bg-overlay-hover hover:text-text-secondary'
            }`}
          >
            {t.label}
          </button>
        ))}
      </div>
      {tab === 'dashboard' ? (
        <ScientiaDashboard {...props} />
      ) : tab === 'claims' ? (
        <ClaimsView {...props} />
      ) : (
        <HarnessIssuesPanel {...props} />
      )}
    </div>
  );
}
