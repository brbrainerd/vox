import React from 'react';
import { Glass } from '../../ui/Glass';
import type { DashboardData } from '../../../types/dashboard';

export function MeshWidget({ data }: { data: DashboardData }) {
  const online = data.peers.filter((p) => p.online).length;
  return (
    <Glass className="flex h-full flex-col justify-between p-4">
      <span className="font-display text-[11px] uppercase tracking-[0.18em] text-text-muted">Mesh Peers</span>
      <span className="font-display text-[28px] font-semibold tabular-nums text-violet-300">{online}</span>
      <span className="text-[10px] text-text-muted">{online === 0 ? 'no peers online' : 'online now'}</span>
    </Glass>
  );
}
