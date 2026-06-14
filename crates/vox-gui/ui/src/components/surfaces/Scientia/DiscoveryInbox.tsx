import React, { useCallback, useEffect, useRef, useState } from 'react';
import type { SurfaceDecoratorProps } from '../decoratorRegistry';
import { listenScientiaQueue } from '../../../transport';
import {
  listDiscoveryInbox,
  acknowledgeDiscovery,
  type DiscoveryInboxRow,
} from './discoveryInboxApi';

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
 * Live updates: subscribes to the `vox://scientia-queue` change ping (the
 * Scientia DB watcher) and refetches; a 10 s interval covers the
 * non-Tauri / no-bridge case. NOTE: the orchestrator-mcp daemon broadcasts a
 * dedicated `scientia.discovery.surfaced` WS topic, but there is no
 * daemon→Tauri re-bridge for it in the GUI yet, so we ride the existing
 * scientia-queue ping + polling rather than a discovery-specific event.
 *
 * On a newly-arrived strong candidate we raise an in-app toast. OS notifications
 * are intentionally NOT wired: the `tauri-plugin-notification` plugin is not a
 * workspace dependency, so adding it here would risk a version-resolution mess
 * for a degradable nicety — the toast is the graceful baseline.
 */
export function DiscoveryInbox({ pushToast }: SurfaceDecoratorProps) {
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
      pushToast({ tone: 'warn', title: 'Discovery inbox', body: String(err) });
      setRows([]);
    } finally {
      setLoading(false);
      fetchingRef.current = false;
    }
  }, [pushToast]);

  // Initial fetch + 10 s interval fallback.
  useEffect(() => {
    refresh();
    const t = setInterval(refresh, 10_000);
    return () => clearInterval(t);
  }, [refresh]);

  // Event-driven refresh on the Scientia-queue ping. Interval above covers the
  // non-Tauri / no-bridge case. Cleans up on unmount.
  useEffect(() => {
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
  }, [refresh]);

  const acknowledge = useCallback(
    async (id: number) => {
      setBusyId(id);
      try {
        await acknowledgeDiscovery(id);
        setRows((prev) => prev.filter((r) => r.id !== id));
        seenStrongIds.current.delete(id);
      } catch (err) {
        pushToast({ tone: 'warn', title: 'Acknowledge failed', body: String(err) });
      } finally {
        setBusyId(null);
      }
    },
    [pushToast],
  );

  const openReview = useCallback((publicationId: string) => {
    window.dispatchEvent(
      new CustomEvent('vox://navigate-surface', {
        detail: { view: 'discovery-review', publicationId },
      }),
    );
  }, []);

  return (
    <section className="space-y-4">
      <div className="flex items-end justify-between">
        <div>
          <h2 className="font-display text-lg tracking-wider text-zinc-100 uppercase">Discovery Inbox</h2>
          <p className="font-mono text-xs text-zinc-500">
            Unacknowledged surfaced research candidates. Open one for review, or acknowledge to dismiss.
          </p>
        </div>
        <button
          type="button"
          onClick={refresh}
          disabled={loading}
          className="rounded-lg border border-white/10 bg-white/[0.03] px-3 py-1.5 text-xs uppercase tracking-wider hover:bg-white/[0.06] disabled:opacity-40"
        >
          {loading ? 'Loading…' : 'Refresh'}
        </button>
      </div>

      <div className="flex flex-col overflow-hidden rounded-xl border border-white/10 bg-white/[0.02]">
        <div className="flex items-center justify-between border-b border-white/5 px-4 py-3">
          <span className="font-display text-[10px] uppercase tracking-[0.2em] text-zinc-400">
            Unacknowledged ({rows.length})
          </span>
        </div>

        {rows.length === 0 && !loading && (
          <div className="px-4 py-10 text-center font-mono text-[11px] text-zinc-600">
            No unacknowledged research candidates.
          </div>
        )}

        <div className="flex flex-col" role="list" aria-live="polite">
          {rows.map((r) => {
            const strong = r.intake_tier === STRONG_TIER;
            return (
              <div
                key={r.id}
                role="listitem"
                className="flex items-start justify-between gap-4 border-b border-white/5 px-4 py-3"
              >
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span
                      className={`rounded-full px-2 py-0.5 font-mono text-[9px] uppercase tracking-wider ${
                        strong
                          ? 'bg-brass/20 text-brass ring-1 ring-brass/40'
                          : 'bg-white/[0.05] text-zinc-400'
                      }`}
                    >
                      {tierLabel(r.intake_tier)}
                    </span>
                    <span className="truncate font-mono text-[12px] text-zinc-200">{r.publication_id}</span>
                    <span className="ml-auto shrink-0 font-mono text-[10px] text-zinc-600">
                      {relativeTime(r.surfaced_at_ms)}
                    </span>
                  </div>
                  {r.signal_codes.length > 0 && (
                    <div className="mt-1.5 flex flex-wrap gap-1.5">
                      {r.signal_codes.map((c) => (
                        <span
                          key={c}
                          className="rounded border border-violet-400/20 bg-violet-400/[0.06] px-1.5 py-0.5 font-mono text-[10px] text-violet-200/90"
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
                    className="rounded-lg border border-white/10 bg-white/[0.03] px-3 py-1.5 text-[11px] text-zinc-200 hover:bg-white/[0.06]"
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
