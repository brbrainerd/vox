// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { LanguageProvider } from '../../hooks/useLanguage';

vi.mock('../../generated/surfaceRegistry.generated', () => ({
  SURFACE_REGISTRY: [
    { viewKey: 'mercatus', navLabel: 'Mercatus', parentSurface: 'operate', tier: 'live_backend' },
    { viewKey: 'activity', navLabel: 'Activity', parentSurface: 'operate', tier: 'live_backend' },
    { viewKey: 'browser', navLabel: 'Browser', parentSurface: 'workspace', tier: 'live_backend' },
    { viewKey: 'console', navLabel: 'Console', parentSurface: 'workspace', tier: 'live_backend' },
    { viewKey: 'harness', navLabel: 'Harness', parentSurface: 'workspace', tier: 'live_backend' },
    { viewKey: 'repository', navLabel: 'Repository', parentSurface: 'workspace', tier: 'live_backend' },
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

describe('ParentSurface sub-tab ordering', () => {
  beforeEach(() => window.localStorage.clear());
  it('renders workspace tabs in intent order (console first), not registry order', () => {
    render(
      <LanguageProvider>
        <ParentSurface parentKey="workspace" activeChild="console" onChildChange={vi.fn()} renderChild={() => <div />} />
      </LanguageProvider>,
    );
    const tabs = screen.getAllByRole('button').map(t => t.textContent);
    expect(tabs).toEqual(['Console', 'Repository', 'Browser', 'Harness']);
  });
});
