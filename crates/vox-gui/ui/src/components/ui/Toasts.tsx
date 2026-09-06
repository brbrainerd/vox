import React from 'react';
import { Icon } from './Icons';
import type { ToastCause } from '../../types/tauri';

export interface ToastItem {
  id: string;
  tone: 'ok' | 'warn' | 'info';
  title: string;
  body?: string;
  cmd?: string;
  cause: ToastCause;
  /** Coalescing identity — see `Toast.groupKey`. Not rendered directly. */
  groupKey?: string;
  /** When > 1, this entry represents multiple coalesced toasts. */
  count?: number;
}

// DS token classes from STATUS_TONE (tokens.ts) — ok=emerald, warn=amber, info=sky
const TONE_ICON_CLASS: Record<ToastItem['tone'], string> = {
  ok:   'bg-emerald-400/15 text-emerald-300',
  warn: 'bg-amber-400/15 text-amber-300',
  info: 'bg-sky-400/15 text-sky-300',
};

interface ToastsProps {
  items: ToastItem[];
  onClose: (id: string) => void;
}

export function Toasts({ items, onClose }: ToastsProps) {
  return (
    <div
      aria-live="polite"
      aria-atomic="false"
      role="status"
      className="pointer-events-none fixed bottom-20 right-6 z-40 flex w-[320px] flex-col gap-2"
    >
      {items.map(t => (
        <div key={t.id} className="pointer-events-auto rounded-xl border border-border-subtle bg-bg-base/90 p-3 backdrop-blur-xl shadow-[0_24px_60px_-20px_rgba(0,0,0,0.9)] animate-vox-toast-in">
          <div className="flex items-start gap-2">
            <div className={`mt-0.5 flex size-6 shrink-0 items-center justify-center rounded-sm ${TONE_ICON_CLASS[t.tone]}`}>
              {t.tone === "ok" ? <Icon.check className="size-3.5" aria-hidden="true"/> : t.tone === "warn" ? <Icon.alert className="size-3.5" aria-hidden="true"/> : <Icon.bolt className="size-3.5" aria-hidden="true"/>}
            </div>
            <div className="flex-1 leading-tight">
              <div className="font-display text-[12px] tracking-wide text-text-primary">
                {t.title}
                {(t.count ?? 1) > 1 && (
                  <span className="ml-1.5 rounded-full bg-white/10 px-1.5 py-0.5 text-[10px] font-normal text-text-muted">
                    ×{t.count}
                  </span>
                )}
              </div>
              {t.body && <div className="mt-0.5 text-[11px] text-text-muted">{t.body}</div>}
              {t.cmd && <div className="mt-1 font-mono text-[10px] text-text-muted">▸ {t.cmd}</div>}
            </div>
            <button
              type="button"
              onClick={() => onClose(t.id)}
              aria-label="Dismiss notification"
              className="text-text-muted hover:text-text-primary"
            >
              <Icon.x className="size-3.5" aria-hidden="true"/>
            </button>
          </div>
        </div>
      ))}
    </div>
  );
}
