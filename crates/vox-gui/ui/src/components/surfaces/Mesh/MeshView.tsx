import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Glass } from '../../ui/Glass';
import { Icon } from '../../ui/Icons';

interface MeshViewProps {
  pushToast: (item: { tone: 'ok' | 'warn' | 'info'; title: string; body?: string }) => void;
}

/** One node row as summarized by the `vox_mesh_nodes` MCP tool. */
interface MeshNode {
  id: string;
  status: string;
  host_triple?: string | null;
  gpu_summary?: string | null;
  trust_tier?: string | null;
  advertised_models?: string[];
  last_seen_unix_ms?: number;
}

interface NodesResult {
  source?: string;
  control_url?: string;
  control_plane_error?: string;
  nodes?: MeshNode[];
  queue_depth?: number | null;
  node_count?: number;
}

interface QueueStatsResult {
  source?: string;
  control_plane_error?: string;
  pending_count?: number | null;
  pending_by_kind?: Record<string, number>;
  pending_by_priority?: Record<string, number>;
}

interface McpEnvelope<T> {
  tool: string;
  is_error: boolean;
  result: T;
}

const REFRESH_MS = 5000;

function formatLastSeen(ms?: number): string {
  if (!ms || !Number.isFinite(ms) || ms <= 0) return '—';
  const deltaSec = Math.round((Date.now() - ms) / 1000);
  if (deltaSec < 5) return 'just now';
  if (deltaSec < 60) return `${deltaSec}s ago`;
  if (deltaSec < 3600) return `${Math.floor(deltaSec / 60)}m ago`;
  if (deltaSec < 86400) return `${Math.floor(deltaSec / 3600)}h ago`;
  return `${Math.floor(deltaSec / 86400)}d ago`;
}

function statusTone(status: string): string {
  switch (status) {
    case 'online':
      return 'border-emerald-400/30 bg-emerald-400/10 text-emerald-300';
    case 'maintenance':
      return 'border-amber-400/30 bg-amber-400/10 text-amber-300';
    case 'quarantined':
      return 'border-rose-400/30 bg-rose-400/10 text-rose-300';
    default:
      return 'border-white/10 bg-white/[0.05] text-zinc-300';
  }
}

