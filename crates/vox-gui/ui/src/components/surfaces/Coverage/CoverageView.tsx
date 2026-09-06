import React from 'react';
import type { SurfaceDecoratorProps } from '../decoratorRegistry';
import { SURFACE_REGISTRY, RepresentationTier } from '../../../generated/surfaceRegistry.generated';
import { useLabel } from '../../../hooks/useLanguage';

const TIER_STYLE: Record<RepresentationTier, { label: string; cls: string }> = {
  none:              { label: 'Unrepresented', cls: 'text-text-muted ring-white/10' },
  generic_form:      { label: 'Generic form',  cls: 'text-cyan-300 ring-cyan-400/25' },
  curated_decorator: { label: 'Curated',       cls: 'text-emerald-300 ring-emerald-400/25' },
  live_backend:      { label: 'Live backend',  cls: 'text-brass ring-brass/30' },
};

export function CoverageView(_props: SurfaceDecoratorProps) {
  const rows = [...SURFACE_REGISTRY].sort((a, b) =>
    (a.cliGroup ?? a.viewKey ?? '').localeCompare(b.cliGroup ?? b.viewKey ?? ''));
  const counts = rows.reduce<Record<string, number>>((acc, r) => {
    acc[r.tier] = (acc[r.tier] ?? 0) + 1; return acc;
  }, {});
  return (
    <section className="space-y-4">
      <h2 className="font-display text-lg text-text-primary tracking-wider uppercase">{useLabel('cov-surface')}</h2>
      <div className="flex flex-wrap gap-2 text-[11px]">
        {(Object.keys(TIER_STYLE) as RepresentationTier[]).map(t => (
          <span key={t} className={`rounded-full px-2 py-0.5 ring-1 ${TIER_STYLE[t].cls}`}>
            {TIER_STYLE[t].label}: {counts[t] ?? 0}
          </span>
        ))}
      </div>
      <div className="overflow-auto rounded-lg border border-border-subtle">
        <table className="w-full text-left text-[12px]">
          <caption className="sr-only">Surface representation coverage by CLI group, view, and representation tier</caption>
          <thead className="text-text-muted">
            <tr>
              <th scope="col" className="p-2">CLI group</th>
              <th scope="col" className="p-2">View</th>
              <th scope="col" className="p-2">Tier</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r, i) => (
              <tr key={i} className="border-t border-border-subtle">
                <td className="p-2 font-mono text-text-secondary">{r.cliGroup ?? '—'}</td>
                <td className="p-2 text-text-muted">{r.viewKey ?? '—'}</td>
                <td className="p-2"><span className={`rounded-sm px-1.5 py-0.5 ring-1 ${TIER_STYLE[r.tier].cls}`}>{TIER_STYLE[r.tier].label}</span></td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}
