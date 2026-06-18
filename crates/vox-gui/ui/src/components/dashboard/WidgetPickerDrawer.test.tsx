// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import React from 'react';
import { WidgetPickerDrawer } from './WidgetPickerDrawer';
import { defaultDashboardLayout, DASHBOARD_WIDGET_KINDS } from '../../lib/dashboardLayout';

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
});
