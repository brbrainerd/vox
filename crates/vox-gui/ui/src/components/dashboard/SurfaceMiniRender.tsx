import React from 'react';
import { Glass } from '../ui/Glass';

export interface SurfaceMiniRenderProps {
  surfaceKey: string;
  label: string;
  /** The real surface node (produced by childRenderer in the Dashboard). */
  children: React.ReactNode;
  /** Visual scale of the embedded surface; default 0.6 (a thumbnail). */
  scale?: number;
}

/**
 * A live, compact thumbnail of a real surface component. It mounts the genuine
 * surface output (honesty: never a fabricated value), scaled down and
 * scroll-clipped.
 *
 * Honest scope of "inertness":
 *  - INPUT is blocked: `pointer-events-none` disables clicks, and the parent
 *    passes INERT no-op action callbacks (no live onPause/onResume/onDoubt/
 *    onOverrule/onAckLudus/pushToast), so the thumbnail cannot mutate state.
 *  - It is NOT fully passive: mounting the real surface issues that surface's
 *    own read-only mount polls (e.g. status/`vox_pending_approvals` fetches).
 *    These are read-only and harmless, but they are real network calls — this
 *    is a monitor, not a frozen snapshot.
 *  ponytail: per-surface poll-gating (an `embedded`/`compact` prop that skips
 *    intervals) is a follow-up; no surface honors such a flag today.
 *
 * Click-through to the full, interactive surface is the parent's job (onOpen).
 */
export function SurfaceMiniRender({ surfaceKey, label, children, scale = 0.6 }: SurfaceMiniRenderProps) {
  return (
    <Glass className="flex h-full min-h-0 flex-col overflow-hidden">
      <div className="flex items-center justify-between border-b border-border-subtle px-3 py-1.5">
        <span className="font-display text-[11px] uppercase tracking-[0.18em] text-text-muted">{label}</span>
        <span className="rounded border border-border-subtle bg-overlay-subtle px-1.5 py-0.5 font-mono text-[9px] text-text-muted">live</span>
      </div>
      <div
        data-testid={`surface-mini-${surfaceKey}`}
        data-compact="true"
        aria-hidden="true"
        className="relative min-h-0 flex-1 overflow-hidden"
      >
        <div
          className="pointer-events-none origin-top-left"
          style={{ transform: `scale(${scale})`, width: `${100 / scale}%`, height: `${100 / scale}%` }}
        >
          {children}
        </div>
      </div>
    </Glass>
  );
}
