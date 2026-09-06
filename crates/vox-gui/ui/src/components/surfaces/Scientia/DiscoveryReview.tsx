import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { sanitizeErrorForToast } from '../../../lib/backendGuard';
import type { SurfaceDecoratorProps } from '../decoratorRegistry';
import { listenScientiaQueue } from '../../../transport';
import { useIsEmbeddedSurface } from '../../dashboard/EmbeddedSurfaceContext';
import {
  listReviewQueue,
  recordDecision,
  nanopublish,
  suggestEvidence,
  type ClaimAwaitingReview,
  type ReviewDecision,
  type EvidenceSuggestion,
} from './discoveryReviewApi';
import { getNoveltyAssessment, type NoveltyAssessment } from './noveltyApi';
import { NoveltyEvidencePanel } from './NoveltyEvidencePanel';

function verdictTone(verdict: string | null): string {
  const v = (verdict ?? '').toLowerCase();
  if (v.includes('support')) return 'text-emerald-300/90';
  if (v.includes('refut') || v.includes('contradict')) return 'text-rose-300/90';
  if (v.includes('novel')) return 'text-violet-300/90';
  return 'text-text-muted';
}

/**
 * P3 Discovery Review — human-gated review of extracted claims before
 * nanopublication. Master/detail: the left pane lists claims awaiting review for
 * the entered publication id; the right pane reviews the selected claim and,
 * once approved in-session, exposes an offline nanopublish action.
 *
 * Post-approval vs queue-drop: an approved claim is terminal and the refetch
 * drops it from the queue. We keep `approvedIds` (a Set) plus a cache of the
 * last-seen detail for each claim, so the just-approved claim's detail and its
 * brass post-approval zone remain visible for the rest of the session.
 */
