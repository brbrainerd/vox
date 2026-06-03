import React, { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { SurfaceDecoratorProps } from '../decoratorRegistry';
import { listenScientiaQueue } from '../../../transport';

interface ExecuteOutput {
  exit_code: number;
  stdout: string;
  stderr: string;
}

interface CandidateRow {
  candidate_id: string;
  candidate_class: string;
  confidence: number;
  state: string;
  created_at_ms: number;
  updated_at_ms: number;
}

interface QueueSnapshot {
  candidates: {
    total: number;
    by_class: Record<string, number>;
    top_5_by_confidence: CandidateRow[];
  };
  claims_pending: { verifiable: number; abstained: number; extraction_running: number };
  manifests_in_reply_window: string[];
  retraction_queue: string[];
  stalls: { candidate_id: string; class: string; stuck_for_ms: number }[];
}

function Kpi({ label, value, tone }: { label: string; value: number; tone?: string }) {
  return (
    <div className="rounded-xl border border-white/10 bg-white/[0.02] p-3">
      <div className={`font-display text-2xl ${tone ?? 'text-zinc-100'}`}>{value}</div>
      <div className="mt-0.5 font-mono text-[10px] uppercase tracking-wider text-zinc-500">{label}</div>
    </div>
  );
}

/**
 * Phase H structured dashboard: assembles a QueueSnapshot from the live DB via
 * `vox scientia dashboard` (the shared execute_command path) and renders it.
 */
export function ScientiaDashboard({ pushToast }: SurfaceDecoratorProps) {
  const [snap, setSnap] = useState<QueueSnapshot | null>(null);
  const [loading, setLoading] = useState(false);
  // Guard against interval-triggered overlapping fetches while one is in flight.
  const fetchingRef = React.useRef(false);

  const refresh = useCallback(async () => {
    if (fetchingRef.current) return;
    fetchingRef.current = true;
    setLoading(true);
    try {
      const out = await invoke<ExecuteOutput>('execute_command', {
        path: ['scientia', 'dashboard'],
        args: { __argv: [] },
      });
      if (out.exit_code !== 0) {
        pushToast({ tone: 'warn', title: 'Scientia dashboard', body: out.stderr || `exit ${out.exit_code}` });
        setSnap(null);
      } else {
        setSnap(JSON.parse(out.stdout) as QueueSnapshot);
      }
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Scientia dashboard', body: String(err) });
      setSnap(null);
    } finally {
      setLoading(false);
      fetchingRef.current = false;
    }
  }, [pushToast]);

  // Initial fetch + 10 s auto-refresh; interval is cleared on unmount.
  useEffect(() => {
    refresh();
    const id = setInterval(refresh, 10_000);
    return () => clearInterval(id);
  }, [refresh]);

  // F2: event-driven refresh — refetch immediately when the Rust DB watcher
  // pushes a "vox://scientia-queue" ping. The 10 s interval above stays as a
  // fallback (e.g. outside Tauri, where listen() rejects). Cleans up on unmount.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listenScientiaQueue(() => {
      void refresh();
    })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {
        /* not in Tauri or no event bridge — interval fallback covers it */
      });
    return () => unlisten?.();
  }, [refresh]);

  return (
    <section className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="font-display text-lg tracking-wider text-zinc-100 uppercase">Vox Scientia</h2>
          <p className="font-mono text-xs text-zinc-500">Publication pipeline queue snapshot</p>
        </div>
        <button
          className="rounded-lg border border-white/10 bg-white/[0.03] px-3 py-1.5 text-xs uppercase tracking-wider hover:bg-white/[0.06] disabled:opacity-40"
          disabled={loading}
          onClick={refresh}
        >
          {loading ? 'Refreshing…' : 'Refresh'}
        </button>
      </div>

      {!snap && <div className="font-mono text-xs text-zinc-500">Loading queue snapshot…</div>}

      {snap && (
        <>
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            <Kpi label="Candidates" value={snap.candidates.total} />
            <Kpi label="Verifiable claims" value={snap.claims_pending.verifiable} tone="text-emerald-300" />
            <Kpi label="Abstained claims" value={snap.claims_pending.abstained} tone="text-zinc-300" />
            <Kpi label="Extraction pending" value={snap.claims_pending.extraction_running} tone="text-amber-300" />
            <Kpi label="Reply window" value={snap.manifests_in_reply_window.length} />
            <Kpi label="Retraction queue" value={snap.retraction_queue.length} tone="text-red-300" />
            <Kpi label="Stalls" value={snap.stalls.length} tone="text-amber-300" />
          </div>

          {Object.keys(snap.candidates.by_class).length > 0 && (
            <div className="rounded-xl border border-white/10 bg-white/[0.02] p-3">
              <div className="mb-2 font-mono text-[10px] uppercase tracking-wider text-zinc-500">Candidates by class</div>
              <div className="flex flex-wrap gap-2">
                {Object.entries(snap.candidates.by_class).map(([cls, n]) => (
                  <span key={cls} className="rounded bg-white/5 px-2 py-0.5 font-mono text-[11px] text-zinc-300">
                    {cls} <span className="text-zinc-500">{n}</span>
                  </span>
                ))}
              </div>
            </div>
          )}

          {snap.candidates.top_5_by_confidence.length > 0 && (
            <div className="rounded-xl border border-white/10 bg-white/[0.02] p-3">
              <div className="mb-2 font-mono text-[10px] uppercase tracking-wider text-zinc-500">Top candidates</div>
              <div className="space-y-1">
                {snap.candidates.top_5_by_confidence.map((c) => (
                  <div key={c.candidate_id} className="flex items-center gap-2 font-mono text-[11px]">
                    <span className="text-cyan">{c.confidence.toFixed(2)}</span>
                    <span className="text-zinc-300">{c.candidate_id}</span>
                    <span className="text-zinc-500">{c.candidate_class}</span>
                    <span className="ml-auto text-zinc-400">{c.state}</span>
                  </div>
                ))}
              </div>
            </div>
          )}

          {snap.stalls.length > 0 && (
            <div className="rounded-xl border border-amber-500/20 bg-amber-500/[0.03] p-3">
              <div className="mb-2 font-mono text-[10px] uppercase tracking-wider text-amber-300/80">Stalled candidates</div>
              <div className="space-y-1">
                {snap.stalls.map((s) => (
                  <div key={s.candidate_id} className="flex items-center gap-2 font-mono text-[11px]">
                    <span className="text-zinc-300">{s.candidate_id}</span>
                    <span className="text-zinc-500">{s.class}</span>
                    <span className="ml-auto text-amber-300/80">{Math.round(s.stuck_for_ms / 86_400_000)}d stuck</span>
                  </div>
                ))}
              </div>
            </div>
          )}
        </>
      )}
    </section>
  );
}
