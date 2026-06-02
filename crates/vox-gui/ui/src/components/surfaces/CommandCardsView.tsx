import React, { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface ExecuteOutput {
  exit_code: number;
  stdout: string;
  stderr: string;
}

type CardResult =
  | { kind: 'ok'; out: ExecuteOutput }
  | { kind: 'error'; message: string };

/** A single read-only command rendered as a dashboard card. */
export interface SurfaceCard {
  key: string;
  title: string;
  description: string;
  /** CLI path passed verbatim to `execute_command` (e.g. ['research', 'status']). */
  path: string[];
}

interface CommandCardsViewProps {
  title: string;
  subtitle: string;
  cards: SurfaceCard[];
  pushToast: (item: { tone: 'ok' | 'warn' | 'info'; title: string; body?: string }) => void;
}

/**
 * Generic decorator body: runs a set of arg-free, read-only CLI commands through
 * the shared `execute_command` Tauri path (the runAction seam) on mount and on
 * Refresh, rendering each result in a card. Used by every surface decorator so
 * Scientia / Mens / Populi / Research share one implementation.
 */
export function CommandCardsView({ title, subtitle, cards, pushToast }: CommandCardsViewProps) {
  const [results, setResults] = useState<Record<string, CardResult>>({});
  const [loading, setLoading] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    const next: Record<string, CardResult> = {};
    await Promise.all(
      cards.map(async (card) => {
        try {
          const out = await invoke<ExecuteOutput>('execute_command', {
            path: card.path,
            args: { __argv: [] },
          });
          next[card.key] = { kind: 'ok', out };
        } catch (err) {
          next[card.key] = { kind: 'error', message: String(err) };
        }
      })
    );
    setResults(next);
    setLoading(false);
    const failures = Object.values(next).filter(
      (r) => r.kind === 'error' || (r.kind === 'ok' && r.out.exit_code !== 0)
    ).length;
    if (failures > 0) {
      pushToast({
        tone: 'warn',
        title,
        body: `${failures} of ${cards.length} reads did not complete cleanly`,
      });
    }
  }, [cards, title, pushToast]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  return (
    <section className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="font-display text-lg tracking-wider text-zinc-100 uppercase">{title}</h2>
          <p className="font-mono text-xs text-zinc-500">{subtitle}</p>
        </div>
        <button
          className="rounded-lg border border-white/10 bg-white/[0.03] px-3 py-1.5 text-xs uppercase tracking-wider hover:bg-white/[0.06] disabled:opacity-40"
          disabled={loading}
          onClick={refresh}
        >
          {loading ? 'Refreshing…' : 'Refresh'}
        </button>
      </div>
      <div className="grid gap-3 lg:grid-cols-2">
        {cards.map((card) => {
          const r = results[card.key];
          return (
            <div key={card.key} className="rounded-xl border border-white/10 bg-white/[0.02] p-3">
              <div className="mb-1 font-display text-sm tracking-wide text-zinc-200">{card.title}</div>
              <div className="mb-2 text-[10px] uppercase tracking-wider text-zinc-500">{card.description}</div>
              {!r && <div className="font-mono text-xs text-zinc-500">Loading…</div>}
              {r && r.kind === 'error' && (
                <div className="font-mono text-xs text-red-400">{r.message}</div>
              )}
              {r && r.kind === 'ok' && (
                <>
                  <div
                    className={`mb-1 font-mono text-[10px] ${
                      r.out.exit_code === 0 ? 'text-emerald-400' : 'text-red-400'
                    }`}
                  >
                    exit {r.out.exit_code} · vox {card.path.join(' ')}
                  </div>
                  <pre className="max-h-56 overflow-auto whitespace-pre-wrap rounded-lg border border-white/10 bg-black/40 p-2 text-[11px] text-zinc-300">
                    {[r.out.stdout, r.out.stderr].filter(Boolean).join('\n').trim() || '(no output)'}
                  </pre>
                </>
              )}
            </div>
          );
        })}
      </div>
    </section>
  );
}
