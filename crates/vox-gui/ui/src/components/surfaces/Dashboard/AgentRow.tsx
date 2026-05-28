import React from 'react';
import { Icon } from '../../ui/Icons';
import { Pill, PHASE_TONE, PhaseKind } from '../../ui/Pill';
import { Agent } from '../../../types/dashboard';

interface AgentRowProps {
  a: Agent;
  onPause: (a: Agent) => void;
  onResume: (a: Agent) => void;
}

export function AgentRow({ a, onPause, onResume }: AgentRowProps) {
  const t = PHASE_TONE[a.phase as PhaseKind] || PHASE_TONE.Paused;
  const pct = Math.round(a.progress * 100);
  const bp = (a.cost / a.budget) * 100;

  return (
    <div className="group relative rounded-xl border border-white/5 bg-white/[0.015] p-3 transition hover:border-white/10 hover:bg-white/[0.03]">
      <div className="flex items-center gap-3">
        <div className={`relative flex size-9 items-center justify-center rounded-lg bg-white/[0.04] ring-1 ${t.ring} ${t.glow}`}>
          <span className={`font-display text-[11px] font-bold tracking-wider ${t.text}`}>{a.id.replace("A-","")}</span>
        </div>
        <div className="flex min-w-0 flex-1 flex-col">
          <div className="flex items-center gap-2">
            <span className="font-display text-[12px] font-medium tracking-wide text-zinc-100">{a.codename}</span>
            <Pill phase={a.phase} className="scale-90 origin-left" />
          </div>
          <div className="mt-0.5 truncate text-[11px] text-zinc-500">{a.task}</div>
        </div>
        <div className="flex items-center gap-3">
          <div className="text-right">
            <div className="font-mono text-[11px] tabular-nums text-zinc-200">
              ${a.cost.toFixed(2)}
              <span className="text-zinc-600"> / ${a.budget.toFixed(2)}</span>
            </div>
            <div className="font-mono text-[10px] text-zinc-500">eta {a.eta}</div>
          </div>
          <button 
            onClick={() => (a.phase === "Paused" ? onResume(a) : onPause(a))} 
            className="rounded-md border border-white/5 bg-white/[0.02] p-1.5 text-zinc-400 hover:border-white/20 hover:text-zinc-100 transition"
          >
            {a.phase === "Paused" ? <Icon.play className="size-3.5" /> : <Icon.pause className="size-3.5" />}
          </button>
        </div>
      </div>
      <div className="mt-2.5 flex items-center gap-2">
        <div className="relative h-1 flex-1 overflow-hidden rounded-full bg-white/5">
          <div 
            className={`absolute inset-y-0 left-0 rounded-full ${
              a.phase === "Verifying" ? "bg-violet-400" : 
              a.phase === "Executing" ? "bg-brass" : 
              a.phase === "Planning" ? "bg-cyan-400" : "bg-zinc-500"
            }`} 
            style={{ width: `${pct}%` }} 
          />
          <div className="absolute inset-y-0 left-0 w-full bg-[linear-gradient(90deg,transparent,rgba(255,255,255,0.18),transparent)] animate-vox-shimmer" style={{ width: `${pct}%` }} />
        </div>
        <span className="w-9 text-right font-mono text-[10px] text-zinc-500 tabular-nums">{pct}%</span>
        <div className="ml-1 h-1 w-12 overflow-hidden rounded-full bg-white/5">
          <div 
            className={`h-full ${bp > 80 ? "bg-rose-400" : bp > 50 ? "bg-amber-400" : "bg-emerald-400"}`} 
            style={{ width: `${Math.min(100, bp)}%` }} 
          />
        </div>
      </div>
    </div>
  );
}
