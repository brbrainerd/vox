import React, { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Glass } from '../../ui/Glass';
import { EmptyState } from '../../ui/EmptyState';
import { Icon } from '../../ui/Icons';
import { RUNS_POLL_MS, RUNS_LIST_LIMIT, SCOREBOARD_WINDOW_DAYS } from '../../../config/constants';
import { useFreshness } from '../../../hooks/useFreshness';

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
}

export function RunsView({ pushToast }: RunsViewProps) {
  const [scoreboard, setScoreboard] = useState<ScoreboardRow[]>([]);
  const [runs, setRuns] = useState<RunRow[]>([]);
  const [decision, setDecision] = useState<any>(null);
  const [selectedRunId, setSelectedRunId] = useState<string | null>(null);
  const [fetchedRun, setFetchedRun] = useState<RunRow | null>(null);
  const [loading, setLoading] = useState(true);
  const [lastRefreshAt, setLastRefreshAt] = useState<number | null>(null);
  const runsFreshness = useFreshness(lastRefreshAt, {
    usesPolling: true,
    freshMs: RUNS_POLL_MS * 2,
  });

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const sb = await invoke<ScoreboardRow[]>('get_model_scoreboard', { windowDays: SCOREBOARD_WINDOW_DAYS });
      setScoreboard(sb);
      const recent = await invoke<RunRow[]>('list_gui_runs', { limit: RUNS_LIST_LIMIT });
      setRuns(recent);
      const summary = await invoke<any>('get_routing_summary_live');
      setDecision(summary?.decision_preview ?? null);
      setLastRefreshAt(Date.now());
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Runs load failed', body: String(err) });
    } finally {
      setLoading(false);
    }
  }, [pushToast]);

  useEffect(() => {
    refresh();
    const id = setInterval(refresh, RUNS_POLL_MS);
    return () => clearInterval(id);
  }, [refresh]);

  // Replay-by-id: when a run is selected, fetch the single row from the backend so
  // runs that have scrolled out of the recent window (e.g. after a restart) still open.
  // Fall back to the in-memory list .find() if the invoke fails.
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

  return (
    <div className="grid grid-cols-12 gap-5">
      {decision ? (
        <Glass className="col-span-12 p-3">
          <div className="font-display text-[11px] tracking-[0.2em] uppercase text-zinc-400">Latest Route Decision</div>
          <div className="mt-1 font-mono text-xs text-zinc-200">{decision.selected_model}</div>
          <div className="text-[10px] text-zinc-500 mt-1">state={decision.discovery_state} · intel={decision.intelligence_score?.toFixed?.(2) ?? '—'} · eff={decision.efficiency_score?.toFixed?.(2) ?? '—'} · lat={decision.latency_score?.toFixed?.(2) ?? '—'}</div>
        </Glass>
      ) : null}

      <Glass className="col-span-12 xl:col-span-7 p-4 overflow-auto">
        <div className="mb-3 font-display text-sm tracking-widest uppercase text-zinc-200">Model Scoreboard (7d)</div>
        {loading && scoreboard.length === 0 ? (
          <div className="text-sm text-zinc-500">Loading scoreboard…</div>
        ) : scoreboard.length === 0 ? (
          <div className="text-sm text-zinc-500">No scoreboard rows yet — run `vox model rollup` after LLM traffic.</div>
        ) : (
          <table className="w-full text-left text-[11px] font-mono">
            <thead className="text-zinc-500 uppercase tracking-widest">
              <tr>
                <th className="pb-2">Model</th>
                <th>Cat</th>
                <th>Calls</th>
                <th>Ok%</th>
                <th>p50</th>
                <th>$/succ</th>
                <th>Q</th>
              </tr>
            </thead>
            <tbody>
              {scoreboard.slice(0, 40).map(row => (
                <tr key={`${row.model_id}-${row.task_category}-${row.strength_tag}`} className="border-t border-white/5">
                  <td className="py-2 pr-2 text-zinc-200 truncate max-w-[180px]" title={row.model_id}>{row.model_id}</td>
                  <td className="text-zinc-500">{row.task_category}</td>
                  <td>{row.n_calls}</td>
                  <td className={row.success_rate > 0.9 ? 'text-emerald-400' : row.success_rate > 0.75 ? 'text-amber-400' : 'text-rose-400'}>
                    {(row.success_rate * 100).toFixed(0)}
                  </td>
                  <td>{row.p50_latency_ms ?? '—'}</td>
                  <td>{row.cost_per_success_usd != null ? `$${row.cost_per_success_usd.toFixed(4)}` : '—'}</td>
                  <td>{row.quality_score.toFixed(2)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Glass>

      <Glass className="col-span-12 xl:col-span-5 p-4 overflow-auto">
        <div className="mb-3 flex items-center justify-between gap-2">
          <div className="font-display text-sm tracking-widest uppercase text-zinc-200">Recent Activity</div>
          <span
            className={`text-[9px] uppercase tracking-widest ${
              runsFreshness === 'stale' ? 'text-amber-400' : 'text-zinc-500'
            }`}
          >
            {runsFreshness === 'stale' ? 'stale' : 'polling'}
          </span>
        </div>
        {runs.length === 0 ? (
          <EmptyState
            icon={<Icon.flow className="size-8" />}
            title="No persisted runs yet"
            description="Submit a task from the composer or run a workflow — runs appear here with replay details."
          />
        ) : (
          <div className="flex flex-col gap-2">
            {runs.map(r => (
              <button
                key={r.run_id}
                onClick={() => setSelectedRunId(r.run_id)}
                className={`rounded-lg border p-3 text-left ${
                  selectedRun?.run_id === r.run_id
                    ? 'border-brass/50 bg-brass/10'
                    : 'border-white/5 bg-white/[0.02]'
                }`}
              >
                <div className="font-mono text-[10px] text-zinc-500">{r.run_id}</div>
                <div className="text-xs text-zinc-200 mt-1">{r.workflow_name}</div>
                <div className="text-[10px] text-zinc-500 mt-1">
                  {r.status} · {r.completed_steps}/{r.planned_steps} steps
                </div>
                {r.last_error ? <div className="text-[10px] text-rose-300 mt-1">{r.last_error}</div> : null}
              </button>
            ))}
            {selectedRun ? (
              <div className="mt-2 rounded-lg border border-white/10 bg-black/30 p-3">
                <div className="font-display text-[10px] tracking-[0.2em] uppercase text-zinc-400">Run Details</div>
                <div className="mt-2 text-[11px] text-zinc-200">{selectedRun.workflow_name}</div>
                <div className="font-mono text-[10px] text-zinc-500 mt-1">{selectedRun.run_id}</div>
                <div className="text-[10px] text-zinc-500 mt-1">
                  status={selectedRun.status} · steps={selectedRun.completed_steps}/{selectedRun.planned_steps}
                </div>
                <div className="text-[10px] text-zinc-500 mt-1">
                  updated={new Date(selectedRun.updated_at_ms).toLocaleString()}
                </div>
                {selectedRun.command ? (
                  <div className="font-mono text-[10px] text-zinc-400 mt-1 break-all">
                    cmd={selectedRun.command}
                  </div>
                ) : null}
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
              </div>
            ) : null}
          </div>
        )}
      </Glass>
    </div>
  );
}
