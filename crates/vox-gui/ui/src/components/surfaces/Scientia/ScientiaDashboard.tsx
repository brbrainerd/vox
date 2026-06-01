import React, { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { SurfaceDecoratorProps } from '../decoratorRegistry';

interface ExecuteOutput {
  exit_code: number;
  stdout: string;
  stderr: string;
}

type CardResult =
  | { kind: 'ok'; out: ExecuteOutput }
  | { kind: 'error'; message: string };

interface ScientiaCard {
  key: string;
  title: string;
  description: string;
  path: string[];
}

// Arg-free, read-only Scientia commands (delegate to `vox db` handlers).
const SCIENTIA_CARDS: ScientiaCard[] = [
  {
    key: 'retrieval',
    title: 'Retrieval Status',
    description: 'Research ingest + retrieval readiness',
    path: ['scientia', 'retrieval-status'],
  },
  {
    key: 'discovery',
    title: 'Publication Discovery Queue',
    description: 'Candidate publications awaiting routing',
    path: ['scientia', 'publication-discovery-scan'],
  },
  {
    key: 'capability',
    title: 'Capability Map',
    description: 'Registered research capabilities',
    path: ['scientia', 'capability-list'],
  },
];

export function ScientiaDashboard({ pushToast }: SurfaceDecoratorProps) {
  const [results, setResults] = useState<Record<string, CardResult>>({});
  const [loading, setLoading] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    const next: Record<string, CardResult> = {};
    await Promise.all(
      SCIENTIA_CARDS.map(async (card) => {
        try {
          // Route through the shared execute path (the same Tauri command the
          // generated panels and GamifyView use), keeping Scientia on one run seam.
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
        title: 'Scientia',
        body: `${failures} of ${SCIENTIA_CARDS.length} reads did not complete cleanly`,
      });
    }
  }, [pushToast]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  return (
    <section className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="font-display text-lg text-zinc-100 tracking-wider uppercase">Vox Scientia</h2>
          <p className="font-mono text-xs text-zinc-500">Research &amp; publication pipeline</p>
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
        {SCIENTIA_CARDS.map((card) => {
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
