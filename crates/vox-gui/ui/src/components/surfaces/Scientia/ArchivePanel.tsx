import React, { useCallback, useEffect, useRef, useState } from 'react';
import type { SurfaceDecoratorProps } from '../decoratorRegistry';
import {
  getCompletionReport,
  runAutofill,
  getArchiveStatus,
  type CompletionReport,
  type ArchiveStatus,
} from './archiveApi';

/** Colour the completeness bar by band: low = rose, mid = amber, high = emerald. */
function meterTone(pct: number): string {
  if (pct >= 80) return 'bg-emerald-400/70';
  if (pct >= 50) return 'bg-amber-400/70';
  return 'bg-rose-400/70';
}

/**
 * Task 19 — Archive Panel. Surfaces the Track B archive pipeline for ONE
 * publication: its metadata-completeness report (a meter + required-missing
 * checklist + provenance chips + "needs human" tags), a one-click deterministic
 * autofill (provenance-carrying, never overwrites), and its deposit status
 * (Zenodo DOI/state, Software Heritage SWHID/task).
 *
 * The publication id is entered at top (mirrors DiscoveryReview); a cross-surface
 * deep-link can seed it via localStorage. Autofill re-renders with the raised
 * completeness and a success toast. Deposit status degrades to an honest
 * "not yet deposited" when nothing is persisted.
 */
