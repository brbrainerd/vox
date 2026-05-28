import React, { useState } from 'react';
import { Glass } from '../../ui/Glass';
import { Pill } from '../../ui/Pill';

function HexCell({ intention, onSelect, selected }: any) {
  const conf = intention.conf;
  const phaseToneMap: Record<string, any> = {
    Validated:   { stroke: "#34d399", fill: "rgba(52,211,153," + (0.06 + conf*0.18) + ")", text: "text-emerald-300", glow: "#34d399" },
    Active:      { stroke: "#22d3ee", fill: "rgba(34,211,238," + (0.06 + conf*0.18) + ")", text: "text-cyan-300",    glow: "#22d3ee" },
    Doubted:     { stroke: "#fbbf24", fill: "rgba(251,191,36," + (0.06 + conf*0.18) + ")", text: "text-amber-300",   glow: "#fbbf24" },
    Speculative: { stroke: "#a78bfa", fill: "rgba(167,139,250," + (0.06 + conf*0.18) + ")", text: "text-violet-300", glow: "#a78bfa" },
  };
  const phaseTone = phaseToneMap[intention.phase] || phaseToneMap.Active;
  const pulseDur = (3.5 - conf * 2).toFixed(2) + "s";

  return (
    <button
      onClick={() => onSelect(intention.id)}
      className="group relative aspect-[1/1.05] [clip-path:polygon(50%_0,100%_25%,100%_75%,50%_100%,0_75%,0_25%)] focus:outline-none"
      style={{ background: phaseTone.fill, boxShadow: `inset 0 0 0 1px ${phaseTone.stroke}40` }}
    >
      <div className="absolute inset-0 [clip-path:polygon(50%_0,100%_25%,100%_75%,50%_100%,0_75%,0_25%)] opacity-60" style={{ background: `radial-gradient(circle at center, ${phaseTone.glow}33, transparent 70%)` }} />
      <div className="relative flex h-full flex-col items-center justify-center px-4 text-center">
        <div className="font-mono text-[9px] uppercase tracking-[0.2em] text-zinc-500">{intention.parent} · {intention.id}</div>
        <div className={`mt-1 font-display text-[13px] font-semibold tracking-tight ${phaseTone.text}`}>{intention.branch}</div>
        <div className="mt-1.5 font-display text-[22px] font-bold tabular-nums text-zinc-100">{Math.round(conf*100)}<span className="text-[12px] text-zinc-500">%</span></div>
      </div>
      {selected && (
        <div className="absolute inset-0 [clip-path:polygon(50%_0,100%_25%,100%_75%,50%_100%,0_75%,0_25%)] ring-2 ring-inset" style={{ boxShadow: `inset 0 0 0 2px ${phaseTone.stroke}` }} />
      )}
    </button>
  );
}

export function Matrix({ intentions = [], onDoubt, onOverrule }: any) {
  const [sel, setSel] = useState(intentions[0]?.id);
  const active = intentions.find((i: any) => i.id === sel) || intentions[0];

  if (!active) return (
    <div className="p-8 text-center">
        <Glass className="p-12 inline-block">
            <p className="font-display uppercase tracking-[0.2em] text-zinc-500">No routing policies active</p>
        </Glass>
    </div>
  );

  const groups: Record<string, any[]> = {};
  intentions.forEach((i: any) => { (groups[i.parent] = groups[i.parent] || []).push(i); });

  return (
    <div className="grid grid-cols-12 gap-5 p-5">
      <Glass className="col-span-12 xl:col-span-8 p-5">
        <div className="flex items-center justify-between">
          <div>
            <h2 className="font-display text-[18px] font-semibold tracking-tight text-zinc-100">Routing Policies</h2>
            <p className="mt-0.5 text-[11px] text-zinc-500">Orchestrator decision modules · speculative branches</p>
          </div>
        </div>
        <div className="mt-5 space-y-6">
          {Object.entries(groups).map(([parent, items]) => (
            <div key={parent}>
              <div className="mb-2 flex items-center gap-2 font-mono text-[10px] uppercase tracking-[0.2em] text-zinc-500">
                <span className="h-px flex-1 bg-gradient-to-r from-white/10 to-transparent" />
                <span className="text-zinc-400">{parent}</span>
                <span className="h-px flex-1 bg-gradient-to-l from-white/10 to-transparent" />
              </div>
              <div className="grid grid-cols-2 gap-3 sm:grid-cols-3">
                {items.map(i => <HexCell key={i.id} intention={i} onSelect={setSel} selected={sel === i.id} />)}
              </div>
            </div>
          ))}
        </div>
      </Glass>

      <Glass className="col-span-12 xl:col-span-4 p-5">
        <div className="flex items-center justify-between">
          <h3 className="font-display text-[14px] font-semibold tracking-wide text-zinc-100">Branch Inspector</h3>
          <Pill phase={active.phase} />
        </div>
        <div className="mt-3 rounded-xl border border-white/5 bg-white/[0.02] p-4">
          <div className="font-mono text-[10px] uppercase tracking-[0.2em] text-zinc-500">{active.parent} · {active.id}</div>
          <div className="mt-1 font-display text-[20px] font-semibold tracking-tight text-zinc-50">{active.branch}</div>
          <div className="mt-2 text-[12px] leading-relaxed text-zinc-400">{active.note}</div>
          <div className="mt-4">
            <div className="flex items-center justify-between text-[10px] uppercase tracking-[0.2em] text-zinc-500">
              <span>Confidence</span><span className="font-mono text-zinc-300">{Math.round(active.conf*100)}%</span>
            </div>
            <div className="mt-1.5 h-2 overflow-hidden rounded-full bg-white/5">
              <div className="h-full rounded-full bg-gradient-to-r from-violet-400 via-cyan-400 to-emerald-400" style={{ width: `${active.conf*100}%` }} />
            </div>
          </div>
          <div className="mt-4 flex gap-2">
            <button onClick={() => onOverrule(active)} className="flex-1 rounded-md border border-emerald-400/30 bg-emerald-400/10 px-3 py-2 font-display text-[11px] uppercase tracking-[0.18em] text-emerald-300 hover:bg-emerald-400/20 transition">Promote</button>
            <button onClick={() => onDoubt(active)}   className="flex-1 rounded-md border border-amber-400/30 bg-amber-400/10 px-3 py-2 font-display text-[11px] uppercase tracking-[0.18em] text-amber-300 hover:bg-amber-400/20 transition">Doubt</button>
          </div>
        </div>
      </Glass>
    </div>
  );
}
