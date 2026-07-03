import React, { useEffect, useState, useCallback } from 'react';
import { Glass } from '../../ui/Glass';
import { EmptyState } from '../../ui/EmptyState';
import { FeedbackCard } from './FeedbackCard';
import { feedbackList, feedbackResolve, listenFeedbackChanged, type FeedbackRow } from '../../../transport';
import { useLabel } from '../../../hooks/useLanguage';
import type { Toast } from '../../../types/tauri';
import { useIsEmbeddedSurface } from '../../dashboard/EmbeddedSurfaceContext';
import type { AttentionInbox } from '../../../hooks/useAttentionInbox';

interface Props {
  onOpenContext: (id: string) => void;
  pushToast: (toast: Toast) => void;
  /** Plumbed through by App.tsx (Task 5). Not yet consumed here — see Task 6. */
  attention?: AttentionInbox;
}

export function NeedsYouSurface({ onOpenContext, pushToast }: Props) {
  const embedded = useIsEmbeddedSurface();
  const [needsYou, setNeedsYou] = useState<FeedbackRow[]>([]);
  const [withheld, setWithheld] = useState<FeedbackRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const data = await feedbackList();
      setNeedsYou(data.needsYou);
      setWithheld(data.withheld);
      setError(null);
    } catch (e: any) {
      setError(e.message || 'Failed to load feedback');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
    // Embedded mini-render: one initial fetch only — no repeating poll and no
    // pushed feedback-changed subscription.
    if (embedded) return;
    let unlisten: (() => void) | null = null;
    listenFeedbackChanged(() => {
      refresh();
    }).then((un) => {
      unlisten = un;
    });

    const timer = setInterval(refresh, 5000);

    return () => {
      if (unlisten) unlisten();
      clearInterval(timer);
    };
  }, [refresh, embedded]);

  const handleResolve = async (id: string, action: Record<string, any>) => {
    try {
      await feedbackResolve(id, action);
      pushToast({ tone: 'ok', title: `Feedback ${id} resolved`, cause: 'backend-ok' });
      refresh();
    } catch (e: any) {
      pushToast({ tone: 'warn', title: 'Failed to resolve feedback', body: e.message || String(e), cause: 'backend-error' });
    }
  };

  if (loading && needsYou.length === 0 && withheld.length === 0) {
    return (
      <div className="flex h-full items-center justify-center p-6 text-zinc-500 text-xs">
        Loading...
      </div>
    );
  }

  if (error && needsYou.length === 0 && withheld.length === 0) {
    return (
      <div className="p-6">
        <EmptyState variant="error" title="Error loading feedback" description={error} />
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full overflow-hidden bg-zinc-950/80">
      <div className="flex items-center justify-between border-b border-white/[0.08] px-4 py-3">
        <h2 className="text-sm font-semibold tracking-wider uppercase text-zinc-100">{useLabel('needs-you')}</h2>
      </div>

      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {needsYou.length === 0 ? (
          <EmptyState
            variant="no-data"
            title="Nothing needs you"
            description="The agent is proceeding autonomously. Check back when questions or doubts arise."
          />
        ) : (
          <div className="space-y-2">
            {needsYou.map((item) => (
              <FeedbackCard
                key={item.feedbackId}
                row={item}
                onResolve={handleResolve}
                onOpenContext={onOpenContext}
              />
            ))}
          </div>
        )}

        {withheld.length > 0 && (
          <details className="group border border-zinc-800/80 rounded-xl bg-zinc-900/10 overflow-hidden">
            <summary className="text-[11px] font-semibold text-zinc-500 hover:text-zinc-300 cursor-pointer p-3 select-none tracking-wider uppercase list-none flex items-center justify-between">
              <span>Withheld by policy ({withheld.length})</span>
              <span className="transition-transform group-open:rotate-180">▼</span>
            </summary>
            <div className="p-2 border-t border-zinc-800/40 space-y-2">
              {withheld.map((item) => (
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