export function MeshView({ pushToast }: MeshViewProps) {
  const [nodes, setNodes] = useState<MeshNode[]>([]);
  const [nodesMeta, setNodesMeta] = useState<NodesResult>({});
  const [queue, setQueue] = useState<QueueStatsResult>({});
  const [loading, setLoading] = useState(true);

  // Dispatch form state.
  const [targetNode, setTargetNode] = useState<string>('');
  const [source, setSource] = useState<string>('');
  const [taskKind, setTaskKind] = useState<string>('');
  const [dispatching, setDispatching] = useState(false);
  const [dispatchResult, setDispatchResult] = useState<string>('');

  const refresh = useCallback(async () => {
    try {
      const [nodesRes, queueRes] = await Promise.all([
        invoke<McpEnvelope<NodesResult>>('invoke_mcp_tool', {
          tool: 'vox_mesh_nodes',
          args: {},
        }),
        invoke<McpEnvelope<QueueStatsResult>>('invoke_mcp_tool', {
          tool: 'vox_mesh_queue_stats',
          args: {},
        }),
      ]);
      const meta = nodesRes?.result ?? {};
      setNodesMeta(meta);
      setNodes(Array.isArray(meta.nodes) ? meta.nodes : []);
      setQueue(queueRes?.result ?? {});
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Mesh refresh failed', body: String(err) });
    } finally {
      setLoading(false);
    }
  }, [pushToast]);

  useEffect(() => {
    refresh();
    const id = setInterval(refresh, REFRESH_MS);
    return () => clearInterval(id);
  }, [refresh]);

  // Dispatch availability: the local-registry source means no control plane is
  // reachable, so dispatch (a write) cannot succeed and should be disabled.
  const dispatchConfigured = nodesMeta.source === 'control_plane';

  const pendingCount = useMemo(() => {
    if (typeof queue.pending_count === 'number') return queue.pending_count;
    if (typeof nodesMeta.queue_depth === 'number') return nodesMeta.queue_depth;
    return null;
  }, [queue.pending_count, nodesMeta.queue_depth]);

  const dispatch = useCallback(async () => {
    if (!source.trim()) {
      pushToast({ tone: 'warn', title: 'Dispatch needs source', body: 'Enter .vox source to run.' });
      return;
    }
    setDispatching(true);
    setDispatchResult('');
    try {
      const args: Record<string, unknown> = { source };
      if (targetNode) args.node_id = targetNode;
      if (taskKind.trim()) args.task_kind = taskKind.trim();
      const res = await invoke<McpEnvelope<any>>('invoke_mcp_tool', {
        tool: 'vox_mesh_dispatch',
        args,
      });
      if (res?.is_error) {
        const msg = res?.result?.error ?? JSON.stringify(res?.result);
        setDispatchResult(String(msg));
        pushToast({ tone: 'warn', title: 'Dispatch failed', body: String(msg) });
      } else {
        const r = res?.result ?? {};
        const id = r.node_id ?? '(unknown node)';
        setDispatchResult(
          `node=${id} success=${r.success} exit=${r.exit_code ?? '—'} (${r.duration_ms ?? 0}ms)\n${r.output ?? ''}`,
        );
        pushToast({
          tone: r.success ? 'ok' : 'warn',
          title: r.success ? 'Dispatched' : 'Dispatch returned failure',
          body: `node ${id}`,
        });
      }
      await refresh();
    } catch (err) {
      setDispatchResult(String(err));
      pushToast({ tone: 'warn', title: 'Dispatch failed', body: String(err) });
    } finally {
      setDispatching(false);
    }
  }, [source, targetNode, taskKind, pushToast, refresh]);

  return (
    <div className="grid grid-cols-12 gap-5">
      {/* Summary / queue header */}
      <Glass className="col-span-12 p-4">
        <div className="mb-3 flex flex-wrap items-center gap-2">
          <span className="flex size-7 items-center justify-center rounded-lg bg-brass/10 text-brass ring-1 ring-brass/30">
            <Icon.cpu className="size-4" />
          </span>
          <div className="font-display text-sm tracking-widest uppercase text-zinc-200">
            Vox Populi Mesh
          </div>
          <span className="ml-1 rounded-full bg-white/[0.05] px-2 py-0.5 font-mono text-[10px] text-zinc-400">
            {nodes.length} node{nodes.length === 1 ? '' : 's'}
          </span>
          <span className="rounded-full bg-white/[0.05] px-2 py-0.5 font-mono text-[10px] text-zinc-400">
            source: {nodesMeta.source ?? '—'}
          </span>
          <span className="rounded-full bg-white/[0.05] px-2 py-0.5 font-mono text-[10px] text-zinc-400">
            pending: {pendingCount ?? '—'}
          </span>
          <button
            onClick={refresh}
            className="ml-auto flex items-center gap-1.5 rounded-md border border-white/10 bg-white/[0.03] px-3 py-1.5 font-display text-[11px] tracking-wider uppercase text-zinc-200 transition hover:bg-white/[0.06]"
          >
            <Icon.refresh className="size-3.5" /> Refresh
          </button>
        </div>

        {nodesMeta.control_plane_error && (
          <div className="mb-2 flex items-start gap-2 rounded-md border border-amber-400/20 bg-amber-400/5 px-3 py-2 text-[11px] text-amber-300">
            <Icon.alert className="mt-0.5 size-3.5 shrink-0" />
            <span>
              Control plane unreachable ({nodesMeta.control_url}) — showing local registry.{' '}
              <span className="font-mono text-amber-200/80">{nodesMeta.control_plane_error}</span>
            </span>
          </div>
        )}

        {queue.pending_by_kind && Object.keys(queue.pending_by_kind).length > 0 && (
          <div className="flex flex-wrap gap-1.5">
            {Object.entries(queue.pending_by_kind).map(([kind, n]) => (
              <span
                key={kind}
                className="rounded-full bg-white/[0.04] px-2 py-0.5 font-mono text-[10px] text-zinc-400"
              >
                {kind}: {n}
              </span>
            ))}
          </div>
        )}
      </Glass>

      {/* Node table */}
      <Glass className="col-span-12 overflow-auto p-4">
        <div className="mb-3 font-display text-xs tracking-widest uppercase text-zinc-400">
          Nodes
        </div>
        {loading && nodes.length === 0 ? (
          <div className="text-sm text-zinc-500">Loading mesh nodes…</div>
        ) : nodes.length === 0 ? (
          <div className="flex flex-col items-center justify-center gap-3 py-12 text-center">
            <span className="flex size-12 items-center justify-center rounded-2xl bg-white/[0.05] text-zinc-400 ring-1 ring-white/10">
              <Icon.cpu className="size-6" />
            </span>
            <div className="font-display text-sm tracking-wider text-zinc-300">No mesh nodes</div>
            <div className="max-w-md text-[11px] leading-relaxed text-zinc-500">
              Join one with <code className="font-mono text-zinc-400">vox populi join</code>, or
              configure a control plane via{' '}
              <code className="font-mono text-zinc-400">VOX_ORCHESTRATOR_DAEMON_SOCKET</code> /{' '}
              <code className="font-mono text-zinc-400">VOX_ORCHESTRATOR_MESH_CONTROL_URL</code>.
            </div>
          </div>
        ) : (
          <table className="w-full text-left text-xs">
            <thead>
              <tr className="text-[10px] uppercase tracking-wider text-zinc-500">
                <th className="px-2 py-1.5 font-display">Node</th>
                <th className="px-2 py-1.5 font-display">Status</th>
                <th className="px-2 py-1.5 font-display">Host</th>
                <th className="px-2 py-1.5 font-display">GPU</th>
                <th className="px-2 py-1.5 font-display">Trust</th>
                <th className="px-2 py-1.5 font-display">Models</th>
                <th className="px-2 py-1.5 font-display">Last seen</th>
              </tr>
            </thead>
            <tbody>
              {nodes.map((n) => (
                <tr key={n.id} className="border-t border-white/5">
                  <td className="px-2 py-1.5 font-mono text-brass break-all">{n.id}</td>
                  <td className="px-2 py-1.5">
                    <span
                      className={`rounded-full border px-2 py-0.5 font-display text-[10px] tracking-wider uppercase ${statusTone(
                        n.status,
                      )}`}
                    >
                      {n.status}
                    </span>
                  </td>
                  <td className="px-2 py-1.5 font-mono text-[11px] text-zinc-400">
                    {n.host_triple ?? '—'}
                  </td>
                  <td className="px-2 py-1.5 font-mono text-[11px] text-zinc-300">
                    {n.gpu_summary ?? '—'}
                  </td>
                  <td className="px-2 py-1.5 text-[11px] text-zinc-300">{n.trust_tier ?? '—'}</td>
                  <td className="px-2 py-1.5 text-[11px] text-zinc-400">
                    {n.advertised_models && n.advertised_models.length > 0
                      ? n.advertised_models.join(', ')
                      : '—'}
                  </td>
                  <td className="px-2 py-1.5 font-mono text-[11px] text-zinc-500">
                    {formatLastSeen(n.last_seen_unix_ms)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Glass>

      {/* Dispatch form */}
      <Glass className="col-span-12 p-4">
        <div className="mb-3 flex items-center gap-2">
          <span className="flex size-7 items-center justify-center rounded-lg bg-brass/10 text-brass ring-1 ring-brass/30">
            <Icon.send className="size-4" />
          </span>
          <div className="font-display text-sm tracking-widest uppercase text-zinc-200">
            Dispatch Job
          </div>
        </div>

        {!dispatchConfigured && (
          <div className="mb-3 flex items-start gap-2 rounded-md border border-amber-400/20 bg-amber-400/5 px-3 py-2 text-[11px] text-amber-300">
            <Icon.alert className="mt-0.5 size-3.5 shrink-0" />
            <span>
              Dispatch is disabled: no populi control plane is reachable. Configure{' '}
              <code className="font-mono text-amber-200/80">VOX_ORCHESTRATOR_MESH_CONTROL_URL</code>{' '}
              (or <code className="font-mono text-amber-200/80">populi_control_url</code>) and rebuild
              with the <code className="font-mono text-amber-200/80">populi-transport</code> feature.
            </span>
          </div>
        )}

        <div className="grid grid-cols-12 gap-3">
          <div className="col-span-12 sm:col-span-6">
            <label className="mb-1 block font-display text-[10px] uppercase tracking-wider text-zinc-500">
              Target node (optional)
            </label>
            <select
              value={targetNode}
              onChange={(e) => setTargetNode(e.target.value)}
              disabled={!dispatchConfigured}
              className="w-full rounded-md border border-white/10 bg-black/40 px-2 py-1.5 text-xs text-zinc-200 disabled:opacity-40"
            >
              <option value="">Auto (control plane picks)</option>
              {nodes.map((n) => (
                <option key={n.id} value={n.id}>
                  {n.id}
                </option>
              ))}
            </select>
          </div>
          <div className="col-span-12 sm:col-span-6">
            <label className="mb-1 block font-display text-[10px] uppercase tracking-wider text-zinc-500">
              Task kind (optional)
            </label>
            <input
              value={taskKind}
              onChange={(e) => setTaskKind(e.target.value)}
              disabled={!dispatchConfigured}
              placeholder="e.g. text_infer"
              className="w-full rounded-md border border-white/10 bg-black/40 px-2 py-1.5 font-mono text-xs text-zinc-200 disabled:opacity-40"
            />
          </div>
          <div className="col-span-12">
            <label className="mb-1 block font-display text-[10px] uppercase tracking-wider text-zinc-500">
              Source (.vox)
            </label>
            <textarea
              value={source}
              onChange={(e) => setSource(e.target.value)}
              disabled={!dispatchConfigured}
              rows={5}
              placeholder={'fn main() {\n  print("hello from the mesh")\n}'}
              className="w-full rounded-md border border-white/10 bg-black/40 p-2 font-mono text-xs text-zinc-200 disabled:opacity-40"
            />
          </div>
        </div>

        <div className="mt-3 flex items-center gap-3">
          <button
            onClick={dispatch}
            disabled={!dispatchConfigured || dispatching || !source.trim()}
            className="flex items-center gap-1.5 rounded-md border border-brass/30 bg-brass/10 px-4 py-1.5 font-display text-[11px] tracking-wider uppercase text-brass transition hover:bg-brass/20 disabled:cursor-not-allowed disabled:opacity-40"
          >
            <Icon.send className="size-3.5" /> {dispatching ? 'Dispatching…' : 'Dispatch'}
          </button>
        </div>

        {dispatchResult && (
          <pre className="mt-3 max-h-64 overflow-auto rounded-md border border-white/10 bg-black/40 p-3 text-[11px] text-zinc-300">
            {dispatchResult}
          </pre>
        )}
      </Glass>
    </div>
  );
}
