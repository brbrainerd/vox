// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { LineChartWidget } from './LineChartWidget';

vi.mock('recharts', () => ({
  ResponsiveContainer: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="responsive-container">{children}</div>
  ),
  LineChart: ({ children }: { children: React.ReactNode }) => (
    <svg>{children}</svg>
  ),
  Line: () => null,
  XAxis: () => null,
  YAxis: () => null,
  CartesianGrid: () => null,
  Tooltip: () => null,
}));

describe('LineChartWidget', () => {
  it('renders with role="img" aria-label containing metric name', () => {
    render(
      <LineChartWidget
        title="Budget Burn"
        series={[
          { t: 0, v: 1 },
          { t: 1, v: 2 },
        ]}
      />,
    );
    expect(screen.getByRole('img', { name: /Budget Burn/i })).toBeDefined();
  });

  it('shows empty state when series empty', () => {
    render(<LineChartWidget title="Queue Depth" series={[]} />);
    expect(screen.getByText(/No data yet/i)).toBeDefined();
    expect(screen.queryByRole('img', { name: /Queue Depth/i })).toBeNull();
  });
});
