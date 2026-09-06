import React, { useEffect, useState, useCallback } from 'react';
import { sanitizeErrorForToast } from '../../../lib/backendGuard';
import { Glass } from '../../ui/Glass';
import { EmptyState } from '../../ui/EmptyState';
import { Icon } from '../../ui/Icons';
import {
  activityQuery,
  listenActivityAppended,
  listenAgentEvents,
  type ActivityRowDto as ActivityRow,
  type ActivityFilterDto as ActivityFilter,
} from '../../../transport';
import type { Toast } from '../../../types/tauri';

export type { ActivityRow, ActivityFilter };

export interface ActivityTimelineProps {
  rows: ActivityRow[];
}

type TimelineItem =
  | { type: 'single'; row: ActivityRow }
  | { type: 'folded'; rows: ActivityRow[]; key: string; totalCost: number; agentId: string };

function getCostUsd(row: ActivityRow): number {
  try {
    const parsed = JSON.parse(row.detail_json);
    if (parsed && typeof parsed === 'object') {
      if ('CostIncurred' in parsed && parsed.CostIncurred && typeof parsed.CostIncurred === 'object' && 'cost_usd' in parsed.CostIncurred) {
        return Number(parsed.CostIncurred.cost_usd) || 0;
      }
      if ('cost_usd' in parsed) {
        return Number(parsed.cost_usd) || 0;
      }
    }
  } catch (e) {
    // Fall back to summary parsing
  }
  const match = row.summary.match(/Cost incurred: \$([0-9.]+)/);
  if (match) {
    return parseFloat(match[1]) || 0;
  }
  return 0;
}

function foldCostRuns(rows: ActivityRow[]): TimelineItem[] {
  const result: TimelineItem[] = [];
  let i = 0;
  while (i < rows.length) {
    const current = rows[i];
    if (current.kind === 'CostIncurred' && current.agent_id) {
      const run: ActivityRow[] = [current];
      let j = i + 1;
      while (j < rows.length) {
        const next = rows[j];
        if (next.kind === 'CostIncurred' && next.agent_id === current.agent_id) {
          run.push(next);
          j++;
        } else {
          break;
        }
      }
      if (run.length >= 3) {
        let totalCost = 0;
        for (const r of run) {
          totalCost += getCostUsd(r);
        }
        result.push({
          type: 'folded',
          rows: run,
          key: `folded-${current.id}-${j}`,
          totalCost,
          agentId: current.agent_id,
        });
        i = j;
      } else {
        for (const r of run) {
          result.push({ type: 'single', row: r });
        }
        i = j;
      }
    } else {
      result.push({ type: 'single', row: current });
      i++;
    }
  }
  return result;
}

