import React from 'react';
import { Icon } from '../../ui/Icons';
import { Pill } from '../../ui/Pill';
import { StreamItem } from '../../../types/dashboard';

interface StreamCardProps {
  item: StreamItem;
  onDoubt?: (item: StreamItem) => void;
  onOverrule?: (item: StreamItem) => void;
}

export function StreamCard({ item, onDoubt, onOverrule }: StreamCardProps) {
  const toneMap: Record<string, { phase: string; icon: React.ReactNode; bar: string }> = {
    validated:   { phase: "Validated",   icon: <Icon.check className="size-3.5" />, bar: "from-emerald-400/40 to-emerald-400/0" },
    "in-progress": { phase: "Executing", icon: <Icon.bolt className="size-3.5" />,  bar: "from-brass/40 to-brass/0" },
    doubted:     { phase: "Doubted",     icon: <Icon.doubt className="size-3.5" />, bar: "from-amber-400/40 to-amber-400/0" },
    speculative: { phase: "Speculative", icon: <Icon.spark className="size-3.5" />, bar: "from-violet-400/40 to-violet-400/0" },
  };
  const tone = toneMap[item.kind] || toneMap.speculative;

  return (
    <div className="group relative overflow-hidden rounded-xl border border-white/5 bg-white/[0.02] p-3.5 transition hover:border-white/15 hover:bg-white/[0.035] hover:translate-y-[-1px]">
      <div className={`pointer-events-none absolute inset-y-0 left-0 w-[3px] bg-gradient-to-b ${tone.bar}`} />
      <div className="flex items-start justify-between gap-3">
        <div className="flex min-w-0 flex-col gap-1.5">
          <div className="flex items-center gap-2">
            <Pill phase={tone.phase} />
            <span className="font-mono text-[10px] text-zinc-500">{item.id}</span>
            <span className="text-[10px] text-zinc-600">·</span>
            <span className="font-display text-[10px] tracking-widest uppercase text-zinc-500">{item.tag}</span>
          </div>
          <div className="font-display text-[14px] font-medium tracking-tight text-zinc-100">{item.title}</div>
          <div className="text-[12px] leading-relaxed text-zinc-400">{item.body}</div>
        </div>
        <div className="flex shrink-0 flex-col items-end gap-2">
          <span className="font-mono text-[10px] text-zinc-500">{item.ts}</span>
          <div className="flex items-center gap-1 opacity-0 transition group-hover:opacity-100">
            {item.kind !== "doubted" && (
              <button onClick={() => onDoubt?.(item)} className="rounded-md border border-white/5 bg-white/[0.02] p-1.5 text-zinc-400 hover:border-amber-400/30 hover:text-amber-300 transition" title="Doubt">
                <Icon.doubt className="size-3.5" />
              </button>
            )}
            {item.kind === "doubted" && (
              <button onClick={() => onOverrule?.(item)} className="rounded-md border border-amber-400/20 bg-amber-400/5 p-1.5 text-amber-300 hover:bg-amber-400/15 transition" title="Overrule">
                <Icon.gavel className="size-3.5" />
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
