// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import React from 'react';
import { render, screen } from '@testing-library/react';
import { AppShell } from './AppShell';
import { INITIAL_KPIS } from '../../data/initialState';
import type { DashboardData } from '../../types/dashboard';
import { INITIAL_DATA } from '../../data/initialState';

vi.mock('./Sidebar', () => ({
  Sidebar: () => <nav data-testid="sidebar" aria-label="Primary" />,
  SidebarMode: {},
}));

vi.mock('./TopHud', () => ({
  TopHud: () => <header data-testid="top-hud" />,
}));

vi.mock('./BreadcrumbBar', () => ({
  BreadcrumbBar: () => <div data-testid="breadcrumb" />,
}));

vi.mock('./StatusBar', () => ({
  StatusBar: () => <div data-testid="status-bar" role="status" aria-label="Operator status" />,
}));

vi.mock('./DockShell', () => ({
  DockShell: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="dock-shell">{children}</div>
  ),
}));

vi.mock('../ui/Backdrop', () => ({
  Backdrop: () => null,
}));

vi.mock('../ui/ErrorBoundary', () => ({
  SurfaceErrorBoundary: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

const baseProps = {
  activeView: 'dashboard',
  onNavigate: vi.fn(),
  sidebarMode: 'default' as const,
  setSidebarMode: vi.fn(),
  agentsCount: 0,
  data: INITIAL_DATA as DashboardData,
  pushToast: vi.fn(),
  appVersion: '0.6.0',
  policyBadge: { count: 0, status: 'not_run' as const },
  approvalsPending: 0,
  kpis: INITIAL_KPIS,
  onCommand: vi.fn(),
  lastOrchEventAt: null,
  orchUsesPolling: false,
  liveFreshMs: 30_000,
  hudMode: 'full' as const,
  setHudMode: vi.fn(),
  surfaceKey: 'dashboard',
  surfaceLabel: 'Dashboard',
};

describe('AppShell', () => {
  it('renders sidebar, hud, status bar, and main children', () => {
    render(
      <AppShell {...baseProps} chatDocked={false}>
        <div data-testid="main-surface">surface</div>
      </AppShell>,
    );
    expect(screen.getByTestId('sidebar')).toBeInTheDocument();
    expect(screen.getByTestId('top-hud')).toBeInTheDocument();
    expect(screen.getByTestId('status-bar')).toBeInTheDocument();
    expect(screen.getByTestId('main-surface')).toBeInTheDocument();
  });

  it('does not render chat dock when chatDocked is false', () => {
    render(
      <AppShell {...baseProps} chatDocked={false}>
        <div>surface</div>
      </AppShell>,
    );
    expect(screen.queryByTestId('loquela-dock')).not.toBeInTheDocument();
  });

  it('renders chat dock slot when chatDocked is true', () => {
    render(
      <AppShell
        {...baseProps}
        chatDocked
        chatDock={<div data-testid="loquela-dock">chat</div>}
      >
        <div>surface</div>
      </AppShell>,
    );
    expect(screen.getAllByTestId('loquela-dock').length).toBeGreaterThan(0);
  });
});
