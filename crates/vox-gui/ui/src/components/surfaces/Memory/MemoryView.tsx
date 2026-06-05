import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Glass } from '../../ui/Glass';
import { Icon } from '../../ui/Icons';
import { Sparkline } from '../../ui/Sparkline';
import { renderHighlights, UnifiedHit, SearchResponse } from '../Search/searchHelpers';

// Corpus vocabulary aligned with vox_db SearchCorpus variants.
// Scope ids must match the corpus names accepted by vox_search_query.
const MEM_CORPORA = [
  { id: 'memory',    name: 'Memory'    },
  { id: 'knowledge', name: 'Knowledge' },
  { id: 'chunk',     name: 'Chunk'     },
] as const;

type MemCorpusId = typeof MEM_CORPORA[number]['id'];

function CorpusChip({
  corpus,
  active,
  onToggle,
}: {
  corpus: (typeof MEM_CORPORA)[number];
  active: boolean;
  onToggle: () => void;
}) {
  return (
    <button
      onClick={onToggle}
      className={`inline-flex items-center gap-1.5 rounded-full border px-2 py-0.5 font-mono text-[10px] transition ${
        active
          ? 'border-white/15 bg-white/[0.04] text-zinc-100'
          : 'border-white/5 bg-white/[0.01] text-zinc-500'
      }`}
    >
      <span className={`size-1.5 rounded-full ${active ? 'bg-brass' : 'bg-white/15'}`} />
      {corpus.name}
    </button>
  );
}

function HitCard({
  hit,
  query,
  onPin,
  onOpen,
}: {
  hit: UnifiedHit;
  query: string;
  onPin: () => void;
  onOpen: () => void;
}) {
  const kindIcon: Record<string, React.ReactNode> = {
    code:   <Icon.file className="size-3.5" />,
    text:   <Icon.catalog className="size-3.5" />,
    chat:   <Icon.bolt className="size-3.5" />,
    policy: <Icon.shield className="size-3.5" />,
    web:    <Icon.link className="size-3.5" />,
  };

  const isOpenable = hit.locator.kind === 'file' || hit.locator.kind === 'web';
  // Bridge: use path as the src display; score as relevance; snippet as text.
  const src = hit.path ?? hit.source;

  const segments = renderHighlights(hit.snippet, query);

  return (
    <div
      className={`group flex items-start gap-3 rounded-md border border-white/5 bg-white/[0.02] p-3 hover:border-white/15 transition ${isOpenable ? 'cursor-pointer' : ''}`}
      onClick={isOpenable ? onOpen : undefined}
    >
      <div className="flex size-7 shrink-0 items-center justify-center rounded bg-white/[0.03] text-zinc-400">
        {kindIcon[hit.kind] ?? <Icon.file className="size-3.5" />}
      </div>
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <span className="font-mono text-[11px] text-zinc-300 truncate">{src}</span>
          {/* Show score instead of line number (no line field in UnifiedHit). */}
          <span className="font-mono text-[9px] text-zinc-500">
            {(hit.score * 100).toFixed(1)}%
          </span>
        </div>
        <div className="mt-1 text-[12px] leading-relaxed text-zinc-300 line-clamp-2">
          {segments.map((seg, i) =>
            seg.mark ? (
              <mark key={i} className="bg-brass/20 text-brass rounded px-0.5">{seg.text}</mark>
            ) : (
              <span key={i}>{seg.text}</span>
            )
          )}
        </div>
      </div>
      <div className="flex flex-col items-end gap-1">
        <div className="h-1 w-16 overflow-hidden rounded-full bg-white/5">
          <div
            className="h-full bg-gradient-to-r from-violet-400 to-emerald-400"
            style={{ width: `${hit.score * 100}%` }}
          />
        </div>
        <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition">
          {isOpenable && (
            <button
              onClick={e => { e.stopPropagation(); onOpen(); }}
              className="rounded border border-white/10 bg-white/[0.02] px-1.5 py-0.5 font-mono text-[9px] text-zinc-300 hover:bg-white/5"
            >
              <Icon.link className="size-2.5 inline mr-0.5" />open
            </button>
          )}
          <button
            onClick={e => { e.stopPropagation(); onPin(); }}
            className="rounded border border-white/10 bg-white/[0.02] px-1.5 py-0.5 font-mono text-[9px] text-zinc-300 hover:bg-white/5"
          >
            <Icon.pin className="size-2.5 inline mr-0.5" />pin
          </button>
        </div>
      </div>
    </div>
  );
}

