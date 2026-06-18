import React from 'react';
import {
  ResponsiveContainer,
  BarChart,
  Bar,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
} from 'recharts';
import {
  BRASS_STROKE,
  GRID_STROKE,
  ChartWidgetEmpty,
  ChartWidgetFrame,
  tooltipStyle,
  type ChartWidgetProps,
} from './chartWidgetShared';

export function BarChartWidget({ title, series }: ChartWidgetProps) {
  if (series.length === 0) {
    return <ChartWidgetEmpty title={title} />;
  }

  return (
    <ChartWidgetFrame title={title}>
      <ResponsiveContainer width="100%" height={160}>
        <BarChart data={series} margin={{ top: 4, right: 8, left: 0, bottom: 0 }}>
          <CartesianGrid stroke={GRID_STROKE} strokeDasharray="3 3" vertical={false} />
          <XAxis dataKey="t" hide />
          <YAxis hide domain={['auto', 'auto']} width={0} />
          <Tooltip {...tooltipStyle} />
          <Bar dataKey="v" fill={BRASS_STROKE} fillOpacity={0.65} radius={[2, 2, 0, 0]} />
        </BarChart>
      </ResponsiveContainer>
    </ChartWidgetFrame>
  );
}
