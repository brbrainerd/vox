import React, { useEffect, useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Glass } from '../../ui/Glass';
import { EmptyState } from '../../ui/EmptyState';
import { Icon } from '../../ui/Icons';

export interface ActivityRow {
  id: number;
  ts_ms: number;
  agent_id?: string;
  session_id?: string;
  kind: string;
  summary: string;
  detail_json: string;
}

export interface ActivityFilter {
  agent_id: string | null;
  kind: string | null;
  limit: number;
  before_id: number | null;
}

export interface ActivityTimelineProps {
  rows: ActivityRow[];
}

export function ActivityTimeline({ rows }: ActivityTimelineProps) {
  const getKindColorClass = (kind: string) => {
    switch (kind) {
      case 'AgentSpawned':
      case 'TaskStarted':
      case 'TaskCompleted':
      case 'WorkflowStarted':
      case 'WorkflowCompleted':
        return 'border-emerald-500/30 bg-emerald-500/5 text-emerald-300';
      case 'TaskFailed':
      case 'WorkflowFailed':
        return 'border-rose-500/30 bg-rose-500/5 text-rose-300';
      case 'BudgetAlert':
      case 'AttentionBudgetAlert':
      case 'ConflictDetected':
        return 'border-amber-500/30 bg-amber-500/5 text-amber-300';
      case 'CostIncurred':
        return 'border-sky-500/30 bg-sky-500/5 text-sky-300';
      default:
        return 'border-zinc-700/50 bg-zinc-800/10 text-zinc-300';
    }
  };

  const getIcon = (kind: string) => {
    switch (kind) {
      case 'AgentSpawned':
      case 'TaskStarted':
      case 'WorkflowStarted':
        return <Icon.bolt className="size-4" />;
      case 'TaskCompleted':
      case 'WorkflowCompleted':
        return <Icon.check className="size-4" />;
      case 'TaskFailed':
      case 'WorkflowFailed':
        return <Icon.x className="size-4" />;
      case 'BudgetAlert':
      case 'AttentionBudgetAlert':
      case 'ConflictDetected':
        return <Icon.alert className="size-4" />;
      case 'CostIncurred':
        return <Icon.cpu className="size-4" />;
      default:
        return <Icon.alert className="size-4" />;
    }
  };

  return (
    <div className="flex flex-col gap-3">
      {rows.map((row) => (
        <div
          key={row.id}
          data-testid="activity-row"
          className={`flex items-start gap-4 p-3 rounded-lg border text-xs leading-relaxed transition-all ${getKindColorClass(
            row.kind
          )}`}
        >
          <div className="flex items-center justify-center p-1.5 rounded bg-zinc-900/50 border border-zinc-800">
            {getIcon(row.kind)}
          </div>
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2 mb-1 flex-wrap">
              <span className="font-semibold uppercase tracking-wider text-[10px] opacity-75">
                {row.kind}
              </span>
              <span className="text-zinc-500">
                {new Date(row.ts_ms).toLocaleTimeString()}
              </span>
              {row.agent_id && (
                <span className="px-1.5 py-0.5 rounded bg-zinc-900/60 border border-zinc-800/50 text-[10px] text-zinc-400">
                  Agent: {row.agent_id}
                </span>
              )}
            </div>
            <p className="text-zinc-200 font-medium">{row.summary}</p>
          </div>
        </div>
      ))}
    </div>
  );
}

export interface ActivitySurfaceProps {
  pushToast: (t: { tone: 'ok' | 'warn' | 'error' | 'info'; title: string; body: string }) => void;
  gamifyEnabled?: boolean;
}

