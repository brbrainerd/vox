import React from 'react';
import { Glass } from '../../ui/Glass';

/**
 * Operator Console — the Vox Axis fleet console, implemented from the
 * Claude Design "Operator Console" template (templates/operator-console) but
 * rendered with the app's own Limes primitives so it inherits the existing
 * sidebar, shell, and fonts. Section dividers underline their heading (never
 * cap from above); gold is the single primary accent, verdigris the live one.
 */

type Tone = 'accent' | 'pass' | 'warn' | 'fail' | 'neutral';

const TONE_COLOR: Record<Tone, string> = {
  accent: 'var(--color-accent-secondary)',
  pass: 'var(--color-status-pass)',
  warn: 'var(--color-status-warn)',
  fail: 'var(--color-status-fail)',
  neutral: 'var(--color-text-muted)',
};

function StatusChip({ tone, label }: { tone: Tone; label: string }) {
  const color = TONE_COLOR[tone];
  return (
    <span
      className="inline-flex items-center gap-1.5 rounded-full bg-overlay-subtle px-2 py-0.5 text-[10px] font-medium uppercase tracking-wider ring-1 ring-border-subtle"
      style={{ color }}
    >
      <span className="size-1.5 rounded-full" style={{ background: color }} />
      {label}
    </span>
  );
}

function ProgressBar({ pct, tone }: { pct: number; tone: Tone }) {
  return (
    <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-overlay-subtle">
      <div
        className="h-full rounded-full"
        style={{ width: `${pct}%`, background: TONE_COLOR[tone] }}
      />
    </div>
  );
}

function SectionHead({ children }: { children: React.ReactNode }) {
  return (
    <div className="mb-3.5 border-b border-border-subtle pb-1.5 font-display text-[10px] uppercase tracking-[0.3em] text-text-muted">
      {children}
    </div>
  );
}

function Kpi({ label, value, delta }: { label: string; value: string; delta?: number }) {
  const up = delta != null && delta >= 0;
  return (
    <div className="flex flex-col">
      <span className="text-[10px] uppercase tracking-[0.18em] text-text-muted">{label}</span>
      <div className="mt-1 flex items-baseline gap-1.5">
        <span className="font-display text-[20px] font-semibold tabular-nums text-text-primary">{value}</span>
        {delta != null && (
          <span
            className="text-[10px] tabular-nums"
            style={{ color: up ? 'var(--color-accent-secondary)' : 'var(--color-status-fail)' }}
          >
            {up ? '▲' : '▼'} {Math.abs(delta)}
          </span>
        )}
      </div>
    </div>
  );
}

interface AgentRow {
  name: string;
  task: string;
  tone: Tone;
  status: string;
  pct: number;
  pending: boolean;
}

const AGENTS: AgentRow[] = [
  { name: 'Atlas', task: 'refactor · auth-service', tone: 'accent', status: 'Live', pct: 68, pending: true },
  { name: 'Surveyor', task: 'migrate · db-schema', tone: 'pass', status: 'Passed', pct: 100, pending: false },
  { name: 'Groma', task: 'write · integration-tests', tone: 'warn', status: 'Doubted', pct: 42, pending: true },
  { name: 'Limes', task: 'patch · rate-limiter', tone: 'fail', status: 'Failed', pct: 13, pending: false },
  { name: 'Castellum', task: 'index · embeddings', tone: 'neutral', status: 'Paused', pct: 0, pending: false },
];

const RESOURCES: { label: string; value: string; tone: Tone; status: string; note: string }[] = [
  { label: 'Compute Mesh', value: '4 peers', tone: 'accent', status: 'Live', note: 'mesh quorum holding' },
  { label: 'Vector Store', value: '12.4k', tone: 'pass', status: 'Synced', note: 'documents indexed' },
  { label: 'Token Budget', value: '$4.20 / $20', tone: 'warn', status: 'Watch', note: 'burn rate elevated' },
];

function ApprovalButtons() {
  return (
    <div className="flex items-center gap-2">
      <button
        type="button"
        className="rounded-md border border-brass/30 bg-brass/10 px-3 py-1.5 font-display text-[11px] uppercase tracking-[0.18em] text-brass transition hover:bg-brass/20"
      >
        Approve
      </button>
      <button
        type="button"
        className="rounded-md border border-border-subtle px-3 py-1.5 font-display text-[11px] uppercase tracking-[0.18em] text-text-muted transition hover:text-text-secondary"
      >
        Reject
      </button>
    </div>
  );
}

