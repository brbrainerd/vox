import React from 'react';
import { Glass } from '../ui/Glass';
import { AxisMark } from '../ui/AxisMark';
import { Icon } from '../ui/Icons';
import { Sparkline } from '../ui/Sparkline';
import { formatBudgetCap } from '../../config/budget';
import { freshnessTone } from '../../hooks/useFreshness';
import {
  HUD_TILE_KINDS,
  type HudTileKind,
} from '../../hooks/useHudTiles';

export type HudMode = 'full' | 'slim' | 'hidden';

/**
 * Tile kinds whose metrics are already shown by the canonical StatusBar
 * "Operator status" row (Agents / Queue / Budget / Mesh / Model). These are
 * filtered out of the TopHud KPI strip to avoid duplicating the same metrics
 * twice in the top area. Only non-duplicated tiles (e.g. openrouter_spend)
 * render in the HUD.
 */
const STATUS_BAR_TILE_KINDS = new Set<HudTileKind>([
  'active_agents',
  'queue_depth',
  'budget_burn',
  'mesh_peers',
  'active_model',
]);

interface KPIProps {
  label: string;
  value: string | number;
  unit?: string;
  delta?: number;
  color: string;
  spark: number[];
  icon: React.ReactNode;
  sub?: string;
  onClick?: () => void;
}

function KPI({ label, value, unit, delta, color, spark, icon, sub, onClick }: KPIProps) {
  const deltaPos = delta != null && delta >= 0;
  const inner = (
    <div className="flex items-center gap-3 px-4 py-2 first:pl-5 last:pr-5">
      <div className={`flex size-9 items-center justify-center rounded-lg bg-overlay-subtle ring-1 ring-border-subtle ${color}`}>
        {icon}
      </div>
      <div className="flex flex-col leading-none">
        <span className="text-[10px] uppercase tracking-[0.18em] text-text-muted">{label}</span>
        <div className="mt-1 flex items-baseline gap-1.5">
          <span className="font-display text-[20px] font-semibold text-text-primary tabular-nums">{value}</span>
          {unit && <span className="text-[11px] text-text-muted">{unit}</span>}
          {delta != null && (
            <span className={`text-[10px] tabular-nums ${deltaPos ? 'text-accent-secondary' : 'text-[var(--color-status-fail)]'}`}>
              {deltaPos ? '▲' : '▼'} {Math.abs(delta)}
            </span>
          )}
        </div>
        {sub && <span className="mt-0.5 text-[10px] text-text-muted">{sub}</span>}
      </div>
      <div className={color}>
        <Sparkline data={spark} width={72} height={22} />
      </div>
    </div>
  );
  if (onClick) {
    return (
      <button type="button" onClick={onClick} className="text-left hover:bg-overlay-hover transition">
        {inner}
      </button>
    );
  }
  return inner;
}

interface TopHudProps {
  kpis: any;
  onCommand: () => void;
  onOpenCommandPalette?: () => void;
  lastOrchEventAt?: number | null;
  orchUsesPolling?: boolean;
  liveFreshMs?: number;
  onNavigate?: (viewKey: string) => void;
  hudMode?: HudMode;
  setHudMode?: (mode: HudMode) => void;
  workspaceTitle?: string;
  visibleTiles?: HudTileKind[];
  activeModel?: string | null;
  openrouterSpendUsd?: number | null;
  pendingApprovals?: number | null;
}

function formatOpenRouterSpend(usd: number | null | undefined): string {
  if (usd == null || Number.isNaN(usd)) {
    return '—';
  }
  return `$${usd.toFixed(2)}`;
}

