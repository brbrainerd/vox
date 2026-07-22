// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { BottomStatusBar } from './BottomStatusBar';
import { defaultHudTiles } from '../../hooks/useHudTiles';
import { INITIAL_KPIS } from '../../data/initialState';
import { WORKBENCH_TABBAR_TRAILING_SLOT_ID } from '../../lib/domIds';

describe('BottomStatusBar', () => {
  it('renders every enabled tile as a compact one-line segment', () => {
    render(
      <BottomStatusBar
        kpis={INITIAL_KPIS}
        hudTilesConfig={defaultHudTiles()}
        onNavigate={vi.fn()}
        lastOrchEventAt={null}
        orchUsesPolling={false}
        liveFreshMs={10_000}
      />,
    );
    expect(screen.getByTestId('bottom-status-bar')).toBeInTheDocument();
    expect(screen.getByText('Agents')).toBeInTheDocument();
    expect(screen.getByText('Mesh')).toBeInTheDocument();
  });

  it('a disabled tile in hudTilesConfig does not render', () => {
    const config = defaultHudTiles();
    config.tiles = config.tiles.map((t) =>
      t.kind === 'mesh_peers' ? { ...t, enabled: false } : t,
    );
    render(
      <BottomStatusBar
        kpis={INITIAL_KPIS}
        hudTilesConfig={config}
        onNavigate={vi.fn()}
        lastOrchEventAt={null}
        orchUsesPolling={false}
        liveFreshMs={10_000}
      />,
    );
    expect(screen.queryByText('Mesh')).not.toBeInTheDocument();
  });

  it('clicking the agents segment navigates to the agents view', () => {
    const onNavigate = vi.fn();
    render(
      <BottomStatusBar
        kpis={INITIAL_KPIS}
        hudTilesConfig={defaultHudTiles()}
        onNavigate={onNavigate}
        lastOrchEventAt={null}
        orchUsesPolling={false}
        liveFreshMs={10_000}
      />,
    );
    fireEvent.click(screen.getByText('Agents').closest('button')!);
    expect(onNavigate).toHaveBeenCalledWith('agents');
  });

  it('clicking the mesh segment navigates to the real mesh view, not the Compute default', () => {
    const onNavigate = vi.fn();
    render(
      <BottomStatusBar
        kpis={INITIAL_KPIS}
        hudTilesConfig={defaultHudTiles()}
        onNavigate={onNavigate}
        lastOrchEventAt={null}
        orchUsesPolling={false}
        liveFreshMs={10_000}
      />,
    );
    fireEvent.click(screen.getByText('Mesh').closest('button')!);
    expect(onNavigate).toHaveBeenCalledWith('mesh');
  });

  it('the configure trigger opens a live-apply checkbox menu that stays open across toggles', () => {
    const onHudTilesChange = vi.fn();
    render(
      <BottomStatusBar
        kpis={INITIAL_KPIS}
        hudTilesConfig={defaultHudTiles()}
        onHudTilesChange={onHudTilesChange}
        onNavigate={vi.fn()}
        lastOrchEventAt={null}
        orchUsesPolling={false}
        liveFreshMs={10_000}
      />,
    );
    fireEvent.click(screen.getByRole('button', { name: /configure/i }));
    const meshCheckbox = screen.getByRole('checkbox', { name: /mesh peers/i });
    expect(meshCheckbox).toBeChecked();
    fireEvent.click(meshCheckbox);
    expect(onHudTilesChange).toHaveBeenCalledTimes(1);
    const budgetCheckbox = screen.getByRole('checkbox', { name: /budget burn/i });
    fireEvent.click(budgetCheckbox);
    expect(onHudTilesChange).toHaveBeenCalledTimes(2);
  });

  it('renders the workbench-tabbar-trailing-slot portal target', () => {
    render(
      <BottomStatusBar
        kpis={INITIAL_KPIS}
        hudTilesConfig={defaultHudTiles()}
        onHudTilesChange={vi.fn()}
        onNavigate={vi.fn()}
        lastOrchEventAt={null}
        orchUsesPolling={false}
        liveFreshMs={10_000}
      />,
    );
    expect(document.getElementById(WORKBENCH_TABBAR_TRAILING_SLOT_ID)).toBeInTheDocument();
  });

  it('mesh segment shows online/total node count from real mesh data, not a bare peer count', () => {
    render(
      <BottomStatusBar
        kpis={INITIAL_KPIS}
        hudTilesConfig={defaultHudTiles()}
        onHudTilesChange={vi.fn()}
        onNavigate={vi.fn()}
        lastOrchEventAt={null}
        orchUsesPolling={false}
        liveFreshMs={10_000}
        meshNodes={[
          { id: 'n1', status: 'online' },
          { id: 'n2', status: 'online' },
          { id: 'n3', status: 'quarantined' },
        ]}
      />,
    );
    expect(screen.getByTestId('bottom-status-bar-mesh')).toHaveTextContent('2/3 online');
  });

  it('mesh segment falls back to a bare peer count, still worded "online", when meshNodes is not supplied', () => {
    render(
      <BottomStatusBar
        kpis={INITIAL_KPIS}
        hudTilesConfig={defaultHudTiles()}
        onHudTilesChange={vi.fn()}
        onNavigate={vi.fn()}
        lastOrchEventAt={null}
        orchUsesPolling={false}
        liveFreshMs={10_000}
      />,
    );
    expect(screen.getByTestId('bottom-status-bar-mesh')).toHaveTextContent(
      `${INITIAL_KPIS.mesh.peers} online`,
    );
  });
});
