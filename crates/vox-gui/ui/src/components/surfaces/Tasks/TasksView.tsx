import React, { useCallback, useEffect, useRef, useState } from 'react';
import { useLudusStore } from '../../gamify/store';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Icon } from '../../ui/Icons';
import { Button } from '../../ui/Button';
import { EmptyState } from '../../ui/EmptyState';
import { StatusPill } from '../../ui/StatusPill';
import { DataTable } from '../../ui/DataTable';
import { TaskRow, cyclePriority, filterBySession, findWriteOverlaps } from './tasksHelpers';
import { recordGamifyGuiEvent } from '../../../lib/gamifyGuiEvents';

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
}: {
  pushToast?: (t: unknown) => void;
  gamifyEnabled?: boolean;
}) {
  const [rows, setRows] = useState<TaskRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [editingId, setEditingId] = useState<number | null>(null);
  const [draft, setDraft] = useState('');
  const [newTask, setNewTask] = useState('');
  const [busy, setBusy] = useState(false);
  const [sessionFilter, setSessionFilter] = useState<string | null>(null);
  const mounted = useRef(true);

  const refresh = useCallback(async () => {
    try {
      const data = await invoke<TaskRow[]>('list_orchestrator_tasks');
      if (mounted.current) {
        setRows(data);
        setError(null);
      }
    } catch (e) {
      if (mounted.current) setError(String(e));
    } finally {
      if (mounted.current) setLoading(false);
    }
  }, []);

  useEffect(() => {
    mounted.current = true;
    refresh();
    const sub = listen<void>('vox://tasks-changed', () => {
      refresh();
    });
    return () => {
      mounted.current = false;
      sub.then((fn) => fn());
    };
  }, [refresh]);

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

  const addTask = () => {
    const description = newTask.trim();
    if (!description) return;
    setNewTask('');
    act(async () => {
      await invoke('submit_orchestrator_task', {
        input: { description, files: [], priority: null, session_id: null },
      });
      void recordGamifyGuiEvent('task_submitted', { description }, { enabled: gamifyEnabled });
    });
  };

  const saveEdit = (id: number) => {
    const description = draft.trim();
    setEditingId(null);
    if (!description) return;
    act(() => invoke('edit_orchestrator_task', { taskId: id, description }));
  };

  const remove = (id: number) => act(() => invoke('cancel_orchestrator_task', { taskId: id }));

  const reprioritize = (t: TaskRow) =>
    act(() =>
      invoke('reorder_orchestrator_task', { taskId: t.id, priority: cyclePriority(t.priority) })
    );

  const sessionTitles = loadSessionTitles();
  const presentSessions = Array.from(
    new Set(rows.map(r => r.session_id).filter((s): s is string => !!s))
  );
  const overlaps = findWriteOverlaps(rows);
  const filteredRows = filterBySession(rows, sessionFilter);

  const columns = [
    {
      key: 'priority',
      header: 'Priority',
      width: 100,
      render: (r: TaskRow) => (
        <button
          type="button"
          onClick={() => reprioritize(r)}
          className="focus:outline-none focus:ring-1 focus:ring-brass/40 rounded"
          aria-label={`Cycle priority for task ${r.id}, current priority: ${r.priority}`}
        >
          <StatusPill
            tone={r.priority === 'urgent' ? 'fail' : r.priority === 'background' ? 'neutral' : 'warn'}
            label={r.priority}
            size="xs"
          />
        </button>
      ),
    },
    {
      key: 'id',
      header: 'Task ID',
      width: 80,
      render: (r: TaskRow) => <span className="font-mono text-text-muted">#{r.id}</span>,
    },
    {
      key: 'description',
      header: 'Description',
      render: (r: TaskRow) => (
        <div className="flex flex-col gap-1 min-w-0">
          <div className="flex items-center gap-2">
            {editingId === r.id ? (
              <input
                type="text"
                value={draft}
                onChange={e => setDraft(e.target.value)}
                onBlur={() => saveEdit(r.id)}
                onKeyDown={e => {
                  if (e.key === 'Enter') saveEdit(r.id);
                  if (e.key === 'Escape') setEditingId(null);
                }}
                className="bg-bg-base border border-border-subtle rounded px-2 py-1 text-text-primary w-full outline-none focus:border-brass text-[13px]"
                autoFocus
              />
            ) : (
              <button
                type="button"
                onClick={() => {
                  setEditingId(r.id);
                  setDraft(r.description);
                  if (r.write_files && r.write_files.length > 0) {
                    useLudusStore.getState().setFocusedFile(r.write_files[0]);
                  }
                }}
                className="hover:text-brass cursor-pointer truncate text-[13px] text-text-secondary bg-transparent border-0 p-0 text-left w-full outline-none focus-visible:ring-1 focus-visible:ring-brass"
                title={r.description}
                aria-label={`Edit task description: ${r.description}`}
              >
                {r.description}
              </button>
            )}
          </div>
          <div className="flex items-center gap-1.5 flex-wrap">
            <span className="font-mono text-[9px] uppercase tracking-widest text-text-muted">
              #{r.id}
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
      ),
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
            Everything queued or running across the agent fleet. Chat submissions land here.
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

      <div className="flex items-center gap-2 rounded-xl border border-border-subtle bg-overlay-subtle px-3 py-2">
        <Icon.plus className="size-4 text-brass" />
        <input
          type="text"
          value={newTask}
          onChange={e => setNewTask(e.target.value)}
          placeholder="Add a task…"
          aria-label="Add a task"
          onKeyDown={e => {
            if (e.key === 'Enter') addTask();
          }}
          className="flex-1 bg-transparent text-[13px] text-text-primary placeholder:text-text-muted outline-none"
        />
        <Button variant="primary" size="sm" onClick={addTask} disabled={busy || !newTask.trim()}>
          Add
        </Button>
      </div>

      {presentSessions.length > 1 && (
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
                    : 'border-border-subtle bg-overlay-subtle text-text-muted hover:border-border-subtle hover:text-text-muted'
                }`}
              >
                {label}
              </button>
            );
          })}
        </div>
      )}

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
          groupBy={r => (r.lifecycle === 'in_progress' ? 'In progress' : 'Queued')}
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
