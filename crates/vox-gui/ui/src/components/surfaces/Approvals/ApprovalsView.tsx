import React, { useCallback, useEffect, useState } from 'react';
import { sanitizeErrorForToast } from '../../../lib/backendGuard';
import { Glass } from '../../ui/Glass';
import { EmptyState } from '../../ui/EmptyState';
import { StatusPill } from '../../ui/StatusPill';
import { DataTable } from '../../ui/DataTable';
import { Button } from '../../ui/Button';
import { Segment } from '../../ui/Segment';
import { Icon } from '../../ui/Icons';
import { APPROVALS_POLL_MS } from '../../../config/constants';
import { useIsEmbeddedSurface } from '../../dashboard/EmbeddedSurfaceContext';
import { useLabel } from '../../../hooks/useLanguage';
import {
  type McpInvokeResult,
  parsePendingApprovals,
  unwrapMcpEnvelope,
} from '../../../lib/mcpToolResult';
import { recordGamifyGuiEvent } from '../../../lib/gamifyGuiEvents';
import { voxTransport, getPermissionMode, setPermissionMode as setTransportPermissionMode } from '../../../transport';

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

/**
 * T0.3: mirrors `vox_orchestrator_mcp::permission_modes::PermissionMode`'s
 * wire strings (`contracts/orchestration/permission-modes.v1.yaml`).
 *
 * The toggle below is the UI for `../../../transport.ts`'s shared
 * `getPermissionMode`/`setPermissionMode` module state — that shared state
 * (not local component state) is what `voxTransport.invokeMcpTool` /
 * `voxTransport.callTool` actually read on every subsequent tool call, so
 * selecting a mode here measurably changes daemon-side auto-approve
 * behavior for calls made from anywhere in the app, not just this view.
 * `DispatchRequest`'s own top-level field carries it — never folded into a
 * tool's `args` — so the dispatch gate reads it from an authenticated
 * channel, not from tool-call JSON.
 */
type PermissionMode = 'ask' | 'accept_edits' | 'accept_all' | 'plan';

const PERMISSION_MODE_OPTIONS: { id: PermissionMode; label: string; hint: string }[] = [
  { id: 'ask', label: 'Ask', hint: 'Always park dangerous tool calls for approval (default).' },
  { id: 'accept_edits', label: 'Accept Edits', hint: 'Auto-approve reversible file edits; still park destructive actions.' },
  { id: 'accept_all', label: 'Accept All', hint: 'Auto-approve mutating and destructive tool calls.' },
  { id: 'plan', label: 'Plan', hint: 'Read-only / planning mode.' },
];

const PERMISSION_MODE_STORAGE_KEY = 'vox.approvals.permission_mode';

function isKnownMode(value: unknown): value is PermissionMode {
  return value === 'ask' || value === 'accept_edits' || value === 'accept_all' || value === 'plan';
}

/**
 * Resolve the mode to show/apply on mount: prefer whatever's already live
 * in the shared transport state (e.g. set earlier this session), then fall
 * back to localStorage, then the safe `ask` default. Also pushes the
 * resolved value back into the shared transport state so it's active
 * immediately, even before the user touches the toggle.
 */
