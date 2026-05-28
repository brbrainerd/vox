import React from 'react';
import { Icon } from '../../ui/Icons';
import { LudusAlert } from '../../../types/dashboard';

interface LudusBannerProps {
  note: LudusAlert;
  onAck: (note: LudusAlert) => void;
}

export function LudusBanner({ note, onAck }: LudusBannerProps) {
  const stylingMap: Record<string, { ring: string; bg: string; text: string; icon: React.ReactNode }> = {
    ok:     { ring: "ring-emerald-400/25", bg: "bg-gradient-to-br from-emerald-500/[0.08] via-emerald-500/[0.02] to-transparent", text: "text-emerald-300", icon: <Icon.check className="size-4"/> },
    warn:   { ring: "ring-amber-400/25", bg: "bg-gradient-to-br from-amber-500/[0.08] via-amber-500/[0.02] to-transparent", text: "text-amber-300", icon: <Icon.alert className="size-4"/> },
    info:   { ring: "ring-cyan-400/25", bg: "bg-gradient-to-br from-cyan-500/[0.08] via-cyan-500/[0.02] to-transparent", text: "text-cyan-300", icon: <Icon.spark className="size-4"/> },
  };
  const styling = stylingMap[note.level] || { ring: "ring-white/10", bg: "", text: "text-zinc-300", icon: <Icon.alert className="size-4"/> };

  return (
    <div className={`relative overflow-hidden rounded-xl ring-1 ${styling.ring} ${styling.bg} p-3`}>
      <div className="flex items-start gap-3">
        <div className={`flex size-8 shrink-0 items-center justify-center rounded-lg bg-white/[0.04] ring-1 ring-white/5 ${styling.text}`}>{styling.icon}</div>
        <div className="min-w-0 flex-1">
          <div className={`font-display text-[12px] font-medium tracking-wide ${styling.text}`}>{note.title}</div>
          <div className="mt-0.5 text-[11px] leading-relaxed text-zinc-400">{note.body}</div>
        </div>
        <button onClick={() => onAck(note)} className="rounded-md border border-white/5 bg-white/[0.03] p-1 text-zinc-500 hover:text-zinc-200 transition" title="Ack">
          <Icon.x className="size-3.5" />
        </button>
      </div>
    </div>
  );
}
