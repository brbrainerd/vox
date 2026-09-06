import React, { useCallback, useEffect, useState } from 'react';
import { sanitizeErrorForToast } from '../../../lib/backendGuard';
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
import type { Toast } from '../../../types/tauri';
import { useIsEmbeddedSurface } from '../../dashboard/EmbeddedSurfaceContext';

interface InlineApprovalsProps {
  pushToast: (t: Toast) => void;
  onViewAll?: () => void;
}

export function InlineApprovals({ pushToast, onViewAll }: InlineApprovalsProps) {
  const embedded = useIsEmbeddedSurface();
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
    // Embedded mini-render: one initial fetch only, no repeating poll.
    if (embedded) return;
    const id = setInterval(refresh, APPROVALS_POLL_MS);
    return () => clearInterval(id);
  }, [refresh, embedded]);

  const resolve = useCallback(
    async (approvalId: string, outcome: 'approved' | 'rejected') => {
      setResolving(approvalId);
      try {
        const res = await invoke<McpInvokeResult>('invoke_mcp_tool', {
          tool: 'vox_resolve_approval',
          args: { approval_id: approvalId, outcome },
        });
        const data = res ? (unwrapMcpEnvelope(res.result) as { resolved?: boolean } | null) : null;
        if (!res || res.is_error || data?.resolved === false) {
          pushToast({
            tone: 'warn',
            title: 'Resolve failed',
            body: approvalId,
            cause: 'backend-error',
          });
        } else {
          pushToast({
            tone: outcome === 'approved' ? 'ok' : 'warn',
            title: outcome === 'approved' ? 'Approved' : 'Rejected',
            body: approvalId,
            cause: 'backend-ok',
          });
        }
        setApprovals(prev => prev.filter(a => a.approval_id !== approvalId));
        await refresh();
      } catch (err) {
        pushToast({ tone: 'warn', title: 'Resolve failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
      } finally {
        setResolving(null);
      }
    },
    [pushToast, refresh],
  );

  if (approvals.length === 0) return null;

  const visible = approvals.slice(0, 2);

  return (
    <Glass
      role="region"
      aria-label="Approval required"
      aria-live="polite"
      className="mb-3 border border-amber-400/20 bg-amber-400/4 p-3"
    >
      <div className="mb-2 flex items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <Icon.shield className="size-3.5 text-amber-300" aria-hidden="true" />
          <span className="font-mono text-[10px] uppercase tracking-widest text-amber-200/90">
            Approval required
          </span>
          <span className="rounded-full bg-overlay-subtle px-1.5 py-px font-mono text-[9px] text-text-muted">
            {approvals.length}
          </span>
        </div>
        {onViewAll && approvals.length > 0 && (
          <button
            type="button"
            onClick={onViewAll}
            className="font-mono text-[9px] uppercase tracking-widest text-text-muted hover:text-brass"
          >
            View all
          </button>
        )}
      </div>
      <ul role="list" className="flex flex-col gap-2">
        {visible.map(a => {
          const busy = resolving === a.approval_id;
          return (
            <li
              key={a.approval_id}
              className="flex flex-col gap-2 rounded-lg border border-border-subtle bg-overlay-subtle p-2 sm:flex-row sm:items-center sm:justify-between"
            >
              <div className="min-w-0">
                <div className="font-mono text-[11px] text-text-secondary truncate">{a.tool}</div>
                <div className="text-[11px] text-text-muted truncate">{a.summary}</div>
              </div>
              <div className="flex shrink-0 gap-2">
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => resolve(a.approval_id, 'rejected')}
                  className="rounded-sm border border-border-subtle px-2 py-1 font-mono text-[9px] uppercase tracking-widest text-text-muted hover:border-rose-400/40 hover:text-rose-300 disabled:opacity-40"
                >
                  Reject
                </button>
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => resolve(a.approval_id, 'approved')}
                  className="rounded-sm border border-brass/30 bg-brass/10 px-2 py-1 font-mono text-[9px] uppercase tracking-widest text-brass hover:bg-brass/20 disabled:opacity-40"
                >
                  Approve
                </button>
              </div>
            </li>
          );
        })}
      </ul>
    </Glass>
  );
}
