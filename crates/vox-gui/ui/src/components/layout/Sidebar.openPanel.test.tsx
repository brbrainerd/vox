// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { Sidebar } from './Sidebar';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn().mockResolvedValue({ display_name: 'x@vox', os_user: 'x' }) }));

const base = {
  view: 'dashboard', setView: vi.fn(), agentsCount: 0,
  data: { agents: [], stream: [], alerts: [], peers: [], skills: [], kpis: {} } as any,
  mode: 'default' as const, setMode: vi.fn(), pushToast: vi.fn(),
};

describe('Sidebar open-in-panel', () => {
  beforeEach(() => {
    Element.prototype.scrollIntoView = vi.fn();
  });

  it('middle-click on a nav item opens it as a panel instead of navigating', () => {
    const onOpenPanel = vi.fn();
    render(<Sidebar {...base} onOpenPanel={onOpenPanel} />);
    const agents = screen.getByRole('button', { name: /^agents/i });
    fireEvent.mouseDown(agents, { button: 1 }); // middle button
    expect(onOpenPanel).toHaveBeenCalledWith('agents');
    expect(base.setView).not.toHaveBeenCalled();
  });
});
