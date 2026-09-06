import React from 'react';
import { Icon } from '../../ui/Icons';
import { LudusAlert } from '../../../types/dashboard';

interface LudusBannerProps {
  note: LudusAlert;
  onAck: (note: LudusAlert) => void;
}

export function LudusBanner({ note, onAck }: LudusBannerProps) {
  const stylingMap: Record<string, { ring: string; bg: string; text: string; icon: React.ReactNode }> = {
    ok:     { ring: "ring-emerald-400/25", bg: "bg-linear-to-br from-emerald-500/8 via-emerald-500/2 to-transparent", text: "text-emerald-300", icon: <Icon.check className="size-4"/> },
    warn:   { ring: "ring-amber-400/25", bg: "bg-linear-to-br from-amber-500/8 via-amber-500/2 to-transparent", text: "text-amber-300", icon: <Icon.alert className="size-4"/> },
    info:   { ring: "ring-cyan-400/25", bg: "bg-linear-to-br from-cyan-500/8 via-cyan-500/2 to-transparent", text: "text-cyan-300", icon: <Icon.spark className="size-4"/> },
    error:  { ring: "ring-rose-400/25", bg: "bg-linear-to-br from-rose-500/8 via-rose-500/2 to-transparent", text: "text-rose-300", icon: <Icon.alert className="size-4"/> },
  };
  const styling = stylingMap[note.level] || { ring: "ring-white/10", bg: "", text: "text-text-secondary", icon: <Icon.alert className="size-4"/> };

  return (
    <div className={`relative overflow-hidden rounded-xl ring-1 ${styling.ring} ${styling.bg} p-3`}>
      <div className="flex items-start gap-3">
        <div className={`flex size-8 shrink-0 items-center justify-center rounded-lg bg-overlay-subtle ring-1 ring-white/5 ${styling.text}`}>{styling.icon}</div>
        <div className="min-w-0 flex-1">
          <div className={`font-display text-[12px] font-medium tracking-wide ${styling.text}`}>{note.title}</div>
          <div className="mt-0.5 text-[11px] leading-relaxed text-text-muted">{note.body}</div>
        </div>
        <button type="button" aria-label="Acknowledge alert" onClick={() => onAck(note)} className="rounded-md border border-border-subtle bg-overlay-subtle p-1 text-text-muted hover:text-text-secondary transition" title="Ack">
          <Icon.x className="size-3.5" aria-hidden="true" />
        </button>
      </div>
    </div>
  );
}
