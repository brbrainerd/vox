import React, { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Glass } from '../../ui/Glass';
import { EmptyState } from '../../ui/EmptyState';
import { StatusPill } from '../../ui/StatusPill';
import { DataTable } from '../../ui/DataTable';
import { Button } from '../../ui/Button';
import { Icon } from '../../ui/Icons';
import { APPROVALS_POLL_MS } from '../../../config/constants';
import {
  type McpInvokeResult,
  parsePendingApprovals,
  unwrapMcpEnvelope,
} from '../../../lib/mcpToolResult';
import { recordGamifyGuiEvent } from '../../../lib/gamifyGuiEvents';

interface PendingApproval {
  approval_id: string;
  tool: string;
  summary: string;
  requested_at_ms: number;
}

interface ApprovalsViewProps {
  pushToast: (t: any) => void;
  gamifyEnabled?: boolean;
}

function formatRequestedAt(ms: number): string {
  if (!Number.isFinite(ms) || ms <= 0) return '—';
  const deltaSec = Math.round((Date.now() - ms) / 1000);
  let rel: string;
  if (deltaSec < 5) rel = 'just now';
  else if (deltaSec < 60) rel = `${deltaSec}s ago`;
  else if (deltaSec < 3600) rel = `${Math.floor(deltaSec / 60)}m ago`;
  else if (deltaSec < 86400) rel = `${Math.floor(deltaSec / 3600)}h ago`;
  else rel = `${Math.floor(deltaSec / 86400)}d ago`;
  return `${rel} · ${new Date(ms).toLocaleTimeString()}`;
}

export function ApprovalsView({ pushToast, gamifyEnabled = false }: ApprovalsViewProps) {
  const [approvals, setApprovals] = useState<PendingApproval[]>([]);
  const [loading, setLoading] = useState(true);
  const [resolving, setResolving] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const res = await invoke<McpInvokeResult>('invoke_mcp_tool', {
        tool: 'vox_pending_approvals',
        args: {},
      });
      setApprovals(parsePendingApprovals(res));
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Approvals load failed', body: String(err) });
    } finally {
      setLoading(false);
    }
  }, [pushToast]);

  useEffect(() => {
    refresh();
    const id = setInterval(refresh, APPROVALS_POLL_MS);
    return () => clearInterval(id);
  }, [refresh]);

  const resolve = useCallback(
    async (approvalId: string, outcome: 'approved' | 'rejected') => {
      setResolving(approvalId);
      try {
        const res = await invoke<McpInvokeResult>('invoke_mcp_tool', {
          tool: 'vox_resolve_approval',
          args: { approval_id: approvalId, outcome },
        });
        const data = unwrapMcpEnvelope(res.result) as { resolved?: boolean } | null;
        if (res.is_error || data?.resolved === false) {
          pushToast({ tone: 'warn', title: 'Resolve failed', body: `Could not ${outcome.replace('ed', '')} ${approvalId}` });
        } else {
          pushToast({
            tone: outcome === 'approved' ? 'ok' : 'warn',
            title: outcome === 'approved' ? 'Approved' : 'Rejected',
            body: approvalId,
          });
          void recordGamifyGuiEvent(
            'approval_decision',
            { approval_id: approvalId, outcome },
            { enabled: gamifyEnabled }
          );
        }
        setApprovals((prev) => prev.filter((a) => a.approval_id !== approvalId));
        await refresh();
      } catch (err) {
        pushToast({ tone: 'warn', title: 'Resolve failed', body: String(err) });
      } finally {
        setResolving(null);
      }
    },
    [pushToast, refresh, gamifyEnabled]
  );

  const columns = [
    { 
      key: 'approval_id', 
      header: 'Request ID', 
      width: 150,
      render: (r: PendingApproval) => <span className="font-mono text-zinc-500">#{r.approval_id}</span> 
    },
    { 
      key: 'tool', 
      header: 'Tool', 
      width: 120,
      render: (r: PendingApproval) => <span className="font-mono text-xs text-brass">{r.tool}</span> 
    },
    { key: 'summary', header: 'Action Description' },
    { 
      key: 'requested_at', 
      header: 'Requested At', 
      width: 180,
      render: (r: PendingApproval) => (
        <span className="flex items-center gap-1 font-mono text-[10px] text-zinc-500">
          <Icon.clock className="size-3" aria-hidden="true" />
          {formatRequestedAt(r.requested_at_ms)}
        </span>
      ) 
    },
    {
      key: 'actions',
      header: 'Actions',
      width: 200,
      render: (r: PendingApproval) => {
        const busy = resolving === r.approval_id;
        return (
          <div
            role="group"
            aria-label={`Resolve approval for ${r.tool}`}
            className="flex items-center gap-2"
          >
            <Button
              onClick={() => resolve(r.approval_id, 'approved')}
              disabled={busy}
              aria-label={`Approve ${r.summary}`}
              variant="outline"
              size="xs"
              className="border-emerald-500/20 text-emerald-400 hover:bg-emerald-500/10"
            >
              <Icon.check className="size-3 mr-1" aria-hidden="true" /> Approve
            </Button>
            <Button
              onClick={() => resolve(r.approval_id, 'rejected')}
              disabled={busy}
              aria-label={`Reject ${r.summary}`}
              variant="outline"
              size="xs"
              className="border-rose-500/20 text-rose-400 hover:bg-rose-500/10"
            >
              <Icon.x className="size-3 mr-1" aria-hidden="true" /> Reject
            </Button>
          </div>
        );
      },
    },
  ];

  return (
    <div className="flex flex-col gap-4 p-4 h-full overflow-auto">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className="flex size-7 items-center justify-center rounded-lg bg-brass/10 text-brass ring-1 ring-brass/30">
            <Icon.shield className="size-4" aria-hidden="true" />
          </span>
          <h2 className="text-lg font-bold tracking-wide text-zinc-200">Pending Approvals</h2>
        </div>
        <Button variant="ghost" size="xs" onClick={refresh} aria-label="Refresh approvals">
          <Icon.refresh className="size-4 text-zinc-400" />
        </Button>
      </div>

      <div role="list" aria-label="Pending approvals" aria-live="polite">
        <DataTable
          rows={approvals}
          columns={columns}
          getRowId={r => r.approval_id}
          loading={loading}
          density="compact"
          emptyState={
            <EmptyState 
              icon={<Icon.check className="size-8 text-emerald-300" />}
              title="No pending approvals" 
              description="Dangerous tool invocations will park here for a human to review."
            />
          }
        />
      </div>
    </div>
  );
}
