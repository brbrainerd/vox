import React from 'react';
import { Glass } from '../ui/Glass';
import { Icon } from '../ui/Icons';
import { formatBudgetCap } from '../../config/budget';
import { useFreshness } from '../../hooks/useFreshness';
import { INITIAL_KPIS } from '../../data/initialState';

type KpiState = typeof INITIAL_KPIS;

export interface StatusBarProps {
  kpis: KpiState;
  lastOrchEventAt: number | null;
  orchUsesPolling: boolean;
  liveFreshMs: number;
  onNavigate: (view: string) => void;
  gamifyEnabled?: boolean;
  onOpenAchievements?: () => void;
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
    pill: 'border-zinc-500/20 bg-zinc-500/[0.04] text-zinc-400',
    dot: 'bg-zinc-500',
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
      className="inline-flex items-center gap-1.5 rounded px-2 py-0.5 text-[10px] text-zinc-400 hover:bg-white/[0.04] hover:text-zinc-200 transition"
    >
      <span className="uppercase tracking-[0.14em] text-zinc-500">{label}</span>
      <span className="font-mono tabular-nums text-zinc-200">{value}</span>
    </button>
  );
}

export function StatusBar({
  kpis,
  lastOrchEventAt,
  orchUsesPolling,
  liveFreshMs,
  onNavigate,
  gamifyEnabled = false,
  onOpenAchievements,
}: StatusBarProps) {
  const tone = useFreshness(lastOrchEventAt, {
    freshMs: liveFreshMs,
    usesPolling: orchUsesPolling,
  });
  const fresh = freshnessClasses(tone);

  const budgetSource = kpis.budgetBurn?.source ?? 'fallback';
  const capDisplay = formatBudgetCap(
    budgetSource === 'daemon' ? kpis.budgetBurn.cap : null,
    budgetSource,
  );
  const budgetValue = `$${kpis.budgetBurn.value.toFixed(2)}/${capDisplay}`;

  return (
    <Glass
      data-testid="status-bar"
      role="status"
      aria-label="Operator status"
      className="mt-2 flex h-7 items-center gap-1 px-3 text-[10px] text-zinc-400"
    >
      <Segment
        testId="status-bar-agents"
        label="Agents"
        value={String(kpis.activeAgents.value)}
        onClick={() => onNavigate('agents')}
      />
      <span className="text-zinc-700" aria-hidden="true">
        ·
      </span>
      <Segment
        testId="status-bar-queue"
        label="Queue"
        value={String(kpis.queueDepth.value)}
        onClick={() => onNavigate('runs')}
      />
      <span className="text-zinc-700" aria-hidden="true">
        ·
      </span>
      <Segment
        testId="status-bar-budget"
        label="Budget"
        value={budgetValue}
        onClick={() => onNavigate('settings')}
      />
      <span className="text-zinc-700" aria-hidden="true">
        ·
      </span>
      <Segment
        testId="status-bar-mesh"
        label="Mesh"
        value={`${kpis.mesh.peers} peers`}
        onClick={() => onNavigate('compute')}
      />
      <span className="text-zinc-700" aria-hidden="true">
        ·
      </span>
      <Segment
        testId="status-bar-model"
        label="Model"
        value="auto-route"
        onClick={() => onNavigate('models')}
      />

      {gamifyEnabled && onOpenAchievements && (
        <>
          <span className="text-zinc-700" aria-hidden="true">
            ·
          </span>
          <button
            type="button"
            data-testid="achievements-trigger"
            aria-label="Open achievements"
            onClick={onOpenAchievements}
            className="inline-flex items-center justify-center rounded px-1.5 py-0.5 text-amber-300/80 hover:bg-white/[0.04] hover:text-amber-200 transition"
          >
            <Icon.trophy className="size-3.5" aria-hidden="true" />
          </button>
        </>
      )}

      <div
        data-testid="status-bar-freshness"
        className={`ml-auto inline-flex items-center gap-1.5 rounded border px-2 py-0.5 ${fresh.pill}`}
      >
        <span className={`size-1.5 rounded-full ${fresh.dot}`} />
        <span className="uppercase tracking-[0.14em]">{fresh.label}</span>
      </div>
    </Glass>
  );
}