export function OperatorConsole() {
  const [hero, ...rest] = AGENTS;
  return (
    <div className="flex flex-col gap-6 p-5" data-testid="operator-console">
      {/* KPI STRIP */}
      <Glass size="sm" className="w-full">
        <div className="grid grid-cols-2 gap-y-4 sm:grid-cols-4">
          <div className="px-3">
            <Kpi label="Active Agents" value="7" delta={2} />
          </div>
          <div className="px-3 sm:border-l sm:border-border-subtle">
            <Kpi label="Queue Depth" value="12" delta={-3} />
          </div>
          <div className="px-3 sm:border-l sm:border-border-subtle">
            <Kpi label="Budget Burn" value="$4.20" />
          </div>
          <div className="px-3 sm:border-l sm:border-border-subtle">
            <Kpi label="Mesh Peers" value="4" />
          </div>
        </div>
      </Glass>

      {/* RESOURCES */}
      <section>
        <SectionHead>Resources</SectionHead>
        <div className="grid grid-cols-1 gap-3.5 md:grid-cols-3">
          {RESOURCES.map((r) => (
            <Glass key={r.label} size="sm">
              <div className="flex flex-col gap-2.5">
                <div className="flex items-center justify-between gap-2.5">
                  <span className="font-display text-[11px] uppercase tracking-[0.18em] text-text-secondary">{r.label}</span>
                  <StatusChip tone={r.tone} label={r.status} />
                </div>
                <div className="font-display text-[22px] font-semibold tabular-nums text-text-primary">{r.value}</div>
                <div className="font-serif text-[13px] italic text-text-muted">{r.note}</div>
              </div>
            </Glass>
          ))}
        </div>
      </section>

      {/* AGENTS */}
      <section>
        <SectionHead>Agents</SectionHead>

        {/* hero agent */}
        <Glass size="sm" className="relative">
          <span className="vox-tick vox-tick-tl" />
          <span className="vox-tick vox-tick-tr" />
          <div className="flex flex-col gap-4 px-1.5 py-1">
            <div className="flex items-start justify-between gap-4">
              <div>
                <div className="font-display text-[22px] font-semibold uppercase tracking-[0.1em] text-text-primary">{hero.name}</div>
                <div className="mt-1 font-serif text-[14px] italic text-text-muted">{hero.task}</div>
              </div>
              <StatusChip tone={hero.tone} label={hero.status} />
            </div>
            <div className="flex items-center gap-3.5">
              <ProgressBar pct={hero.pct} tone={hero.tone} />
              <div className="min-w-[42px] text-right font-display text-[13px] font-semibold tabular-nums text-text-secondary">{hero.pct}%</div>
            </div>
            {hero.pending && (
              <div className="flex justify-end">
                <ApprovalButtons />
              </div>
            )}
          </div>
        </Glass>

        {/* agent rows */}
        <div className="mt-3.5 flex flex-col gap-3">
          {rest.map((a) => (
            <Glass key={a.name} size="sm">
              <div className="flex flex-col gap-3 px-0.5 lg:flex-row lg:items-center lg:gap-[18px]">
                <div className="lg:min-w-[210px]">
                  <div className="font-display text-[14px] font-semibold uppercase tracking-[0.08em] text-text-primary">{a.name}</div>
                  <div className="mt-0.5 font-serif text-[13px] italic text-text-muted">{a.task}</div>
                </div>
                <div className="lg:min-w-[92px]">
                  <StatusChip tone={a.tone} label={a.status} />
                </div>
                <div className="flex flex-1 items-center gap-3">
                  <ProgressBar pct={a.pct} tone={a.tone} />
                  <div className="min-w-[42px] text-right font-display text-[12px] font-semibold tabular-nums text-text-secondary">{a.pct}%</div>
                </div>
                <div className="flex justify-end lg:min-w-[204px]">
                  {a.pending && <ApprovalButtons />}
                </div>
              </div>
            </Glass>
          ))}
        </div>
      </section>
    </div>
  );
}
