import React, { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listenScientiaQueue } from '../../../transport';
import type { SurfaceDecoratorProps } from '../decoratorRegistry';
import { PipelineTimeline } from '../../PipelineTimeline';
import { RESEARCH_STAGES, deriveStages } from '../../../lib/pipeline';
import { startResearchAsync } from './researchActions';

interface ResearchSession { id: number; status: string; query_text: string; started_at_ms: number; finished_at_ms: number | null; }
interface ResearchDetail { session: ResearchSession; report_markdown: string | null; artifact_json: string | null; }

export function ResearchView({ pushToast }: SurfaceDecoratorProps) {
  const [query, setQuery] = useState('');
  const [running, setRunning] = useState(false);
  const [activeSessionId, setActiveSessionId] = useState<number | null>(null);
  const [sessions, setSessions] = useState<ResearchSession[]>([]);
  const [detail, setDetail] = useState<ResearchDetail | null>(null);

  const loadHistory = useCallback(async () => {
    try {
      setSessions(await invoke<ResearchSession[]>('list_research_sessions', { limit: 25 }));
    } catch (err) {
      pushToast({ tone: 'warn', title: 'History load failed', body: String(err) });
    }
  }, [pushToast]);

  const openDetail = useCallback(async (id: number) => {
    try {
      setDetail(await invoke<ResearchDetail>('get_research_session_detail', { sessionId: id }));
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Session load failed', body: String(err) });
    }
  }, [pushToast]);

  useEffect(() => { loadHistory(); }, [loadHistory]);

  // A2: refetch history whenever the persistent daemon's Scientia-queue watcher
  // signals a research-session transition; a 10 s interval is the fallback
  // (e.g. outside Tauri, where listen() rejects), mirroring ScientiaDashboard.
  useEffect(() => {
    const id = setInterval(loadHistory, 10_000);
    let unlisten: (() => void) | undefined;
    listenScientiaQueue(() => { void loadHistory(); })
      .then((fn) => { unlisten = fn; })
      .catch(() => { /* not in Tauri — interval fallback covers it */ });
    return () => { clearInterval(id); unlisten?.(); };
  }, [loadHistory]);

  // Once the active run reaches a terminal status, stop the running indicator
  // and open its detail so the answer (report_markdown ?? artifact_json) shows.
  useEffect(() => {
    if (activeSessionId == null) return;
    const s = sessions.find((x) => x.id === activeSessionId);
    if (s && (s.status === 'completed' || s.status === 'failed')) {
      setRunning(false);
      void openDetail(activeSessionId);
      setActiveSessionId(null);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessions, activeSessionId]);

  const run = async () => {
    if (!query.trim()) return;
    setRunning(true);
    setActiveSessionId(null);
    try {
      // A2: fire-and-forget via the persistent daemon's async executor. Returns
      // {session_id, task_id, status: "running"} immediately — does NOT block on
      // the pipeline. Status transitions arrive through the queue watcher below.
      const handle = await startResearchAsync({ query });
      setActiveSessionId(handle.session_id);
      await loadHistory();
    } catch (err) {
      setRunning(false);
      pushToast({ tone: 'warn', title: 'Research run failed', body: String(err) });
    }
  };

  return (
    <section className="space-y-4">
      <h2 className="font-display text-lg text-zinc-100 tracking-wider uppercase">Research</h2>

      <div className="flex gap-2">
        <input value={query} onChange={e => setQuery(e.target.value)} placeholder="Ask a research question…"
          className="flex-1 rounded-lg border border-white/10 bg-black/40 px-3 py-2 text-sm text-zinc-200 outline-none focus:border-brass/40" />
        <button onClick={run} disabled={running}
          className="rounded-lg border border-brass/30 bg-brass/10 px-4 py-2 text-sm text-brass hover:bg-brass/20 disabled:opacity-50">
          {running ? 'Running…' : 'Run'}
        </button>
      </div>
      {running && (
        <div className="rounded-lg border border-white/10 bg-white/[0.02] p-3">
          <PipelineTimeline stages={RESEARCH_STAGES} statuses={deriveStages('active')} />
          <div className="mt-2 text-[11px] text-zinc-500">
            Running in the background{activeSessionId != null ? ` (session ${activeSessionId})` : ''} — the answer opens automatically when it completes.
          </div>
        </div>
      )}

      <div>
        <div className="mb-2 flex items-center justify-between">
          <span className="font-display text-[12px] uppercase tracking-wide text-zinc-400">Recent sessions</span>
          <button onClick={loadHistory} className="text-[11px] text-zinc-500 hover:text-zinc-200">Refresh</button>
        </div>
        {sessions.length === 0 ? (
          <div className="rounded-lg border border-dashed border-white/5 py-6 text-center text-[11px] text-zinc-600">
            No research sessions yet — ask a question above to run one.
          </div>
        ) : (
          <ul className="space-y-1">
            {sessions.map(s => (
              <li key={s.id}>
                <button onClick={() => openDetail(s.id)}
                  className="flex w-full items-center justify-between rounded-lg border border-white/10 bg-white/[0.02] px-3 py-2 text-left hover:bg-white/[0.04]">
                  <span className="truncate text-[12px] text-zinc-300">{s.query_text}</span>
                  <span className="ml-3 shrink-0 font-mono text-[10px] text-zinc-500">{s.status}</span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>

      {detail && (
        <div className="rounded-lg border border-white/10 bg-white/[0.02] p-3">
          <div className="mb-2 flex items-center justify-between">
            <span className="text-[12px] text-zinc-300">Session {detail.session.id}</span>
            <button onClick={() => setDetail(null)} className="text-[11px] text-zinc-500 hover:text-zinc-200">Close</button>
          </div>
          <PipelineTimeline stages={RESEARCH_STAGES} statuses={deriveStages(detail.session.status)} />
          <pre className="mt-2 max-h-[360px] overflow-auto whitespace-pre-wrap text-[12px] text-zinc-300">
            {detail.report_markdown ?? detail.artifact_json ?? '(no artifact persisted)'}
          </pre>
        </div>
      )}
    </section>
  );
}
