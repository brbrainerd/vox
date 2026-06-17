import React, { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Glass } from '../../ui/Glass';
import { EmptyState } from '../../ui/EmptyState';
import { StatusPill } from '../../ui/StatusPill';
import { DataTable } from '../../ui/DataTable';
import { Button } from '../../ui/Button';
import { Icon } from '../../ui/Icons';
import { RUNS_POLL_MS, RUNS_LIST_LIMIT, SCOREBOARD_WINDOW_DAYS } from '../../../config/constants';
import { recordGamifyGuiEvent } from '../../../lib/gamifyGuiEvents';

interface ScoreboardRow {
  model_id: string;
  task_category: string;
  strength_tag: string;
  n_calls: number;
  success_rate: number;
  p50_latency_ms?: number | null;
  cost_per_success_usd?: number | null;
  quality_score: number;
}

interface RunRow {
  run_id: string;
  workflow_name: string;
  status: string;
  planned_steps: number;
  completed_steps: number;
  updated_at_ms: number;
  last_error?: string | null;
  command?: string | null;
  model?: string | null;
  cost_usd?: number | null;
}

interface RunsViewProps {
  pushToast: (t: any) => void;
  gamifyEnabled?: boolean;
}

function isRunCompleted(status: string): boolean {
  return /complete|done|success/i.test(status);
}

