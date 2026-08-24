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

// ---------------------------------------------------------------------------
// Budget wiring + configurability. The spend tile used to render lifetime
// spend with no cap; the cap the guard actually enforces is the daily one.
// ---------------------------------------------------------------------------

const baseProps = {
  kpis: INITIAL_KPIS,
  onNavigate: vi.fn(),
  lastOrchEventAt: null,
  orchUsesPolling: false,
  liveFreshMs: 10_000,
};

const spend = (over: Partial<Record<string, number | string | null>> = {}) => ({
  sessionUsd: 0.1,
  dayUsd: 1.0,
  totalUsd: 42.0,
  dailyBudgetUsd: 10,
  perSessionBudgetUsd: 2,
  warnThresholdPct: 0.8,
  error: null,
  ...over,
});

describe('BottomStatusBar LLM spend tile', () => {
  it('shows daily spend against the daily cap, not lifetime spend', () => {
    render(<BottomStatusBar {...baseProps} hudTilesConfig={defaultHudTiles()} llmSpend={spend()} />);
    const tile = screen.getByTestId('bottom-status-bar-openrouter');
    expect(tile).toHaveTextContent('$1.00/$10.00');
    expect(tile).not.toHaveTextContent('42');
  });

  it('is labeled LLM Spend, since the figure sums every provider', () => {
    render(<BottomStatusBar {...baseProps} hudTilesConfig={defaultHudTiles()} llmSpend={spend()} />);
    expect(screen.getByTestId('bottom-status-bar-openrouter')).toHaveTextContent(/LLM Spend/i);
  });

  it('goes to the warn tone at the configured threshold, not a hardcoded one', () => {
    // 8.0/10 == the 0.8 threshold the budget guard warns at.
    render(<BottomStatusBar {...baseProps} hudTilesConfig={defaultHudTiles()} llmSpend={spend({ dayUsd: 8.0 })} />);
    expect(screen.getByTestId('bottom-status-bar-openrouter')).toHaveAttribute('data-tone', 'warn');
  });

  it('respects a threshold change rather than assuming 0.8', () => {
    render(<BottomStatusBar {...baseProps} hudTilesConfig={defaultHudTiles()} llmSpend={spend({ dayUsd: 8.0, warnThresholdPct: 0.95 })} />);
    expect(screen.getByTestId('bottom-status-bar-openrouter')).toHaveAttribute('data-tone', 'ok');
  });

  it('goes to the over tone at or past the cap that blocks dispatch', () => {
    render(<BottomStatusBar {...baseProps} hudTilesConfig={defaultHudTiles()} llmSpend={spend({ dayUsd: 10 })} />);
    expect(screen.getByTestId('bottom-status-bar-openrouter')).toHaveAttribute('data-tone', 'over');
  });

  it('shows a distinct error tone rather than looking unconfigured', () => {
    render(<BottomStatusBar {...baseProps} hudTilesConfig={defaultHudTiles()} llmSpend={spend({ dayUsd: null, dailyBudgetUsd: null, error: 'store unavailable' })} />);
    const tile = screen.getByTestId('bottom-status-bar-openrouter');
    expect(tile).toHaveAttribute('data-tone', 'error');
    expect(tile).toHaveAttribute('title', expect.stringContaining('store unavailable'));
  });

  it('uses theme status tokens, not hardcoded Tailwind color literals', () => {
    render(<BottomStatusBar {...baseProps} hudTilesConfig={defaultHudTiles()} llmSpend={spend({ dayUsd: 8.0 })} />);
    const cls = screen.getByTestId('bottom-status-bar-openrouter').className;
    expect(cls).toContain('status-warn');
    expect(cls).not.toMatch(/\b(emerald|amber|red|green)-\d{3}\b/);
  });
});

describe('BottomStatusBar configurability', () => {
  const withTiles = (kinds: string[]) => {
    const cfg = defaultHudTiles();
    cfg.tiles = cfg.tiles.map((t) => ({ ...t, enabled: kinds.includes(t.kind) }));
    return cfg;
  };

  it('renders the mesh VRAM tile from live orchestrator status', () => {
    const kpis = { ...INITIAL_KPIS, mesh: { ...INITIAL_KPIS.mesh, vramGb: 48 } };
    render(<BottomStatusBar {...baseProps} kpis={kpis} hudTilesConfig={withTiles(['vram_total'])} />);
    expect(screen.getByTestId('bottom-status-bar-vram')).toHaveTextContent('48');
  });

  it('renders the session spend tile against the per-session cap', () => {
    render(<BottomStatusBar {...baseProps} hudTilesConfig={withTiles(['session_spend'])} llmSpend={spend()} />);
    expect(screen.getByTestId('bottom-status-bar-session-spend')).toHaveTextContent('$0.10/$2.00');
  });

  it('renders the build version tile', () => {
    render(<BottomStatusBar {...baseProps} hudTilesConfig={withTiles(['build_version'])} buildDisplay="0.6.0+local (dev)" />);
    expect(screen.getByTestId('bottom-status-bar-build')).toHaveTextContent('0.6.0+local (dev)');
  });

  it('compact density drops tile labels but keeps values', () => {
    const cfg = defaultHudTiles();
    cfg.options = { ...cfg.options, density: 'compact' };
    render(<BottomStatusBar {...baseProps} hudTilesConfig={cfg} llmSpend={spend()} />);
    expect(screen.queryByText('Agents')).not.toBeInTheDocument();
    expect(screen.getByTestId('bottom-status-bar-agents')).toHaveTextContent('0');
  });

  it('hides the freshness pill when the option is off', () => {
    const cfg = defaultHudTiles();
    cfg.options = { ...cfg.options, showFreshness: false };
    render(<BottomStatusBar {...baseProps} hudTilesConfig={cfg} />);
    expect(screen.queryByTestId('bottom-status-bar-freshness')).not.toBeInTheDocument();
  });

  it('labels the orchestrator run cap distinctly from LLM spend', () => {
    render(<BottomStatusBar {...baseProps} hudTilesConfig={defaultHudTiles()} llmSpend={spend()} />);
    expect(screen.getByTestId('bottom-status-bar-budget')).toHaveTextContent(/Run Cap/i);
  });
});
