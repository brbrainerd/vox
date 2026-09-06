import React, { useEffect, useRef, useState } from 'react';
import { Glass } from '../ui/Glass';
import { Icon } from '../ui/Icons';
import { formatBudgetCap } from '../../config/budget';
import { useFreshness } from '../../hooks/useFreshness';
import {
  resolveVisibleHudTiles,
  toggleHudTile,
  HUD_TILE_LABELS,
  type HudTilesConfig,
  type HudTileKind,
} from '../../hooks/useHudTiles';
import { INITIAL_KPIS } from '../../data/initialState';
import { WORKBENCH_TABBAR_TRAILING_SLOT_ID } from '../../lib/domIds';
import type { MeshNode } from '../surfaces/Mesh/MeshView';

type KpiState = typeof INITIAL_KPIS;

export interface BottomStatusBarProps {
  kpis: KpiState;
  hudTilesConfig: HudTilesConfig;
  onHudTilesChange: (config: HudTilesConfig) => void;
  onNavigate: (view: string) => void;
  lastOrchEventAt: number | null;
  orchUsesPolling: boolean;
  liveFreshMs: number;
  activeModel?: string | null;
  openrouterSpendUsd?: number | null;
  pendingApprovals?: number | null;
  meshNodes?: MeshNode[];
  gamifyEnabled?: boolean;
  onOpenAchievements?: () => void;
}

