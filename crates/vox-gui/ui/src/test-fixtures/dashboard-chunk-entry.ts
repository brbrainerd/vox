/**
 * Chart/grid vendor entry for dashboard bundle budget measurement.
 * Mirrors dnd-kit + split recharts imports used by dashboard widgets.
 */
import { createElement } from 'react';
import { DndContext } from '@dnd-kit/core';
import { SortableContext, rectSortingStrategy } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import {
  ResponsiveContainer,
  LineChart,
  BarChart,
  AreaChart,
  Line,
  Bar,
  Area,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
} from 'recharts';

const series = [{ t: 0, v: 1 }];
const gridProps = { strokeDasharray: '3 3' } as const;
const axisProps = { dataKey: 't', hide: true } as const;
const yProps = { hide: true, domain: ['auto', 'auto'] as const, width: 0 };

function chartForType(chartType: 'line' | 'bar' | 'area') {
  const Chart = chartType === 'bar' ? BarChart : chartType === 'area' ? AreaChart : LineChart;

  return createElement(ResponsiveContainer, {
    width: '100%',
    height: 160,
    children: createElement(
      Chart,
      { data: series },
      createElement(CartesianGrid, gridProps),
      createElement(XAxis, axisProps),
      createElement(YAxis, yProps),
      createElement(Tooltip, {}),
      chartType === 'bar'
        ? createElement(Bar, { dataKey: 'v' })
        : chartType === 'area'
          ? createElement(Area, { dataKey: 'v' })
          : createElement(Line, { dataKey: 'v' }),
    ),
  });
}

export default createElement(DndContext, {
  children: createElement(SortableContext, {
    items: ['line', 'bar', 'area'],
    strategy: rectSortingStrategy,
    children: createElement(
      'div',
      { style: { transform: CSS.Transform.toString({ x: 0, y: 0, scaleX: 1, scaleY: 1 }) } },
      chartForType('line'),
      chartForType('bar'),
      chartForType('area'),
    ),
  }),
});