export function RunsView({ pushToast, gamifyEnabled = false }: RunsViewProps) {
  const [scoreboard, setScoreboard] = useState<ScoreboardRow[]>([]);
  const [runs, setRuns] = useState<RunRow[]>([]);
  const [decision, setDecision] = useState<any>(null);
  const [selectedRunId, setSelectedRunId] = useState<string | null>(null);
  const [fetchedRun, setFetchedRun] = useState<RunRow | null>(null);
  const [loading, setLoading] = useState(true);
  const rewardedRunIds = useRef<Set<string>>(new Set());

  const refresh = useCallback(async () => {
    try {
      const sb = await invoke<ScoreboardRow[]>('get_model_scoreboard', { windowDays: SCOREBOARD_WINDOW_DAYS });
      setScoreboard(sb);
      const recent = await invoke<RunRow[]>('list_gui_runs', { limit: RUNS_LIST_LIMIT });
      setRuns(recent);
      for (const run of recent) {
        if (isRunCompleted(run.status) && !rewardedRunIds.current.has(run.run_id)) {
          rewardedRunIds.current.add(run.run_id);
          void recordGamifyGuiEvent(
            'workflow_completed',
            { run_id: run.run_id, workflow_name: run.workflow_name },
            { enabled: gamifyEnabled }
          );
        }
      }
      const summary = await invoke<any>('get_routing_summary_live');
      setDecision(summary?.decision_preview ?? null);
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Runs load failed', body: String(err) });
    } finally {
      setLoading(false);
    }
  }, [pushToast, gamifyEnabled]);

  useEffect(() => {
    refresh();
    const id = setInterval(refresh, RUNS_POLL_MS);
    return () => clearInterval(id);
  }, [refresh]);

  useEffect(() => {
    if (!selectedRunId) {
      setFetchedRun(null);
      return;
    }
    let cancelled = false;
    invoke<RunRow | null>('get_gui_run', { runId: selectedRunId })
      .then((row) => {
        if (!cancelled && row) setFetchedRun(row);
      })
      .catch(() => {
        if (!cancelled) setFetchedRun(null);
      });
    return () => {
      cancelled = true;
    };
  }, [selectedRunId]);

  const listSelected = runs.find((r) => r.run_id === selectedRunId) ?? runs[0] ?? null;
  const selectedRun =
    fetchedRun && fetchedRun.run_id === (selectedRunId ?? listSelected?.run_id)
      ? fetchedRun
      : listSelected;

  const scoreboardCols = [
    { key: 'model_id', header: 'Model' },
    { key: 'task_category', header: 'Cat' },
    { key: 'n_calls', header: 'Calls' },
    { 
      key: 'success_rate', 
      header: 'Ok%', 
      render: (r: ScoreboardRow) => (
        <span className={r.success_rate > 0.9 ? 'text-emerald-400' : r.success_rate > 0.75 ? 'text-amber-400' : 'text-rose-400'}>
          {(r.success_rate * 100).toFixed(0)}%
        </span>
      ) 
    },
    { 
      key: 'p50_latency_ms', 
      header: 'p50', 
      render: (r: ScoreboardRow) => <span>{r.p50_latency_ms ? `${(r.p50_latency_ms / 1000).toFixed(1)}s` : '—'}</span> 
    },
    {
      key: 'cost_per_success_usd',
      header: '$/succ',
      render: (r: ScoreboardRow) => <span>{r.cost_per_success_usd != null ? `$${r.cost_per_success_usd.toFixed(4)}` : '—'}</span>
    },
    { 
      key: 'quality_score', 
      header: 'Q', 
      render: (r: ScoreboardRow) => <span className="font-mono font-bold text-brass">{r.quality_score.toFixed(2)}</span> 
    },
  ];

  const runsCols = [
    { 
      key: 'run_id', 
      header: 'Run ID', 
      render: (r: RunRow) => <span className="font-mono text-zinc-400">#{r.run_id}</span> 
    },
    { 
      key: 'workflow_name', 
      header: 'Workflow',
      render: (r: RunRow) => (
        <button
          type="button"
          onClick={() => setSelectedRunId(r.run_id)}
          aria-pressed={selectedRun?.run_id === r.run_id}
          className="hover:text-brass text-left font-medium outline-none focus:ring-1 focus:ring-brass/40 rounded px-1 -mx-1"
        >
          {r.workflow_name}
        </button>
      )
    },
    { 
      key: 'status', 
      header: 'Status', 
      render: (r: RunRow) => (
        <StatusPill 
          tone={r.status === 'success' || r.status === 'complete' || r.status === 'done' ? 'pass' : r.status === 'failed' ? 'fail' : 'Executing'} 
          label={r.status} 
          size="xs" 
        />
      ) 
    },
    { 
      key: 'steps', 
      header: 'Steps', 
      render: (r: RunRow) => <span>{r.completed_steps}/{r.planned_steps}</span> 
    },
  ];

  return (
    <div className="grid grid-cols-12 gap-5 p-4 h-full overflow-auto">
      {decision && (
        <Glass className="col-span-12 p-3">
          <div className="font-display text-[11px] tracking-[0.2em] uppercase text-zinc-400">Latest Route Decision</div>
          <div className="mt-1 font-mono text-xs text-zinc-200">{decision.selected_model}</div>
          <div className="text-[10px] text-zinc-500 mt-1">
            state={decision.discovery_state}
            {decision.intelligence_score != null && ` · intel=${decision.intelligence_score.toFixed(2)}`}
            {decision.efficiency_score != null && ` · eff=${decision.efficiency_score.toFixed(2)}`}
            {decision.latency_score != null && ` · lat=${decision.latency_score.toFixed(2)}`}
          </div>
        </Glass>
      )}

      <div className="col-span-12 xl:col-span-7 flex flex-col gap-3">
        <h3 className="font-display text-sm tracking-widest uppercase text-zinc-200">Model Scoreboard (7d)</h3>
        <DataTable
          rows={scoreboard}
          columns={scoreboardCols}
          getRowId={r => `${r.model_id}-${r.task_category}`}
          loading={loading}
          density="compact"
          emptyState={
            <EmptyState 
              variant="no-data" 
              title="No model runs tracked yet" 
              description="Scoreboard data accumulates dynamically once agents complete routing workflows."
            />
          }
        />
      </div>

      <div className="col-span-12 xl:col-span-5 flex flex-col gap-3">
        <h3 className="font-display text-sm tracking-widest uppercase text-zinc-200">Recent Activity</h3>
        <DataTable
          rows={runs}
          columns={runsCols}
          getRowId={r => r.run_id}
          loading={loading}
          density="compact"
          emptyState={
            <EmptyState 
              variant="no-data" 
              title="No recent workflows" 
              description="A list of task executions and model calls appears here dynamically."
            />
          }
        />
        {selectedRun && (
          <Glass size="sm" className="mt-2 rounded-lg border border-white/10 bg-black/30 p-3">
            <div className="font-display text-[10px] tracking-[0.2em] uppercase text-zinc-400">Run Details</div>
            <div className="mt-2 text-[11px] text-zinc-200">{selectedRun.workflow_name}</div>
            <div className="font-mono text-[10px] text-zinc-500 mt-1">{selectedRun.run_id}</div>
            <div className="text-[10px] text-zinc-500 mt-1">
              status={selectedRun.status} · steps={selectedRun.completed_steps}/{selectedRun.planned_steps}
            </div>
            <div className="text-[10px] text-zinc-500 mt-1">
              updated={new Date(selectedRun.updated_at_ms).toLocaleString()}
            </div>
            {selectedRun.command && (
              <div className="font-mono text-[10px] text-zinc-400 mt-1 break-all">
                cmd={selectedRun.command}
              </div>
            )}
            {selectedRun.model || selectedRun.cost_usd != null ? (
              <div className="text-[10px] text-zinc-500 mt-1">
                {selectedRun.model ? `model=${selectedRun.model}` : null}
                {selectedRun.model && selectedRun.cost_usd != null ? ' · ' : null}
                {selectedRun.cost_usd != null ? `cost=$${selectedRun.cost_usd.toFixed(4)}` : null}
              </div>
            ) : null}
            {selectedRun.last_error ? (
              <pre className="mt-2 whitespace-pre-wrap rounded border border-rose-300/20 bg-rose-950/20 p-2 text-[10px] text-rose-200">
                {selectedRun.last_error}
              </pre>
            ) : (
              <div className="mt-2 text-[10px] text-emerald-300">No recorded error for this run.</div>
            )}
          </Glass>
        )}
      </div>
    </div>
  );
}
