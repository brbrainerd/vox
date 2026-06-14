import React from 'react';
import { Glass } from '../ui/Glass';
import { Icon } from '../ui/Icons';
import { Sparkline } from '../ui/Sparkline';
import { formatBudgetCap } from '../../config/budget';
import { freshnessTone } from '../../hooks/useFreshness';

export type HudMode = 'full' | 'slim' | 'hidden';

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
      <div className={`flex size-9 items-center justify-center rounded-lg bg-white/[0.03] ring-1 ring-white/5 ${color}`}>
        {icon}
      </div>
      <div className="flex flex-col leading-none">
        <span className="text-[10px] uppercase tracking-[0.18em] text-zinc-500">{label}</span>
        <div className="mt-1 flex items-baseline gap-1.5">
          <span className="font-display text-[20px] font-semibold text-zinc-50 tabular-nums">{value}</span>
          {unit && <span className="text-[11px] text-zinc-500">{unit}</span>}
          {delta != null && (
            <span className={`text-[10px] tabular-nums ${deltaPos ? 'text-emerald-400' : 'text-rose-400'}`}>
              {deltaPos ? '▲' : '▼'} {Math.abs(delta)}
            </span>
          )}
        </div>
        {sub && <span className="mt-0.5 text-[10px] text-zinc-500">{sub}</span>}
      </div>
      <div className={color}>
        <Sparkline data={spark} width={72} height={22} />
      </div>
    </div>
  );
  if (onClick) {
    return (
      <button type="button" onClick={onClick} className="text-left hover:bg-white/[0.02] transition">
        {inner}
      </button>
    );
  }
  return inner;
}

interface TopHudProps {
  kpis: any;
  onCommand: () => void;
  lastOrchEventAt?: number | null;
  orchUsesPolling?: boolean;
  liveFreshMs?: number;
  onNavigate?: (viewKey: string) => void;
  hudMode?: HudMode;
  setHudMode?: (mode: HudMode) => void;
}

export function TopHud({
  kpis,
  onCommand,
  lastOrchEventAt = null,
  orchUsesPolling = false,
  liveFreshMs = 10_000,
  onNavigate,
  hudMode = 'full',
  setHudMode,
}: TopHudProps) {
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
        : 'border-zinc-500/20 bg-zinc-500/[0.04] text-zinc-400';
  const liveDot =
    tone === 'live' ? 'bg-emerald-400' : tone === 'poll' ? 'bg-amber-400' : 'bg-zinc-500';
  const liveLabel = tone === 'live' ? 'Live' : tone === 'poll' ? 'Poll' : 'Offline';

  if (hudMode === 'hidden') {
    return (
      <div className="group relative h-3">
        <button
          type="button"
          onClick={cycleHud}
          aria-label="Show HUD"
          className="absolute inset-x-0 top-0 mx-auto w-24 rounded-b-md border border-white/10 bg-zinc-950/80 py-0.5 text-[9px] uppercase tracking-widest text-zinc-500 opacity-0 group-hover:opacity-100 transition"
        >
          Show HUD
        </button>
      </div>
    );
  }

  if (hudMode === 'slim') {
    return (
      <Glass className="flex h-7 items-center gap-3 px-4 text-[10px] text-zinc-400">
        <span className="font-mono tabular-nums">
          agents {kpis.activeAgents.value} · queue {kpis.queueDepth.value} ·
          ${kpis.budgetBurn.value.toFixed(2)}/{capDisplay} · mesh {kpis.mesh.peers} peers
        </span>
        <span className={`inline-flex items-center gap-1 rounded px-1.5 py-0.5 ${liveClasses}`}>
          <span className={`size-1.5 rounded-full ${liveDot}`} />
          {liveLabel}
        </span>
        <button type="button" onClick={onCommand} className="ml-auto text-zinc-500 hover:text-brass">⌘K</button>
        <button type="button" onClick={cycleHud} aria-label="Expand HUD" className="text-zinc-600 hover:text-zinc-300" title="Expand HUD"><span aria-hidden="true">▲</span></button>
      </Glass>
    );
  }

  return (
    <Glass className="flex items-stretch overflow-hidden">
      <div className="flex items-center gap-3 px-5 border-r border-white/5">
        <div className="relative">
          <div className="size-8 rounded-lg bg-gradient-to-br from-[rgb(var(--brass))] via-[rgb(var(--brass)_/_0.85)] to-[rgb(var(--brass)_/_0.55)] shadow-[0_0_24px_-4px_rgb(var(--brass)_/_0.6)]" />
          <div className="absolute inset-0 flex items-center justify-center font-display text-[14px] font-bold text-zinc-950">V</div>
        </div>
        <div className="flex flex-col leading-none">
          <span className="font-display text-[13px] tracking-[0.22em] text-zinc-100">IMPERIUM</span>
          <span className="text-[9px] uppercase tracking-[0.3em] text-zinc-500">vox · orchestrator</span>
        </div>
      </div>

      <div className="flex min-w-0 flex-1 items-stretch divide-x divide-white/5 overflow-x-auto">
        <KPI
          label="Active Agents"
          value={kpis.activeAgents.value}
          delta={kpis.activeAgents.delta}
          color="text-cyan-300"
          spark={kpis.activeAgents.spark}
          icon={<Icon.users className="size-4" />}
          onClick={() => onNavigate?.('agents')}
        />
        <KPI
          label="Queue Depth"
          value={kpis.queueDepth.value}
          delta={kpis.queueDepth.delta}
          color="text-zinc-300"
          spark={kpis.queueDepth.spark}
          icon={<Icon.scale className="size-4" />}
          onClick={() => onNavigate?.('runs')}
        />
        <KPI
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
        <KPI
          label="Mesh"
          value={kpis.mesh.value}
          unit={kpis.mesh.unit}
          delta={kpis.mesh.delta}
          color="text-violet-300"
          spark={kpis.mesh.spark}
          icon={<Icon.link className="size-4" />}
          sub={`${kpis.mesh.peers} peers online`}
          onClick={() => onNavigate?.('compute')}
        />
      </div>

      <div className="ml-auto flex items-center gap-2 px-4 border-l border-white/5">
        <button
          type="button"
          onClick={onCommand}
          className="group flex items-center gap-2 rounded-lg border border-white/5 bg-white/[0.02] px-3 py-1.5 text-xs text-zinc-400 hover:border-brass/40 hover:text-brass transition"
        >
          <Icon.search className="size-3.5" />
          <span className="font-display tracking-wider hidden sm:inline">Search</span>
          <span className="ml-2 rounded border border-white/10 bg-white/5 px-1.5 py-0.5 text-[9px] tracking-widest text-zinc-500">⌘K</span>
        </button>
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
          className="rounded-md border border-white/5 px-2 py-1 text-[10px] text-zinc-500 hover:text-zinc-200"
          title="Collapse HUD (Ctrl+Shift+H)"
          aria-label="Collapse HUD"
        >
          <span aria-hidden="true">▼</span>
        </button>
      </div>
    </Glass>
  );
}