interface MemoryStatusPayload {
  corpus_counts: Record<string, number>;
  shards: Array<{ id: string; depth: number; entries: number; hot: boolean; dirty: boolean; spark: number[] }>;
  recent_recalls: Array<{ q: string; n: number; when: string }>;
}

interface MemoryViewProps {
  pushToast: (t: any) => void;
}

export function MemoryView({ pushToast }: MemoryViewProps) {
  const [memStatus, setMemStatus] = useState<MemoryStatusPayload | null>(null);
  const [query, setQuery] = useState('');
  // Scope uses the corpus vocabulary aligned with vox_search_query.
  // Default: all three memory/knowledge/chunk corpora active.
  const [scope, setScope] = useState<MemCorpusId[]>(['memory', 'knowledge', 'chunk']);
  const [topK, setTopK] = useState(8);
  const [recallOn, setRecallOn] = useState(false);
  const [hits, setHits] = useState<UnifiedHit[]>([]);
  const [recalling, setRecalling] = useState(false);

  useEffect(() => {
    invoke<MemoryStatusPayload>('get_memory_status')
      .then(setMemStatus)
      .catch((err) => pushToast({ tone: 'warn', title: 'Memory status unavailable', body: String(err) }));
  }, [pushToast]);

  const corpora = MEM_CORPORA.map((c) => ({
    ...c,
    entries: memStatus?.corpus_counts?.[c.id] ?? 0,
  }));
  const corpusTotal = corpora.reduce(
    (s, c) => s + (scope.includes(c.id) ? c.entries : 0),
    0
  );
  // Fall back to summing shard entries when corpus_counts is absent or all-zero
  // (backend may omit corpus_counts on a partial status payload).
  const shardTotal = (memStatus?.shards ?? []).reduce((s, sh) => s + sh.entries, 0);
  const totalEntries = corpusTotal > 0 ? corpusTotal : shardTotal;
  const toggleScope = (id: MemCorpusId) =>
    setScope(s => (s.includes(id) ? s.filter(x => x !== id) : [...s, id]));

  const recall = async (q?: string) => {
    const qq = (q ?? query).trim();
    if (!qq) return;
    setRecalling(true);
    setHits([]);

    try {
      // Repointed from mnemosyne_recall to vox_search_query scoped to memory corpora.
      const res = await invoke<SearchResponse>('vox_search_query', {
        query: qq,
        scope: scope.length > 0 ? scope : null,
        limit: topK,
      });
      setHits(res.hits.slice(0, topK));
      pushToast({
        tone: 'ok',
        title: 'Recall complete',
        body: `Top hits across ${scope.length} corpora`,
        cmd: `vox_search_query • "${qq}"`,
      });
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Recall backend error', body: String(err) });
    } finally {
      setRecalling(false);
      invoke<MemoryStatusPayload>('get_memory_status').then(setMemStatus).catch(() => {});
    }
  };

  const openHit = async (hit: UnifiedHit) => {
    if (hit.locator.kind === 'file' || hit.locator.kind === 'web') {
      try {
        await invoke('open_locator', { locator: hit.locator });
      } catch (err) {
        pushToast({ tone: 'warn', title: 'Could not open', body: String(err) });
      }
    }
  };

  return (
    <div className="grid grid-cols-12 gap-5">
      {/* Header + Search */}
      <Glass className="col-span-12 p-5">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <h2 className="font-display text-[18px] font-semibold tracking-tight text-zinc-100">
              Mnemosyne · Memory
            </h2>
            <p className="mt-0.5 text-[11px] text-zinc-500">
              Vector + symbolic recall · {scope.length} corpora active · {(totalEntries ?? 0).toLocaleString()} indexed entries
            </p>
          </div>
          <div className="flex items-center gap-2">
            <button
              onClick={() => setRecallOn(r => !r)}
              className={`inline-flex items-center gap-1.5 rounded-md border px-2 py-1.5 font-mono text-[10px] transition ${
                recallOn
                  ? 'border-violet-400/40 bg-violet-400/10 text-violet-300'
                  : 'border-white/10 bg-white/[0.02] text-zinc-400 hover:text-zinc-200'
              }`}
            >
              <Icon.eye className="size-3" /> Auto-recall
            </button>
            <button
              onClick={() =>
                invoke('mnemosyne_reindex')
                  .then(() => pushToast({ tone: 'ok', title: 'Reindex complete', cmd: 'mnemosyne reindex' }))
                  .catch((err) => pushToast({ tone: 'warn', title: 'Reindex failed', body: String(err) }))
              }
              className="inline-flex items-center gap-1.5 rounded-md border border-white/10 bg-white/[0.02] px-2 py-1.5 font-mono text-[10px] text-zinc-400 hover:text-zinc-200"
            >
              <Icon.refresh className="size-3" /> Reindex
            </button>
          </div>
        </div>

        {/* Search bar */}
        <div className="mt-4 flex items-center gap-2 rounded-xl border border-white/10 bg-white/[0.02] px-3 py-2">
          <Icon.search className="size-3.5 text-zinc-500" />
          <input
            value={query}
            onChange={e => setQuery(e.target.value)}
            onKeyDown={e => e.key === 'Enter' && recall()}
            placeholder="Recall… e.g. 'ed25519 invariants', 'checkpoint stall'"
            className="flex-1 bg-transparent text-[13px] text-zinc-100 placeholder:text-zinc-600 outline-none"
          />
          <span className="font-mono text-[10px] text-zinc-500">top</span>
          <input
            type="number" min={1} max={50} value={topK}
            onChange={e => setTopK(parseInt(e.target.value) || 8)}
            className="w-12 rounded border border-white/10 bg-white/[0.02] px-1.5 py-0.5 text-center font-mono text-[11px] text-zinc-200 outline-none"
          />
          <button
            onClick={() => recall()}
            disabled={!query.trim() || recalling}
            className={`inline-flex items-center gap-1.5 rounded-md border px-2.5 py-1 font-display text-[10px] uppercase tracking-widest transition ${
              query.trim()
                ? 'border-brass/40 bg-brass/15 text-brass hover:bg-brass/25'
                : 'border-white/5 bg-white/[0.02] text-zinc-600 cursor-not-allowed'
            }`}
          >
            {recalling ? '…' : 'Recall'}
          </button>
        </div>

        {/* Scope chips — corpus vocabulary: memory / knowledge / chunk */}
        <div className="mt-3 flex flex-wrap items-center gap-1.5">
          <span className="font-display text-[9px] uppercase tracking-[0.22em] text-zinc-500">Scope</span>
          {corpora.map(c => (
            <CorpusChip
              key={c.id}
              corpus={c}
              active={scope.includes(c.id)}
              onToggle={() => toggleScope(c.id)}
            />
          ))}
        </div>
      </Glass>

      {/* Recent recalls */}
      <Glass className="col-span-12 xl:col-span-4 p-5">
        <div className="flex items-center justify-between">
          <h3 className="font-display text-[13px] uppercase tracking-[0.18em] text-zinc-200">Recent recalls</h3>
          <Icon.clock className="size-3.5 text-zinc-500" />
        </div>
        <div className="mt-3 space-y-1.5">
          {(memStatus?.recent_recalls ?? []).map((r, i) => (
            <button
              key={i}
              onClick={() => { setQuery(r.q); recall(r.q); }}
              className="flex w-full items-center justify-between rounded-md border border-white/5 bg-white/[0.02] px-2.5 py-1.5 text-left hover:border-white/15 hover:bg-white/[0.04] transition"
            >
              <div className="min-w-0">
                <div className="truncate text-[12px] text-zinc-200">{r.q}</div>
                <div className="font-mono text-[9px] text-zinc-500">{r.n} hits · {r.when} ago</div>
              </div>
              <Icon.chevR className="size-3 text-zinc-500 shrink-0" />
            </button>
          ))}
        </div>
      </Glass>

      {/* Hits */}
      <Glass className="col-span-12 xl:col-span-8 p-5">
        <div className="flex items-center justify-between">
          <h3 className="font-display text-[13px] uppercase tracking-[0.18em] text-zinc-200">
            Citations {hits.length > 0 && <span className="text-zinc-500">· {hits.length}</span>}
          </h3>
          {hits.length > 0 && (
            <button
              onClick={() =>
                pushToast({ tone: 'ok', title: 'Pinned to context', body: `${hits.length} citations → Loquela`, cmd: 'context.attach' })
              }
              className="inline-flex items-center gap-1 rounded-md border border-cyan-400/30 bg-cyan-400/10 px-2 py-1 font-mono text-[10px] text-cyan-300 hover:bg-cyan-400/15"
            >
              <Icon.pin className="size-3" /> Pin all to context
            </button>
          )}
        </div>
        <div className="mt-3 space-y-2">
          {recalling &&
            Array.from({ length: 4 }).map((_, i) => (
              <div key={i} className="h-12 rounded-md border border-white/5 bg-white/[0.02] relative overflow-hidden">
                <span className="absolute inset-0 -translate-x-full animate-vox-shimmer bg-gradient-to-r from-transparent via-white/5 to-transparent" />
              </div>
            ))}
          {!recalling && hits.length === 0 && (
            <div className="flex flex-col items-center justify-center rounded-xl border border-dashed border-white/10 bg-white/[0.01] py-12 text-center">
              <Icon.memory className="size-6 text-zinc-600 mb-2" />
              <div className="font-display text-[12px] tracking-wider text-zinc-400">No recall yet</div>
              <div className="font-mono text-[10px] text-zinc-500">Type a query or click a recent recall</div>
            </div>
          )}
          {!recalling &&
            hits.map((h, i) => (
              <HitCard
                key={i}
                hit={h}
                query={query}
                onOpen={() => openHit(h)}
                onPin={() => pushToast({ tone: 'ok', title: 'Cited', body: h.path ?? h.source, cmd: 'context.pin' })}
              />
            ))}
        </div>
      </Glass>

      {/* Memory shards */}
      <Glass className="col-span-12 p-5">
        <div className="flex items-center justify-between">
          <h3 className="font-display text-[13px] uppercase tracking-[0.18em] text-zinc-200">Memory shards</h3>
          <span className="font-mono text-[10px] text-zinc-500">{(memStatus?.shards ?? []).length} live · HNSW · dim 1024</span>
        </div>
        <div className="mt-3 grid grid-cols-2 gap-3 md:grid-cols-3 xl:grid-cols-6">
          {(memStatus?.shards ?? []).map(s => (
            <div
              key={s.id}
              className={`rounded-xl border p-3 transition hover:border-white/15 ${
                s.hot   ? 'border-brass/30 bg-brass/[0.04]' :
                s.dirty ? 'border-amber-400/30 bg-amber-400/[0.04]' :
                          'border-white/5 bg-white/[0.02]'
              }`}
            >
              <div className="flex items-center justify-between">
                <span className="font-mono text-[11px] text-zinc-300">shard-{s.id}</span>
                {s.hot   && <span className="rounded-full bg-brass/15 px-1.5 py-0.5 font-display text-[9px] uppercase tracking-widest text-brass">hot</span>}
                {s.dirty && <span className="rounded-full bg-amber-400/15 px-1.5 py-0.5 font-display text-[9px] uppercase tracking-widest text-amber-300">dirty</span>}
              </div>
              <div className="mt-2 grid grid-cols-2 gap-1.5 text-[9px]">
                <div className="rounded border border-white/5 bg-zinc-950/40 px-2 py-1.5">
                  <div className="uppercase tracking-widest text-zinc-500">Depth</div>
                  <div className="mt-0.5 font-mono text-[11px] text-zinc-200">{s.depth}</div>
                </div>
                <div className="rounded border border-white/5 bg-zinc-950/40 px-2 py-1.5">
                  <div className="uppercase tracking-widest text-zinc-500">Entries</div>
                  <div className="mt-0.5 font-mono text-[11px] text-zinc-200">{(s.entries ?? 0).toLocaleString()}</div>
                </div>
              </div>
              <div className="mt-2 h-8">
                <Sparkline
                  data={s.spark}
                  color={s.hot ? '#d4af37' : s.dirty ? '#fbbf24' : '#71717a'}
                  width={160}
                  height={28}
                />
              </div>
            </div>
          ))}
        </div>
      </Glass>
    </div>
  );
}
