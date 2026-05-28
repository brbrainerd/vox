import React from 'react';
import { Glass } from '../../ui/Glass';
import { Icon } from '../../ui/Icons';
import { StreamCard } from './StreamCard';
import { AgentRow } from './AgentRow';
import { LudusBanner } from './LudusBanner';
import { DashboardData, Agent, StreamItem, LudusAlert } from '../../../types/dashboard';

interface DashboardProps {
  data: DashboardData;
  onPause: (a: Agent) => void;
  onResume: (a: Agent) => void;
  onDoubt: (item: StreamItem) => void;
  onOverrule: (item: StreamItem) => void;
  onAckLudus: (note: LudusAlert) => void;
  filterKind: string;
  setFilterKind: (k: string) => void;
}

export function Dashboard({ 
  data, 
  onPause, 
  onResume, 
  onDoubt, 
  onOverrule, 
  onAckLudus, 
  filterKind, 
  setFilterKind 
}: DashboardProps) {
  const filters = ["all", "validated", "in-progress", "doubted", "speculative"];
  const stream = data.stream.filter(s => filterKind === "all" ? true : s.kind === filterKind);

  return (
    <div className="grid grid-cols-12 gap-5 p-5">
      {/* Left: The Stream */}
      <Glass className="col-span-12 xl:col-span-8 p-5">
        <div className="flex items-center justify-between">
          <div>
            <div className="flex items-center gap-3">
              <h2 className="font-display text-[18px] font-semibold tracking-tight text-zinc-100">The Stream</h2>
              <span className="rounded-full border border-white/5 bg-white/[0.02] px-2 py-0.5 font-mono text-[10px] text-zinc-500">{stream.length} events</span>
            </div>
            <p className="mt-0.5 text-[11px] text-zinc-500">Mission-control feed · live agent telemetry</p>
          </div>
          <div className="flex gap-1 rounded-lg border border-white/5 bg-white/[0.02] p-1">
            {filters.map(f => (
              <button 
                key={f} 
                onClick={() => setFilterKind(f)} 
                className={`rounded-md px-2.5 py-1 text-[10px] font-display uppercase tracking-wider transition ${
                  filterKind === f ? "bg-white/10 text-zinc-100" : "text-zinc-500 hover:text-zinc-300"
                }`}
              >
                {f === "in-progress" ? "in-prog" : f}
              </button>
            ))}
          </div>
        </div>
        <div className="mt-4 flex flex-col gap-2.5">
          {stream.map(s => <StreamCard key={s.id} item={s} onDoubt={onDoubt} onOverrule={onOverrule} />)}
        </div>
      </Glass>

      {/* Right: Ludus + Active Agents */}
      <div className="col-span-12 xl:col-span-4 flex flex-col gap-5">
        <Glass className="p-5">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Icon.alert className="size-4 text-amber-300" />
              <h3 className="font-display text-[14px] font-semibold tracking-wide text-zinc-100">System · Telemetry & Alerts</h3>
            </div>
            <span className="font-mono text-[10px] text-zinc-500">{data.alerts.length} open</span>
          </div>
          <div className="mt-3 flex flex-col gap-2">
            {data.alerts.map(n => <LudusBanner key={n.id} note={n} onAck={onAckLudus} />)}
          </div>
        </Glass>

        <Glass className="p-5">
          <div className="flex items-center justify-between">
            <h3 className="font-display text-[14px] font-semibold tracking-wide text-zinc-100">Active Agents</h3>
            <span className="font-mono text-[10px] text-zinc-500">{data.agents.length} shards</span>
          </div>
          <div className="mt-3 flex flex-col gap-2">
            {data.agents.map(a => <AgentRow key={a.id} a={a} onPause={onPause} onResume={onResume} />)}
          </div>
        </Glass>
      </div>
    </div>
  );
}
