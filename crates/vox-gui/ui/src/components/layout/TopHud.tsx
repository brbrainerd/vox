import React from 'react';
import { Glass } from '../ui/Glass';
import { Icon } from '../ui/Icons';
import { Sparkline } from '../ui/Sparkline';

interface KPIProps {
  label: string;
  value: string | number;
  unit?: string;
  delta?: number;
  color: string;
  spark: number[];
  icon: React.ReactNode;
  sub?: string;
}

function KPI({ label, value, unit, delta, color, spark, icon, sub }: KPIProps) {
  const deltaPos = delta != null && delta >= 0;
  return (
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
            <span className={`text-[10px] tabular-nums ${deltaPos ? "text-emerald-400" : "text-rose-400"}`}>
              {deltaPos ? "▲" : "▼"} {Math.abs(delta)}
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
}

interface TopHudProps {
  kpis: any;
  onCommand: () => void;
}

export function TopHud({ kpis, onCommand }: TopHudProps) {
  return (
    <Glass className="flex items-stretch overflow-hidden">
      <div className="flex items-center gap-3 px-5 border-r border-white/5">
        <div className="relative">
          <div className="size-8 rounded-lg bg-gradient-to-br from-brass via-amber-500 to-amber-700 shadow-[0_0_24px_-4px_rgba(212,175,55,0.6)]" />
          <div className="absolute inset-0 flex items-center justify-center font-display text-[14px] font-bold text-zinc-950">V</div>
        </div>
        <div className="flex flex-col leading-none">
          <span className="font-display text-[13px] tracking-[0.22em] text-zinc-100">IMPERIUM</span>
          <span className="text-[9px] uppercase tracking-[0.3em] text-zinc-500">vox · orchestrator</span>
        </div>
      </div>

      <div className="flex items-stretch divide-x divide-white/5 overflow-hidden">
        <KPI 
          label="Active Agents" 
          value={kpis.activeAgents.value} 
          delta={kpis.activeAgents.delta} 
          color="text-cyan-300" 
          spark={kpis.activeAgents.spark} 
          icon={<Icon.cpu className="size-4"/>} 
        />
        <KPI 
          label="Queue Depth"   
          value={kpis.queueDepth.value}   
          delta={kpis.queueDepth.delta}   
          color="text-zinc-300" 
          spark={kpis.queueDepth.spark} 
          icon={<Icon.scale className="size-4"/>} 
        />
        <KPI 
          label="Budget Burn"   
          value={`$${kpis.budgetBurn.value.toFixed(2)}`} 
          unit={`/ $${kpis.budgetBurn.cap.toFixed(2)}`} 
          delta={kpis.budgetBurn.delta} 
          color="text-amber-300" 
          spark={kpis.budgetBurn.spark} 
          icon={<Icon.bolt className="size-4"/>} 
          sub={`${Math.round(kpis.budgetBurn.value/kpis.budgetBurn.cap*100)}% of cap`} 
        />
        <KPI 
          label="Mesh" 
          value={kpis.mesh.value} 
          unit={kpis.mesh.unit} 
          delta={kpis.mesh.delta} 
          color="text-violet-300" 
          spark={kpis.mesh.spark} 
          icon={<Icon.cpu className="size-4"/>}
          sub={`${kpis.mesh.peers} peers online`} 
        />
      </div>

      <div className="ml-auto flex items-center gap-2 px-4 border-l border-white/5">
        <button onClick={onCommand} className="group flex items-center gap-2 rounded-lg border border-white/5 bg-white/[0.02] px-3 py-1.5 text-xs text-zinc-400 hover:border-brass/40 hover:text-brass transition">
          <Icon.search className="size-3.5" />
          <span className="font-display tracking-wider hidden sm:inline">Command</span>
          <span className="ml-2 rounded border border-white/10 bg-white/5 px-1.5 py-0.5 text-[9px] tracking-widest text-zinc-500">⌘K</span>
        </button>
        <div className="flex items-center gap-2 rounded-lg border border-emerald-400/20 bg-emerald-400/[0.04] px-2.5 py-1.5">
          <span className="relative inline-block size-1.5 rounded-full bg-emerald-400">
            <span className="absolute inset-0 rounded-full bg-emerald-400 animate-vox-ping" />
          </span>
          <span className="text-[10px] uppercase tracking-[0.2em] text-emerald-300 hidden sm:inline">Live</span>
        </div>
      </div>
    </Glass>
  );
}