function freshnessClasses(tone: 'live' | 'poll' | 'stale') {
  if (tone === 'live') {
    return {
      pill: 'border-emerald-400/20 bg-emerald-400/4 text-emerald-300',
      dot: 'bg-emerald-400',
      label: 'Live',
    };
  }
  if (tone === 'poll') {
    return {
      pill: 'border-amber-400/20 bg-amber-400/4 text-amber-300',
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
      className="inline-flex items-center gap-1.5 rounded-sm px-2 py-0.5 text-[10px] text-text-muted hover:bg-overlay-subtle hover:text-text-secondary transition"
    >
      <span className="uppercase tracking-[0.14em] text-text-muted">{label}</span>
      <span className="font-mono tabular-nums text-text-secondary">{value}</span>
    </button>
  );
}

export function BottomStatusBar({
  kpis,
  hudTilesConfig,
  onHudTilesChange,
  onNavigate,
  lastOrchEventAt,
  orchUsesPolling,
  liveFreshMs,
  activeModel = null,
  openrouterSpendUsd = null,
  pendingApprovals = null,
  meshNodes,
  gamifyEnabled = false,
  onOpenAchievements,
}: BottomStatusBarProps) {
  const tone = useFreshness(lastOrchEventAt, {
    freshMs: liveFreshMs,
    usesPolling: orchUsesPolling,
  });
  const fresh = freshnessClasses(tone);
  const visible = resolveVisibleHudTiles(hudTilesConfig);

  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!menuOpen) return;
    const onOutside = (e: MouseEvent) => {
      const target = e.target as Node;
      if (menuRef.current?.contains(target)) return;
      if (triggerRef.current?.contains(target)) return;
      setMenuOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setMenuOpen(false);
        triggerRef.current?.focus();
      }
    };
    document.addEventListener('mousedown', onOutside);
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('mousedown', onOutside);
      document.removeEventListener('keydown', onKey);
    };
  }, [menuOpen]);

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
      case 'mesh_peers': {
        // Keep the trailing "online" wording stable across both states so the
        // segment doesn't visibly change shape once richer node data loads —
        // before meshNodes arrives we don't know the online/offline split,
        // so show the peer count as a single figure rather than switching
        // from "N peers" to "X/Y online" (two different phrasings for what
        // reads as the same kind of number).
        const onlineCount = meshNodes?.filter((n) => n.status === 'online').length ?? 0;
        const totalCount = meshNodes?.length ?? 0;
        const meshValue =
          meshNodes == null ? `${kpis.mesh.peers} online` : `${onlineCount}/${totalCount} online`;
        return (
          <Segment
            key={kind}
            testId="bottom-status-bar-mesh"
            label="Mesh"
            value={meshValue}
            onClick={() => onNavigate('mesh')}
          />
        );
      }
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
      className="flex h-7 w-full items-center gap-1 p-0 px-3 rounded-none border-x-0 border-b-0 shadow-none text-[10px] text-text-muted"
    >
      <div className="flex min-w-0 flex-1 items-center gap-1 overflow-x-auto">
        {visible.map((kind) => renderSegment(kind))}
      </div>
      {gamifyEnabled && onOpenAchievements && (
        <button
          type="button"
          data-testid="achievements-trigger"
          aria-label="Open achievements"
          onClick={onOpenAchievements}
          className="inline-flex shrink-0 items-center justify-center rounded-sm px-1.5 py-0.5 text-amber-300/80 hover:bg-overlay-subtle hover:text-amber-200 transition"
        >
          <Icon.trophy className="size-3.5" aria-hidden="true" />
        </button>
      )}
      <div className="relative shrink-0">
        <button
          ref={triggerRef}
          type="button"
          onClick={() => setMenuOpen((o) => !o)}
          aria-expanded={menuOpen}
          aria-label="Configure status bar"
          className="rounded-sm px-1.5 py-0.5 text-[10px] text-text-muted hover:bg-overlay-subtle hover:text-text-secondary transition"
        >
          Configure ▾
        </button>
        {menuOpen ? (
          <div
            ref={menuRef}
            className="absolute bottom-full right-0 z-50 mb-1 w-56 rounded-lg border border-border-subtle bg-bg-base p-2 shadow-2xl"
          >
            {hudTilesConfig.tiles.map((tile) => (
              <label
                key={tile.id}
                className="flex items-center gap-2 rounded-sm px-2 py-1 text-[11px] text-text-secondary hover:bg-overlay-subtle"
              >
                <input
                  type="checkbox"
                  checked={tile.enabled}
                  onChange={(e) =>
                    onHudTilesChange(toggleHudTile(hudTilesConfig, tile.id, e.target.checked))
                  }
                  className="rounded-sm border-border-subtle bg-bg-base text-brass focus:ring-brass/40 focus:ring-offset-bg-base size-3.5"
                />
                {HUD_TILE_LABELS[tile.kind]}
              </label>
            ))}
          </div>
        ) : null}
      </div>
      <div
        data-testid="bottom-status-bar-freshness"
        className={`ml-auto inline-flex shrink-0 items-center gap-1.5 rounded-sm border px-2 py-0.5 ${fresh.pill}`}
      >
        <span className={`size-1.5 rounded-full ${fresh.dot}`} />
        <span className="uppercase tracking-[0.14em]">{fresh.label}</span>
      </div>

      {/* Fixed home for surface-level chrome that needs to sit inline with
          persistent app chrome rather than in the per-surface content area
          (e.g. Chat's "Panels ▾" dock-visibility menu, portaled in here from
          ChatSurface). BottomStatusBar is a single, non-wrapping row rendered
          once in the app shell's footer — unlike WorkbenchTabBar's tablist,
          it never grows to multiple lines as more tabs open, so anything
          docked here stays put and reachable regardless of how many
          top-level tabs are open or how the tab bar wraps. It's also
          independent of the tab bar's own lifecycle: the tab bar is slated
          for eventual removal, this slot is not. `shrink-0` (and the KPI
          segments' own scroll region above) keeps it pinned at the right
          edge even when the window itself is too narrow to fit everything. */}
      <div
        id={WORKBENCH_TABBAR_TRAILING_SLOT_ID}
        data-testid={WORKBENCH_TABBAR_TRAILING_SLOT_ID}
        className="ml-2 flex shrink-0 items-center"
      />
    </Glass>
  );
}
