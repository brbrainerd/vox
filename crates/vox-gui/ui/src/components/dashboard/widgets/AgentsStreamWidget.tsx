import React from 'react';
import { Glass } from '../../ui/Glass';
import type { DashboardData } from '../../../types/dashboard';

export function AgentsStreamWidget({ data }: { data: DashboardData }) {
  return (
    <Glass className="flex h-full min-h-0 flex-col p-4">
      <div className="flex items-center justify-between">
        <span className="font-display text-[11px] uppercase tracking-[0.18em] text-text-muted">Agent Runs</span>
        <span className="font-mono text-[10px] text-text-muted">{data.agents.length} active</span>
      </div>
      <div className="mt-2 flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto">
        {data.agents.length === 0 ? (
          <div role="status" className="rounded-lg border border-dashed border-border-subtle py-4 text-center text-[11px] text-text-muted">
            No active agents
          </div>
        ) : (
          data.agents.slice(0, 6).map((a) => (
            <div key={a.id} className="flex items-center justify-between rounded-sm border border-border-subtle bg-overlay-subtle px-2 py-1 text-[11px]">
              <span className="truncate text-text-secondary">{a.codename}</span>
              <span className="font-mono text-[10px] text-text-muted">{a.phase}</span>
            </div>
          ))
        )}
      </div>
    </Glass>
  );
}
