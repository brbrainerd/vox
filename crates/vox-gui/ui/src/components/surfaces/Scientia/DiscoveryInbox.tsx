import React, { useCallback, useEffect, useRef, useState } from 'react';
import { sanitizeErrorForToast } from '../../../lib/backendGuard';
import type { SurfaceDecoratorProps } from '../decoratorRegistry';
import { useLabel } from '../../../hooks/useLanguage';
import { listenDiscoverySurfaced, listenScientiaQueue } from '../../../transport';
import {
  listDiscoveryInbox,
  acknowledgeDiscovery,
  type DiscoveryInboxRow,
} from './discoveryInboxApi';
import { useIsEmbeddedSurface } from '../../dashboard/EmbeddedSurfaceContext';
import { seedDiscoveryPresetForLegacyKey } from '../../../lib/navigation';

/** The intake tier we treat as "strong" — gets a highlighted badge + a toast on arrival. */
const STRONG_TIER = 'strong_candidate';

function tierLabel(tier: string): string {
  return tier.replace(/_/g, ' ');
}

/** Compact relative-time string for a past epoch-ms timestamp. */
function relativeTime(ms: number): string {
  const delta = Date.now() - ms;
  if (delta < 0 || !Number.isFinite(delta)) return 'just now';
  const sec = Math.floor(delta / 1000);
  if (sec < 60) return `${sec}s ago`;
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min}m ago`;
  const hr = Math.floor(min / 60);
  if (hr < 24) return `${hr}h ago`;
  return `${Math.floor(hr / 24)}d ago`;
}

/**
 * Task 18 — Discovery Inbox. Lists unacknowledged surfaced research candidates
 * (rows in `scientia_discovery_inbox`). Each row can be acknowledged (drops it
 * from the list) or opened in the Discovery Review surface (deep-link).
 *
 * Live updates: subscribes to `vox://scientia-discovery-surfaced` (one row per
 * emission, mirroring the orchestrator-mcp `scientia.discovery.surfaced` WS topic
 * at the Tauri boundary) and to the `vox://scientia-queue` change ping. A 10 s
 * interval covers the non-Tauri / no-bridge case.
 *
 * On a newly-arrived strong candidate we raise an in-app toast. OS notifications
 * are intentionally NOT wired: the `tauri-plugin-notification` plugin is not a
 * workspace dependency, so adding it here would risk a version-resolution mess
 * for a degradable nicety — the toast is the graceful baseline.
 */
