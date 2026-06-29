// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { LanguageProvider } from '../../hooks/useLanguage';

vi.mock('../../generated/surfaceRegistry.generated', () => ({
  SURFACE_REGISTRY: [
    { viewKey: 'mercatus', navLabel: 'Mercatus', parentSurface: 'operate', tier: 'live_backend' },
    { viewKey: 'activity', navLabel: 'Activity', parentSurface: 'operate', tier: 'live_backend' },
  ],
}));

import { ParentSurface } from './ParentSurface';

function renderIt() {
  return render(
    <LanguageProvider>
      <ParentSurface parentKey="operate" activeChild="mercatus" onChildChange={vi.fn()} renderChild={() => <div />} />
    </LanguageProvider>,
  );
}

describe('ParentSurface sub-tab labels', () => {
  beforeEach(() => window.localStorage.clear());
  it('shows English by default', () => {
    renderIt();
    expect(screen.getByText('Market')).toBeInTheDocument();
  });
  it('shows Latin when vox.lang=la', () => {
    window.localStorage.setItem('vox.lang', 'la');
    renderIt();
    expect(screen.getByText('Mercatus')).toBeInTheDocument();
  });
});