export function TopHud({
  kpis,
  onCommand,
  onOpenCommandPalette,
  lastOrchEventAt = null,
  orchUsesPolling = false,
  liveFreshMs = 10_000,
  onNavigate,
  hudMode = 'full',
  setHudMode,
  workspaceTitle = 'Operator',
  visibleTiles = [...HUD_TILE_KINDS],
  activeModel = null,
  openrouterSpendUsd = null,
  pendingApprovals = null,
}: TopHudProps) {
  const openPalette = onOpenCommandPalette ?? onCommand;
  const cycleHud = () => {
    if (!setHudMode) return;
    setHudMode(hudMode === 'full' ? 'slim' : hudMode === 'slim' ? 'hidden' : 'full');
  };
  const tone = freshnessTone(lastOrchEventAt, { freshMs: liveFreshMs, usesPolling: orchUsesPolling });
  const budgetSource = kpis.budgetBurn?.source ?? 'fallback';
  const capDisplay = formatBudgetCap(
    budgetSource === 'daemon' ? kpis.budgetBurn.cap : null,
    budgetSource,
  );
  const pctSub =
    budgetSource === 'daemon' && kpis.budgetBurn.cap > 0
      ? `${Math.round((kpis.budgetBurn.value / kpis.budgetBurn.cap) * 100)}% of cap`
      : 'awaiting daemon cap';

  const liveClasses =
    tone === 'live'
      ? 'border-emerald-400/20 bg-emerald-400/[0.04] text-emerald-300'
      : tone === 'poll'
        ? 'border-amber-400/20 bg-amber-400/[0.04] text-amber-300'
        : 'border-border-subtle bg-overlay-subtle text-text-muted';
  const liveDot =
    tone === 'live' ? 'bg-accent-secondary' : tone === 'poll' ? 'bg-brass' : 'bg-text-muted';
  const liveLabel = tone === 'live' ? 'Live' : tone === 'poll' ? 'Poll' : 'Offline';

  const renderTile = (kind: HudTileKind): React.ReactNode => {
    switch (kind) {
      case 'active_agents':
        return (
          <KPI
            key={kind}
            label="Active Agents"
            value={kpis.activeAgents.value}
            delta={kpis.activeAgents.delta}
            color="text-cyan-300"
            spark={kpis.activeAgents.spark}
            icon={<Icon.users className="size-4" />}
            onClick={() => onNavigate?.('agents')}
          />
        );
      case 'queue_depth':
        return (
          <KPI
            key={kind}
            label="Queue Depth"
            value={kpis.queueDepth.value}
            delta={kpis.queueDepth.delta}
            color="text-text-secondary"
            spark={kpis.queueDepth.spark}
            icon={<Icon.scale className="size-4" />}
            onClick={() => onNavigate?.('runs')}
          />
        );
      case 'budget_burn':
        return (
          <KPI
            key={kind}
            label="Budget Burn"
            value={`$${kpis.budgetBurn.value.toFixed(2)}`}
            unit={`/ ${capDisplay}`}
            delta={kpis.budgetBurn.delta}
            color="text-amber-300"
            spark={kpis.budgetBurn.spark}
            icon={<Icon.bolt className="size-4" />}
            sub={pctSub}
            onClick={() => onNavigate?.('settings')}
          />
        );
      case 'mesh_peers':
        return (
          <KPI
            key={kind}
            label="Mesh"
            value={kpis.mesh.value}
            unit={kpis.mesh.unit}
            delta={kpis.mesh.delta}
            color="text-violet-300"
            spark={kpis.mesh.spark}
            icon={<Icon.link className="size-4" />}
            sub={`${kpis.mesh.peers} peers online`}
            onClick={() => onNavigate?.('mesh')}
          />
        );
      case 'active_model':
        return (
          <KPI
            key={kind}
            label="Active Model"
            value={activeModel ?? 'auto-route'}
            color="text-sky-300"
            spark={kpis.activeAgents.spark}
            icon={<Icon.cpu className="size-4" />}
            onClick={() => onNavigate?.('models')}
          />
        );
      case 'openrouter_spend':
        return (
          <KPI
            key={kind}
            label="OpenRouter Spend"
            value={formatOpenRouterSpend(openrouterSpendUsd)}
            color="text-emerald-300"
            spark={kpis.budgetBurn.spark}
            icon={<Icon.globe className="size-4" />}
            onClick={() => onNavigate?.('settings')}
          />
        );
      case 'pending_approvals':
        return (
          <KPI
            key={kind}
            label="Pending Approvals"
            value={pendingApprovals ?? 0}
            color="text-amber-300"
            spark={kpis.queueDepth.spark}
            icon={<Icon.shield className="size-4" />}
            onClick={() => onNavigate?.('approvals')}
          />
        );
      default:
        return null;
    }
  };

  if (hudMode === 'hidden') {
    return (
      <div className="group relative h-3">
        <button
          type="button"
          onClick={cycleHud}
          aria-label="Show HUD"
          className="absolute inset-x-0 top-0 mx-auto w-24 rounded-b-md border border-border-subtle bg-bg-base/80 py-0.5 text-[9px] uppercase tracking-widest text-text-muted opacity-0 group-hover:opacity-100 transition"
        >
          Show HUD
        </button>
      </div>
    );
  }

  if (hudMode === 'slim') {
    return (
      <Glass className="flex h-7 items-center gap-3 px-4 text-[10px] text-text-muted">
        <button
          type="button"
          data-testid="omnisearch-trigger"
          onClick={openPalette}
          className="inline-flex items-center gap-1.5 rounded border border-border-subtle bg-overlay-subtle px-2 py-0.5 text-text-muted hover:border-brass/40 hover:text-brass transition"
        >
          <span>Search or jump…</span>
          <span className="rounded border border-border-subtle bg-overlay-subtle px-1 text-[9px] tracking-widest text-text-muted">⌘K</span>
        </button>
        <span className={`ml-auto inline-flex items-center gap-1 rounded px-1.5 py-0.5 ${liveClasses}`}>
          <span className={`size-1.5 rounded-full ${liveDot}`} />
          {liveLabel}
        </span>
        <button type="button" onClick={cycleHud} aria-label="Expand HUD" className="text-text-muted hover:text-text-secondary" title="Expand HUD"><span aria-hidden="true">▲</span></button>
      </Glass>
    );
  }

  return (
    <Glass className="flex items-stretch overflow-hidden">
      <div className="relative flex items-center gap-3 px-5 border-r border-border-subtle">
        <span className="vox-tick vox-tick-tl" />
        <span className="vox-tick vox-tick-tr" />
        <AxisMark size={28} />
        <div className="flex flex-col leading-none">
          <span className="font-display text-[13px] tracking-[0.22em] text-text-primary">{workspaceTitle}</span>
          <span className="text-[9px] uppercase tracking-[0.3em] text-text-muted">axis operator console</span>
        </div>
      </div>

      <div className="flex min-w-0 flex-1 items-stretch divide-x divide-white/5 overflow-x-auto">
        <div className="flex items-center px-4">
          <button
            type="button"
            data-testid="omnisearch-trigger"
            onClick={openPalette}
            className="group flex min-w-[12rem] items-center gap-2 rounded-lg border border-border-subtle bg-overlay-subtle px-3 py-1.5 text-xs text-text-muted hover:border-brass/40 hover:text-brass transition"
          >
            <Icon.search className="size-3.5 shrink-0" />
            <span className="font-display tracking-wide text-left">Search or jump…</span>
            <span className="ml-auto rounded border border-white/10 bg-overlay-subtle px-1.5 py-0.5 text-[9px] tracking-widest text-text-muted">⌘K</span>
          </button>
        </div>
        {visibleTiles
          .filter((kind) => !STATUS_BAR_TILE_KINDS.has(kind))
          .map((kind) => renderTile(kind))}
      </div>

      <div className="ml-auto flex items-center gap-2 px-4 border-l border-border-subtle">
        <div className={`flex items-center gap-2 rounded-lg border px-2.5 py-1.5 ${liveClasses}`}>
          <span className={`relative inline-block size-1.5 rounded-full ${liveDot}`}>
            {tone === 'live' && (
              <span className="absolute inset-0 rounded-full bg-emerald-400 animate-vox-ping" />
            )}
          </span>
          <span className="text-[10px] uppercase tracking-[0.2em] hidden sm:inline">{liveLabel}</span>
        </div>
        <button
          type="button"
          onClick={cycleHud}
          className="rounded-md border border-border-subtle px-2 py-1 text-[10px] text-text-muted hover:text-text-secondary"
          title="Collapse HUD (Ctrl+Shift+H)"
          aria-label="Collapse HUD"
        >
          <span aria-hidden="true">▼</span>
        </button>
      </div>
    </Glass>
  );
}
