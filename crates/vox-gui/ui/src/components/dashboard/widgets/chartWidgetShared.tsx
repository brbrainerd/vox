import React from 'react';
import { Glass } from '../../ui/Glass';
import type { MetricPoint } from '../../../hooks/useMetricSeries';

export const BRASS_STROKE = 'rgb(var(--brass))';
export const GRID_STROKE = '#27272a';

export interface ChartWidgetProps {
  title: string;
  series: MetricPoint[];
}

export function ChartWidgetEmpty({ title }: Pick<ChartWidgetProps, 'title'>) {
  return (
    <Glass className="h-full p-5">
      <h3 className="font-display text-[14px] font-semibold tracking-wide text-zinc-100">{title}</h3>
      <div
        role="status"
        className="mt-4 rounded-lg border border-dashed border-white/5 py-8 text-center text-[11px] text-zinc-600"
      >
        No data yet — waiting for metric samples.
      </div>
    </Glass>
  );
}

export function ChartWidgetFrame({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <Glass className="flex h-full flex-col p-5">
      <h3 className="font-display text-[14px] font-semibold tracking-wide text-zinc-100">{title}</h3>
      <div role="img" aria-label={`${title} chart`} className="mt-3 min-h-[160px] flex-1">
        {children}
      </div>
    </Glass>
  );
}

export const tooltipStyle = {
  contentStyle: {
    background: 'rgba(24,24,27,0.95)',
    border: '1px solid rgba(255,255,255,0.08)',
    borderRadius: 8,
    fontSize: 11,
  },
  labelStyle: { color: '#a1a1aa' },
  itemStyle: { color: BRASS_STROKE },
} as const;
