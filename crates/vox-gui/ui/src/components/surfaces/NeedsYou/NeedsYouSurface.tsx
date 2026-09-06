import React, { useEffect, useState, useCallback } from 'react';
import { Glass } from '../../ui/Glass';
import { EmptyState } from '../../ui/EmptyState';
import { FeedbackCard } from './FeedbackCard';
import { sanitizeErrorForToast } from '../../../lib/backendGuard';
import { feedbackList, feedbackResolve, listenFeedbackChanged, type FeedbackRow } from '../../../transport';
import { useLabel } from '../../../hooks/useLanguage';
import type { Toast } from '../../../types/tauri';
import { useIsEmbeddedSurface } from '../../dashboard/EmbeddedSurfaceContext';
import type { AttentionInbox } from '../../../hooks/useAttentionInbox';

interface Props {
  onOpenContext: (id: string) => void;
  pushToast: (toast: Toast) => void;
  /** When provided, this surface sources its data from the shared inbox
   *  instead of self-fetching (App owns polling). See Task 6. */
  attention?: AttentionInbox;
  condensed?: boolean;
}

export function NeedsYouSurface({ onOpenContext, pushToast, attention, condensed }: Props) {
  const embedded = useIsEmbeddedSurface();
  const [needsYou, setNeedsYou] = useState<FeedbackRow[]>([]);
  const [withheld, setWithheld] = useState<FeedbackRow[]>([]);
  const [loading, setLoading] = useState(!attention);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const data = await feedbackList();
      setNeedsYou(data.needsYou);
      setWithheld(data.withheld);
      setError(null);
    } catch (e: any) {
      setError(sanitizeErrorForToast(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    // Shared-inbox mode: App already owns polling via useAttentionInbox.
    // Skip the self-fetch effect entirely.
    if (attention) return;

    refresh();
    // Embedded mini-render: one initial fetch only — no repeating poll and no
    // pushed feedback-changed subscription.
    if (embedded) return;
    let disposed = false;
    let unlisten: (() => void) | null = null;
    listenFeedbackChanged(() => {
      refresh();
    })
      .then((un) => {
        // Unmount may win the race against the async subscription resolving.
        if (disposed) un();
        else unlisten = un;
      })
      .catch(() => { /* event bridge unavailable (bare browser/tests) — poll still runs */ });

    const timer = setInterval(refresh, 5000);

    return () => {
      disposed = true;
      if (unlisten) unlisten();
      clearInterval(timer);
    };
  }, [refresh, embedded, attention]);

  const approvals = attention?.approvals ?? [];
  const effectiveNeedsYou = attention ? attention.needsYou : needsYou;
  const effectiveWithheld = attention ? attention.withheld : withheld;

  const handleResolve = async (id: string, action: Record<string, any>) => {
    try {
      if (attention) {
        await attention.resolveFeedback(id, action);
      } else {
        await feedbackResolve(id, action);
        refresh();
      }
      pushToast({ tone: 'ok', title: `Feedback ${id} resolved`, cause: 'backend-ok' });
    } catch (e: any) {
      pushToast({ tone: 'warn', title: 'Failed to resolve feedback', body: sanitizeErrorForToast(e), cause: 'backend-error' });
    }
  };

  const handleResolveApproval = async (approvalId: string, summary: string, outcome: 'approved' | 'rejected') => {
    if (!attention) return;
    try {
      await attention.resolveApproval(approvalId, outcome);
      pushToast({
        tone: outcome === 'approved' ? 'ok' : 'warn',
        title: outcome === 'approved' ? 'Approved' : 'Rejected',
        body: summary,
        cause: 'backend-ok',
      });
    } catch (e: any) {
      pushToast({ tone: 'warn', title: 'Resolve failed', body: sanitizeErrorForToast(e), cause: 'backend-error' });
    }
  };

  if (condensed) {
    const total = effectiveNeedsYou.length + effectiveWithheld.length + approvals.length;
    return (
      <div className="p-2 text-[11px] text-text-muted">
        <div className="mb-1 text-xs font-semibold uppercase tracking-wider text-text-primary">{useLabel('needs-you')}</div>
        {loading ? <div className="animate-pulse">Loading…</div> : <div>{total} {total === 1 ? 'needs' : 'need'} attention</div>}
      </div>
    );
  }

  if (loading && effectiveNeedsYou.length === 0 && effectiveWithheld.length === 0 && approvals.length === 0) {
    return (
      <div className="flex h-full items-center justify-center p-6 text-text-muted text-xs">
        Loading...
      </div>
    );
  }

  if (error && effectiveNeedsYou.length === 0 && effectiveWithheld.length === 0 && approvals.length === 0) {
    return (
      <div className="p-6">
        <EmptyState variant="error" title="Error loading feedback" description={error} />
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full overflow-hidden">
      <div className="flex items-center justify-between border-b border-border-subtle px-4 py-3">
        <h2 className="text-sm font-semibold tracking-wider uppercase text-text-primary">{useLabel('needs-you')}</h2>
      </div>

      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {approvals.length > 0 && (
          <div>
            <h3 className="ds-section-head">Approvals</h3>
            <ul role="list" className="flex flex-col gap-2 mt-2">
              {approvals.map((a) => (
                <li
                  key={a.approval_id}
                  className="flex flex-col gap-2 rounded-lg border border-border-subtle bg-overlay-subtle p-2 sm:flex-row sm:items-center sm:justify-between"
                >
                  <div className="min-w-0">
                    <div className="font-mono text-[11px] text-text-secondary truncate">{a.tool}</div>
                    <div className="text-[11px] text-text-muted truncate">{a.summary}</div>
                  </div>
                  <div className="flex shrink-0 gap-2">
                    <button
                      type="button"
                      aria-label={`Reject ${a.summary}`}
                      onClick={() => handleResolveApproval(a.approval_id, a.summary, 'rejected')}
                      className="rounded-sm border border-border-subtle px-2 py-1 font-mono text-[9px] uppercase tracking-widest text-text-muted hover:border-rose-400/40 hover:text-rose-300"
                    >
                      Reject
                    </button>
                    <button
                      type="button"
                      aria-label={`Approve ${a.summary}`}
                      onClick={() => handleResolveApproval(a.approval_id, a.summary, 'approved')}
                      className="rounded-sm border border-brass/30 bg-brass/10 px-2 py-1 font-mono text-[9px] uppercase tracking-widest text-brass hover:bg-brass/20"
                    >
                      Approve
                    </button>
                  </div>
                </li>
              ))}
            </ul>
          </div>
        )}

        <div>
          <h3 className="ds-section-head">Questions &amp; doubts</h3>
          {effectiveNeedsYou.length === 0 ? (
            <EmptyState
              variant="no-data"
              title="Nothing needs you"
              description="The agent is proceeding autonomously. Check back when questions or doubts arise."
            />
          ) : (
            <div className="space-y-2 mt-2">
              {effectiveNeedsYou.map((item) => (
                <FeedbackCard
                  key={item.feedbackId}
                  row={item}
                  onResolve={handleResolve}
                  onOpenContext={onOpenContext}
                />
              ))}
            </div>
          )}
        </div>

        {effectiveWithheld.length > 0 && (
          <details className="group border border-border-subtle rounded-xl bg-overlay-subtle overflow-hidden">
            <summary className="text-[11px] font-semibold text-text-muted hover:text-text-secondary cursor-pointer p-3 select-none tracking-wider uppercase list-none flex items-center justify-between">
              <span>Withheld by policy ({effectiveWithheld.length})</span>
              <span className="transition-transform group-open:rotate-180">▼</span>
            </summary>
            <div className="p-2 border-t border-border-subtle space-y-2">
              {effectiveWithheld.map((item) => (
                <FeedbackCard
                  key={item.feedbackId}
                  row={item}
                  onResolve={handleResolve}
                  onOpenContext={onOpenContext}
                />
              ))}
            </div>
          </details>
        )}
      </div>
    </div>
  );
}