export function DiscoveryReview({ pushToast }: SurfaceDecoratorProps) {
  const embedded = useIsEmbeddedSurface();
  // Seed the publication id from a cross-surface deep-link (Discovery Inbox's
  // "Open review" stashes it in localStorage before switching here). Consumed
  // once so a manual edit later isn't clobbered.
  const [pubId, setPubId] = useState(() => {
    try {
      const seed = window.localStorage.getItem('vox_discovery_review_seed');
      if (seed) {
        window.localStorage.removeItem('vox_discovery_review_seed');
        return seed;
      }
    } catch { /* localStorage unavailable */ }
    return '';
  });
  const [queue, setQueue] = useState<ClaimAwaitingReview[]>([]);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [reason, setReason] = useState('');
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [approvedIds, setApprovedIds] = useState<Set<number>>(new Set());
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [suggestions, setSuggestions] = useState<EvidenceSuggestion[]>([]);
  const [novelty, setNovelty] = useState<NoveltyAssessment | null>(null);
  const [noveltyLoading, setNoveltyLoading] = useState(false);
  const [noveltyError, setNoveltyError] = useState<string | null>(null);
  // Cache of last-seen claim detail so approved (terminal) claims stay viewable
  // after the refetch removes them from the live queue.
  const detailCache = useRef<Map<number, ClaimAwaitingReview>>(new Map());
  const fetchingRef = useRef(false);

  const refresh = useCallback(async () => {
    const id = pubId.trim();
    if (!id) {
      setQueue([]);
      return;
    }
    if (fetchingRef.current) return;
    fetchingRef.current = true;
    setLoading(true);
    try {
      const rows = await listReviewQueue(id);
      for (const r of rows) detailCache.current.set(r.claim_id, r);
      setQueue(rows);
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Review queue', body: sanitizeErrorForToast(err), cause: 'backend-error' });
      setQueue([]);
    } finally {
      setLoading(false);
      fetchingRef.current = false;
    }
  }, [pubId, pushToast]);

  // Initial fetch + 10 s interval fallback; cleared on unmount / pub change.
  useEffect(() => {
    refresh();
    // Embedded mini-render: one initial fetch only, no repeating poll.
    if (embedded) return;
    const t = setInterval(refresh, 10_000);
    return () => clearInterval(t);
  }, [refresh, embedded]);

  // Event-driven refresh — refetch on the F2 scientia-queue ping. Interval above
  // covers the non-Tauri / no-bridge case. Cleans up on unmount.
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
        /* not in Tauri — interval fallback covers it */
      });
    return () => unlisten?.();
  }, [refresh, embedded]);

  const selected: ClaimAwaitingReview | null = useMemo(() => {
    if (selectedId == null) return null;
    return queue.find((c) => c.claim_id === selectedId) ?? detailCache.current.get(selectedId) ?? null;
  }, [selectedId, queue]);

  const isApproved = selectedId != null && approvedIds.has(selectedId);

  // Lazily load the novelty assessment for the selected claim's publication.
  // Keyed on the publication id (the bundle is per-publication, not per-claim);
  // only fetches once a claim is selected so the detail pane is on screen.
  useEffect(() => {
    const id = pubId.trim();
    if (!id || selectedId == null) {
      setNovelty(null);
      setNoveltyError(null);
      setNoveltyLoading(false);
      return;
    }
    let cancelled = false;
    setNoveltyLoading(true);
    setNoveltyError(null);
    getNoveltyAssessment(id)
      .then((a) => {
        if (!cancelled) setNovelty(a);
      })
      .catch((err) => {
        if (!cancelled) {
          setNovelty(null);
          setNoveltyError(sanitizeErrorForToast(err));
        }
      })
      .finally(() => {
        if (!cancelled) setNoveltyLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [pubId, selectedId]);

  const decide = useCallback(
    async (decision: ReviewDecision) => {
      if (selectedId == null || !pubId.trim()) return;
      setBusy(true);
      try {
        await recordDecision(pubId.trim(), selectedId, decision, reason.trim() || undefined);
        pushToast({ tone: 'ok', title: `Claim #${selectedId} ${decision}`, body: `Publication ${pubId.trim()}`, cause: 'backend-ok' });
        if (decision === 'approved') {
          setApprovedIds((prev) => new Set(prev).add(selectedId));
        }
        setReason('');
        await refresh();
      } catch (err) {
        pushToast({ tone: 'warn', title: 'Review decision failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
      } finally {
        setBusy(false);
      }
    },
    [selectedId, pubId, reason, pushToast, refresh],
  );

  const doNanopublish = useCallback(async () => {
    if (selectedId == null || !pubId.trim()) return;
    setConfirmOpen(false);
    setBusy(true);
    try {
      const res = await nanopublish(pubId.trim(), selectedId);
      pushToast({
        tone: 'ok',
        title: `Nanopublished claim #${selectedId}`,
        body: `${res.published_state} · ${res.validated_offline ? 'validated offline' : 'unvalidated'} · ${res.trusty_uri}`,
        cause: 'backend-ok',
      });
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Nanopublish failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setBusy(false);
    }
  }, [selectedId, pubId, pushToast]);

  const doSuggest = useCallback(async () => {
    if (selectedId == null || !pubId.trim()) return;
    setBusy(true);
    try {
      const out = await suggestEvidence(pubId.trim(), selectedId);
      setSuggestions(out);
      if (out.length === 0) {
        pushToast({ tone: 'info', title: 'No evidence suggestions', body: 'The model returned nothing to act on.', cause: 'backend-ok' });
      }
    } catch (err) {
      setSuggestions([]);
      pushToast({ tone: 'warn', title: 'Evidence assist unavailable', body: sanitizeErrorForToast(err), cause: 'backend-error' });
    } finally {
      setBusy(false);
    }
  }, [selectedId, pubId, pushToast]);

  // Esc closes the nanopublish confirm overlay.
  useEffect(() => {
    if (!confirmOpen) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setConfirmOpen(false);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [confirmOpen]);

  return (
    <section className="space-y-4">
      {/* header + publication input */}
      <div className="flex items-end justify-between">
        <div>
          <h2 className="font-display text-lg tracking-wider text-text-primary uppercase">Discovery Review</h2>
          <p className="font-mono text-xs text-text-muted">
            Human-gated review of extracted claims before nanopublication. Nothing is published to any network.
          </p>
        </div>
        <label className="flex items-center gap-2">
          <span className="font-mono text-[10px] uppercase tracking-wider text-text-muted">Publication</span>
          <input
            value={pubId}
            onChange={(e) => setPubId(e.target.value)}
            placeholder="publication id"
            className="rounded-lg border border-border-subtle bg-overlay-subtle px-3 py-1.5 font-mono text-[12px] text-text-secondary placeholder:text-text-muted focus:border-brass/40 focus:outline-hidden"
          />
          <button
            type="button"
            onClick={refresh}
            disabled={loading}
            className="rounded-lg border border-border-subtle bg-overlay-subtle px-3 py-1.5 text-xs uppercase tracking-wider hover:bg-overlay-subtle disabled:opacity-40"
          >
            {loading ? 'Loading…' : 'Refresh'}
          </button>
        </label>
      </div>

      <div className="grid gap-4" style={{ gridTemplateColumns: '360px 1fr' }}>
        {/* LIST */}
        <div className="flex flex-col overflow-hidden rounded-xl border border-border-subtle bg-overlay-subtle">
          <div className="flex items-center justify-between border-b border-border-subtle px-4 py-3">
            <span className="font-display text-[10px] uppercase tracking-[0.2em] text-text-muted">
              Awaiting Review ({queue.length})
            </span>
          </div>
          <div className="flex flex-col" aria-live="polite">
            {!pubId.trim() && (
              <div className="px-4 py-8 text-center font-mono text-[11px] text-text-muted">
                Enter a publication id to load its review queue.
              </div>
            )}
            {pubId.trim() && queue.length === 0 && !loading && (
              <div className="px-4 py-8 text-center font-mono text-[11px] text-text-muted">
                No claims awaiting review.
              </div>
            )}
            {queue.map((c) => {
              const active = c.claim_id === selectedId;
              return (
                <button
                  type="button"
                  key={c.claim_id}
                  aria-pressed={active}
                  onClick={() => {
                    setSelectedId(c.claim_id);
                    setReason('');
                    setSuggestions([]);
                  }}
                  className={`relative border-b border-border-subtle px-4 py-3 text-left transition ${
                    active ? 'bg-overlay-subtle' : 'hover:bg-overlay-subtle'
                  }`}
                >
                  {active && (
                    <span
                      className="absolute left-0 top-1/2 h-6 w-[2px] -translate-y-1/2 rounded-r bg-brass"
                      style={{ boxShadow: '0 0 12px 2px rgba(212,175,55,.5)' }}
                    />
                  )}
                  <div className="flex items-center justify-between">
                    <span className="font-mono text-[11px] text-text-muted">#{c.claim_id}</span>
                    <span className={`font-mono text-[10px] ${verdictTone(c.verdict)}`}>{c.verdict ?? 'unverified'}</span>
                  </div>
                  <div className="mt-1 text-[12.5px] leading-snug text-text-secondary">{c.text}</div>
                  <div className="mt-1.5 flex items-center gap-2 font-mono text-[10px] text-text-muted">
                    {c.confidence != null && <span className="text-brass"><span aria-hidden="true">★</span> {c.confidence.toFixed(2)}</span>}
                    {c.is_numeric && <span>numeric</span>}
                  </div>
                </button>
              );
            })}
          </div>
        </div>

        {/* DETAIL */}
        <div className="flex flex-col overflow-hidden rounded-xl border border-border-subtle bg-overlay-subtle">
          <div className="flex items-center justify-between border-b border-border-subtle px-5 py-3">
            <span className="font-display text-[10px] uppercase tracking-[0.2em] text-text-muted">Claim Detail</span>
            {selected && <span className="font-mono text-[10px] text-text-muted">claim #{selected.claim_id}</span>}
          </div>

          {!selected && (
            <div className="px-5 py-10 text-center font-mono text-[11px] text-text-muted">
              Select a claim to review.
            </div>
          )}

          {selected && (
            <div className="flex flex-col gap-4 p-5">
              <blockquote className="border-l-2 border-brass/40 pl-4 text-[15px] leading-relaxed text-text-primary">
                “{selected.text}”
              </blockquote>

              <div className="grid grid-cols-2 gap-x-8 gap-y-2 font-mono text-[11px]">
                <div className="flex justify-between border-b border-border-subtle pb-1">
                  <span className="text-text-muted">Verdict</span>
                  <span className={verdictTone(selected.verdict)}>{selected.verdict ?? 'unverified'}</span>
                </div>
                <div className="flex justify-between border-b border-border-subtle pb-1">
                  <span className="text-text-muted">Confidence</span>
                  <span className="text-brass">{selected.confidence != null ? selected.confidence.toFixed(2) : '—'}</span>
                </div>
                <div className="flex justify-between border-b border-border-subtle pb-1">
                  <span className="text-text-muted">Numeric</span>
                  <span className="text-text-secondary">{selected.is_numeric ? 'yes' : 'no'}</span>
                </div>
                <div className="flex justify-between border-b border-border-subtle pb-1">
                  <span className="text-text-muted">Verifier</span>
                  <span className="text-text-secondary">{selected.verifier_model ?? '—'}</span>
                </div>
              </div>

              {noveltyLoading && (
                <div className="font-mono text-[11px] text-text-muted">Loading novelty…</div>
              )}
              {!noveltyLoading && noveltyError && (
                <div className="font-mono text-[11px] text-text-muted">
                  Novelty evidence unavailable.
                </div>
              )}
              {!noveltyLoading && !noveltyError && novelty && (
                <NoveltyEvidencePanel assessment={novelty} />
              )}

              <div>
                <label className="font-mono text-[10px] uppercase tracking-wider text-text-muted">Reason (optional)</label>
                <textarea
                  rows={2}
                  value={reason}
                  onChange={(e) => setReason(e.target.value)}
                  placeholder="Why approve / reject / defer…"
                  className="mt-1 w-full rounded-lg border border-border-subtle bg-overlay-subtle px-3 py-2 text-[12px] text-text-secondary placeholder:text-text-muted focus:border-brass/40 focus:outline-hidden"
                />
              </div>

              <div className="flex items-center gap-2">
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => decide('approved')}
                  className="flex items-center gap-1.5 rounded-lg border border-emerald-400/30 bg-emerald-400/10 px-4 py-2 text-[12px] text-emerald-200 hover:bg-emerald-400/15 disabled:opacity-40"
                >
                  <span aria-hidden="true">✓</span> Approve
                </button>
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => decide('rejected')}
                  className="flex items-center gap-1.5 rounded-lg border border-rose-400/30 bg-rose-400/[0.07] px-4 py-2 text-[12px] text-rose-200 hover:bg-rose-400/10 disabled:opacity-40"
                >
                  <span aria-hidden="true">✗</span> Reject
                </button>
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => decide('deferred')}
                  className="flex items-center gap-1.5 rounded-lg border border-border-subtle bg-overlay-subtle px-4 py-2 text-[12px] text-text-secondary hover:bg-overlay-subtle disabled:opacity-40"
                >
                  <span aria-hidden="true">⏸</span> Defer
                </button>
                <button
                  type="button"
                  disabled={busy}
                  onClick={doSuggest}
                  className="ml-auto flex items-center gap-1.5 rounded-lg border border-violet-400/30 bg-violet-400/[0.07] px-4 py-2 text-[12px] text-violet-200 hover:bg-violet-400/10 disabled:opacity-40"
                >
                  <span aria-hidden="true">✦</span> Suggest evidence improvements (LLM)
                </button>
              </div>

              {suggestions.length > 0 && (
                <div className="rounded-xl border border-violet-400/20 bg-violet-400/4 p-4">
                  <div className="mb-2 font-mono text-[10px] uppercase tracking-wider text-violet-300/80">
                    Evidence suggestions
                  </div>
                  <div className="space-y-2">
                    {suggestions.map((s, i) => (
                      <div key={i} className="font-mono text-[11px]">
                        <span className="text-violet-300/90">[{s.kind}]</span>{' '}
                        <span className="text-text-secondary">{s.summary}</span>
                        <div className="text-text-muted">{s.rationale}</div>
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {isApproved && (
                <div className="rounded-xl border border-brass/20 bg-brass/4 p-4">
                  <div className="font-mono text-[11px] text-emerald-300/90">
                    ✓ Approved by you · approval token bound to this claim
                  </div>
                  <div className="mt-3 flex items-center gap-3">
                    <button
                      type="button"
                      disabled={busy}
                      onClick={() => setConfirmOpen(true)}
                      className="flex items-center gap-2 rounded-lg border border-brass/40 bg-brass/15 px-4 py-2 text-[12px] text-brass hover:bg-brass/25 disabled:opacity-40"
                    >
                      <span aria-hidden="true">⬆</span> Nanopublish (offline)
                    </button>
                    <span className="font-mono text-[10px] text-text-muted">
                      builds + signs + offline-validates, stores locally · no network
                    </span>
                  </div>
                </div>
              )}
            </div>
          )}
        </div>
      </div>

      {/* confirm dialog */}
      {confirmOpen && selected && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center"
          style={{ background: 'rgba(0,0,0,.6)' }}
          onClick={() => setConfirmOpen(false)}
        >
          <div
            role="dialog"
            aria-modal="true"
            aria-label={`Nanopublish claim #${selected.claim_id}`}
            onClick={(e) => e.stopPropagation()}
            className="w-[460px] rounded-xl border border-border-subtle bg-bg-base/90 p-5 backdrop-blur-xl"
          >
            <div className="mb-1 font-display text-[11px] uppercase tracking-[0.2em] text-brass">
              Nanopublish claim #{selected.claim_id} (offline)
            </div>
            <p className="text-[12.5px] leading-relaxed text-text-secondary">
              Builds + signs + offline-validates, stores locally. Nothing is sent to any network.
            </p>
            <div className="mt-4 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1.5 font-mono text-[11px]">
              <span className="text-text-muted">Publication</span>
              <span className="text-text-secondary">{pubId.trim()}</span>
              <span className="text-text-muted">Claim</span>
              <span className="text-text-secondary">#{selected.claim_id}</span>
            </div>
            <div className="mt-5 flex items-center justify-end gap-2">
              <button
                type="button"
                onClick={() => setConfirmOpen(false)}
                className="rounded-lg border border-border-subtle bg-overlay-subtle px-4 py-2 text-[12px] text-text-secondary hover:bg-overlay-subtle"
              >
                Cancel
              </button>
              <button
                type="button"
                disabled={busy}
                onClick={doNanopublish}
                className="rounded-lg border border-brass/40 bg-brass/20 px-4 py-2 text-[12px] text-brass hover:bg-brass/30 disabled:opacity-40"
              >
                Build &amp; sign locally
              </button>
            </div>
          </div>
        </div>
      )}
    </section>
  );
}
