import React, { useCallback, useEffect, useState } from 'react';
import { sanitizeErrorForToast } from '../../../lib/backendGuard';
import { invoke } from '@tauri-apps/api/core';
import type { SurfaceDecoratorProps } from '../decoratorRegistry';
import { useLabel } from '../../../hooks/useLanguage';
import { listenScientiaQueue } from '../../../transport';
import { fetchCostRollup, providerRows, quarterlyRows } from './costRollup';
import type { CostRollup } from './costRollup';
import { ArchiveStatusSummary } from './ArchiveStatusSummary';
import { useIsEmbeddedSurface } from '../../dashboard/EmbeddedSurfaceContext';

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
    <div className="rounded-xl border border-border-subtle bg-overlay-subtle p-3">
      <div className={`font-display text-2xl ${tone ?? 'text-text-primary'}`}>{value}</div>
      <div className="mt-0.5 font-mono text-[10px] uppercase tracking-wider text-text-muted">{label}</div>
    </div>
  );
}

/**
 * Phase H structured dashboard: assembles a QueueSnapshot from the live DB via
 * the native `scientia_dashboard_snapshot` command, which reads through this
 * app's own already-open DB pool instead of shelling out to a `vox scientia
 * dashboard` subprocess (which used to contend with this app's own
 * connection for the same DB file lock — "Locking error ... os error 33").
 */
