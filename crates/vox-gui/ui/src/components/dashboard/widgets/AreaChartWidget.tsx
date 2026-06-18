import React from 'react';
import {
  ResponsiveContainer,
  AreaChart,
  Area,
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

export function AreaChartWidget({ title, series }: ChartWidgetProps) {
  if (series.length === 0) {
    return <ChartWidgetEmpty title={title} />;
  }

  return (
    <ChartWidgetFrame title={title}>
      <ResponsiveContainer width="100%" height={160}>
        <AreaChart data={series} margin={{ top: 4, right: 8, left: 0, bottom: 0 }}>
          <CartesianGrid stroke={GRID_STROKE} strokeDasharray="3 3" vertical={false} />
          <XAxis dataKey="t" hide />
          <YAxis hide domain={['auto', 'auto']} width={0} />
          <Tooltip {...tooltipStyle} />
          <Area
            type="monotone"
            dataKey="v"
            stroke={BRASS_STROKE}
            fill={BRASS_STROKE}
            fillOpacity={0.2}
            strokeWidth={2}
            dot={false}
          />
        </AreaChart>
      </ResponsiveContainer>
    </ChartWidgetFrame>
  );
}