export function DiscoveryInbox({ pushToast }: SurfaceDecoratorProps) {
  const embedded = useIsEmbeddedSurface();
  const [rows, setRows] = useState<DiscoveryInboxRow[]>([]);
  const [loading, setLoading] = useState(false);
  const [busyId, setBusyId] = useState<number | null>(null);
  const fetchingRef = useRef(false);
  // Ids we've already toasted, so a refetch doesn't re-announce the same rows.
  const seenStrongIds = useRef<Set<number>>(new Set());
  // First load shouldn't toast the whole backlog as if it just arrived.
  const primedRef = useRef(false);

  const refresh = useCallback(async () => {
    if (fetchingRef.current) return;
    fetchingRef.current = true;
    setLoading(true);
    try {
      const next = await listDiscoveryInbox();
      // Announce strong candidates that are new since the last fetch.
      if (primedRef.current) {
        for (const r of next) {
          if (r.intake_tier === STRONG_TIER && !seenStrongIds.current.has(r.id)) {
            pushToast({
              tone: 'info',
              title: 'New research candidate',
              body: `${r.publication_id}${r.signal_codes.length ? ` · ${r.signal_codes.join(', ')}` : ''}`,
              cause: 'backend-ok',
            });
          }
        }
      }
      for (const r of next) {
        if (r.intake_tier === STRONG_TIER) seenStrongIds.current.add(r.id);
      }
      primedRef.current = true;
      setRows(next);
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Discovery inbox', body: sanitizeErrorForToast(err), cause: 'backend-error' });
      setRows([]);
    } finally {
      setLoading(false);
      fetchingRef.current = false;
    }
  }, [pushToast]);

  // Initial fetch + 10 s interval fallback.
  useEffect(() => {
    refresh();
    // Embedded mini-render: one initial fetch only, no repeating poll.
    if (embedded) return;
    const t = setInterval(refresh, 10_000);
    return () => clearInterval(t);
  }, [refresh, embedded]);

  // Event-driven refresh on discovery-surfaced rows + scientia-queue ping.
  useEffect(() => {
    // Embedded mini-render: no pushed subscriptions either — stays static.
    if (embedded) return;
    let unlistenDiscovery: (() => void) | undefined;
    let unlistenQueue: (() => void) | undefined;

    listenDiscoverySurfaced((row) => {
      const incoming: DiscoveryInboxRow = {
        id: row.id,
        publication_id: row.publication_id,
        surfaced_at_ms: row.surfaced_at_ms,
        intake_tier: row.intake_tier,
        signal_codes: row.signal_codes,
        origin: row.origin,
      };
      setRows((prev) => {
        if (prev.some((r) => r.id === incoming.id)) return prev;
        return [incoming, ...prev].sort((a, b) => b.surfaced_at_ms - a.surfaced_at_ms);
      });
      if (primedRef.current && row.intake_tier === STRONG_TIER && !seenStrongIds.current.has(row.id)) {
        pushToast({
          tone: 'info',
          title: 'New research candidate',
          body: `${row.publication_id}${row.signal_codes.length ? ` · ${row.signal_codes.join(', ')}` : ''}`,
          cause: 'backend-ok',
        });
      }
      if (row.intake_tier === STRONG_TIER) seenStrongIds.current.add(row.id);
    })
      .then((fn) => {
        unlistenDiscovery = fn;
      })
      .catch(() => {
        /* not in Tauri — interval fallback covers it */
      });

    listenScientiaQueue(() => {
      void refresh();
    })
      .then((fn) => {
        unlistenQueue = fn;
      })
      .catch(() => {
        /* not in Tauri — interval fallback covers it */
      });

    return () => {
      unlistenDiscovery?.();
      unlistenQueue?.();
    };
  }, [pushToast, refresh, embedded]);

  const acknowledge = useCallback(
    async (id: number) => {
      setBusyId(id);
      try {
        await acknowledgeDiscovery(id);
        setRows((prev) => prev.filter((r) => r.id !== id));
        seenStrongIds.current.delete(id);
      } catch (err) {
        pushToast({ tone: 'warn', title: 'Acknowledge failed', body: sanitizeErrorForToast(err), cause: 'backend-error' });
      } finally {
        setBusyId(null);
      }
    },
    [pushToast],
  );

  const openReview = useCallback((publicationId: string) => {
    seedDiscoveryPresetForLegacyKey('discovery-review');
    window.dispatchEvent(
      new CustomEvent('vox://navigate-surface', {
        detail: { view: 'activity', publicationId },
      }),
    );
  }, []);

  return (
    <section className="space-y-4">
      <div className="flex items-end justify-between">
        <div>
          <h2 className="font-display text-lg tracking-wider text-text-primary uppercase">{useLabel('discovery-inbox')}</h2>
          <p className="font-mono text-xs text-text-muted">
            Unacknowledged surfaced research candidates. Open one for review, or acknowledge to dismiss.
          </p>
        </div>
        <button
          type="button"
          onClick={refresh}
          disabled={loading}
          className="rounded-lg border border-border-subtle bg-overlay-subtle px-3 py-1.5 text-xs uppercase tracking-wider hover:bg-overlay-subtle disabled:opacity-40"
        >
          {loading ? 'Loading…' : 'Refresh'}
        </button>
      </div>

      <div className="flex flex-col overflow-hidden rounded-xl border border-border-subtle bg-overlay-subtle">
        <div className="flex items-center justify-between border-b border-border-subtle px-4 py-3">
          <span className="font-display text-[10px] uppercase tracking-[0.2em] text-text-muted">
            Unacknowledged ({rows.length})
          </span>
        </div>

        {rows.length === 0 && !loading && (
          <div className="px-4 py-10 text-center font-mono text-[11px] text-text-muted">
            No unacknowledged research candidates.
          </div>
        )}

        <div className="flex flex-col" role="list" aria-live="polite">
          {rows.map((r) => {
            const strong = r.intake_tier === STRONG_TIER;
            const researchOriginated = r.origin === 'research';
            return (
              <div
                key={r.id}
                role="listitem"
                className="flex items-start justify-between gap-4 border-b border-border-subtle px-4 py-3"
              >
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span
                      className={`rounded-full px-2 py-0.5 font-mono text-[9px] uppercase tracking-wider ${
                        strong
                          ? 'bg-brass/20 text-brass ring-1 ring-brass/40'
                          : 'bg-overlay-subtle text-text-muted'
                      }`}
                    >
                      {tierLabel(r.intake_tier)}
                    </span>
                    {researchOriginated && (
                      <span className="rounded-full bg-cyan/15 px-2 py-0.5 font-mono text-[9px] uppercase tracking-wider text-cyan ring-1 ring-cyan/30">
                        research
                      </span>
                    )}
                    <span className="truncate font-mono text-[12px] text-text-secondary">{r.publication_id}</span>
                    <span className="ml-auto shrink-0 font-mono text-[10px] text-text-muted">
                      {relativeTime(r.surfaced_at_ms)}
                    </span>
                  </div>
                  {r.signal_codes.length > 0 && (
                    <div className="mt-1.5 flex flex-wrap gap-1.5">
                      {r.signal_codes.map((c) => (
                        <span
                          key={c}
                          className="rounded-sm border border-violet-400/20 bg-violet-400/6 px-1.5 py-0.5 font-mono text-[10px] text-violet-200/90"
                        >
                          {c}
                        </span>
                      ))}
                    </div>
                  )}
                </div>
                <div className="flex shrink-0 items-center gap-2">
                  <button
                    type="button"
                    onClick={() => openReview(r.publication_id)}
                    className="rounded-lg border border-border-subtle bg-overlay-subtle px-3 py-1.5 text-[11px] text-text-secondary hover:bg-overlay-subtle"
                  >
                    Open review
                  </button>
                  <button
                    type="button"
                    disabled={busyId === r.id}
                    onClick={() => acknowledge(r.id)}
                    className="rounded-lg border border-emerald-400/30 bg-emerald-400/10 px-3 py-1.5 text-[11px] text-emerald-200 hover:bg-emerald-400/15 disabled:opacity-40"
                  >
                    {busyId === r.id ? 'Acknowledging…' : 'Acknowledge'}
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </section>
  );
}
