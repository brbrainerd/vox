import React, { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Glass } from '../../ui/Glass';
import { Icon } from '../../ui/Icons';
import { EmptyState } from '../../ui/EmptyState';
import { Button } from '../../ui/Button';
import { APPROVALS_POLL_MS } from '../../../config/constants';
import {
  type McpInvokeResult,
  parsePendingApprovals,
  unwrapMcpEnvelope,
} from '../../../lib/mcpToolResult';

interface PendingApproval {
  approval_id: string;
  tool: string;
  summary: string;
  requested_at_ms: number;
}

interface ApprovalsViewProps {
  pushToast: (t: any) => void;
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

export function ApprovalsView({ pushToast }: ApprovalsViewProps) {
  const [approvals, setApprovals] = useState<PendingApproval[]>([]);
  const [loading, setLoading] = useState(true);
  // approval_id currently being resolved → disables that row's buttons.
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
        }
        // Optimistically drop the row, then re-sync with the backend.
        setApprovals((prev) => prev.filter((a) => a.approval_id !== approvalId));
        await refresh();
      } catch (err) {
        pushToast({ tone: 'warn', title: 'Resolve failed', body: String(err) });
      } finally {
        setResolving(null);
      }
    },
    [pushToast, refresh],
  );

  return (
    <div className="grid grid-cols-12 gap-5">
      <Glass className="col-span-12 p-4 overflow-auto">
        <div className="mb-3 flex items-center gap-2">
          <span className="flex size-7 items-center justify-center rounded-lg bg-brass/10 text-brass ring-1 ring-brass/30">
            <Icon.shield className="size-4" aria-hidden="true" />
          </span>
          <div className="font-display text-sm tracking-widest uppercase text-zinc-200">
            Pending Approvals
          </div>
          <span className="ml-2 rounded-full bg-white/[0.05] px-2 py-0.5 font-mono text-[10px] text-zinc-400">
            {approvals.length}
          </span>
        </div>

        {loading && approvals.length === 0 ? (
          <div className="text-sm text-zinc-500">Loading approvals…</div>
        ) : approvals.length === 0 ? (
          <EmptyState
            icon={<Icon.check className="size-8 text-emerald-300" />}
            title="No pending approvals"
            description="Dangerous tool invocations will park here for a human to review."
          />
        ) : (
          <div
            role="list"
            aria-label="Pending approvals"
            aria-live="polite"
            className="flex flex-col gap-2"
          >
            {approvals.map((a) => {
              const busy = resolving === a.approval_id;
              return (
                <div
                  key={a.approval_id}
                  role="listitem"
                  className="flex flex-col gap-3 rounded-lg border border-white/5 bg-white/[0.02] p-3 sm:flex-row sm:items-center sm:justify-between"
                >
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="font-mono text-xs text-brass">{a.tool}</span>
                      <span className="flex items-center gap-1 font-mono text-[10px] text-zinc-500">
                        <Icon.clock className="size-3" aria-hidden="true" />
                        {formatRequestedAt(a.requested_at_ms)}
                      </span>
                    </div>
                    <div className="mt-1 text-xs text-zinc-200 break-words">{a.summary}</div>
                    <div className="mt-1 font-mono text-[10px] text-zinc-600 break-all">{a.approval_id}</div>
                  </div>
                  <div
                    role="group"
                    aria-label={`Resolve approval for ${a.tool}`}
                    className="flex shrink-0 items-center gap-2"
                  >
                    <Button
                      onClick={() => resolve(a.approval_id, 'approved')}
                      disabled={busy}
                      aria-label={`Approve ${a.summary}`}
                      className="flex items-center gap-1.5 rounded-md border border-emerald-400/30 bg-emerald-400/10 px-3 py-1.5 font-display text-[11px] tracking-wider uppercase text-emerald-300 transition hover:bg-emerald-400/20 disabled:cursor-not-allowed disabled:opacity-40"
                    >
                      <Icon.check className="size-3.5" aria-hidden="true" /> Approve
                    </Button>
                    <Button
                      onClick={() => resolve(a.approval_id, 'rejected')}
                      disabled={busy}
                      aria-label={`Reject ${a.summary}`}
                      className="flex items-center gap-1.5 rounded-md border border-rose-400/30 bg-rose-400/10 px-3 py-1.5 font-display text-[11px] tracking-wider uppercase text-rose-300 transition hover:bg-rose-400/20 disabled:cursor-not-allowed disabled:opacity-40"
                    >
                      <Icon.x className="size-3.5" aria-hidden="true" /> Reject
                    </Button>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </Glass>
    </div>
  );
}
