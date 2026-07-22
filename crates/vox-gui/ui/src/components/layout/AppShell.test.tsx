// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import React from 'react';
import { render, screen } from '@testing-library/react';
import { AppShell } from './AppShell';
import { INITIAL_KPIS } from '../../data/initialState';
import type { DashboardData } from '../../types/dashboard';
import { INITIAL_DATA } from '../../data/initialState';
import { defaultHudTiles } from '../../hooks/useHudTiles';

vi.mock('./Sidebar', () => ({
  Sidebar: () => <nav data-testid="sidebar" aria-label="Primary" />,
  SidebarMode: {},
}));

vi.mock('./BreadcrumbBar', () => ({
  BreadcrumbBar: () => <div data-testid="breadcrumb" />,
}));

vi.mock('./BottomStatusBar', () => ({
  BottomStatusBar: () => <div data-testid="bottom-status-bar" role="status" aria-label="Operator status" />,
}));

vi.mock('./SurfaceScrollHost', () => ({
  SurfaceScrollHost: ({ children }: { children: React.ReactNode }) => (
    <div data-testid="surface-scroll-host">{children}</div>
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
  onOpenParent: vi.fn(),
  onOpenTab: vi.fn(),
  sidebarMode: 'default' as const,
  setSidebarMode: vi.fn(),
  agentsCount: 0,
  data: INITIAL_DATA as DashboardData,
  pushToast: vi.fn(),
  appVersion: '0.6.0',
  policyBadge: { count: 0, status: 'not_run' as const },
  needsYouCount: 0,
  pendingApprovals: 0,
  kpis: INITIAL_KPIS,
  onOpenCommandPalette: vi.fn(),
  lastOrchEventAt: null,
  orchUsesPolling: false,
  liveFreshMs: 30_000,
  surfaceKey: 'dashboard',
  surfaceLabel: 'Dashboard',
  hudTilesConfig: defaultHudTiles(),
  onHudTilesChange: vi.fn(),
  meshNodes: undefined,
};

describe('AppShell', () => {
  it('renders sidebar, breadcrumb, status bar, and main children', () => {
    render(
      <AppShell {...baseProps} chatDocked={false}>
        <div data-testid="main-surface">surface</div>
      </AppShell>,
    );
    expect(screen.getByTestId('sidebar')).toBeInTheDocument();
    expect(screen.getByTestId('breadcrumb')).toBeInTheDocument();
    expect(screen.getByTestId('bottom-status-bar')).toBeInTheDocument();
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
