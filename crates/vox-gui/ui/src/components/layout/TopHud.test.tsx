// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import React from 'react';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { TopHud } from './TopHud';
import { INITIAL_KPIS } from '../../data/initialState';

vi.mock('../ui/Glass', () => ({
  Glass: ({ children, className }: { children: React.ReactNode; className?: string }) => (
    <div data-testid="glass" className={className}>
      {children}
    </div>
  ),
}));

vi.mock('../ui/Icons', () => ({
  Icon: {
    users: () => <span data-testid="icon-users" />,
    scale: () => <span data-testid="icon-scale" />,
    bolt: () => <span data-testid="icon-bolt" />,
    link: () => <span data-testid="icon-link" />,
    search: () => <span data-testid="icon-search" />,
    cpu: () => <span data-testid="icon-cpu" />,
    globe: () => <span data-testid="icon-globe" />,
    shield: () => <span data-testid="icon-shield" />,
  },
}));

vi.mock('../ui/Sparkline', () => ({
  Sparkline: () => <span data-testid="sparkline" />,
}));

const baseProps = {
  kpis: INITIAL_KPIS,
  onCommand: vi.fn(),
  lastOrchEventAt: null,
  orchUsesPolling: false,
  liveFreshMs: 30_000,
  onNavigate: vi.fn(),
  hudMode: 'full' as const,
  setHudMode: vi.fn(),
};

describe('TopHud omnisearch trigger', () => {
  it('renders faux-search with Search or jump text and ⌘K hint', () => {
    render(<TopHud {...baseProps} onOpenCommandPalette={vi.fn()} />);
    const trigger = screen.getByTestId('omnisearch-trigger');
    expect(trigger.textContent).toMatch(/Search or jump/i);
    expect(trigger.textContent).toMatch(/⌘K/);
  });

  it('click calls onOpenCommandPalette', async () => {
    const user = userEvent.setup();
    const onOpenCommandPalette = vi.fn();
    render(<TopHud {...baseProps} onOpenCommandPalette={onOpenCommandPalette} />);
    await user.click(screen.getByTestId('omnisearch-trigger'));
    expect(onOpenCommandPalette).toHaveBeenCalledTimes(1);
  });
});

describe('TopHud branding', () => {
  it('does not render IMPERIUM and shows default Operator title', () => {
    render(<TopHud {...baseProps} />);
    expect(screen.queryByText('IMPERIUM')).not.toBeInTheDocument();
    expect(screen.getByText('Operator')).toBeInTheDocument();
    expect(screen.getByText('axis operator console')).toBeInTheDocument();
  });

  it('shows the AxisMark brand glyph, not a "V" letterform', () => {
    const { container } = render(<TopHud {...baseProps} />);
    expect(container.querySelector('svg[aria-label="Axis"]')).toBeTruthy();
    // the old standalone "V" glyph (a div whose only text is "V") is gone
    const vGlyph = Array.from(container.querySelectorAll('div')).find(d => d.textContent === 'V');
    expect(vGlyph).toBeUndefined();
  });

  it('shows custom workspaceTitle when provided', () => {
    render(<TopHud {...baseProps} workspaceTitle="Acme Workspace" />);
    expect(screen.getByText('Acme Workspace')).toBeInTheDocument();
    expect(screen.queryByText('Operator')).not.toBeInTheDocument();
  });
});

describe('TopHud pending approvals tile', () => {
  it('renders the pending-approvals tile when visible', () => {
    render(
      <TopHud
        {...baseProps}
        visibleTiles={['pending_approvals']}
        pendingApprovals={3}
      />,
    );
    expect(screen.getByText('Pending Approvals')).toBeTruthy();
    expect(screen.getByText('3')).toBeTruthy();
  });
});
