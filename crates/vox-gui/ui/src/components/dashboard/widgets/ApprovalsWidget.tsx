import React from 'react';
import { Glass } from '../../ui/Glass';
import type { DashboardData } from '../../../types/dashboard';

export function ApprovalsWidget({ data }: { data: DashboardData }) {
  const pending = data.alerts.length;
  return (
    <Glass className="flex h-full flex-col justify-between p-4">
      <span className="font-display text-[11px] uppercase tracking-[0.18em] text-text-muted">Approvals · Doubt</span>
      <span className="font-display text-[28px] font-semibold tabular-nums text-amber-300">{pending}</span>
      <span className="text-[10px] text-text-muted">{pending === 0 ? 'all clear' : 'awaiting you'}</span>
    </Glass>
  );
}
