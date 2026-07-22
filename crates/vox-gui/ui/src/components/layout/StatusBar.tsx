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
      className="mt-2 flex h-7 items-center gap-1 px-3 text-[10px] text-text-muted"
    >
      {/* Scrolls independently at narrow widths so the KPI segments never
          push the freshness pill / trailing slot off the right edge of the
          viewport — those two stay pinned (shrink-0, outside this scroll
          region) and reachable no matter how narrow the window gets. */}
      <div className="flex min-w-0 flex-1 items-center gap-1 overflow-x-auto">
        <Segment
          testId="status-bar-agents"
          label="Agents"
          value={String(kpis.activeAgents.value)}
          onClick={() => onNavigate('agents')}
        />
        <span className="text-text-muted" aria-hidden="true">
          ·
        </span>
        <Segment
          testId="status-bar-queue"
          label="Queue"
          value={String(kpis.queueDepth.value)}
          onClick={() => onNavigate('runs')}
        />
        <span className="text-text-muted" aria-hidden="true">
          ·
        </span>
        <Segment
          testId="status-bar-budget"
          label="Budget"
          value={budgetValue}
          onClick={() => onNavigate('settings')}
        />
        <span className="text-text-muted" aria-hidden="true">
          ·
        </span>
        <Segment
          testId="status-bar-mesh"
          label="Mesh"
          value={`${kpis.mesh.peers} peers`}
          onClick={() => onNavigate('mesh')}
        />
        <span className="text-text-muted" aria-hidden="true">
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
            <span className="text-text-muted" aria-hidden="true">
              ·
            </span>
            <button
              type="button"
              data-testid="achievements-trigger"
              aria-label="Open achievements"
              onClick={onOpenAchievements}
              className="inline-flex items-center justify-center rounded px-1.5 py-0.5 text-amber-300/80 hover:bg-overlay-subtle hover:text-amber-200 transition"
            >
              <Icon.trophy className="size-3.5" aria-hidden="true" />
            </button>
          </>
        )}
      </div>

      <div
        data-testid="status-bar-freshness"
        className={`ml-auto inline-flex shrink-0 items-center gap-1.5 rounded border px-2 py-0.5 ${fresh.pill}`}
      >
        <span className={`size-1.5 rounded-full ${fresh.dot}`} />
        <span className="uppercase tracking-[0.14em]">{fresh.label}</span>
      </div>

      {/* Fixed home for surface-level chrome that needs to sit inline with
          persistent app chrome rather than in the per-surface content area
          (e.g. Chat's "Panels ▾" dock-visibility menu, portaled in here from
          ChatSurface). StatusBar is a single, non-wrapping row rendered once
          in the app shell's header — unlike WorkbenchTabBar's tablist, it
          never grows to multiple lines as more tabs open, so anything docked
          here stays put and reachable regardless of how many top-level tabs
          are open or how the tab bar wraps. It's also independent of the tab
          bar's own lifecycle: the tab bar is slated for eventual removal,
          this slot is not. `shrink-0` (and the KPI segments' own scroll
          region above) keeps it pinned at the right edge even when the
          window itself is too narrow to fit everything. */}
      <div
        id="workbench-tabbar-trailing-slot"
        data-testid="workbench-tabbar-trailing-slot"
        className="ml-2 flex shrink-0 items-center"
      />
    </Glass>
  );
}
