import React, { useState } from 'react';
import { EmptyState } from '../../ui/EmptyState';
import {
  updatePlanNode,
  insertPlanNode,
  approvePlanNodes,
  type PlanNodeStatus,
  type PlanNodeDto,
} from '../../../transport';

export type { PlanNodeStatus } from '../../../transport';
export type PlanNodeView = PlanNodeDto;

interface PlanPanelProps {
  planSessionId: string | null | undefined;
  planVersion: number | null | undefined;
  nodes: PlanNodeView[];
  /** Called when the user discards a still-unapproved plan (Task F2). No
   *  backend primitive for "discard" exists — the blocked nodes simply never
   *  dispatch — so this is a local dismissal the parent wires to clearing its
   *  own `planSessionId`/`planVersion` state. */
  onDiscard?: () => void;
}

const STATUS_ICON: Record<PlanNodeStatus, string> = {
  pending: '○',
  queued: '◔',
  in_progress: '◑',
  completed: '●',
  failed: '✕',
  cancelled: '−',
  superseded: '»',
  blocked_on_approval: '⏸',
};

const STATUS_COLOR: Record<PlanNodeStatus, string> = {
  pending: 'text-text-muted',
  queued: 'text-cyan-400',
  in_progress: 'text-amber-400',
  completed: 'text-emerald-400',
  failed: 'text-red-400',
  cancelled: 'text-text-muted line-through',
  superseded: 'text-text-muted line-through',
  blocked_on_approval: 'text-amber-400',
};

const EDITABLE_STATUSES = new Set<PlanNodeStatus>(['pending']);

function PlanNodeRow({
  node,
  planSessionId,
  planVersion,
}: {
  node: PlanNodeView;
  planSessionId: string;
  planVersion: number;
}) {
  const editable = EDITABLE_STATUSES.has(node.status);
  const [value, setValue] = useState(node.description);

  const commit = () => {
    if (value === node.description) return;
    void updatePlanNode(planSessionId, planVersion, node.node_id, value);
  };

  return (
    <div
      className="flex items-center gap-2 py-1 text-[12px]"
      data-testid={`plan-node-${node.node_id}`}
    >
      <span aria-hidden="true" className={`w-4 shrink-0 text-center ${STATUS_COLOR[node.status]}`}>
        {STATUS_ICON[node.status]}
      </span>
      {editable ? (
        <input
          aria-label={`Edit step: ${node.description}`}
          className="flex-1 rounded border border-transparent bg-transparent px-1 text-text-secondary hover:border-border-subtle focus:border-brass/40 focus:outline-none"
          value={value}
          onChange={e => setValue(e.target.value)}
          onBlur={commit}
          onKeyDown={e => {
            if (e.key === 'Enter') (e.target as HTMLInputElement).blur();
          }}
        />
      ) : (
        <span className={`flex-1 ${node.status === 'cancelled' || node.status === 'superseded' ? 'text-text-muted line-through' : 'text-text-secondary'}`}>
          {node.description}
        </span>
      )}
    </div>
  );
}

export function PlanPanel({ planSessionId, planVersion, nodes, onDiscard }: PlanPanelProps) {
  const [adding, setAdding] = useState(false);
  const [newDescription, setNewDescription] = useState('');
  const [approving, setApproving] = useState(false);

  if (!planSessionId || planVersion == null) {
    return (
      <div data-testid="plan-panel" className="p-2">
        <EmptyState
          title="No to-dos yet"
          description="Start a task to see its to-do list here."
        />
      </div>
    );
  }

  const sessionId = planSessionId;
  const version = planVersion;
  const blockedCount = nodes.filter(n => n.status === 'blocked_on_approval').length;

  const submitNew = () => {
    if (!newDescription.trim()) return;
    void insertPlanNode(sessionId, version, `n-${crypto.randomUUID()}`, newDescription, []);
    setNewDescription('');
    setAdding(false);
  };

  const approve = async () => {
    setApproving(true);
    try {
      await approvePlanNodes(sessionId);
    } finally {
      setApproving(false);
    }
  };

  return (
    <div className="flex flex-col gap-1 p-2" data-testid="plan-panel">
      {nodes.length === 0 ? (
        <p className="text-[11px] text-text-muted">Nothing to do yet.</p>
      ) : (
        nodes.map(n => (
          <PlanNodeRow key={n.node_id} node={n} planSessionId={sessionId} planVersion={version} />
        ))
      )}
      {blockedCount > 0 && (
        <div
          data-testid="plan-approval-footer"
          className="mt-1 flex items-center justify-between gap-2 rounded border border-amber-400/40 bg-amber-400/10 px-2 py-1 text-[11px]"
        >
          <span className="text-amber-400">
            {blockedCount} {blockedCount === 1 ? 'step' : 'steps'} awaiting approval
          </span>
          <div className="flex gap-2">
            <button
              type="button"
              onClick={() => void approve()}
              disabled={approving}
              className="text-emerald-400 hover:text-emerald-300 disabled:opacity-50"
            >
              Approve
            </button>
            <button
              type="button"
              onClick={() => onDiscard?.()}
              className="text-text-muted hover:text-red-400"
            >
              Discard
            </button>
          </div>
        </div>
      )}
      {adding ? (
        <input
          autoFocus
          className="mt-1 rounded border border-border-subtle bg-transparent px-1 text-[12px] text-text-secondary"
          placeholder="new step…"
          value={newDescription}
          onChange={e => setNewDescription(e.target.value)}
          onKeyDown={e => {
            if (e.key === 'Enter') submitNew();
            if (e.key === 'Escape') setAdding(false);
          }}
          onBlur={submitNew}
        />
      ) : (
        <button
          type="button"
          onClick={() => setAdding(true)}
          className="mt-1 self-start text-[11px] text-text-muted hover:text-brass"
        >
          + Add step
        </button>
      )}
    </div>
  );
}
