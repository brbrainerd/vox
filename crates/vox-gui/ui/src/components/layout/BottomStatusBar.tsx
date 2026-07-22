import React from 'react';
import { Glass } from '../ui/Glass';
import { formatBudgetCap } from '../../config/budget';
import { useFreshness } from '../../hooks/useFreshness';
import { resolveVisibleHudTiles, type HudTilesConfig, type HudTileKind } from '../../hooks/useHudTiles';
import { INITIAL_KPIS } from '../../data/initialState';

type KpiState = typeof INITIAL_KPIS;

export interface BottomStatusBarProps {
  kpis: KpiState;
  hudTilesConfig: HudTilesConfig;
  onNavigate: (view: string) => void;
  lastOrchEventAt: number | null;
  orchUsesPolling: boolean;
  liveFreshMs: number;
  activeModel?: string | null;
  openrouterSpendUsd?: number | null;
  pendingApprovals?: number | null;
}

function freshnessClasses(tone: 'live' | 'poll' | 'stale') {
  if (tone === 'live') {
    return {
      pill: 'border-emerald-400/20 bg-emerald-400/[0.04] text-emerald-300',
      dot: 'bg-emerald-400',
      label: 'Live',
    };
  }
  if (tone === 'poll') {
    return {
      pill: 'border-amber-400/20 bg-amber-400/[0.04] text-amber-300',
      dot: 'bg-amber-400',
      label: 'Poll',
    };
  }
  return {
    pill: 'border-border-subtle bg-overlay-subtle text-text-muted',
    dot: 'bg-text-muted',
    label: 'Offline',
  };
}

function Segment({
  testId,
  label,
  value,
  onClick,
}: {
  testId: string;
  label: string;
  value: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      data-testid={testId}
      onClick={onClick}
      className="inline-flex items-center gap-1.5 rounded px-2 py-0.5 text-[10px] text-text-muted hover:bg-overlay-subtle hover:text-text-secondary transition"
    >
      <span className="uppercase tracking-[0.14em] text-text-muted">{label}</span>
      <span className="font-mono tabular-nums text-text-secondary">{value}</span>
    </button>
  );
}

export function BottomStatusBar({
  kpis,
  hudTilesConfig,
  onNavigate,
  lastOrchEventAt,
  orchUsesPolling,
  liveFreshMs,
  activeModel = null,
  openrouterSpendUsd = null,
  pendingApprovals = null,
}: BottomStatusBarProps) {
  const tone = useFreshness(lastOrchEventAt, {
    freshMs: liveFreshMs,
    usesPolling: orchUsesPolling,
  });
  const fresh = freshnessClasses(tone);
  const visible = resolveVisibleHudTiles(hudTilesConfig);

  const budgetSource = kpis.budgetBurn?.source ?? 'fallback';
  const capDisplay = formatBudgetCap(
    budgetSource === 'daemon' ? kpis.budgetBurn.cap : null,
    budgetSource,
  );
  const budgetValue = `$${kpis.budgetBurn.value.toFixed(2)}/${capDisplay}`;

  const renderSegment = (kind: HudTileKind): React.ReactNode => {
    switch (kind) {
      case 'active_agents':
        return (
          <Segment
            key={kind}
            testId="bottom-status-bar-agents"
            label="Agents"
            value={String(kpis.activeAgents.value)}
            onClick={() => onNavigate('agents')}
          />
        );
      case 'queue_depth':
        return (
          <Segment
            key={kind}
            testId="bottom-status-bar-queue"
            label="Queue"
            value={String(kpis.queueDepth.value)}
            onClick={() => onNavigate('runs')}
          />
        );
      case 'budget_burn':
        return (
          <Segment
            key={kind}
            testId="bottom-status-bar-budget"
            label="Budget"
            value={budgetValue}
            onClick={() => onNavigate('settings')}
          />
        );
      case 'mesh_peers':
        // Bare peer count for now — Task 11 upgrades this to online/total or queue-depth.
        return (
          <Segment
            key={kind}
            testId="bottom-status-bar-mesh"
            label="Mesh"
            value={`${kpis.mesh.peers} peers`}
            onClick={() => onNavigate('mesh')}
          />
        );
      case 'active_model':
        return (
          <Segment
            key={kind}
            testId="bottom-status-bar-model"
            label="Model"
            value={activeModel ?? 'auto-route'}
            onClick={() => onNavigate('models')}
          />
        );
      case 'openrouter_spend':
        return (
          <Segment
            key={kind}
            testId="bottom-status-bar-openrouter"
            label="OR Spend"
            value={openrouterSpendUsd == null ? '—' : `$${openrouterSpendUsd.toFixed(2)}`}
            onClick={() => onNavigate('settings')}
          />
        );
      case 'pending_approvals':
        return (
          <Segment
            key={kind}
            testId="bottom-status-bar-approvals"
            label="Approvals"
            value={String(pendingApprovals ?? 0)}
            onClick={() => onNavigate('approvals')}
          />
        );
      default:
        return null;
    }
  };

  return (
    <Glass
      data-testid="bottom-status-bar"
      role="status"
      aria-label="Operator status"
      className="flex h-7 items-center gap-1 px-3 text-[10px] text-text-muted"
    >
      <div className="flex min-w-0 flex-1 items-center gap-1 overflow-x-auto">
        {visible.map((kind) => renderSegment(kind))}
      </div>
      <div
        data-testid="bottom-status-bar-freshness"
        className={`ml-auto inline-flex shrink-0 items-center gap-1.5 rounded border px-2 py-0.5 ${fresh.pill}`}
      >
        <span className={`size-1.5 rounded-full ${fresh.dot}`} />
        <span className="uppercase tracking-[0.14em]">{fresh.label}</span>
      </div>
    </Glass>
  );
}
