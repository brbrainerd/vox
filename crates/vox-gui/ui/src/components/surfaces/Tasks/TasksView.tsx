import React, { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Icon } from '../../ui/Icons';
import { TaskRow, groupTasks, cyclePriority, filterBySession, findWriteOverlaps } from './tasksHelpers';

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

const POLL_MS = 4000;

const PRIORITY_STYLE: Record<string, string> = {
  urgent: 'text-red-300 border-red-400/30 bg-red-400/10',
  normal: 'text-zinc-300 border-white/10 bg-white/[0.03]',
  background: 'text-zinc-500 border-white/5 bg-transparent',
};

function PriorityChip({ value, onCycle }: { value: string; onCycle: () => void }) {
  return (
    <button
      onClick={onCycle}
      title="Click to cycle priority (urgent → background → normal)"
      className={`shrink-0 rounded border px-1.5 py-px font-mono text-[9px] uppercase tracking-widest transition focus:outline-none focus:ring-1 focus:ring-brass/40 ${PRIORITY_STYLE[value] ?? PRIORITY_STYLE.normal}`}
    >
      {value}
    </button>
  );
}

export function TasksView(_props: { pushToast?: (t: unknown) => void }) {
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
    const t = setInterval(refresh, POLL_MS);
    return () => {
      mounted.current = false;
      clearInterval(t);
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
    [refresh],
  );

  const addTask = () => {
    const description = newTask.trim();
    if (!description) return;
    setNewTask('');
    act(() =>
      invoke('submit_orchestrator_task', {
        input: { description, files: [], priority: null, session_id: null },
      }),
    );
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
      invoke('reorder_orchestrator_task', { taskId: t.id, priority: cyclePriority(t.priority) }),
    );

  const sessionTitles = loadSessionTitles();
  const presentSessions = Array.from(
    new Set(rows.map(r => r.session_id).filter((s): s is string => !!s)),
  );
  const overlaps = findWriteOverlaps(rows);
  const { inProgress, queued } = groupTasks(filterBySession(rows, sessionFilter));

  const renderRow = (t: TaskRow, editable: boolean) => (
    <div
      key={t.id}
      className="group flex items-center gap-2 rounded-lg border border-white/5 bg-white/[0.02] px-3 py-2"
    >
      <PriorityChip value={t.priority} onCycle={() => editable && reprioritize(t)} />
      <div className="min-w-0 flex-1">
        {editingId === t.id ? (
          <input
            autoFocus
            value={draft}
            onChange={e => setDraft(e.target.value)}
            onKeyDown={e => {
              if (e.key === 'Enter') saveEdit(t.id);
              if (e.key === 'Escape') setEditingId(null);
            }}
            onBlur={() => saveEdit(t.id)}
            className="w-full bg-transparent text-[13px] text-zinc-100 outline-none border-b border-brass/40"
          />
        ) : (
          <span className="block truncate text-[13px] text-zinc-200" title={t.description}>
            {t.description}
          </span>
        )}
        <div className="flex items-center gap-1.5 flex-wrap">
          <span className="font-mono text-[9px] uppercase tracking-widest text-zinc-600">
            #{t.id}
            {t.agent_id != null ? ` · agent ${t.agent_id}` : ''}
            {' · '}
            {t.lifecycle}
          </span>
          {t.depends_on.length > 0 && (
            <span
              title="Runs after the listed task(s) complete"
              className="rounded border border-white/10 bg-white/[0.03] px-1 font-mono text-[9px] text-zinc-400"
            >
              → after #{t.depends_on.join(', #')}
            </span>
          )}
          {(overlaps.get(t.id)?.length ?? 0) > 0 && (
            <span
              title="These tasks write the same files — the orchestrator serializes them via file locks and may split VCS changes"
              className="rounded border border-amber-400/30 bg-amber-400/10 px-1 font-mono text-[9px] text-amber-300"
            >
              ⚠ overlaps #{overlaps.get(t.id)!.join(', #')}
            </span>
          )}
        </div>
      </div>
      {editable && (
        <div className="flex shrink-0 items-center gap-1 opacity-0 transition group-hover:opacity-100 focus-within:opacity-100">
          <button
            title="Edit task text"
            onClick={() => {
              setEditingId(t.id);
              setDraft(t.description);
            }}
            className="rounded p-1 text-zinc-400 hover:bg-white/[0.06] hover:text-zinc-100 focus:outline-none focus:ring-1 focus:ring-brass/40"
          >
            <Icon.edit className="size-3.5" />
          </button>
          <button
            title="Remove task"
            onClick={() => remove(t.id)}
            className="rounded p-1 text-zinc-400 hover:bg-red-400/10 hover:text-red-300 focus:outline-none focus:ring-1 focus:ring-red-400/40"
          >
            <Icon.trash className="size-3.5" />
          </button>
        </div>
      )}
    </div>
  );

  return (
    <div className="flex h-full flex-col gap-4 p-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-[15px] font-medium text-zinc-100">Tasks</h1>
          <p className="text-[11px] text-zinc-500">
            Everything queued or running across the agent fleet. Chat submissions land here.
          </p>
        </div>
        <button
          onClick={refresh}
          title="Refresh"
          className="rounded-lg border border-white/10 p-1.5 text-zinc-400 hover:bg-white/[0.05] focus:outline-none focus:ring-1 focus:ring-brass/40"
        >
          <Icon.refresh className="size-4" />
        </button>
      </div>

      <div className="flex items-center gap-2 rounded-xl border border-white/10 bg-white/[0.02] px-3 py-2">
        <Icon.plus className="size-4 text-brass" />
        <input
          value={newTask}
          onChange={e => setNewTask(e.target.value)}
          onKeyDown={e => e.key === 'Enter' && addTask()}
          placeholder="Add a task…"
          className="flex-1 bg-transparent text-[13px] text-zinc-100 placeholder:text-zinc-600 outline-none"
        />
        <button
          onClick={addTask}
          disabled={busy || !newTask.trim()}
          className="rounded-lg border border-brass/30 bg-brass/10 px-2.5 py-1 text-[11px] text-brass disabled:opacity-40 focus:outline-none focus:ring-1 focus:ring-brass/40"
        >
          Add
        </button>
      </div>

      {presentSessions.length > 1 && (
        <div className="flex items-center gap-1.5 flex-wrap">
          {[null, ...presentSessions].map(sid => {
            const active = sessionFilter === sid;
            const label = sid == null ? 'All sessions' : (sessionTitles[sid] ?? sid);
            return (
              <button
                key={sid ?? '__all__'}
                onClick={() => setSessionFilter(sid)}
                className={`rounded-full border px-2.5 py-0.5 font-mono text-[10px] uppercase tracking-widest transition-colors focus:outline-none focus-visible:ring-1 focus-visible:ring-brass/40 ${
                  active
                    ? 'border-brass/40 bg-brass/10 text-brass'
                    : 'border-white/5 bg-white/[0.01] text-zinc-500 hover:border-white/10 hover:text-zinc-400'
                }`}
              >
                {label}
              </button>
            );
          })}
        </div>
      )}

      {error && (
        <div className="rounded-lg border border-red-400/20 bg-red-400/5 px-3 py-2 text-[11px] text-red-300">
          {error}
        </div>
      )}

      <div className="flex-1 space-y-5 overflow-auto custom-scrollbar">
        <section>
          <h2 className="mb-2 px-1 text-[10px] uppercase tracking-widest text-zinc-500">
            In progress ({inProgress.length})
          </h2>
          <div className="space-y-1.5">
            {inProgress.map(t => renderRow(t, false))}
            {inProgress.length === 0 && !loading && (
              <p className="px-1 text-[11px] text-zinc-600">Nothing running.</p>
            )}
          </div>
        </section>
        <section>
          <h2 className="mb-2 px-1 text-[10px] uppercase tracking-widest text-zinc-500">
            Queued ({queued.length})
          </h2>
          <div className="space-y-1.5">
            {queued.map(t => renderRow(t, true))}
            {queued.length === 0 && !loading && (
              <p className="px-1 text-[11px] text-zinc-600">
                Queue is empty — the agent is all yours.
              </p>
            )}
          </div>
        </section>
      </div>
    </div>
  );
}
