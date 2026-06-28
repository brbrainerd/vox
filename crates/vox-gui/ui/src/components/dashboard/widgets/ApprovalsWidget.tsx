import React from 'react';
import { Glass } from '../../ui/Glass';
import { useAgentApprovals } from '../../../hooks/useAgentApprovals';

/**
 * Pending-approvals count, sourced HONESTLY from the real `vox_pending_approvals`
 * feed (via useAgentApprovals) — NOT from `status.alerts` (LudusAlert[], which is
 * gamification, not approvals). The empty `agentKeys` arg is intentional: this
 * widget only needs the total count, not per-agent mapping.
 */
export function ApprovalsWidget() {
  const { count } = useAgentApprovals([]);
  return (
    <Glass className="flex h-full flex-col justify-between p-4">
      <span className="font-display text-[11px] uppercase tracking-[0.18em] text-text-muted">Approvals · Doubt</span>
      <span className="font-display text-[28px] font-semibold tabular-nums text-amber-300">{count}</span>
      <span className="text-[10px] text-text-muted">{count === 0 ? 'all clear' : 'awaiting you'}</span>
    </Glass>
  );
}