export function ArchivePanel({ pushToast }: SurfaceDecoratorProps) {
  const [pubId, setPubId] = useState(() => {
    try {
      const seed = window.localStorage.getItem('vox_archive_panel_seed');
      if (seed) {
        window.localStorage.removeItem('vox_archive_panel_seed');
        return seed;
      }
    } catch {
      /* localStorage unavailable */
    }
    return '';
  });
  const [report, setReport] = useState<CompletionReport | null>(null);
  const [status, setStatus] = useState<ArchiveStatus | null>(null);
  const [loading, setLoading] = useState(false);
  const [autofilling, setAutofilling] = useState(false);
  // Monotonic token so a slow in-flight load can't clobber a newer one.
  const loadTokenRef = useRef(0);

  const load = useCallback(async () => {
    const id = pubId.trim();
    if (!id) {
      setReport(null);
      setStatus(null);
      return;
    }
    const token = ++loadTokenRef.current;
    setLoading(true);
    try {
      const [rep, st] = await Promise.all([getCompletionReport(id), getArchiveStatus(id)]);
      if (token !== loadTokenRef.current) return;
      setReport(rep);
      if (token !== loadTokenRef.current) return;
      setStatus(st);
    } catch (err) {
      if (token !== loadTokenRef.current) return;
      pushToast({ tone: 'warn', title: 'Archive panel', body: String(err) });
      setReport(null);
      setStatus(null);
    } finally {
      if (token === loadTokenRef.current) setLoading(false);
    }
  }, [pubId, pushToast]);

  // Auto-load only when an id was seeded; manual entry loads on submit.
  useEffect(() => {
    if (pubId.trim()) void load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const autofill = useCallback(async () => {
    const id = pubId.trim();
    if (!id) return;
    setAutofilling(true);
    try {
      const result = await runAutofill(id, true);
      // Re-render with the raised completeness + refreshed status/provenance.
      await load();
      const delta = result.completeness_after - result.completeness_before;
      pushToast({
        tone: 'ok',
        title: 'Autofill applied',
        body: `${result.fills.length} field(s) filled · completeness ${result.completeness_before}% → ${result.completeness_after}%${
          delta > 0 ? ` (+${delta})` : ''
        }`,
      });
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Autofill failed', body: String(err) });
    } finally {
      setAutofilling(false);
    }
  }, [pubId, load, pushToast]);

  const pct = report?.completeness_0_100 ?? 0;

  return (
    <section className="space-y-4">
      <div>
        <h2 className="font-display text-lg tracking-wider text-zinc-100 uppercase">Archive Panel</h2>
        <p className="font-mono text-xs text-zinc-500">
          Metadata completeness, deterministic autofill, and deposit status (Zenodo / Software Heritage).
        </p>
      </div>

      {/* Publication-id entry */}
      <form
        className="flex items-center gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          void load();
        }}
      >
        <input
          value={pubId}
          onChange={(e) => setPubId(e.target.value)}
          placeholder="publication id"
          className="flex-1 rounded-lg border border-white/10 bg-white/[0.03] px-3 py-1.5 font-mono text-xs text-zinc-200 placeholder:text-zinc-600 focus:outline-none focus:ring-1 focus:ring-brass/40"
        />
        <button
          type="submit"
          disabled={loading || !pubId.trim()}
          className="rounded-lg border border-white/10 bg-white/[0.03] px-3 py-1.5 text-xs uppercase tracking-wider hover:bg-white/[0.06] disabled:opacity-40"
        >
          {loading ? 'Loading…' : 'Load'}
        </button>
      </form>

      {report && (
        <div className="space-y-4">
          {/* Completeness meter */}
          <div className="rounded-xl border border-white/10 bg-white/[0.02] p-4">
            <div className="flex items-end justify-between">
              <span className="font-display text-[10px] uppercase tracking-[0.2em] text-zinc-400">
                Metadata completeness
              </span>
              <span className="font-mono text-2xl text-zinc-100" aria-label="completeness percent">
                {pct}
                <span className="text-sm text-zinc-500">%</span>
              </span>
            </div>
            <div className="mt-2 h-2 w-full overflow-hidden rounded-full bg-white/[0.06]">
              <div
                role="progressbar"
                aria-valuenow={pct}
                aria-valuemin={0}
                aria-valuemax={100}
                className={`h-full rounded-full ${meterTone(pct)}`}
                style={{ width: `${Math.max(0, Math.min(100, pct))}%` }}
              />
            </div>
            <div className="mt-3 flex justify-end">
              <button
                type="button"
                onClick={autofill}
                disabled={autofilling}
                className="rounded-lg border border-brass/40 bg-brass/15 px-3 py-1.5 text-[11px] uppercase tracking-wider text-brass hover:bg-brass/20 disabled:opacity-40"
              >
                {autofilling ? 'Auto-filling…' : 'Auto-fill'}
              </button>
            </div>
          </div>

          {/* Required-missing checklist */}
          <div className="rounded-xl border border-white/10 bg-white/[0.02] p-4">
            <span className="font-display text-[10px] uppercase tracking-[0.2em] text-zinc-400">
              Required missing ({report.required_missing.length})
            </span>
            {report.required_missing.length === 0 ? (
              <p className="mt-2 font-mono text-[11px] text-emerald-300/80">All required fields present.</p>
            ) : (
              <ul className="mt-2 space-y-1">
                {report.required_missing.map((f) => (
                  <li key={f} className="flex items-center gap-2 font-mono text-[12px] text-rose-200/90">
                    <span aria-hidden className="text-rose-400/70">
                      ☐
                    </span>
                    {f}
                  </li>
                ))}
              </ul>
            )}
          </div>

          {/* "Needs human" tags */}
          {report.human_only_pending.length > 0 && (
            <div className="rounded-xl border border-white/10 bg-white/[0.02] p-4">
              <span className="font-display text-[10px] uppercase tracking-[0.2em] text-zinc-400">
                Needs human ({report.human_only_pending.length})
              </span>
              <div className="mt-2 flex flex-wrap gap-1.5">
                {report.human_only_pending.map((f) => (
                  <span
                    key={f}
                    className="rounded border border-amber-400/20 bg-amber-400/[0.06] px-1.5 py-0.5 font-mono text-[10px] text-amber-200/90"
                  >
                    {f}
                  </span>
                ))}
              </div>
            </div>
          )}

          {/* Field provenance chips */}
          {report.field_provenance.length > 0 && (
            <div className="rounded-xl border border-white/10 bg-white/[0.02] p-4">
              <span className="font-display text-[10px] uppercase tracking-[0.2em] text-zinc-400">
                Field provenance
              </span>
              <div className="mt-2 flex flex-wrap gap-1.5">
                {report.field_provenance.map((p, i) => (
                  <span
                    key={`${p.field}-${i}`}
                    title={p.notes ?? undefined}
                    className="rounded border border-violet-400/20 bg-violet-400/[0.06] px-1.5 py-0.5 font-mono text-[10px] text-violet-200/90"
                  >
                    {p.field} → {p.origin}
                  </span>
                ))}
              </div>
            </div>
          )}

          {/* Deposit status */}
          <div className="rounded-xl border border-white/10 bg-white/[0.02] p-4">
            <span className="font-display text-[10px] uppercase tracking-[0.2em] text-zinc-400">
              Deposit status
            </span>
            <dl className="mt-2 grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 font-mono text-[12px]">
              <dt className="text-zinc-500">Zenodo DOI</dt>
              <dd className="text-zinc-200">
                {status?.zenodo_doi ?? <span className="text-zinc-600">not yet deposited</span>}
              </dd>
              <dt className="text-zinc-500">Zenodo state</dt>
              <dd className="text-zinc-200">
                {status?.zenodo_state ?? <span className="text-zinc-600">—</span>}
              </dd>
              <dt className="text-zinc-500">SWHID</dt>
              <dd className="break-all text-zinc-200">
                {status?.swhid ?? <span className="text-zinc-600">not yet deposited</span>}
              </dd>
              <dt className="text-zinc-500">SWH task</dt>
              <dd className="text-zinc-200">
                {status?.swh_task_status ?? <span className="text-zinc-600">—</span>}
              </dd>
            </dl>
          </div>
        </div>
      )}

      {!report && !loading && (
        <div className="rounded-xl border border-white/10 bg-white/[0.02] px-4 py-10 text-center font-mono text-[11px] text-zinc-600">
          Enter a publication id to view its archive readiness.
        </div>
      )}
    </section>
  );
}
