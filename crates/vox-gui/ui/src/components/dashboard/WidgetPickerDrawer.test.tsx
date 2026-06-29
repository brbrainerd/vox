// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import React from 'react';
import { WidgetPickerDrawer } from './WidgetPickerDrawer';
import { defaultDashboardLayout, DASHBOARD_WIDGET_KINDS } from '../../lib/dashboardLayout';
import { DASHBOARD_SECTIONS } from '../../lib/dashboardSections';

describe('WidgetPickerDrawer', () => {
  it('lists widget kinds from the dashboard-layout contract', () => {
    const layout = defaultDashboardLayout();
    const presentKinds = new Set(layout.widgets.map((w) => w.kind));
    const availableKinds = DASHBOARD_WIDGET_KINDS.filter((k) => !presentKinds.has(k));

    render(
      <WidgetPickerDrawer
        layout={layout}
        open
        onClose={vi.fn()}
        onAdd={vi.fn()}
      />,
    );

    for (const kind of availableKinds) {
      expect(screen.getByRole('button', { name: new RegExp(kind.replace(/_/g, '[ _]'), 'i') })).toBeDefined();
    }
  });

  it('clicking a kind calls onAdd with that kind', async () => {
    const user = userEvent.setup();
    const onAdd = vi.fn();
    const layout = defaultDashboardLayout();

    render(
      <WidgetPickerDrawer
        layout={layout}
        open
        onClose={vi.fn()}
        onAdd={onAdd}
      />,
    );

    await user.click(screen.getByRole('button', { name: /line chart/i }));
    expect(onAdd).toHaveBeenCalledWith('line_chart');
  });

  it('does not list kinds already present in the layout', () => {
    render(
      <WidgetPickerDrawer
        layout={defaultDashboardLayout()}
        open
        onClose={vi.fn()}
        onAdd={vi.fn()}
      />,
    );

    expect(screen.queryByRole('button', { name: /^stream$/i })).toBeNull();
    expect(screen.queryByRole('button', { name: /^agents$/i })).toBeNull();
    expect(screen.queryByRole('button', { name: /^alerts$/i })).toBeNull();
  });

  it('lists surface widgets grouped by section, including a new surface', async () => {
    const user = userEvent.setup();
    const onAddSurface = vi.fn();
    render(
      <WidgetPickerDrawer
        layout={{ version: 1, columns: 12, widgets: [] }}
        open
        onClose={() => {}}
        onAdd={() => {}}
        onAddSurface={onAddSurface}
      />,
    );
    // The four dashboard sections exist as a contract.
    expect(DASHBOARD_SECTIONS).toContain('operations');
    // Section headers render for each dashboard section that has offerings.
    expect(screen.getByTestId('picker-section-operations')).toBeTruthy();
    // The synthetic Cost widget is offered.
    expect(screen.getByTestId('picker-surface-cost')).toBeTruthy();
    // Adding a surface widget calls onAddSurface with the surface key.
    await user.click(screen.getByTestId('picker-surface-mesh'));
    expect(onAddSurface).toHaveBeenCalledWith('mesh');
  });
});