export function ActivitySurface({ pushToast }: ActivitySurfaceProps) {
  const [rows, setRows] = useState<ActivityRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [agentFilter, setAgentFilter] = useState<string>('');
  const [kindFilter, setKindFilter] = useState<string>('');

  const fetchLogs = useCallback(async () => {
    setLoading(true);
    try {
      const filter = {
        agent_id: agentFilter === '' ? null : agentFilter,
        kind: kindFilter === '' ? null : kindFilter,
        limit: 50,
        before_id: null,
      };
      const res = await invoke<ActivityRow[]>('activity_query', { filter });
      setRows(res);
    } catch (err) {
      pushToast({
        tone: 'error',
        title: 'Query Failed',
        body: String(err),
      });
    } finally {
      setLoading(false);
    }
  }, [agentFilter, kindFilter, pushToast]);

  useEffect(() => {
    fetchLogs();
  }, [fetchLogs]);

  // Reactive updates on "vox://activity-appended"
  useEffect(() => {
    const unlistenPromise = listen('vox://activity-appended', () => {
      fetchLogs();
    });
    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, [fetchLogs]);

  // Extract unique agents and kinds from current rows to populate filter lists dynamically
  const uniqueAgents = Array.from(
    new Set(rows.map((r) => r.agent_id).filter((id): id is string => !!id))
  ).sort();

  const uniqueKinds = Array.from(new Set(rows.map((r) => r.kind))).sort();

  return (
    <div className="flex flex-col h-full gap-4 p-4 text-zinc-300">
      <div className="flex flex-col gap-2">
        <h2 className="text-lg font-semibold tracking-wider text-zinc-100 flex items-center gap-2">
          <Icon.bolt className="text-emerald-400 size-5" />
          Agent Activity Timeline
        </h2>
        <p className="text-xs text-zinc-500">
          Durable log of high-signal events emitted across agent orchestrations.
        </p>
      </div>

      <Glass className="flex items-center gap-3 p-3 flex-wrap">
        <div className="flex flex-col gap-1 min-w-[120px]">
          <label className="text-[10px] uppercase font-bold tracking-wider text-zinc-500">
            Agent
          </label>
          <select
            value={agentFilter}
            onChange={(e) => setAgentFilter(e.target.value)}
            className="bg-zinc-900 border border-zinc-800 rounded px-2 py-1 text-xs text-zinc-300 focus:outline-none focus:border-zinc-700"
          >
            <option value="">All Agents</option>
            {uniqueAgents.map((id) => (
              <option key={id} value={id}>
                {id}
              </option>
            ))}
          </select>
        </div>

        <div className="flex flex-col gap-1 min-w-[150px]">
          <label className="text-[10px] uppercase font-bold tracking-wider text-zinc-500">
            Event Type
          </label>
          <select
            value={kindFilter}
            onChange={(e) => setKindFilter(e.target.value)}
            className="bg-zinc-900 border border-zinc-800 rounded px-2 py-1 text-xs text-zinc-300 focus:outline-none focus:border-zinc-700"
          >
            <option value="">All Kinds</option>
            {uniqueKinds.map((kind) => (
              <option key={kind} value={kind}>
                {kind}
              </option>
            ))}
          </select>
        </div>

        <div className="flex items-end h-full mt-auto">
          <button
            onClick={fetchLogs}
            disabled={loading}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded bg-zinc-800 border border-zinc-700 hover:bg-zinc-700 disabled:opacity-50 text-xs font-medium text-zinc-200 transition-colors"
          >
            <Icon.alert className={`size-3.5 ${loading ? 'animate-spin' : ''}`} />
            Refresh
          </button>
        </div>
      </Glass>

      <div className="flex-1 overflow-y-auto pr-1">
        {loading && rows.length === 0 ? (
          <div className="flex justify-center items-center py-20">
            <Icon.alert className="animate-spin text-zinc-600 size-8" />
          </div>
        ) : rows.length === 0 ? (
          <EmptyState
            variant="no-data"
            title="No Activity Logged"
            description="No high-signal agent events match your current filter settings."
          />
        ) : (
          <ActivityTimeline rows={rows} />
        )}
      </div>
    </div>
  );
}
