// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

// Mock Tauri invoke — SettingsView fires multiple invoke() calls on mount.
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue(null),
}));

// Mock Tauri event listen — transport.ts calls listen() on import.
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

// Mock voxTransport — SettingsView calls getRoutingSummaryLive on mount.
vi.mock('../../../transport', () => ({
  voxTransport: {
    getRoutingSummaryLive: vi.fn().mockResolvedValue({ routing_priority: null }),
    setRoutingPriority: vi.fn().mockResolvedValue(undefined),
  },
}));

// Mock PriorityChainEditor — it makes its own invoke calls.
vi.mock('./PriorityChainEditor', () => ({
  PriorityChainEditor: () => <div data-testid="priority-chain-editor" />,
}));

import { SettingsView } from './SettingsView';

function wrapper({ children }: { children: React.ReactNode }) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return <QueryClientProvider client={qc}>{children}</QueryClientProvider>;
}

describe('SettingsView', () => {
  it('renders without crashing', () => {
    expect(() =>
      render(<SettingsView pushToast={vi.fn()} />, { wrapper })
    ).not.toThrow();
  });

  it('all buttons have type=button', () => {
    render(<SettingsView pushToast={vi.fn()} />, { wrapper });
    const buttons = screen.getAllByRole('button');
    buttons.forEach(btn => {
      expect(btn.getAttribute('type')).toBe('button');
    });
  });

  it('search input is accessible via aria-label', () => {
    render(<SettingsView pushToast={vi.fn()} />, { wrapper });
    const searchInput = screen.getByLabelText('Search settings');
    expect(searchInput).toBeDefined();
  });

  it('renders section nav items for all 12 settings sections', () => {
    render(<SettingsView pushToast={vi.fn()} />, { wrapper });
    // The nav renders one button per section label (12 sections defined in SECTIONS).
    const navButtons = screen.getAllByRole('button');
    // At least the 12 nav section buttons should be present.
    expect(navButtons.length).toBeGreaterThanOrEqual(12);
    // Spot-check a few known section labels (use getAllByText since headings also appear).
    expect(screen.getAllByText('Orchestrator').length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText('Theme').length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText('Keybinds').length).toBeGreaterThanOrEqual(1);
  });
});
