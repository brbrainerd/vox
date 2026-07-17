import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useLudusStore } from '../../gamify/store';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Icon } from '../../ui/Icons';
import { Button } from '../../ui/Button';
import { EmptyState } from '../../ui/EmptyState';
import { StatusPill } from '../../ui/StatusPill';
import { DataTable } from '../../ui/DataTable';
import { TaskRow, filterBySession, findWriteOverlaps, mapHopperTasksToRows, type HopperTaskDto } from './tasksHelpers';
import { recordGamifyGuiEvent } from '../../../lib/gamifyGuiEvents';
import { TaskComposer } from './TaskComposer';
import { feedbackList, hopperList, listenFeedbackChanged, type FeedbackRow } from '../../../transport';
import type { AttentionInbox } from '../../../hooks/useAttentionInbox';

interface StoredSession { id: string; title: string }

function loadSessionTitles(): Record<string, string> {
  try {
    const raw = localStorage.getItem('vox_chat_sessions');
    if (!raw) return {};
    const list = JSON.parse(raw) as StoredSession[];
    return Object.fromEntries(list.map(s => [s.id, s.title]));
  } catch {
    return {};
  }
}

export function TasksView({
  pushToast: _pushToast,
  gamifyEnabled = false,
  attention,
}: {
  pushToast?: (t: unknown) => void;
  gamifyEnabled?: boolean;
  /** When provided, this surface sources its task/feedback data from the
   *  shared inbox instead of self-fetching (App owns polling via
   *  useAttentionInbox) — mirrors NeedsYouSurface's dual-mode pattern. */
  attention?: AttentionInbox;
}) {
  const [selfHopperTasks, setSelfHopperTasks] = useState<HopperTaskDto[]>([]);
  const [selfNeedsYou, setSelfNeedsYou] = useState<FeedbackRow[]>([]);
  const [loading, setLoading] = useState(!attention);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [sessionFilter, setSessionFilter] = useState<string | null>(null);
  const [showBlocked, setShowBlocked] = useState(true);
  const mounted = useRef(true);

  const selfRefresh = useCallback(async () => {
    try {
      const [data, feedback] = await Promise.all([
        hopperList(),
        feedbackList().catch(() => ({ needsYou: [] as FeedbackRow[] })),
      ]);
      if (mounted.current) {
        setSelfHopperTasks(data);
        setSelfNeedsYou(feedback?.needsYou ?? []);
        setError(null);
      }
    } catch (e) {
      if (mounted.current) setError(String(e));
    } finally {
      if (mounted.current) setLoading(false);
    }
  }, []);

  const refresh = useCallback(async () => {
    if (attention) {
      await attention.refresh();
      return;
    }
    await selfRefresh();
  }, [attention, selfRefresh]);

  useEffect(() => {
    mounted.current = true;
    // Shared-inbox mode: App already owns polling via useAttentionInbox.
    // Skip the self-fetch effect (and its listeners) entirely.
    if (attention) return () => { mounted.current = false; };

    selfRefresh();
    // listen() rejects when the Tauri event bridge is unavailable (bare
    // browser, tests) — guard so nothing leaks an unhandled rejection and
    // cleanup still resolves.
    const sub = listen<void>('vox://tasks-changed', () => {
      selfRefresh();
    }).catch(() => undefined);
    const subFeedback = listenFeedbackChanged(() => {
      selfRefresh();
    }).catch(() => undefined);
    return () => {
      mounted.current = false;
      sub.then((fn) => fn?.());
      subFeedback.then((fn) => fn?.());
    };
  }, [attention, selfRefresh]);

  const rows: TaskRow[] = useMemo(() => {
    const tasks = attention ? attention.hopperTasks : selfHopperTasks;
    const feedbackNeedsYou = attention ? attention.needsYou : selfNeedsYou;
    const gateSet = new Set<number>(feedbackNeedsYou.flatMap(f => f.gates ?? []));
    return mapHopperTasksToRows(tasks, gateSet);
  }, [attention, selfHopperTasks, selfNeedsYou]);

  const act = useCallback(
    async (fn: () => Promise<unknown>) => {
      setBusy(true);
      try {
        await fn();
        await refresh();
      } catch (e) {
        setError(String(e));
      } finally {
        setBusy(false);
      }
    },
    [refresh]
  );

  const addTask = (intent: string) => {
    act(async () => {
      await invoke('hopper_submit', { intent, affinity: [] });
      void recordGamifyGuiEvent('task_submitted', { description: intent }, { enabled: gamifyEnabled });
    });
  };

  const remove = (id: string | number) => act(() => invoke('hopper_cancel', { itemId: String(id) }));

  const reprioritize = (id: string | number, priority: number) =>
    act(() => invoke('hopper_reprioritize', { itemId: String(id), priority }));

  const sessionTitles = loadSessionTitles();
  const presentSessions = Array.from(
    new Set(rows.map(r => r.session_id).filter((s): s is string => !!s))
  );
  const overlaps = findWriteOverlaps(rows);
  const filteredRows = filterBySession(rows, sessionFilter).filter(
    r => showBlocked || r.lifecycle !== 'blocked'
  );

  const columns = [
    {
      key: 'priority',
      header: 'Priority',
      width: 110,
      render: (r: TaskRow) => (
        <select
          value={r.priority === 'urgent' ? 2 : r.priority === 'background' ? 0 : 1}
          onChange={(e) => {
            const val = Number(e.target.value);
            reprioritize(r.id, val);
          }}
          disabled={busy || r.lifecycle === 'blocked'}
          className="bg-zinc-900 text-zinc-100 border border-white/10 rounded px-1.5 py-0.5 text-xs outline-none focus:border-brass/50 cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed"
        >
          <option value={2}>Urgent</option>
          <option value={1}>Normal</option>
          <option value={0}>Background</option>
        </select>
      ),
    },
    {
      key: 'id',
      header: 'Task ID',
      width: 80,
      render: (r: TaskRow) => <span className="font-mono text-text-muted">#{r.id.toString().slice(0, 8)}</span>,
    },
    {
      key: 'description',
      header: 'Description',
      render: (r: TaskRow) => {
        const isBlocked = r.lifecycle === 'blocked';
        return (
          <div className={`flex flex-col gap-1 min-w-0 ${isBlocked ? 'opacity-55' : ''}`}>
            <div className="flex items-center gap-2">
              <span
                onClick={() => {
                  if (!isBlocked && r.write_files && r.write_files.length > 0) {
                    useLudusStore.getState().setFocusedFile(r.write_files[0]);
                  }
                }}
                className={`text-[13px] text-text-secondary truncate ${isBlocked ? '' : 'cursor-pointer hover:text-brass'}`}
                title={r.description}
              >
                {r.description}
              </span>
              {isBlocked && (
                <span className="text-[10px] text-amber-400 font-medium inline-flex items-center gap-1">
                  ⛔ waiting on Needs You
                </span>
              )}
            </div>
            <div className="flex items-center gap-1.5 flex-wrap">
              <span className="font-mono text-[9px] uppercase tracking-widest text-text-muted">
                #{r.id.toString().slice(0, 8)}
                {r.agent_id != null ? ` · agent ${r.agent_id}` : ''}
                {' · '}
                {r.lifecycle}
              </span>
              {r.depends_on.length > 0 && (
                <span
                  title="Runs after the listed task(s) complete"
                  className="rounded border border-border-subtle bg-overlay-subtle px-1 font-mono text-[9px] text-text-muted"
                >
                  → after #{r.depends_on.join(', #')}
                </span>
              )}
              {(overlaps.get(r.id)?.length ?? 0) > 0 && (
                <span
                  title="These tasks write the same files — the orchestrator serializes them via file locks and may split VCS changes"
                  className="rounded border border-amber-400/30 bg-amber-400/10 px-1 font-mono text-[9px] text-amber-300"
                >
                  ⚠ overlaps #{overlaps.get(r.id)!.join(', #')}
                </span>
              )}
              {r.remote_node && (
                <span
                  title="Executing remotely on a mesh node via A2A lease"
                  className="rounded border border-cyan-400/30 bg-cyan-400/10 px-1 font-mono text-[9px] text-cyan-300"
                >
                  mesh: {r.remote_node}
                </span>
              )}
            </div>
          </div>
        );
      },
    },
    {
      key: 'actions',
      header: '',
      width: 50,
      render: (r: TaskRow) => (
        <div className="flex justify-end">
          <Button
            variant="ghost"
            size="xs"
            onClick={() => remove(r.id)}
            disabled={busy}
            title="Cancel task"
          >
            <Icon.x className="size-3.5 text-text-muted hover:text-red-400 transition" />
          </Button>
        </div>
      ),
    },
  ];

  return (
    <div className="flex flex-col gap-4 p-6 h-full overflow-auto">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-[15px] font-medium text-text-primary">Tasks</h1>
          <p className="text-[11px] text-text-muted">
            The hopper to-do queue — items added from the composer below. Chat
            submissions run in the orchestrator task graph and are not listed here yet.
          </p>
        </div>
        <Button
          variant="ghost"
          size="icon"
          onClick={refresh}
          aria-label="Refresh tasks"
          title="Refresh"
          className="border border-border-subtle text-text-muted hover:bg-overlay-subtle"
        >
          <Icon.refresh className="size-4" />
        </Button>
      </div>

      <TaskComposer onSubmit={addTask} busy={busy} />

      <div className="flex items-center justify-between gap-1.5 flex-wrap">
        {presentSessions.length > 1 ? (
          <div className="flex items-center gap-1.5 flex-wrap">
            {[null, ...presentSessions].map(sid => {
              const active = sessionFilter === sid;
              const label = sid == null ? 'All sessions' : (sessionTitles[sid] ?? sid);
              return (
                <button
                  key={sid ?? '__all__'}
                  type="button"
                  onClick={() => setSessionFilter(sid)}
                  aria-pressed={active}
                  className={`rounded-full border px-2.5 py-0.5 font-mono text-[10px] uppercase tracking-widest transition-colors focus:outline-none focus-visible:ring-1 focus-visible:ring-brass/40 ${
                    active
                      ? 'border-brass/40 bg-brass/10 text-brass'
                      : 'border-border-subtle bg-overlay-subtle text-text-muted hover:border-border-subtle hover:text-text-secondary'
                  }`}
                >
                  {label}
                </button>
              );
            })}
          </div>
        ) : <div />}
        <label className="flex items-center gap-2 text-[11px] text-text-muted cursor-pointer select-none">
          <input
            type="checkbox"
            checked={showBlocked}
            onChange={(e) => setShowBlocked(e.target.checked)}
            className="rounded border-border-subtle bg-bg-base text-brass focus:ring-brass/40 focus:ring-offset-bg-base size-3.5"
          />
          Show blocked tasks
        </label>
      </div>

      {error && (
        <div
          role="alert"
          className="flex items-center gap-1.5 rounded-lg border border-red-400/20 bg-red-400/5 px-3 py-2 text-[11px] text-red-300"
        >
          <span className="font-bold">Error:</span>
          <span>{error}</span>
        </div>
      )}

      <div className="flex-1 overflow-auto custom-scrollbar">
        <DataTable
          rows={filteredRows}
          columns={columns}
          getRowId={r => String(r.id)}
          groupBy={r => (r.lifecycle === 'in_progress' ? 'In progress' : r.lifecycle === 'blocked' ? 'Blocked' : 'Queued')}
          loading={loading}
          emptyState={
            <EmptyState
              variant="no-data"
              title="No tasks in this workspace"
              description="Create a new task at the top to instruct the background agent."
            />
          }
        />
      </div>
    </div>
  );
}

