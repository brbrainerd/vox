// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';

beforeEach(() => {
  // jsdom doesn't implement scrollIntoView; Sidebar calls it on the active nav ref.
  Element.prototype.scrollIntoView = vi.fn();
});

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue({ display_name: 'operator@vox' }),
}));
vi.mock('../../generated/surfaceRegistry.generated', () => ({
  SURFACE_REGISTRY: [
    { viewKey: 'dashboard', navLabel: 'Dashboard', parentSurface: 'agents', tier: 'surface' },
    { viewKey: 'settings', navLabel: 'Settings', parentSurface: null, tier: 'surface' },
  ],
}));

import { Sidebar } from './Sidebar';
import { LanguageProvider } from '../../hooks/useLanguage';

const baseProps = {
  view: 'dashboard',
  onOpenParent: vi.fn(),
  onOpenTab: vi.fn(),
  agentsCount: 2,
  data: { agents: [], stream: [], alerts: [], skills: [], peers: [], kpis: {} as any, contextChips: [] },
  mode: 'default' as const, // NOT 'rail' — the wide lockup
  setMode: vi.fn(),
  pushToast: vi.fn(),
  appVersion: '0.6.0',
} as React.ComponentProps<typeof Sidebar>;

function renderSidebar(extraProps: Partial<React.ComponentProps<typeof Sidebar>> = {}) {
  return render(<LanguageProvider><Sidebar {...baseProps} {...extraProps} /></LanguageProvider>);
}

describe('Axis branding — sidebar', () => {
  it('renders the AxisMark glyph + AXIS wordmark, no VOX/V letterform', () => {
    const { container } = renderSidebar();
    expect(container.querySelector('svg[aria-label="Axis"]')).toBeTruthy();
    expect(screen.getByText('AXIS')).toBeTruthy();
    expect(screen.queryByText('VOX')).toBeNull();
  });

  it('footer spells out the Vox Axis full brand', () => {
    renderSidebar();
    expect(screen.getByText(/Vox Axis/)).toBeTruthy();
  });

  it('keeps the mark visible in rail (collapsed) mode', () => {
    const { container } = renderSidebar({ mode: 'rail' as const });
    expect(container.querySelector('svg[aria-label="Axis"]')).toBeTruthy();
  });
});
