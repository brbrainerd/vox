import React from 'react';
import { useGraphifyStatus } from '../../../hooks/useGraphifyStatus';

export function GraphifyStatusPanel() {
  const { data, isLoading, isError, error } = useGraphifyStatus();

  if (isLoading) {
    return (
      <div className="flex h-full items-center justify-center p-8 text-sm text-zinc-400">
        Loading graphify status…
      </div>
    );
  }

  if (isError) {
    return (
      <div className="p-4" role="alert">
        <div className="flex items-center gap-2 rounded-lg border border-red-900/50 bg-red-950/20 p-3 text-sm text-red-400">
          <span>Graphify status unavailable: {(error as Error)?.message ?? 'unknown error'}</span>
        </div>
      </div>
    );
  }

  if (!data) return <div className="p-4 text-zinc-400">No graphify data available</div>;

  return (
    <div className="flex flex-col gap-4 p-4">
      <div className="flex items-center justify-between border-b border-white/5 pb-2">
        <h2 className="text-sm font-semibold tracking-wide text-zinc-100 uppercase">
          Graphify Corpus Health
        </h2>
        <span className="font-mono text-[10px] text-zinc-500">
          Default: {data.default_corpus_id}
        </span>
      </div>

      <div className="grid gap-3 sm:grid-cols-1 md:grid-cols-2">
        {data.corpora.map((c) => (
          <div
            key={c.corpus_id}
            className={`group rounded-lg border p-4 transition-all duration-200 ${
              c.is_fresh
                ? 'border-emerald-500/10 bg-emerald-500/[0.02] hover:border-emerald-500/20'
                : 'border-amber-500/10 bg-amber-500/[0.02] hover:border-amber-500/20'
            }`}
          >
            <div className="flex items-start justify-between">
              <div>
                <h3 className="font-medium text-zinc-200">{c.title}</h3>
                <p className="font-mono text-[10px] text-zinc-500">{c.corpus_id}</p>
              </div>
              <span
                className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[10px] font-medium ${
                  c.is_fresh
                    ? 'bg-emerald-500/10 text-emerald-400'
                    : 'bg-amber-500/10 text-amber-400'
                }`}
              >
                <span
                  className={`h-1.5 w-1.5 rounded-full ${
                    c.is_fresh ? 'bg-emerald-400' : 'bg-amber-400'
                  }`}
                />
                {c.is_fresh ? 'Fresh' : 'Stale'}
              </span>
            </div>

            <div className="mt-3 grid grid-cols-2 gap-2 border-t border-white/5 pt-3 font-mono text-[11px] text-zinc-400">
              <div>
                <span className="text-zinc-500 block text-[9px] uppercase">Nodes</span>
                <span className="font-semibold text-zinc-300">
                  {c.node_count?.toLocaleString() ?? '—'}
                </span>
              </div>
              <div>
                <span className="text-zinc-500 block text-[9px] uppercase">Edges</span>
                <span className="font-semibold text-zinc-300">
                  {c.edge_count?.toLocaleString() ?? '—'}
                </span>
              </div>
            </div>

            {!c.is_fresh && (
              <div className="mt-3 space-y-2 border-t border-white/5 pt-3">
                <div className="text-[11px] text-zinc-400">
                  <span className="text-zinc-500 text-[9px] block uppercase">Stale Reasons</span>
                  <div className="flex flex-wrap gap-1 mt-1">
                    {c.stale_reasons.map((r) => (
                      <span
                        key={r}
                        className="rounded bg-amber-500/10 px-1.5 py-0.5 text-[9px] font-mono text-amber-400"
                      >
                        {r}
                      </span>
                    ))}
                  </div>
                </div>

                <div className="relative mt-2 rounded bg-zinc-950/40 p-2 border border-white/5">
                  <span className="text-[9px] text-zinc-500 block uppercase mb-1">Rebuild Command</span>
                  <code className="block select-all font-mono text-[10px] text-zinc-300 break-all leading-normal">
                    vox graphify rebuild --corpus {c.corpus_id}
                  </code>
                </div>
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
