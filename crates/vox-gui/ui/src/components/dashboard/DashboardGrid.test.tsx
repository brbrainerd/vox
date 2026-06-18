// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { defaultDashboardLayout } from '../../lib/dashboardLayout';
import { DashboardGrid } from './DashboardGrid';

describe('DashboardGrid', () => {
  it('renders widgets from layout in DOM order', () => {
    const layout = defaultDashboardLayout();
    render(
      <DashboardGrid
        layout={layout}
        customizeMode={false}
        onLayoutChange={vi.fn()}
        renderWidget={(widget) => (
          <div data-testid={`widget-${widget.id}`}>{widget.kind}</div>
        )}
      />,
    );

    const nodes = screen.getAllByTestId(/^widget-/);
    expect(nodes.map((n) => n.getAttribute('data-testid'))).toEqual([
      'widget-stream',
      'widget-alerts',
      'widget-agents',
    ]);
  });

  it('shows drag handles when customizeMode is true', () => {
    const layout = defaultDashboardLayout();
    render(
      <DashboardGrid
        layout={layout}
        customizeMode
        onLayoutChange={vi.fn()}
        renderWidget={(widget) => <div>{widget.kind}</div>}
      />,
    );

    expect(screen.getAllByRole('button', { name: /drag to reorder/i }).length).toBe(
      layout.widgets.length,
    );
  });

  it('shows resize handles when customizeMode is true', () => {
    const layout = defaultDashboardLayout();
    render(
      <DashboardGrid
        layout={layout}
        customizeMode
        onLayoutChange={vi.fn()}
        renderWidget={(widget) => <div>{widget.kind}</div>}
      />,
    );

    expect(screen.getAllByRole('button', { name: /^resize widget$/i }).length).toBe(
      layout.widgets.length,
    );
  });

  it('does not show resize handles when customizeMode is false', () => {
    const layout = defaultDashboardLayout();
    render(
      <DashboardGrid
        layout={layout}
        customizeMode={false}
        onLayoutChange={vi.fn()}
        renderWidget={(widget) => <div>{widget.kind}</div>}
      />,
    );

    expect(screen.queryByRole('button', { name: /^resize widget$/i })).toBeNull();
  });
});