export function ActivityTimeline({ rows }: ActivityTimelineProps) {
  const [expandedKeys, setExpandedKeys] = useState<Record<string, boolean>>({});

  const toggleExpand = (key: string) => {
    setExpandedKeys((prev) => ({ ...prev, [key]: !prev[key] }));
  };

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

  const items = foldCostRuns(rows);

  return (
    <div className="flex flex-col gap-3">
      {items.map((item) => {
        if (item.type === 'single') {
          const row = item.row;
          return (
            <div
              key={row.id}
              data-testid="activity-row"
              className={`flex items-start gap-4 p-3 rounded-lg border text-xs leading-relaxed transition-all ${getKindColorClass(
                row.kind
              )}`}
            >
              <div className="flex items-center justify-center p-1.5 rounded-sm bg-zinc-900/50 border border-zinc-800">
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
                    <span className="px-1.5 py-0.5 rounded-sm bg-zinc-900/60 border border-zinc-800/50 text-[10px] text-zinc-400">
                      Agent: {row.agent_id}
                    </span>
                  )}
                </div>
                <p className="text-zinc-200 font-medium">{row.summary}</p>
              </div>
            </div>
          );
        } else {
          const isExpanded = !!expandedKeys[item.key];
          return (
            <div
              key={item.key}
              data-testid="activity-row"
              className={`flex flex-col gap-2 p-3 rounded-lg border text-xs leading-relaxed transition-all ${getKindColorClass(
                'CostIncurred'
              )}`}
            >
              <div className="flex items-start gap-4">
                <div className="flex items-center justify-center p-1.5 rounded-sm bg-zinc-900/50 border border-zinc-800">
                  {getIcon('CostIncurred')}
                </div>
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2 mb-1 flex-wrap">
                    <span className="font-semibold uppercase tracking-wider text-[10px] opacity-75">
                      CostIncurred (Folded)
                    </span>
                    <span className="text-zinc-500">
                      {new Date(item.rows[0].ts_ms).toLocaleTimeString()}
                    </span>
                    {item.agentId && (
                      <span className="px-1.5 py-0.5 rounded-sm bg-zinc-900/60 border border-zinc-800/50 text-[10px] text-zinc-400">
                        Agent: {item.agentId}
                      </span>
                    )}
                  </div>
                  <div className="flex items-center justify-between gap-4">
                    <p className="text-zinc-200 font-medium">
                      spent ${item.totalCost.toFixed(4)} ({item.rows.length} calls)
                    </p>
                    <button
                      onClick={() => toggleExpand(item.key)}
                      className="px-2 py-0.5 rounded-sm bg-zinc-900/60 border border-zinc-800 hover:bg-zinc-800 text-[10px] text-zinc-400 hover:text-zinc-200 font-medium transition-colors"
                      data-testid="fold-toggle"
                    >
                      {isExpanded ? 'Hide' : 'Expand'}
                    </button>
                  </div>
                </div>
              </div>
              {isExpanded && (
                <div className="pl-12 pr-2 py-1 flex flex-col gap-1.5 border-t border-zinc-800/30 mt-1">
                  {item.rows.map((row) => (
                    <div
                      key={row.id}
                      className="flex items-center justify-between text-[11px] text-zinc-400 hover:text-zinc-300 py-0.5"
                    >
                      <span className="truncate">{row.summary}</span>
                      <span className="text-zinc-500 ml-2 whitespace-nowrap">
                        {new Date(row.ts_ms).toLocaleTimeString()}
                      </span>
                    </div>
                  ))}
                </div>
              )}
            </div>
          );
        }
      })}
    </div>
  );
}

export interface ActivitySurfaceProps {
  pushToast: (t: Toast) => void;
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
      const res = await activityQuery(filter);
      // activityQuery is typed ActivityRowDto[], but a backend/IPC (or the visual-audit
      // mock) can yield null/undefined; never let a non-array reach state (rows.map()).
      setRows(Array.isArray(res) ? res : []);
    } catch (err) {
      pushToast({
        tone: 'warn',
        title: 'Query Failed',
        body: sanitizeErrorForToast(err),
        cause: 'backend-error',
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
    // listen() rejects when the Tauri event bridge is unavailable (bare
    // browser, tests, headless capture) — guard so nothing leaks an
    // unhandled rejection and cleanup still resolves.
    const unlistenPromise = listenActivityAppended(() => {
      fetchLogs();
    }).catch(() => undefined);
    return () => {
      unlistenPromise.then((unlisten) => unlisten?.());
    };
  }, [fetchLogs]);

  // Also refresh on "vox://agent-events" — this event IS already emitted by the
  // Rust daemon bridge (spawn_agent_event_stream), whereas "vox://activity-appended"
  // has no Rust emitter yet. This makes the timeline update live without any new
  // backend work (Option B: lazy reactive refresh).
  useEffect(() => {
    // Guarded like the effect above: listen() rejects outside Tauri.
    const unlistenPromise = listenAgentEvents(() => {
      fetchLogs();
    }).catch(() => undefined);
    return () => {
      unlistenPromise.then((unlisten) => unlisten?.());
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
            className="bg-zinc-900 border border-zinc-800 rounded-sm px-2 py-1 text-xs text-zinc-300 focus:outline-hidden focus:border-zinc-700"
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
            className="bg-zinc-900 border border-zinc-800 rounded-sm px-2 py-1 text-xs text-zinc-300 focus:outline-hidden focus:border-zinc-700"
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
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-sm bg-zinc-800 border border-zinc-700 hover:bg-zinc-700 disabled:opacity-50 text-xs font-medium text-zinc-200 transition-colors"
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