function resolveAndApplyInitialPermissionMode(): PermissionMode {
  const live = getPermissionMode();
  if (isKnownMode(live)) return live;

  let stored: unknown = null;
  try {
    stored = window.localStorage?.getItem(PERMISSION_MODE_STORAGE_KEY);
  } catch {
    // localStorage unavailable (e.g. embedded surface) — fall back to the safe default.
  }
  const resolved = isKnownMode(stored) ? stored : 'ask';
  setTransportPermissionMode(resolved);
  return resolved;
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
  const embedded = useIsEmbeddedSurface();
  const [approvals, setApprovals] = useState<PendingApproval[]>([]);
  const [loading, setLoading] = useState(true);
  const [resolving, setResolving] = useState<string | null>(null);
  const [permissionMode, setPermissionModeState] = useState<PermissionMode>(
    resolveAndApplyInitialPermissionMode
  );
  const [alwaysAllow, setAlwaysAllow] = useState<Record<string, boolean>>({});

  const setMode = useCallback((mode: PermissionMode) => {
    setPermissionModeState(mode);
    // Write through to the shared transport state — this is what actually
    // reaches `invoke_mcp_tool` calls made from anywhere in the app.
    setTransportPermissionMode(mode);
    try {
      window.localStorage?.setItem(PERMISSION_MODE_STORAGE_KEY, mode);
    } catch {
      // best-effort only — an in-memory mode for this session is still correct.
    }
  }, []);

  const refresh = useCallback(async () => {
    try {
      const res = await voxTransport.invokeMcpTool('vox_pending_approvals', {});
      // F-02 gap: a null/undefined resolution (e.g. no response from the
      // backend) must not be dereferenced — treat it as a failed call
      // rather than throwing a raw TypeError into the catch below.
      setApprovals(
        parsePendingApprovals({
          tool: 'vox_pending_approvals',
          is_error: !res || !!res.is_error,
          result: res ? res.result : undefined,
        })
      );
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Approvals load failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setLoading(false);
    }
  }, [pushToast]);

  useEffect(() => {
    refresh();
    // Embedded mini-render: one initial fetch only, no repeating poll.
    if (embedded) return;
    const id = setInterval(refresh, APPROVALS_POLL_MS);
    return () => clearInterval(id);
  }, [refresh, embedded]);

  const resolve = useCallback(
    async (approvalId: string, outcome: 'approved' | 'rejected', tool: string) => {
      setResolving(approvalId);
      try {
        // T0.3: "always allow this tool in this repo" — persist the
        // allowlist entry alongside resolving the current approval. Fires
        // before resolve so a failure here doesn't silently drop the
        // approval decision itself; failures are toasted but non-fatal.
        if (outcome === 'approved' && alwaysAllow[approvalId]) {
          try {
            // Note: vox_add_approval_allowlist_entry is itself
            // `always_requires_approval: true` server-side (T0.3 follow-up
            // fix) — this call parks for a human decision regardless of the
            // currently selected mode, exactly like any other dangerous
            // tool. It shows up as a NEW pending approval that `refresh()`
            // below will surface.
            const allowRes = await voxTransport.invokeMcpTool('vox_add_approval_allowlist_entry', { tool });
            if (!allowRes || allowRes.is_error) {
              pushToast({ tone: 'warn', title: 'Allowlist not saved', body: tool, cause: 'backend-error' });
            }
          } catch (err) {
            pushToast({ tone: 'warn', title: 'Allowlist not saved', body: sanitizeErrorForToast(err), cause: 'backend-error' });
          }
        }

        const res = await voxTransport.invokeMcpTool('vox_resolve_approval', { approval_id: approvalId, outcome });
        const data = res ? (unwrapMcpEnvelope(res.result) as { resolved?: boolean } | null) : null;
        if (!res || res.is_error || data?.resolved === false) {
          pushToast({ tone: 'warn', title: 'Resolve failed', body: `Could not ${outcome.replace('ed', '')} ${approvalId}`, cause: 'backend-error' });
        } else {
          pushToast({
            tone: outcome === 'approved' ? 'ok' : 'warn',
            title: outcome === 'approved' ? 'Approved' : 'Rejected',
            body: approvalId,
            cause: 'backend-ok',
          });
          void recordGamifyGuiEvent(
            'approval_decision',
            { approval_id: approvalId, outcome },
            { enabled: gamifyEnabled }
          );
        }
        setApprovals((prev) => prev.filter((a) => a.approval_id !== approvalId));
        setAlwaysAllow((prev) => {
          const next = { ...prev };
          delete next[approvalId];
          return next;
        });
        await refresh();
      } catch (err) {
        pushToast({ tone: 'warn', title: 'Resolve failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
      } finally {
        setResolving(null);
      }
    },
    [pushToast, refresh, gamifyEnabled, alwaysAllow]
  );

  const columns = [
    {
      key: 'approval_id',
      header: 'Request ID',
      width: 150,
      render: (r: PendingApproval) => <span className="font-mono text-text-muted">#{r.approval_id}</span>
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
        <span className="flex items-center gap-1 font-mono text-[10px] text-text-muted">
          <Icon.clock className="size-3" aria-hidden="true" />
          {formatRequestedAt(r.requested_at_ms)}
        </span>
      )
    },
    {
      key: 'actions',
      header: 'Actions',
      width: 260,
      render: (r: PendingApproval) => {
        const busy = resolving === r.approval_id;
        return (
          <div
            role="group"
            aria-label={`Resolve approval for ${r.tool}`}
            className="flex items-center gap-2"
          >
            <label className="flex items-center gap-1 text-[10px] text-text-muted cursor-pointer select-none">
              <input
                type="checkbox"
                checked={!!alwaysAllow[r.approval_id]}
                onChange={(e) =>
                  setAlwaysAllow((prev) => ({ ...prev, [r.approval_id]: e.target.checked }))
                }
                aria-label={`Always allow ${r.tool} in this repository`}
              />
              Always allow
            </label>
            <Button
              onClick={() => resolve(r.approval_id, 'approved', r.tool)}
              disabled={busy}
              aria-label={`Approve ${r.summary}`}
              variant="outline"
              size="xs"
              className="border-emerald-500/20 text-emerald-400 hover:bg-emerald-500/10"
            >
              <Icon.check className="size-3 mr-1" aria-hidden="true" /> Approve
            </Button>
            <Button
              onClick={() => resolve(r.approval_id, 'rejected', r.tool)}
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
          <h2 className="text-lg font-bold tracking-wide text-text-secondary">{useLabel('appr-pending')}</h2>
        </div>
        <div className="flex items-center gap-3">
          <div role="group" aria-label="Permission mode" className="flex items-center gap-2">
            <span className="text-[10px] uppercase tracking-[0.15em] text-text-muted">Mode</span>
            <Segment
              value={permissionMode}
              onChange={(id) => setMode(id as PermissionMode)}
              options={PERMISSION_MODE_OPTIONS}
              size="xs"
            />
          </div>
          <Button variant="ghost" size="xs" onClick={refresh} aria-label="Refresh approvals">
            <Icon.refresh className="size-4 text-text-muted" />
          </Button>
        </div>
      </div>

      <div aria-live="polite">
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
