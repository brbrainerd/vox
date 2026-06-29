import React from 'react';
import { Glass } from '../../ui/Glass';
import type { DashboardData } from '../../../types/dashboard';

export function CoverageWidget({ data }: { data: DashboardData }) {
  // Honest read: surface the queue/agents signal as a build-spine proxy until a
  // dedicated coverage feed is wired; never a fabricated percentage.
  const agents = data.agents.length;
  return (
    <Glass className="flex h-full flex-col justify-between p-4">
      <span className="font-display text-[11px] uppercase tracking-[0.18em] text-text-muted">Coverage · Build-spine</span>
      <span className="font-display text-[28px] font-semibold tabular-nums text-cyan-300">{agents}</span>
      <span className="text-[10px] text-text-muted">active build agents</span>
    </Glass>
  );
}
