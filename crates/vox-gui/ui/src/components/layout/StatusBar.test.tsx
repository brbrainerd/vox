// @vitest-environment jsdom
import { describe, it, expect, vi, afterEach } from 'vitest';
import React from 'react';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { StatusBar } from './StatusBar';
import { INITIAL_KPIS } from '../../data/initialState';
import { WORKBENCH_TABBAR_TRAILING_SLOT_ID } from '../../lib/domIds';

describe('StatusBar', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('renders with role="status" and aria-label containing Operator status', () => {
    render(
      <StatusBar
        kpis={INITIAL_KPIS}
        lastOrchEventAt={null}
        orchUsesPolling={false}
        liveFreshMs={30_000}
        onNavigate={vi.fn()}
      />,
    );
    const bar = screen.getByRole('status', { name: /operator status/i });
    expect(bar).toBeInTheDocument();
  });

  it('shows orchestrator freshness segment using freshnessTone', () => {
    vi.spyOn(Date, 'now').mockReturnValue(10_000);
    render(
      <StatusBar
        kpis={INITIAL_KPIS}
        lastOrchEventAt={9_500}
        orchUsesPolling={false}
        liveFreshMs={1_000}
        onNavigate={vi.fn()}
      />,
    );
    const freshness = screen.getByTestId('status-bar-freshness');
    expect(freshness).toBeInTheDocument();
    expect(freshness).toHaveTextContent(/live/i);
  });

  it('shows poll freshness when polling without recent events', () => {
    render(
      <StatusBar
        kpis={INITIAL_KPIS}
        lastOrchEventAt={null}
        orchUsesPolling
        liveFreshMs={30_000}
        onNavigate={vi.fn()}
      />,
    );
    const freshness = screen.getByTestId('status-bar-freshness');
    expect(freshness).toHaveTextContent(/poll/i);
  });

  it('does not render achievements trigger when gamifyEnabled is false', () => {
    render(
      <StatusBar
        kpis={INITIAL_KPIS}
        lastOrchEventAt={null}
        orchUsesPolling={false}
        liveFreshMs={30_000}
        onNavigate={vi.fn()}
        gamifyEnabled={false}
        onOpenAchievements={vi.fn()}
      />,
    );
    expect(screen.queryByTestId('achievements-trigger')).not.toBeInTheDocument();
  });

  it('renders achievements trigger when gamifyEnabled is true', async () => {
    const onOpenAchievements = vi.fn();
    const user = userEvent.setup();
    render(
      <StatusBar
        kpis={INITIAL_KPIS}
        lastOrchEventAt={null}
        orchUsesPolling={false}
        liveFreshMs={30_000}
        onNavigate={vi.fn()}
        gamifyEnabled
        onOpenAchievements={onOpenAchievements}
      />,
    );
    const trigger = screen.getByTestId('achievements-trigger');
    expect(trigger).toBeInTheDocument();
    await user.click(trigger);
    expect(onOpenAchievements).toHaveBeenCalledTimes(1);
  });

  it('renders clickable segments for queue, budget, model, and mesh peers', async () => {
    const onNavigate = vi.fn();
    const user = userEvent.setup();
    render(
      <StatusBar
        kpis={INITIAL_KPIS}
        lastOrchEventAt={null}
        orchUsesPolling={false}
        liveFreshMs={30_000}
        onNavigate={onNavigate}
      />,
    );

    await user.click(screen.getByTestId('status-bar-queue'));
    expect(onNavigate).toHaveBeenCalledWith('runs');

    await user.click(screen.getByTestId('status-bar-budget'));
    expect(onNavigate).toHaveBeenCalledWith('settings');

    await user.click(screen.getByTestId('status-bar-model'));
    expect(onNavigate).toHaveBeenCalledWith('models');

    await user.click(screen.getByTestId('status-bar-mesh'));
    expect(onNavigate).toHaveBeenCalledWith('mesh');
  });

  it('mesh segment navigates to the real agents/mesh view, not the Compute section default', async () => {
    const onNavigate = vi.fn();
    const user = userEvent.setup();
    render(
      <StatusBar
        kpis={INITIAL_KPIS}
        lastOrchEventAt={null}
        orchUsesPolling={false}
        liveFreshMs={30_000}
        onNavigate={onNavigate}
      />,
    );

    await user.click(screen.getByTestId('status-bar-mesh'));
    expect(onNavigate).toHaveBeenCalledWith('mesh');
    expect(onNavigate).not.toHaveBeenCalledWith('compute');
  });

  it('renders a fixed trailing slot for tab-row-adjacent chrome (e.g. Chat\'s Panels menu) to portal into, independent of the tab bar', () => {
    render(
      <StatusBar
        kpis={INITIAL_KPIS}
        lastOrchEventAt={null}
        orchUsesPolling={false}
        liveFreshMs={30_000}
        onNavigate={vi.fn()}
      />,
    );
    const slot = screen.getByTestId(WORKBENCH_TABBAR_TRAILING_SLOT_ID);
    expect(slot).toBeInTheDocument();
    expect(slot.id).toBe(WORKBENCH_TABBAR_TRAILING_SLOT_ID);
    // StatusBar is a single, never-wrapping row rendered once in the app
    // shell header — this slot must live here, not inside anything that can
    // wrap to multiple lines (like WorkbenchTabBar's tablist), so content
    // portaled in stays reachable regardless of how many tabs are open.
    const bar = screen.getByRole('status', { name: /operator status/i });
    expect(bar.contains(slot)).toBe(true);
  });
});
