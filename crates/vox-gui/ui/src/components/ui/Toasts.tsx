import React from 'react';
import { Icon } from './Icons';

export interface ToastItem {
  id: string;
  tone: 'ok' | 'warn' | 'info';
  title: string;
  body?: string;
  cmd?: string;
}

interface ToastsProps {
  items: ToastItem[];
  onClose: (id: string) => void;
}

export function Toasts({ items, onClose }: ToastsProps) {
  return (
    <div className="pointer-events-none fixed bottom-[200px] right-6 z-40 flex w-[320px] flex-col gap-2">
      {items.map(t => (
        <div key={t.id} className="pointer-events-auto rounded-xl border border-white/10 bg-zinc-950/90 p-3 backdrop-blur-xl shadow-[0_24px_60px_-20px_rgba(0,0,0,0.9)] animate-vox-toast-in">
          <div className="flex items-start gap-2">
            <div className={`mt-0.5 flex size-6 shrink-0 items-center justify-center rounded ${t.tone === "ok" ? "bg-emerald-400/15 text-emerald-300" : t.tone === "warn" ? "bg-amber-400/15 text-amber-300" : "bg-cyan-400/15 text-cyan-300"}`}>
              {t.tone === "ok" ? <Icon.check className="size-3.5"/> : t.tone === "warn" ? <Icon.alert className="size-3.5"/> : <Icon.bolt className="size-3.5"/>}
            </div>
            <div className="flex-1 leading-tight">
              <div className="font-display text-[12px] tracking-wide text-zinc-100">{t.title}</div>
              {t.body && <div className="mt-0.5 text-[11px] text-zinc-400">{t.body}</div>}
              {t.cmd && <div className="mt-1 font-mono text-[10px] text-zinc-500">▸ {t.cmd}</div>}
            </div>
            <button onClick={() => onClose(t.id)} className="text-zinc-500 hover:text-zinc-100">
              <Icon.x className="size-3.5"/>
            </button>
          </div>
        </div>
      ))}
    </div>
  );
}
