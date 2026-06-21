import React from 'react';
import { Glass } from '../../ui/Glass';
import { useMemoryStatus } from '../../../hooks/useMemoryStatus';
import type { DashboardData } from '../../../types/dashboard';

function compact(n: number): string {
  if (n >= 1000) return `${(n / 1000).toFixed(1)}k`;
  return String(n);
}

function ResourceCard({ label, value, tone, status, note }: {
  label: string; value: string; tone: string; status: string; note: string;
}) {
  return (
    <Glass size="sm">
      <div className="flex flex-col gap-2.5">
        <div className="flex items-center justify-between gap-2.5">
          <span className="font-display text-[11px] uppercase tracking-[0.18em] text-text-secondary">{label}</span>
          <span className="inline-flex items-center gap-1.5 rounded-full bg-overlay-subtle px-2 py-0.5 text-[10px] font-medium uppercase tracking-wider ring-1 ring-border-subtle" style={{ color: tone }}>
            <span className="size-1.5 rounded-full" style={{ background: tone }} />{status}
          </span>
        </div>
        <div className="font-display text-[22px] font-semibold tabular-nums text-text-primary">{value}</div>
        <div className="font-serif text-[13px] italic text-text-muted">{note}</div>
      </div>
    </Glass>
  );
}

export function ResourcesWidget({ data }: { data: DashboardData }) {
  const mem = useMemoryStatus();
  const onlinePeers = data.peers.filter((p) => p.online).length;
  const budget = data.kpis.budgetBurn;
  const budgetValue = typeof budget.value === 'number' ? `$${budget.value.toFixed(2)} / $${budget.cap}` : String(budget.value);
  const overBudget = typeof budget.value === 'number' && budget.cap > 0 && budget.value / budget.cap > 0.5;

  return (
    <Glass className="h-full p-5">
      <div className="mb-3.5 border-b border-border-subtle pb-1.5 font-display text-[10px] uppercase tracking-[0.3em] text-text-muted">Resources</div>
      <div className="grid grid-cols-1 gap-3.5 md:grid-cols-3">
        <ResourceCard label="Compute Mesh" value={`${onlinePeers} ${onlinePeers === 1 ? 'peer' : 'peers'}`}
          tone="var(--color-accent-secondary)" status={onlinePeers > 0 ? 'Live' : 'Offline'} note={onlinePeers > 0 ? 'mesh quorum holding' : 'no peers online'} />
        <ResourceCard label="Vector Store"
          value={mem.loading ? '…' : mem.vectorCount == null ? '—' : compact(mem.vectorCount)}
          tone="var(--color-status-pass)" status={mem.error ? 'Unavailable' : 'Synced'}
          note={mem.error ? 'memory db not reachable' : 'documents indexed'} />
        <ResourceCard label="Token Budget" value={budgetValue}
          tone={overBudget ? 'var(--color-status-warn)' : 'var(--color-accent-secondary)'}
          status={overBudget ? 'Watch' : 'OK'} note={overBudget ? 'burn rate elevated' : 'within budget'} />
      </div>
    </Glass>
  );
}
