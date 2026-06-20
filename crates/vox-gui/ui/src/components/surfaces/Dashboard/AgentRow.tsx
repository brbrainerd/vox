import React from 'react';
import { Icon } from '../../ui/Icons';
import { Pill, PHASE_TONE, PhaseKind } from '../../ui/Pill';
import { Agent } from '../../../types/dashboard';

interface AgentRowProps {
  a: Agent;
  onPause: (a: Agent) => void;
  onResume: (a: Agent) => void;
  /** Optional: open this agent's live stream in the Console surface. */
  onOpenInConsole?: (a: Agent) => void;
}

export function AgentRow({ a, onPause, onResume, onOpenInConsole }: AgentRowProps) {
  const t = PHASE_TONE[a.phase as PhaseKind] || PHASE_TONE.Paused;
  const pct = a.progress != null ? Math.round(a.progress * 100) : null;
  const bp = a.budget != null && a.budget > 0 ? (a.cost / a.budget) * 100 : null;

  return (
    <div className="group relative rounded-xl border border-border-subtle bg-overlay-subtle p-3 transition hover:border-border-subtle hover:bg-overlay-subtle">
      <div className="flex items-center gap-3">
        <div className={`relative flex size-9 items-center justify-center rounded-lg bg-overlay-subtle ring-1 ${t.ring} ${t.glow}`}>
          <span className={`font-display text-[11px] font-bold tracking-wider ${t.text}`}>{a.id.replace("A-","")}</span>
        </div>
        <div className="flex min-w-0 flex-1 flex-col">
          <div className="flex items-center gap-2">
            <span className="font-display text-[12px] font-medium tracking-wide text-text-primary">{a.codename}</span>
            <Pill phase={a.phase} className="scale-90 origin-left" />
          </div>
          <div className="mt-0.5 truncate text-[11px] text-text-muted">{a.task}</div>
        </div>
        <div className="flex items-center gap-3">
          <div className="text-right">
            <div className="font-mono text-[11px] tabular-nums text-text-secondary">
              ${a.cost.toFixed(2)}
              <span className="text-text-muted">
                {a.budget != null ? ` / $${a.budget.toFixed(2)}` : ' / —'}
              </span>
            </div>
            <div className="font-mono text-[10px] text-text-muted">eta {a.eta}</div>
          </div>
          <button
            type="button"
            aria-label={a.phase === "Paused" ? "Resume agent" : "Pause agent"}
            onClick={() => (a.phase === "Paused" ? onResume(a) : onPause(a))}
            className="rounded-md border border-border-subtle bg-overlay-subtle p-1.5 text-text-muted hover:border-white/20 hover:text-text-primary transition"
          >
            {a.phase === "Paused" ? <Icon.play className="size-3.5" aria-hidden="true" /> : <Icon.pause className="size-3.5" aria-hidden="true" />}
          </button>
          {onOpenInConsole && (
            <button
              type="button"
              onClick={() => onOpenInConsole(a)}
              title="Open in Console"
              aria-label="Open in Console"
              className="rounded-md border border-border-subtle bg-overlay-subtle p-1.5 text-text-muted hover:border-white/20 hover:text-text-primary transition"
            >
              <Icon.command className="size-3.5" aria-hidden="true" />
            </button>
          )}
        </div>
      </div>
      <div className="mt-2.5 flex items-center gap-2">
        <div className="relative h-1 flex-1 overflow-hidden rounded-full bg-overlay-subtle">
          {pct != null ? (
            <>
              <div
                className={`absolute inset-y-0 left-0 rounded-full ${
                  a.phase === "Verifying" ? "bg-violet-400" :
                  a.phase === "Executing" ? "bg-brass" :
                  a.phase === "Planning" ? "bg-accent-secondary" : "bg-text-muted"
                }`}
                style={{ width: `${pct}%` }}
              />
              <div className="absolute inset-y-0 left-0 w-full bg-[linear-gradient(90deg,transparent,rgba(255,255,255,0.18),transparent)] animate-vox-shimmer" style={{ width: `${pct}%` }} />
            </>
          ) : (
            <div className="absolute inset-y-0 left-0 w-1/3 rounded-full bg-text-muted animate-pulse" />
          )}
        </div>
        <span className="w-9 text-right font-mono text-[10px] text-text-muted tabular-nums">
          {pct != null ? `${pct}%` : '…'}
        </span>
        {bp != null && (
          <div className="ml-1 h-1 w-12 overflow-hidden rounded-full bg-overlay-subtle">
            <div
              className={`h-full ${bp > 80 ? "bg-rose-400" : bp > 50 ? "bg-amber-400" : "bg-emerald-400"}`}
              style={{ width: `${Math.min(100, bp)}%` }}
            />
          </div>
        )}
      </div>
    </div>
  );
}
