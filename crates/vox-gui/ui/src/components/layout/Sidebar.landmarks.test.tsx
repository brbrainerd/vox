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
  mode: 'default' as const,
  setMode: vi.fn(),
  pushToast: vi.fn(),
  appVersion: '0.6.0',
} as React.ComponentProps<typeof Sidebar>;

function renderSidebar(extraProps: Partial<React.ComponentProps<typeof Sidebar>> = {}) {
  return render(<LanguageProvider><Sidebar {...baseProps} {...extraProps} /></LanguageProvider>);
}

describe('Axis sidebar — landmark uniqueness', () => {
  it('sidebar nav and aside landmarks carry aria-labels (axe landmark-unique)', () => {
    renderSidebar();
    expect(screen.getByRole('navigation')).toHaveAttribute('aria-label');
    expect(screen.getByRole('complementary')).toHaveAttribute('aria-label');
  });
});
