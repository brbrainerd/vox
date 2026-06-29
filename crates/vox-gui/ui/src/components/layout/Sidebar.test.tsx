// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue({ display_name: 'operator@vox' }),
}));

vi.mock('../../generated/surfaceRegistry.generated', () => ({
  SURFACE_REGISTRY: [
    { viewKey: 'dashboard', navLabel: 'Dashboard', parentSurface: 'agents', tier: 'surface' },
    { viewKey: 'flow', navLabel: 'Flow', parentSurface: 'agents', tier: 'surface' },
    { viewKey: 'memory', navLabel: 'Memory', parentSurface: 'search', tier: 'surface' },
    { viewKey: 'settings', navLabel: 'Settings', parentSurface: null, tier: 'surface' },
    { viewKey: 'coverage', navLabel: 'Coverage', parentSurface: 'settings', tier: 'surface' },
  ],
}));

import { Sidebar } from './Sidebar';
import { LanguageProvider } from '../../hooks/useLanguage';

const baseProps = {
  view: 'dashboard',
  setView: vi.fn(),
  agentsCount: 2,
  data: {
    agents: [],
    stream: [],
    alerts: [],
    skills: [],
    peers: [],
    kpis: {} as any,
    contextChips: [],
  },
  mode: 'default' as const,
  setMode: vi.fn(),
  pushToast: vi.fn(),
  appVersion: '0.6.0',
};

function renderSidebar(extraProps: Partial<typeof baseProps> = {}) {
  return render(
    <LanguageProvider>
      <Sidebar {...baseProps} {...extraProps} />
    </LanguageProvider>,
  );
}

describe('Sidebar badges', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    Element.prototype.scrollIntoView = vi.fn();
    window.localStorage.clear();
  });

  it('includes pending count in Runs nav aria-label', () => {
    renderSidebar({ approvalsPending: 3 });
    expect(screen.getByRole('button', { name: /Runs.*3 pending/i })).toBeDefined();
  });

  it('uses default Runs aria-label when nothing is pending', () => {
    renderSidebar({ approvalsPending: 0 });
    expect(screen.getByRole('button', { name: 'Runs and Approvals' })).toBeDefined();
  });

  it('includes failing count in Settings nav aria-label', () => {
    renderSidebar({ policyBadge: { count: 2, status: 'fail' } });
    expect(screen.getByRole('button', { name: /Settings.*2 policy failures/i })).toBeDefined();
  });

  it('uses default Settings aria-label when nothing is failing', () => {
    renderSidebar({ policyBadge: { count: 0, status: 'pass' } });
    expect(screen.getByRole('button', { name: 'Settings' })).toBeDefined();
  });

  it('exposes Coverage shortcut with CI surface gaps aria-label', () => {
    renderSidebar();
    expect(screen.getByRole('button', { name: /Coverage.*CI surface gaps/i })).toBeDefined();
  });
});

describe('Sidebar has no nav filter', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    Element.prototype.scrollIntoView = vi.fn();
    window.localStorage.clear();
  });

  it('renders no nav filter input or toggle', () => {
    renderSidebar();
    expect(screen.queryByTestId('sidebar-nav-filter')).toBeNull();
    expect(screen.queryByRole('button', { name: /filter navigation/i })).toBeNull();
  });

  it('still renders top-level nav items', () => {
    renderSidebar();
    expect(screen.getByRole('button', { name: 'Chat' })).toBeDefined();
    expect(screen.getByRole('button', { name: /Agents/ })).toBeDefined();
  });
});

describe('Sidebar orchestrator freshness dot', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    Element.prototype.scrollIntoView = vi.fn();
    window.localStorage.clear();
  });

  afterEach(() => {
    vi.spyOn(Date, 'now').mockRestore();
  });

  it('uses live styling when orchestrator events are fresh', () => {
    vi.spyOn(Date, 'now').mockReturnValue(10_000);
    renderSidebar({ lastOrchEventAt: 9_500, orchUsesPolling: false, liveFreshMs: 1_000 });
    const dot = screen.getByTestId('sidebar-orch-freshness-dot');
    expect(dot.className).toMatch(/bg-accent-secondary/);
    expect(dot.className).not.toMatch(/bg-text-muted/);
  });

  it('uses stale styling when orchestrator events are stale', () => {
    vi.spyOn(Date, 'now').mockReturnValue(10_000);
    renderSidebar({ lastOrchEventAt: 5_000, orchUsesPolling: false, liveFreshMs: 1_000 });
    const dot = screen.getByTestId('sidebar-orch-freshness-dot');
    expect(dot.className).toMatch(/bg-text-muted/);
    expect(dot.className).not.toMatch(/bg-accent-secondary/);
  });
});

describe('Sidebar language labels', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    Element.prototype.scrollIntoView = vi.fn();
    window.localStorage.clear();
  });

  it('shows English nav label by default', () => {
    renderSidebar();
    expect(screen.getByRole('button', { name: 'Market' })).toBeDefined();
  });

  it('shows Latin nav label when vox.lang=la', () => {
    window.localStorage.setItem('vox.lang', 'la');
    renderSidebar();
    expect(screen.getByRole('button', { name: 'Mercatus' })).toBeDefined();
  });
});
