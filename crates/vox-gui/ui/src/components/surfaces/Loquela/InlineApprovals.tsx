import React, { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Glass } from '../../ui/Glass';
import { Icon } from '../../ui/Icons';
import { APPROVALS_POLL_MS } from '../../../config/constants';
import {
  type McpInvokeResult,
  type PendingApprovalRow,
  parsePendingApprovals,
  unwrapMcpEnvelope,
} from '../../../lib/mcpToolResult';

interface InlineApprovalsProps {
  pushToast: (t: { tone: 'ok' | 'warn' | 'info'; title: string; body?: string }) => void;
  onViewAll?: () => void;
}

export function InlineApprovals({ pushToast, onViewAll }: InlineApprovalsProps) {
  const [approvals, setApprovals] = useState<PendingApprovalRow[]>([]);
  const [resolving, setResolving] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const res = await invoke<McpInvokeResult>('invoke_mcp_tool', {
        tool: 'vox_pending_approvals',
        args: {},
      });
      setApprovals(parsePendingApprovals(res));
    } catch {
      setApprovals([]);
    }
  }, []);

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
          pushToast({
            tone: 'warn',
            title: 'Resolve failed',
            body: approvalId,
          });
        } else {
          pushToast({
            tone: outcome === 'approved' ? 'ok' : 'warn',
            title: outcome === 'approved' ? 'Approved' : 'Rejected',
            body: approvalId,
          });
        }
        setApprovals(prev => prev.filter(a => a.approval_id !== approvalId));
        await refresh();
      } catch (err) {
        pushToast({ tone: 'warn', title: 'Resolve failed', body: String(err) });
      } finally {
        setResolving(null);
      }
    },
    [pushToast, refresh],
  );

  if (approvals.length === 0) return null;

  const visible = approvals.slice(0, 2);

  return (
    <Glass className="mb-3 border border-amber-400/20 bg-amber-400/[0.04] p-3">
      <div className="mb-2 flex items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <Icon.shield className="size-3.5 text-amber-300" />
          <span className="font-mono text-[10px] uppercase tracking-widest text-amber-200/90">
            Approval required
          </span>
          <span className="rounded-full bg-white/10 px-1.5 py-px font-mono text-[9px] text-zinc-400">
            {approvals.length}
          </span>
        </div>
        {onViewAll && approvals.length > 0 && (
          <button
            type="button"
            onClick={onViewAll}
            className="font-mono text-[9px] uppercase tracking-widest text-zinc-500 hover:text-brass"
          >
            View all
          </button>
        )}
      </div>
      <div className="flex flex-col gap-2">
        {visible.map(a => {
          const busy = resolving === a.approval_id;
          return (
            <div
              key={a.approval_id}
              className="flex flex-col gap-2 rounded-lg border border-white/5 bg-black/20 p-2 sm:flex-row sm:items-center sm:justify-between"
            >
              <div className="min-w-0">
                <div className="font-mono text-[11px] text-zinc-200 truncate">{a.tool}</div>
                <div className="text-[11px] text-zinc-500 truncate">{a.summary}</div>
              </div>
              <div className="flex shrink-0 gap-2">
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => resolve(a.approval_id, 'rejected')}
                  className="rounded border border-white/10 px-2 py-1 font-mono text-[9px] uppercase tracking-widest text-zinc-400 hover:border-rose-400/40 hover:text-rose-300 disabled:opacity-40"
                >
                  Reject
                </button>
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => resolve(a.approval_id, 'approved')}
                  className="rounded border border-brass/30 bg-brass/10 px-2 py-1 font-mono text-[9px] uppercase tracking-widest text-brass hover:bg-brass/20 disabled:opacity-40"
                >
                  Approve
                </button>
              </div>
            </div>
          );
        })}
      </div>
    </Glass>
  );
}