export function ScientiaDashboard({ pushToast }: SurfaceDecoratorProps) {
  const embedded = useIsEmbeddedSurface();
  const [snap, setSnap] = useState<QueueSnapshot | null>(null);
  const [cost, setCost] = useState<CostRollup | null>(null);
  const [loading, setLoading] = useState(false);
  // Guard against interval-triggered overlapping fetches while one is in flight.
  const fetchingRef = React.useRef(false);

  const refresh = useCallback(async () => {
    if (fetchingRef.current) return;
    fetchingRef.current = true;
    setLoading(true);
    try {
      setSnap(await invoke<QueueSnapshot>('scientia_dashboard_snapshot'));
      // Cost rollup is an independent producer (`vox scientia cost`); a failure
      // here must not blank the queue snapshot above.
      try {
        setCost(await fetchCostRollup());
      } catch (costErr) {
        pushToast({ tone: 'warn', title: 'Scientia cost', body: sanitizeErrorForToast(costErr), cause: 'backend-error' });
        setCost(null);
      }
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Scientia dashboard', body: sanitizeErrorForToast(err), cause: 'backend-error' });
      setSnap(null);
    } finally {
      setLoading(false);
      fetchingRef.current = false;
    }
  }, [pushToast]);

  // Initial fetch + 10 s auto-refresh; interval is cleared on unmount.
  useEffect(() => {
    refresh();
    // Embedded mini-render: one initial fetch only, no repeating poll.
    if (embedded) return;
    const id = setInterval(refresh, 10_000);
    return () => clearInterval(id);
  }, [refresh, embedded]);

  // F2: event-driven refresh — refetch immediately when the Rust DB watcher
  // pushes a "vox://scientia-queue" ping. The 10 s interval above stays as a
  // fallback (e.g. outside Tauri, where listen() rejects). Cleans up on unmount.
  useEffect(() => {
    // Embedded mini-render: no pushed subscription either — stays static.
    if (embedded) return;
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
  }, [refresh, embedded]);

  return (
    <section className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="font-display text-lg tracking-wider text-text-primary uppercase">{useLabel('sci-home')}</h2>
          <p className="font-mono text-xs text-text-muted">Publication pipeline queue snapshot</p>
        </div>
        <button
          type="button"
          className="rounded-lg border border-border-subtle bg-overlay-subtle px-3 py-1.5 text-xs uppercase tracking-wider hover:bg-overlay-subtle disabled:opacity-40"
          disabled={loading}
          onClick={refresh}
        >
          {loading ? 'Refreshing…' : 'Refresh'}
        </button>
      </div>

      {/* Snapshot region: refetched on a 10s interval + event ping, so announce
          updates politely to assistive tech. */}
      <div aria-live="polite">
        {!snap && <div className="font-mono text-xs text-text-muted">Loading queue snapshot…</div>}

        {snap && (
          <>
            <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
              <Kpi label="Candidates" value={snap.candidates.total} />
              <Kpi label="Verifiable claims" value={snap.claims_pending.verifiable} tone="text-emerald-300" />
              <Kpi label="Abstained claims" value={snap.claims_pending.abstained} tone="text-text-secondary" />
              <Kpi label="Extraction pending" value={snap.claims_pending.extraction_running} tone="text-amber-300" />
              <Kpi label="Reply window" value={snap.manifests_in_reply_window.length} />
              <Kpi label="Retraction queue" value={snap.retraction_queue.length} tone="text-red-300" />
              <Kpi label="Stalls" value={snap.stalls.length} tone="text-amber-300" />
            </div>

            {Object.keys(snap.candidates.by_class).length > 0 && (
              <div className="rounded-xl border border-border-subtle bg-overlay-subtle p-3">
                <div className="mb-2 font-mono text-[10px] uppercase tracking-wider text-text-muted">Candidates by class</div>
                <div className="flex flex-wrap gap-2">
                  {Object.entries(snap.candidates.by_class).map(([cls, n]) => (
                    <span key={cls} className="rounded-sm bg-overlay-subtle px-2 py-0.5 font-mono text-[11px] text-text-secondary">
                      {cls} <span className="text-text-muted">{n}</span>
                    </span>
                  ))}
                </div>
              </div>
            )}

            {snap.candidates.top_5_by_confidence.length > 0 && (
              <div className="rounded-xl border border-border-subtle bg-overlay-subtle p-3">
                <div className="mb-2 font-mono text-[10px] uppercase tracking-wider text-text-muted">Top candidates</div>
                <div className="space-y-1">
                  {snap.candidates.top_5_by_confidence.map((c) => (
                    <div key={c.candidate_id} className="flex items-center gap-2 font-mono text-[11px]">
                      <span className="text-cyan">{c.confidence.toFixed(2)}</span>
                      <span className="text-text-secondary">{c.candidate_id}</span>
                      <span className="text-text-muted">{c.candidate_class}</span>
                      <span className="ml-auto text-text-muted">{c.state}</span>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {snap.stalls.length > 0 && (
              <div className="rounded-xl border border-amber-500/20 bg-amber-500/3 p-3">
                <div className="mb-2 font-mono text-[10px] uppercase tracking-wider text-amber-300/80">Stalled candidates</div>
                <div className="space-y-1">
                  {snap.stalls.map((s) => (
                    <div key={s.candidate_id} className="flex items-center gap-2 font-mono text-[11px]">
                      <span className="text-text-secondary">{s.candidate_id}</span>
                      <span className="text-text-muted">{s.class}</span>
                      <span className="ml-auto text-amber-300/80">{Math.round(s.stuck_for_ms / 86_400_000)}d stuck</span>
                    </div>
                  ))}
                </div>
              </div>
            )}
          </>
        )}
      </div>

      <ArchiveStatusSummary
        onFetchError={(message) =>
          pushToast({ tone: 'warn', title: 'Archive rollup', body: message, cause: 'backend-error' })
        }
      />

      {cost && (
        <div className="rounded-xl border border-border-subtle bg-overlay-subtle p-3">
          <div className="mb-2 flex items-center justify-between">
            <div className="font-mono text-[10px] uppercase tracking-wider text-text-muted">
              Cost this quarter
            </div>
            <div className="font-mono text-[10px] text-text-muted">
              avg/finding{' '}
              <span className="text-text-secondary">${cost.per_finding_average_usd.toFixed(2)}</span>
            </div>
          </div>

          {cost.this_quarter.total_usd === 0 && cost.by_provider.length === 0 ? (
            <div className="font-mono text-[11px] text-text-muted">
              No cost recorded this quarter.
            </div>
          ) : (
            <div className="grid gap-3 sm:grid-cols-2">
              <div className="space-y-1">
                {quarterlyRows(cost).map((r) => (
                  <div
                    key={r.label}
                    className={`flex items-center justify-between font-mono text-[11px] ${
                      r.label === 'Total' ? 'border-t border-border-subtle pt-1 text-text-secondary' : 'text-text-muted'
                    }`}
                  >
                    <span>{r.label}</span>
                    <span>{r.usd}</span>
                  </div>
                ))}
              </div>

              <div className="space-y-1">
                <div className="font-mono text-[10px] uppercase tracking-wider text-text-muted">
                  By provider
                </div>
                {providerRows(cost).length > 0 ? (
                  providerRows(cost).map((p) => (
                    <div
                      key={p.provider}
                      className="flex items-center justify-between font-mono text-[11px] text-text-muted"
                    >
                      <span>{p.provider}</span>
                      <span className="text-text-secondary">{p.usd}</span>
                    </div>
                  ))
                ) : (
                  <div className="font-mono text-[11px] text-text-muted">No provider spend.</div>
                )}
              </div>
            </div>
          )}
        </div>
      )}
    </section>
  );
}
